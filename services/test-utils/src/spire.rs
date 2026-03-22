use std::io::Write;
use std::sync::Arc;
use std::time::Duration;

use spiffe::X509Svid;
use spiffe::x509_source::SvidPicker;
use testcontainers::ContainerAsync;
use testcontainers::core::{CmdWaitFor, ExecCommand, IntoContainerPort, Mount, WaitFor};
use testcontainers::runners::AsyncRunner;
use testcontainers::{GenericImage, ImageExt};
use tokio::time;

/// Selects the SVID whose SPIFFE ID matches the given string.
struct SpiffeIdPicker(String);

impl SvidPicker for SpiffeIdPicker {
    fn pick_svid(&self, svids: &[Arc<X509Svid>]) -> Option<usize> {
        svids
            .iter()
            .position(|svid| svid.spiffe_id().to_string() == self.0)
    }
}

const SPIRE_VERSION: &str = "1.12.0";
const TRUST_DOMAIN: &str = "home.ryanseipp.com";

/// Port inside the socat container that bridges unix->TCP.
const SOCAT_PORT: u16 = 8443;

/// A SPIRE workload entry to register during cluster startup.
pub struct SpireWorkloadEntry {
    /// SPIFFE ID, e.g. `spiffe://home.ryanseipp.com/gateway`
    pub spiffe_id: String,
    /// DNS SAN for the entry (required by spiffe_rustls WebPKI verifier)
    pub dns: String,
}

/// A self-contained SPIRE test cluster using testcontainers.
///
/// Starts a SPIRE server, agent, and socat bridge. The socat bridge
/// exposes the agent's unix socket as TCP so the host can connect.
pub struct SpireTestCluster {
    server: Option<ContainerAsync<GenericImage>>,
    agent: Option<ContainerAsync<GenericImage>>,
    socat: Option<ContainerAsync<GenericImage>>,
    socat_host_port: u16,
    volume_name: String,
    _tmpdir: tempfile::TempDir,
}

impl SpireTestCluster {
    /// Explicitly shut down all containers and remove the named volume.
    ///
    /// Must be called from within a tokio runtime context (e.g. inside
    /// `rt.block_on`). If not called, the `Drop` impl will attempt
    /// cleanup but volume removal may race with container teardown.
    pub fn shutdown(&mut self) {
        // Drop containers that mount the volume first.
        drop(self.socat.take());
        drop(self.agent.take());
        // Server doesn't use the volume, but clean it up too.
        drop(self.server.take());
        // Now the volume is unreferenced and can be removed.
        let _ = std::process::Command::new("docker")
            .args(["volume", "rm", "-f", &self.volume_name])
            .output();
    }
}

impl Drop for SpireTestCluster {
    fn drop(&mut self) {
        self.shutdown();
    }
}

impl SpireTestCluster {
    /// Start the SPIRE cluster and register the given workload entries.
    pub async fn start(entries: &[SpireWorkloadEntry]) -> Self {
        let tmpdir = tempfile::tempdir().unwrap();
        // Shared Docker volume name for the agent socket (stays inside the VM).
        let volume_name = format!("spire-agent-sock-{}", std::process::id());

        // --- SPIRE Server ---
        eprintln!("[spire] starting server...");
        let server = GenericImage::new("ghcr.io/spiffe/spire-server", SPIRE_VERSION)
            .with_exposed_port(8081.tcp())
            .with_wait_for(WaitFor::message_on_stdout("Starting Server APIs"))
            .with_mount(Mount::bind_mount(
                concat!(env!("CARGO_MANIFEST_DIR"), "/../../spire/server.conf"),
                "/opt/spire/conf/server/server.conf",
            ))
            .with_cmd(["-config", "/opt/spire/conf/server/server.conf"])
            .with_startup_timeout(Duration::from_secs(30))
            .start()
            .await
            .unwrap();
        eprintln!("[spire] server ready");

        // Generate a join token
        eprintln!("[spire] generating join token...");
        let mut token_result = server
            .exec(
                ExecCommand::new([
                    "/opt/spire/bin/spire-server",
                    "token",
                    "generate",
                    "-spiffeID",
                    &format!("spiffe://{TRUST_DOMAIN}/agent"),
                    "-output",
                    "json",
                ])
                .with_cmd_ready_condition(CmdWaitFor::exit_code(0)),
            )
            .await
            .unwrap();

        let token_stdout = token_result.stdout_to_vec().await.unwrap();
        let token_json: serde_json::Value =
            serde_json::from_slice(&token_stdout).expect("failed to parse join token JSON");
        let join_token = token_json["value"]
            .as_str()
            .expect("no 'value' field in join token output")
            .to_string();
        eprintln!("[spire] join token obtained");

        let server_ip = server.get_bridge_ip_address().await.unwrap();
        eprintln!("[spire] server bridge IP: {server_ip}");

        // Write a dynamic agent.conf
        let agent_conf = format!(
            r#"agent {{
    data_dir = "/opt/spire/data/agent"
    log_level = "DEBUG"
    server_address = "{server_ip}"
    server_port = "8081"
    socket_path = "/tmp/spire-agent/public/api.sock"
    trust_domain = "{TRUST_DOMAIN}"
    insecure_bootstrap = true
}}

plugins {{
    KeyManager "disk" {{
        plugin_data {{
            directory = "/opt/spire/data/agent"
        }}
    }}

    NodeAttestor "join_token" {{
        plugin_data {{}}
    }}

    WorkloadAttestor "unix" {{
        plugin_data {{}}
    }}
}}
"#
        );

        let agent_conf_path = tmpdir.path().join("agent.conf");
        let mut f = std::fs::File::create(&agent_conf_path).unwrap();
        f.write_all(agent_conf.as_bytes()).unwrap();
        f.sync_all().unwrap();

        // --- SPIRE Agent ---
        eprintln!("[spire] starting agent...");
        let agent = GenericImage::new("ghcr.io/spiffe/spire-agent", SPIRE_VERSION)
            .with_wait_for(WaitFor::message_on_stdout("Starting Workload and SDS APIs"))
            .with_mount(Mount::bind_mount(
                agent_conf_path.to_str().unwrap(),
                "/opt/spire/conf/agent/agent.conf",
            ))
            .with_mount(Mount::volume_mount(&volume_name, "/tmp/spire-agent/public"))
            .with_cmd([
                "-config",
                "/opt/spire/conf/agent/agent.conf",
                "-joinToken",
                &join_token,
            ])
            .with_host_config_modifier(|hc| {
                hc.pid_mode = Some("host".into());
            })
            .with_startup_timeout(Duration::from_secs(60))
            .start()
            .await
            .unwrap();
        eprintln!("[spire] agent ready");

        // --- socat sidecar ---
        eprintln!("[spire] starting socat bridge...");
        let socat = GenericImage::new("alpine/socat", "latest")
            .with_exposed_port(SOCAT_PORT.tcp())
            .with_wait_for(WaitFor::seconds(1))
            .with_mount(Mount::volume_mount(&volume_name, "/tmp/spire-agent/public"))
            .with_cmd([
                &format!("TCP-LISTEN:{SOCAT_PORT},fork,reuseaddr"),
                "UNIX-CONNECT:/tmp/spire-agent/public/api.sock",
            ])
            .with_startup_timeout(Duration::from_secs(15))
            .start()
            .await
            .unwrap();

        let socat_host_port = socat.get_host_port_ipv4(SOCAT_PORT.tcp()).await.unwrap();
        eprintln!("[spire] socat bridge on host port {socat_host_port}");

        // Register workload entries
        for entry in entries {
            eprintln!(
                "[spire] registering: {} (dns: {})",
                entry.spiffe_id, entry.dns
            );
            let result = server
                .exec(
                    ExecCommand::new([
                        "/opt/spire/bin/spire-server",
                        "entry",
                        "create",
                        "-spiffeID",
                        &entry.spiffe_id,
                        "-parentID",
                        &format!("spiffe://{TRUST_DOMAIN}/agent"),
                        "-selector",
                        "unix:uid:0",
                        "-dns",
                        &entry.dns,
                    ])
                    .with_cmd_ready_condition(CmdWaitFor::exit_code(0)),
                )
                .await
                .unwrap();

            let exit = result.exit_code().await.unwrap();
            assert_eq!(
                exit,
                Some(0),
                "failed to register SPIRE entry for {}",
                entry.spiffe_id
            );
        }

        // Poll until SVIDs are actually available from the agent
        eprintln!("[spire] waiting for SVIDs to propagate...");
        let deadline = time::Instant::now() + Duration::from_secs(60);
        loop {
            match spiffe::X509Source::builder()
                .endpoint(format!("tcp://127.0.0.1:{socat_host_port}"))
                .build()
                .await
            {
                Ok(probe) => {
                    drop(probe);
                    break;
                }
                Err(_) if time::Instant::now() < deadline => {
                    time::sleep(Duration::from_millis(500)).await;
                }
                Err(e) => panic!("SVIDs not available after 60s: {e}"),
            }
        }
        eprintln!("[spire] SVIDs available");

        SpireTestCluster {
            server: Some(server),
            agent: Some(agent),
            socat: Some(socat),
            socat_host_port,
            volume_name,
            _tmpdir: tmpdir,
        }
    }

    /// TCP endpoint for `X509Source::builder().endpoint()`.
    pub fn endpoint(&self) -> String {
        format!("tcp://127.0.0.1:{}", self.socat_host_port)
    }

    /// Build an `X509Source` connected to this cluster's agent via TCP,
    /// with a picker that selects the SVID matching `spiffe_id`.
    pub async fn x509_source(&self, spiffe_id: &str) -> spiffe::X509Source {
        let ep = self.endpoint();
        let id = spiffe_id.to_string();
        eprintln!("[spire] building X509Source from {ep} (pick: {id})");
        let source = spiffe::X509Source::builder()
            .endpoint(&ep)
            .picker(SpiffeIdPicker(id))
            .build()
            .await
            .unwrap();
        eprintln!("[spire] X509Source ready");
        source
    }
}
