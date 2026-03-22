use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use gateway::config::{AppConfig, IdentityConfig as GatewayIdentityConfig};
use gateway::pool::IdentityChannel;
use gateway::routes::AppState;
use gateway::server::run as run_gateway;
use identity::config::{AppConfig as IdentityConfig, DbConfig, KafkaConfig};
use identity::crypto::kek::Kek;
use identity::db::DatabasePool;
use identity::server::run as run_identity;
use identity::services::ensure_signing_key;
use rustls::crypto::aws_lc_rs::default_provider;
use rustls::pki_types::ServerName;
use spiffe_rustls::authorizer;
use test_utils::{SpireTestCluster, SpireWorkloadEntry};
use testcontainers_modules::postgres::Postgres;
use testcontainers_modules::testcontainers::ContainerAsync;
use tokio::net::TcpListener;
use tokio::runtime::Runtime;
use tokio::task;

struct BenchEnv {
    spire: Option<SpireTestCluster>,
    db_container: Option<ContainerAsync<Postgres>>,
    rt: Runtime,
    gateway_addr: SocketAddr,
    client: reqwest::Client,
}

impl Drop for BenchEnv {
    fn drop(&mut self) {
        let db = self.db_container.take();
        let spire = self.spire.take();
        self.rt.block_on(async move {
            drop(db);
            drop(spire);
        });
    }
}

static COUNTER: AtomicU64 = AtomicU64::new(0);

async fn start_identity_server(
    pool: deadpool_postgres::Pool,
    server_tls: Arc<rustls::ServerConfig>,
) -> SocketAddr {
    let mut kek_bytes = vec![0u8; 32];
    aws_lc_rs::rand::fill(&mut kek_bytes).unwrap();
    let kek = Arc::new(Kek::from_bytes(kek_bytes).unwrap());

    ensure_signing_key(&pool, &kek).await.unwrap();

    let listener = TcpListener::bind("[::1]:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let config = IdentityConfig {
        listen_addr: addr,
        web_base_url: "https://test.example.com".into(),
        db: DbConfig {
            host: String::new(),
            port: 5432,
            database: String::new(),
            username: String::new(),
            password: None,
            max_connections: 2,
        },
        db_read: None,
        kafka: KafkaConfig::default(),
        spiffe_endpoint_socket: None,
    };
    let db = DatabasePool::from_pool(pool);

    tokio::spawn(async move {
        run_identity(listener, &config.web_base_url, db, kek, Some(server_tls))
            .await
            .unwrap();
    });
    task::yield_now().await;

    addr
}

async fn start_gateway_server(source: spiffe::X509Source, identity_addr: SocketAddr) -> SocketAddr {
    let identity = Arc::new(
        IdentityChannel::new(
            format!("https://[::1]:{}", identity_addr.port()),
            source,
            ServerName::try_from("identity").unwrap(),
        )
        .await
        .unwrap(),
    );

    let state = AppState { identity };

    let listener = TcpListener::bind("[::1]:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let config = AppConfig {
        listen_addr: addr,
        tls_cert_file: "/nonexistent/tls.crt".into(),
        tls_key_file: "/nonexistent/tls.key".into(),
        identity: GatewayIdentityConfig::default(),
    };

    tokio::spawn(async move {
        run_gateway(listener, &config, Some(state)).await.unwrap();
    });
    task::yield_now().await;

    addr
}

fn start_env() -> BenchEnv {
    let _ = default_provider().install_default();
    test_utils::init_tracing();

    let rt = Runtime::new().unwrap();
    let (gateway_addr, spire, db_container) = rt.block_on(async {
        let spire = SpireTestCluster::start(&[
            SpireWorkloadEntry {
                spiffe_id: "spiffe://home.ryanseipp.com/gateway".into(),
                dns: "gateway".into(),
            },
            SpireWorkloadEntry {
                spiffe_id: "spiffe://home.ryanseipp.com/identity".into(),
                dns: "identity".into(),
            },
        ])
        .await;

        let (db_container, pool) = test_utils::test_db_pool().await;

        let identity_source = spire
            .x509_source("spiffe://home.ryanseipp.com/identity")
            .await;
        let identity_server_tls = spiffe_rustls::mtls_server(identity_source)
            .authorize(authorizer::any())
            .with_alpn_protocols([b"h2".as_slice()])
            .build()
            .unwrap();
        let identity_addr = start_identity_server(pool, Arc::new(identity_server_tls)).await;

        let gateway_source = spire
            .x509_source("spiffe://home.ryanseipp.com/gateway")
            .await;
        let gateway_addr = start_gateway_server(gateway_source, identity_addr).await;

        (gateway_addr, spire, db_container)
    });

    BenchEnv {
        spire: Some(spire),
        db_container: Some(db_container),
        rt,
        gateway_addr,
        client: reqwest::Client::new(),
    }
}

#[divan::bench]
fn bench_e2e_sign_up(bencher: divan::Bencher) {
    let env = start_env();

    bencher.bench(|| {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        env.rt.block_on(async {
            let resp = env
                .client
                .post(format!(
                    "http://[::1]:{}/api/v1/sign-up",
                    env.gateway_addr.port()
                ))
                .json(&serde_json::json!({
                    "username": format!("e2euser-{n}"),
                    "given_name": "E2E",
                    "family_name": "User",
                    "email": format!("e2e-{n}@example.com"),
                    "password": "secure-password-123"
                }))
                .send()
                .await
                .unwrap();

            assert_eq!(resp.status(), 204);
        })
    });
}

fn main() {
    divan::main();
}
