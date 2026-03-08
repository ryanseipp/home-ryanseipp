#![cfg(feature = "db-tests")]

use std::net::SocketAddr;
use std::sync::Arc;

use identity::config::{AppConfig, DbConfig, KafkaConfig};
use identity::crypto::kek::Kek;
use identity::db::DatabasePool;
use identity::proto::GetJwksRequest;
use identity::proto::identity_service_client::IdentityServiceClient;
use sqlx::PgPool;
use testcontainers_modules::postgres::Postgres;
use testcontainers_modules::testcontainers::runners::AsyncRunner;
use tokio::net::TcpListener;

async fn test_db_pool() -> (
    testcontainers_modules::testcontainers::ContainerAsync<Postgres>,
    PgPool,
) {
    let container = Postgres::default().start().await.unwrap();
    let host = container.get_host().await.unwrap();
    let port = container.get_host_port_ipv4(5432).await.unwrap();

    let pool = PgPool::connect(&format!(
        "postgres://postgres:postgres@{host}:{port}/postgres"
    ))
    .await
    .unwrap();

    sqlx::migrate!("./migrations").run(&pool).await.unwrap();

    (container, pool)
}

fn test_kek() -> Arc<Kek> {
    let mut bytes = vec![0u8; 32];
    aws_lc_rs::rand::fill(&mut bytes).unwrap();
    Arc::new(Kek::from_bytes(bytes).unwrap())
}

fn test_config(addr: SocketAddr) -> AppConfig {
    AppConfig {
        listen_addr: addr,
        tls_cert_file: "/nonexistent/tls.crt".into(),
        tls_key_file: "/nonexistent/tls.key".into(),
        web_base_url: "https://test.example.com".into(),
        db: DbConfig {
            host: String::new(),
            port: 5432,
            database: String::new(),
            username: String::new(),
            password: None,
            ssl_mode: "disable".into(),
            ssl_root_cert: String::new(),
            ssl_client_cert: String::new(),
            ssl_client_key: String::new(),
            max_connections: 2,
            min_connections: 1,
        },
        db_read: None,
        kafka: KafkaConfig::default(),
    }
}

/// Directly set a user's status to Active in the database (for login tests).
async fn activate_user_by_email(pool: &PgPool, email: &str) {
    sqlx::query!(
        "UPDATE users SET status = 'active', email_verified = TRUE, updated_at = NOW()
         WHERE LOWER(email) = LOWER($1) AND deleted_at IS NULL",
        email,
    )
    .execute(pool)
    .await
    .unwrap();
}

async fn start_test_server() -> (
    SocketAddr,
    testcontainers_modules::testcontainers::ContainerAsync<Postgres>,
    PgPool,
) {
    let (_container, pool) = test_db_pool().await;

    let kek = test_kek();

    // Auto-create signing key for tests
    identity::services::ensure_signing_key(&pool, &kek)
        .await
        .unwrap();

    let listener = TcpListener::bind("[::1]:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let config = test_config(addr);
    let db = DatabasePool::from_pool(pool.clone());

    tokio::spawn(async move {
        identity::server::run(listener, &config, db, kek)
            .await
            .unwrap();
    });

    tokio::task::yield_now().await;

    (addr, _container, pool)
}

#[tokio::test]
async fn server_starts_and_accepts_connections() {
    let (addr, _container, _pool) = start_test_server().await;

    let mut client = IdentityServiceClient::connect(format!("http://[::1]:{}", addr.port()))
        .await
        .unwrap();

    // GetJwks is implemented — should return OK with JWKS containing the auto-created key
    let response = client.get_jwks(GetJwksRequest {}).await.unwrap();
    let jwks: serde_json::Value = serde_json::from_str(&response.into_inner().keys).unwrap();
    assert_eq!(jwks["keys"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn sign_up_succeeds() {
    let (addr, _container, _pool) = start_test_server().await;

    let mut client = IdentityServiceClient::connect(format!("http://[::1]:{}", addr.port()))
        .await
        .unwrap();

    client
        .sign_up(identity::proto::SignUpRequest {
            username: "testuser".into(),
            given_name: "Test".into(),
            family_name: "User".into(),
            email: "test@example.com".into(),
            password: Some("password123".into()),
        })
        .await
        .unwrap();
}

#[tokio::test]
async fn sign_up_duplicate_username_fails() {
    let (addr, _container, _pool) = start_test_server().await;

    let mut client = IdentityServiceClient::connect(format!("http://[::1]:{}", addr.port()))
        .await
        .unwrap();

    client
        .sign_up(identity::proto::SignUpRequest {
            username: "dupuser".into(),
            given_name: "First".into(),
            family_name: "User".into(),
            email: "first@example.com".into(),
            password: Some("password123".into()),
        })
        .await
        .unwrap();

    let err = client
        .sign_up(identity::proto::SignUpRequest {
            username: "dupuser".into(),
            given_name: "Second".into(),
            family_name: "User".into(),
            email: "second@example.com".into(),
            password: Some("password123".into()),
        })
        .await
        .unwrap_err();
    assert_eq!(err.code(), tonic::Code::AlreadyExists);
}

#[tokio::test]
async fn sign_up_validation_errors() {
    let (addr, _container, _pool) = start_test_server().await;

    let mut client = IdentityServiceClient::connect(format!("http://[::1]:{}", addr.port()))
        .await
        .unwrap();

    // No password
    let err = client
        .sign_up(identity::proto::SignUpRequest {
            username: "testuser".into(),
            given_name: "Test".into(),
            family_name: "User".into(),
            email: "test@example.com".into(),
            password: None,
        })
        .await
        .unwrap_err();
    assert_eq!(err.code(), tonic::Code::InvalidArgument);

    // Short password
    let err = client
        .sign_up(identity::proto::SignUpRequest {
            username: "testuser".into(),
            given_name: "Test".into(),
            family_name: "User".into(),
            email: "test@example.com".into(),
            password: Some("short".into()),
        })
        .await
        .unwrap_err();
    assert_eq!(err.code(), tonic::Code::InvalidArgument);

    // Invalid username
    let err = client
        .sign_up(identity::proto::SignUpRequest {
            username: "bad user!".into(),
            given_name: "Test".into(),
            family_name: "User".into(),
            email: "test@example.com".into(),
            password: Some("password123".into()),
        })
        .await
        .unwrap_err();
    assert_eq!(err.code(), tonic::Code::InvalidArgument);
}

#[tokio::test]
async fn verify_email_with_bad_token_returns_not_found() {
    let (addr, _container, _pool) = start_test_server().await;

    let mut client = IdentityServiceClient::connect(format!("http://[::1]:{}", addr.port()))
        .await
        .unwrap();

    let err = client
        .verify_email(identity::proto::VerifyEmailRequest {
            token: "invalid-token-not-base64url".into(),
        })
        .await
        .unwrap_err();
    assert_eq!(err.code(), tonic::Code::NotFound);
}

#[tokio::test]
async fn resend_verification_for_unknown_email_returns_ok() {
    let (addr, _container, _pool) = start_test_server().await;

    let mut client = IdentityServiceClient::connect(format!("http://[::1]:{}", addr.port()))
        .await
        .unwrap();

    // Should return OK even for unknown email (prevent enumeration)
    client
        .resend_verification(identity::proto::ResendVerificationRequest {
            email: "nonexistent@example.com".into(),
        })
        .await
        .unwrap();
}

#[tokio::test]
async fn unimplemented_rpcs() {
    let (addr, _container, _pool) = start_test_server().await;

    let mut client = IdentityServiceClient::connect(format!("http://[::1]:{}", addr.port()))
        .await
        .unwrap();

    let user_info_err = client
        .user_info(identity::proto::UserInfoRequest {})
        .await
        .unwrap_err();
    assert_eq!(user_info_err.code(), tonic::Code::Unimplemented);
}

#[tokio::test]
async fn login_with_valid_credentials_succeeds() {
    let (addr, _container, pool) = start_test_server().await;

    let mut client = IdentityServiceClient::connect(format!("http://[::1]:{}", addr.port()))
        .await
        .unwrap();

    // Sign up
    client
        .sign_up(identity::proto::SignUpRequest {
            username: "loginuser".into(),
            given_name: "Login".into(),
            family_name: "User".into(),
            email: "login@example.com".into(),
            password: Some("password123".into()),
        })
        .await
        .unwrap();

    // Activate user directly in DB
    activate_user_by_email(&pool, "login@example.com").await;

    // Login
    let response = client
        .login(identity::proto::LoginRequest {
            email: "login@example.com".into(),
            password: "password123".into(),
        })
        .await
        .unwrap();

    let inner = response.into_inner();
    assert!(!inner.access_token.is_empty());
    assert!(!inner.refresh_token.is_empty());
    assert!(!inner.id_token.is_empty());
    assert!(inner.expires_at.is_some());
}

#[tokio::test]
async fn login_with_wrong_password_fails() {
    let (addr, _container, pool) = start_test_server().await;

    let mut client = IdentityServiceClient::connect(format!("http://[::1]:{}", addr.port()))
        .await
        .unwrap();

    // Sign up and activate
    client
        .sign_up(identity::proto::SignUpRequest {
            username: "wrongpw".into(),
            given_name: "Wrong".into(),
            family_name: "Password".into(),
            email: "wrongpw@example.com".into(),
            password: Some("password123".into()),
        })
        .await
        .unwrap();
    activate_user_by_email(&pool, "wrongpw@example.com").await;

    let err = client
        .login(identity::proto::LoginRequest {
            email: "wrongpw@example.com".into(),
            password: "wrong-password".into(),
        })
        .await
        .unwrap_err();
    assert_eq!(err.code(), tonic::Code::Unauthenticated);
}

#[tokio::test]
async fn login_with_unknown_email_fails() {
    let (addr, _container, _pool) = start_test_server().await;

    let mut client = IdentityServiceClient::connect(format!("http://[::1]:{}", addr.port()))
        .await
        .unwrap();

    let err = client
        .login(identity::proto::LoginRequest {
            email: "nonexistent@example.com".into(),
            password: "password123".into(),
        })
        .await
        .unwrap_err();
    assert_eq!(err.code(), tonic::Code::Unauthenticated);
}

#[tokio::test]
async fn login_with_pending_verification_fails() {
    let (addr, _container, _pool) = start_test_server().await;

    let mut client = IdentityServiceClient::connect(format!("http://[::1]:{}", addr.port()))
        .await
        .unwrap();

    // Sign up but do NOT verify email — user stays PendingVerification
    client
        .sign_up(identity::proto::SignUpRequest {
            username: "pending".into(),
            given_name: "Pending".into(),
            family_name: "User".into(),
            email: "pending@example.com".into(),
            password: Some("password123".into()),
        })
        .await
        .unwrap();

    let err = client
        .login(identity::proto::LoginRequest {
            email: "pending@example.com".into(),
            password: "password123".into(),
        })
        .await
        .unwrap_err();
    assert_eq!(err.code(), tonic::Code::PermissionDenied);
}

#[tokio::test]
async fn server_accepts_tls_connections() {
    use rcgen::generate_simple_self_signed;
    use std::io::Write;
    use tonic::transport::{Certificate, Channel, ClientTlsConfig};

    // Ensure aws-lc-rs is installed as the rustls crypto provider for the
    // test client. The server side handles this via tonic's tls-aws-lc feature,
    // but the client needs it explicitly when rustls has no default provider.
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

    let (_container, pool) = test_db_pool().await;

    let kek = test_kek();
    identity::services::ensure_signing_key(&pool, &kek)
        .await
        .unwrap();

    // Generate self-signed cert for localhost/[::1]
    let subject_alt_names = vec!["::1".into(), "localhost".into()];
    let certified_key = generate_simple_self_signed(subject_alt_names).unwrap();

    let cert_pem = certified_key.cert.pem();
    let key_pem = certified_key.signing_key.serialize_pem();

    // Write to temp files
    let dir = tempfile::tempdir().unwrap();
    let cert_path = dir.path().join("tls.crt");
    let key_path = dir.path().join("tls.key");

    std::fs::File::create(&cert_path)
        .unwrap()
        .write_all(cert_pem.as_bytes())
        .unwrap();
    std::fs::File::create(&key_path)
        .unwrap()
        .write_all(key_pem.as_bytes())
        .unwrap();

    // Start TLS server
    let listener = TcpListener::bind("[::1]:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let config = AppConfig {
        listen_addr: addr,
        tls_cert_file: cert_path.to_str().unwrap().into(),
        tls_key_file: key_path.to_str().unwrap().into(),
        web_base_url: "https://test.example.com".into(),
        db: DbConfig {
            host: String::new(),
            port: 5432,
            database: String::new(),
            username: String::new(),
            password: None,
            ssl_mode: "disable".into(),
            ssl_root_cert: String::new(),
            ssl_client_cert: String::new(),
            ssl_client_key: String::new(),
            max_connections: 2,
            min_connections: 1,
        },
        db_read: None,
        kafka: KafkaConfig::default(),
    };
    let db = DatabasePool::from_pool(pool);

    tokio::spawn(async move {
        identity::server::run(listener, &config, db, kek)
            .await
            .unwrap();
    });
    tokio::task::yield_now().await;

    // Connect with TLS client trusting our self-signed CA
    let tls_config = ClientTlsConfig::new()
        .ca_certificate(Certificate::from_pem(&cert_pem))
        .domain_name("::1");

    let channel = Channel::from_shared(format!("https://[::1]:{}", addr.port()))
        .unwrap()
        .tls_config(tls_config)
        .unwrap()
        .connect()
        .await
        .unwrap();

    let mut client = IdentityServiceClient::new(channel);

    let response = client.get_jwks(GetJwksRequest {}).await.unwrap();
    let jwks: serde_json::Value = serde_json::from_str(&response.into_inner().keys).unwrap();
    assert_eq!(jwks["keys"].as_array().unwrap().len(), 1);
}
