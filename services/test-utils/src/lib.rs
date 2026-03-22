mod spire;

pub use spire::{SpireTestCluster, SpireWorkloadEntry};
pub use testcontainers_modules::postgres::Postgres;
pub use testcontainers_modules::scylladb::ScyllaDB;
pub use testcontainers_modules::testcontainers::ContainerAsync;

use std::sync::Arc;
use std::time::Duration;

use tokio::time;

use deadpool_postgres::Pool;
use gateway::config::ScyllaConfig;
use gateway::session::SessionStore;
use identity::db::DatabasePool;
use testcontainers::GenericImage;
use testcontainers_modules::testcontainers::ImageExt;
use testcontainers_modules::testcontainers::core::Mount;
use testcontainers_modules::testcontainers::runners::AsyncRunner;

/// Start a testcontainers Postgres instance, run identity migrations, and
/// return the container handle (must be kept alive) and a connection pool.
///
/// Uses a tmpfs mount for PGDATA to avoid accumulating anonymous Docker
/// volumes from the Postgres image's VOLUME directive.
///
/// # Panics
///
/// Panics if the container fails to start or database setup fails.
pub async fn test_db_pool() -> (ContainerAsync<Postgres>, Pool) {
    let container = Postgres::default()
        .with_tag("17-alpine")
        .with_mount(Mount::tmpfs_mount("/var/lib/postgresql/data"))
        .start()
        .await
        .unwrap();
    let host = container.get_host().await.unwrap();
    let port = container.get_host_port_ipv4(5432).await.unwrap();

    let url = format!("postgres://postgres:postgres@{host}:{port}/postgres");
    let db = DatabasePool::from_url(&url, 5).unwrap();
    db.migrate().await.unwrap();

    (container, db.writer().clone())
}

/// Start a testcontainers `ScyllaDB` instance and return the container handle
/// (must be kept alive) and a connected `SessionStore`.
///
/// Automatically bumps the Docker VM's `aio-max-nr` if needed (Docker Desktop
/// defaults are too low for `ScyllaDB`).
///
/// # Panics
///
/// Panics if the container fails to start or `ScyllaDB` does not become ready
/// within 60 seconds.
pub async fn test_scylla_store() -> (ContainerAsync<ScyllaDB>, Arc<SessionStore>) {
    ensure_aio_max_nr().await;

    let container = ScyllaDB::default()
        .with_tag("2026.1.0")
        .with_cmd(["--smp", "1", "--memory", "256M", "--overprovisioned", "1"])
        .with_startup_timeout(Duration::from_secs(120))
        .start()
        .await
        .expect("failed to start ScyllaDB container");

    let host = container.get_host().await.unwrap();
    let port = container.get_host_port_ipv4(9042).await.unwrap();

    let config = ScyllaConfig {
        contact_points: vec![format!("{host}:{port}")],
        keyspace: "gateway_test".into(),
    };

    let store = time::timeout(Duration::from_secs(60), async {
        loop {
            match SessionStore::connect(&config, None).await {
                Ok(store) => break store,
                Err(e) => {
                    tracing::debug!(error = %e, "ScyllaDB not ready, retrying...");
                    time::sleep(Duration::from_secs(2)).await;
                }
            }
        }
    })
    .await
    .expect("ScyllaDB did not become ready within 60s");

    (container, Arc::new(store))
}

/// Bump `aio-max-nr` in the Docker VM if it's below `ScyllaDB`'s minimum.
///
/// Docker Desktop's default (65536) is often too low. This runs a privileged
/// alpine container to raise it. Safe on Linux hosts too (no-op if already high).
async fn ensure_aio_max_nr() {
    use testcontainers::core::{CmdWaitFor, ExecCommand, WaitFor};

    let min_required: u64 = 1_048_576;

    let container: ContainerAsync<GenericImage> = GenericImage::new("alpine", "latest")
        .with_wait_for(WaitFor::seconds(1))
        .with_cmd(["sleep", "infinity"])
        .with_host_config_modifier(|hc| {
            hc.privileged = Some(true);
        })
        .start()
        .await
        .expect("failed to start privileged alpine for AIO check");

    let mut result = container
        .exec(
            ExecCommand::new(["cat", "/proc/sys/fs/aio-max-nr"])
                .with_cmd_ready_condition(CmdWaitFor::exit_code(0)),
        )
        .await
        .unwrap();

    let stdout = result.stdout_to_vec().await.unwrap();
    let current: u64 = String::from_utf8_lossy(&stdout).trim().parse().unwrap_or(0);

    if current < min_required {
        tracing::info!(
            current,
            target = min_required,
            "bumping aio-max-nr for ScyllaDB"
        );
        container
            .exec(
                ExecCommand::new([
                    "sh",
                    "-c",
                    &format!("echo {min_required} > /proc/sys/fs/aio-max-nr"),
                ])
                .with_cmd_ready_condition(CmdWaitFor::exit_code(0)),
            )
            .await
            .unwrap();
    }

    // Container is dropped here — cleanup is automatic.
}

/// Initialize a tracing subscriber for tests. Safe to call multiple times
/// (subsequent calls are no-ops).
pub fn init_tracing() {
    use tracing_subscriber::EnvFilter;
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_test_writer()
        .try_init();
}
