#![cfg(feature = "db-tests")]

mod spire_fixture;

use std::net::SocketAddr;
use std::sync::Arc;

use deadpool_postgres::Pool;
use gateway::config::{AppConfig, IdentityConfig as GatewayIdentityConfig, ScyllaConfig};
use gateway::pool::IdentityChannel;
use gateway::routes::AppState;
use gateway::server::run as run_gateway;
use gateway::session::SessionStore;
use identity::config::{AppConfig as IdentityConfig, DbConfig, KafkaConfig};
use identity::crypto::kek::Kek;
use identity::db::DatabasePool;
use identity::server::run as run_identity;
use identity::services::ensure_signing_key;
use rustls::crypto::aws_lc_rs::default_provider;
use rustls::pki_types::ServerName;
use spiffe_rustls::authorizer;
use test_utils::{ContainerAsync, Postgres, ScyllaDB};

use aws_lc_rs::rand;
use tokio::net::TcpListener;
use tokio::task;

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
/// connection pool to the identity backend and a ScyllaDB session store.
async fn start_gateway_server(
    source: spiffe::X509Source,
    identity_addr: SocketAddr,
    sessions: Arc<SessionStore>,
) -> SocketAddr {
    let identity = Arc::new(
        IdentityChannel::new(
            format!("https://[::1]:{}", identity_addr.port()),
            source,
            ServerName::try_from("identity").unwrap(),
        )
        .await
        .unwrap(),
    );

    let state = AppState { identity, sessions };

    let listener = TcpListener::bind("[::1]:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let config = AppConfig {
        listen_addr: addr,
        tls_cert_file: "/nonexistent/tls.crt".into(),
        tls_key_file: "/nonexistent/tls.key".into(),
        identity: GatewayIdentityConfig::default(),
        scylla: ScyllaConfig::default(),
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
    _scylla_container: ContainerAsync<ScyllaDB>,
}

impl TestEnv {
    async fn start() -> Self {
        let _ = default_provider().install_default();
        test_utils::init_tracing();

        let spire = spire_fixture::start().await;
        let (db_container, pool) = test_utils::test_db_pool().await;
        let (scylla_container, sessions) = test_utils::test_scylla_store().await;

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
        let gateway_addr = start_gateway_server(gateway_source, identity_addr, sessions).await;

        TestEnv {
            gateway_addr,
            _spire: spire,
            _db_container: db_container,
            _scylla_container: scylla_container,
        }
    }

    fn url(&self, path: &str) -> String {
        format!("http://[::1]:{}{path}", self.gateway_addr.port())
    }
}

// ---------------------------------------------------------------------------
// Existing tests
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Auth session tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn unauthenticated_userinfo_returns_401() {
    let env = TestEnv::start().await;

    let resp = reqwest::Client::new()
        .get(env.url("/api/v1/userinfo"))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 401);
}

#[tokio::test]
async fn unauthenticated_profile_returns_401() {
    let env = TestEnv::start().await;

    let resp = reqwest::Client::new()
        .patch(env.url("/api/v1/profile"))
        .json(&serde_json::json!({"given_name": "New"}))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 401);
}

#[tokio::test]
async fn unauthenticated_update_password_returns_401() {
    let env = TestEnv::start().await;

    let resp = reqwest::Client::new()
        .post(env.url("/api/v1/update-password"))
        .json(&serde_json::json!({
            "current_password": "old",
            "new_password": "new"
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 401);
}

#[tokio::test]
async fn unauthenticated_logout_returns_401() {
    let env = TestEnv::start().await;

    let resp = reqwest::Client::new()
        .post(env.url("/api/v1/logout"))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 401);
}

#[tokio::test]
async fn resend_verification_no_auth_required() {
    let env = TestEnv::start().await;

    // Should return 204 regardless of whether the email exists (prevents enumeration)
    let resp = reqwest::Client::new()
        .post(env.url("/api/v1/resend-verification"))
        .json(&serde_json::json!({"email": "nobody@example.com"}))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 204);
}

#[tokio::test]
async fn invalid_session_cookie_returns_401() {
    let env = TestEnv::start().await;

    let resp = reqwest::Client::new()
        .get(env.url("/api/v1/userinfo"))
        .header("Cookie", "__Host-sid=not-valid-base64url")
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 401);
}

#[tokio::test]
async fn unknown_session_token_returns_401() {
    let env = TestEnv::start().await;

    // Valid base64url token (32 random bytes) that doesn't correspond to any session.
    let fake_token = base64ct::Base64UrlUnpadded::encode_string(&[0xAB; 32]);
    let resp = reqwest::Client::new()
        .get(env.url("/api/v1/userinfo"))
        .header("Cookie", format!("__Host-sid={fake_token}"))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 401);
}
