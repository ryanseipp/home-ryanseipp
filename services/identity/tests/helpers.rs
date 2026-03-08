use std::net::SocketAddr;
use std::sync::Arc;

use identity::config::{AppConfig, DbConfig, KafkaConfig};
use identity::crypto::kek::Kek;
use identity::db::DatabasePool;
use identity::proto::identity_service_client::IdentityServiceClient;
use sqlx::PgPool;
use testcontainers_modules::postgres::Postgres;
use testcontainers_modules::testcontainers::runners::AsyncRunner;
use tokio::net::TcpListener;
use tonic::transport::Channel;
use uuid::Uuid;

pub struct TestUser {
    pub username: String,
    pub email: String,
    pub given_name: String,
    pub family_name: String,
    pub password: String,
}

pub fn random_user_data() -> TestUser {
    let id = &Uuid::new_v4().to_string()[..8];
    TestUser {
        username: format!("user-{id}"),
        email: format!("user-{id}@test.example.com"),
        given_name: format!("First-{id}"),
        family_name: format!("Last-{id}"),
        password: format!("password-{id}"),
    }
}

pub async fn test_db_pool() -> (
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

pub fn test_kek() -> Arc<Kek> {
    let mut bytes = vec![0u8; 32];
    aws_lc_rs::rand::fill(&mut bytes).unwrap();
    Arc::new(Kek::from_bytes(bytes).unwrap())
}

pub fn test_config(addr: SocketAddr) -> AppConfig {
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

pub async fn activate_user_by_email(pool: &PgPool, email: &str) {
    sqlx::query!(
        "UPDATE users SET status = 'active', email_verified = TRUE, updated_at = NOW()
         WHERE LOWER(email) = LOWER($1) AND deleted_at IS NULL",
        email,
    )
    .execute(pool)
    .await
    .unwrap();
}

pub async fn start_test_server() -> (
    SocketAddr,
    testcontainers_modules::testcontainers::ContainerAsync<Postgres>,
    PgPool,
) {
    let (_container, pool) = test_db_pool().await;

    let kek = test_kek();

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

/// Register a user, activate them, log in, and return the access token.
pub async fn register_activate_login(
    client: &mut IdentityServiceClient<Channel>,
    pool: &PgPool,
    user: &TestUser,
) -> String {
    client
        .sign_up(identity::proto::SignUpRequest {
            username: user.username.clone(),
            given_name: user.given_name.clone(),
            family_name: user.family_name.clone(),
            email: user.email.clone(),
            password: Some(user.password.clone()),
        })
        .await
        .unwrap();

    activate_user_by_email(pool, &user.email).await;

    let response = client
        .login(identity::proto::LoginRequest {
            email: user.email.clone(),
            password: user.password.clone(),
        })
        .await
        .unwrap();

    response.into_inner().access_token
}

/// Build an authenticated tonic request with a Bearer token.
pub fn authenticated_request<T>(inner: T, token: &str) -> tonic::Request<T> {
    let mut request = tonic::Request::new(inner);
    request
        .metadata_mut()
        .insert("authorization", format!("Bearer {token}").parse().unwrap());
    request
}
