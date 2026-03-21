#![cfg(feature = "db-tests")]

mod spire_fixture;

use std::net::SocketAddr;
use std::sync::Arc;

use deadpool_postgres::Pool;
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

use aws_lc_rs::rand;
use testcontainers_modules::postgres::Postgres;
use testcontainers_modules::testcontainers::ContainerAsync;
use testcontainers_modules::testcontainers::ImageExt;
use testcontainers_modules::testcontainers::runners::AsyncRunner;
use tokio::net::TcpListener;
use tokio::task;

fn init_tracing() {
    use tracing_subscriber::EnvFilter;
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("gateway=debug,spiffe_rustls=debug,info")),
        )
        .with_test_writer()
        .try_init();
}

async fn test_db_pool() -> (ContainerAsync<Postgres>, Pool) {
    let container = Postgres::default()
        .with_tag("17-alpine")
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

/// Start the identity gRPC server with SPIFFE mTLS and return its address.
async fn start_identity_server(pool: Pool, server_tls: Arc<rustls::ServerConfig>) -> SocketAddr {
    let mut kek_bytes = vec![0u8; 32];
    rand::fill(&mut kek_bytes).unwrap();
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

/// Start the gateway HTTP server (plain HTTP externally) with a SPIFFE mTLS
/// connection pool to the identity backend.
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

struct TestEnv {
    gateway_addr: SocketAddr,
    _spire: spire_fixture::SpireTestCluster,
    _db_container: ContainerAsync<Postgres>,
}

impl TestEnv {
    async fn start() -> Self {
        let _ = default_provider().install_default();
        init_tracing();

        let spire = spire_fixture::SpireTestCluster::start().await;
        let (db_container, pool) = test_db_pool().await;

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

        TestEnv {
            gateway_addr,
            _spire: spire,
            _db_container: db_container,
        }
    }

    fn url(&self, path: &str) -> String {
        format!("http://[::1]:{}{path}", self.gateway_addr.port())
    }
}

#[tokio::test]
async fn sign_up_through_gateway() {
    let env = TestEnv::start().await;

    let resp = reqwest::Client::new()
        .post(env.url("/api/v1/sign-up"))
        .json(&serde_json::json!({
            "username": "testuser",
            "given_name": "Test",
            "family_name": "User",
            "email": "test@example.com",
            "password": "secure-password-123"
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 204);
}

#[tokio::test]
async fn sign_up_duplicate_returns_conflict() {
    let env = TestEnv::start().await;

    let client = reqwest::Client::new();
    let body = serde_json::json!({
        "username": "dupeuser",
        "given_name": "Test",
        "family_name": "User",
        "email": "dupe@example.com",
        "password": "secure-password-123"
    });

    let resp1 = client
        .post(env.url("/api/v1/sign-up"))
        .json(&body)
        .send()
        .await
        .unwrap();
    assert_eq!(resp1.status(), 204);

    // Second sign-up with same username should return 409
    let resp2 = client
        .post(env.url("/api/v1/sign-up"))
        .json(&body)
        .send()
        .await
        .unwrap();
    assert_eq!(resp2.status(), 409);
}

#[tokio::test]
async fn verify_email_invalid_token_returns_not_found() {
    let env = TestEnv::start().await;

    let resp = reqwest::Client::new()
        .post(env.url("/api/v1/verify-email"))
        .json(&serde_json::json!({
            "token": "invalid-token-value"
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 404);
}
