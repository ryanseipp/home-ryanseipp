use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use deadpool_postgres::Pool;
use identity::config::{AppConfig, DbConfig, KafkaConfig};
use identity::crypto::kek::Kek;
use identity::db::DatabasePool;
use identity::proto::identity_service_client::IdentityServiceClient;
use identity::proto::{GetJwksRequest, LoginRequest, SignUpRequest};
use identity::server::run;
use identity::services::ensure_signing_key;
use testcontainers_modules::postgres::Postgres;
use testcontainers_modules::testcontainers::ContainerAsync;
use tokio::net::TcpListener;
use tokio::runtime::Runtime;
use tokio::task;
use tonic::transport::Channel;
use uuid::Uuid;

struct TestState {
    container: Option<ContainerAsync<Postgres>>,
    rt: Runtime,
    addr: SocketAddr,
    pool: Pool,
}

impl Drop for TestState {
    fn drop(&mut self) {
        if let Some(container) = self.container.take() {
            self.rt.block_on(async move { drop(container) });
        }
    }
}

static SIGN_UP_COUNTER: AtomicU64 = AtomicU64::new(0);

fn start_server() -> TestState {
    let rt = Runtime::new().unwrap();
    let (addr, container, pool) = rt.block_on(async {
        let (container, pool) = test_utils::test_db_pool().await;

        let kek = test_kek();
        ensure_signing_key(&pool, &kek).await.unwrap();

        let listener = TcpListener::bind("[::1]:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let config = test_config(addr);
        let db = DatabasePool::from_pool(pool.clone());

        tokio::spawn(async move {
            run(listener, &config.web_base_url, db, kek, None)
                .await
                .unwrap();
        });

        task::yield_now().await;

        (addr, container, pool)
    });
    TestState {
        container: Some(container),
        rt,
        addr,
        pool,
    }
}

fn test_kek() -> Arc<Kek> {
    let mut bytes = vec![0u8; 32];
    aws_lc_rs::rand::fill(&mut bytes).unwrap();
    Arc::new(Kek::from_bytes(bytes).unwrap())
}

fn test_config(addr: SocketAddr) -> AppConfig {
    AppConfig {
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
    }
}

async fn activate_user_by_email(pool: &Pool, email: &str) {
    let client = pool.get().await.unwrap();
    client
        .execute(
            "UPDATE users SET status = 'active', email_verified = TRUE, updated_at = NOW()
             WHERE LOWER(email) = LOWER($1) AND deleted_at IS NULL",
            &[&email],
        )
        .await
        .unwrap();
}

fn client(s: &TestState) -> IdentityServiceClient<Channel> {
    s.rt.block_on(async {
        IdentityServiceClient::connect(format!("http://[::1]:{}", s.addr.port()))
            .await
            .unwrap()
    })
}

#[divan::bench]
fn bench_grpc_sign_up(bencher: divan::Bencher) {
    let s = start_server();
    let c = client(&s);

    bencher.bench(|| {
        let n = SIGN_UP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let mut c = c.clone();
        s.rt.block_on(async {
            c.sign_up(SignUpRequest {
                username: format!("benchuser-{n}"),
                given_name: "Bench".into(),
                family_name: "User".into(),
                email: format!("bench-{n}@example.com"),
                password: Some("secure-password-123".into()),
            })
            .await
            .unwrap()
        })
    });
}

#[divan::bench]
fn bench_grpc_login(bencher: divan::Bencher) {
    let s = start_server();
    let c = client(&s);

    let id = &Uuid::new_v4().to_string()[..8];
    let email = format!("login-{id}@example.com");
    let password = "secure-password-123".to_string();

    s.rt.block_on(async {
        let mut c = c.clone();
        c.sign_up(SignUpRequest {
            username: format!("loginuser-{id}"),
            given_name: "Bench".into(),
            family_name: "User".into(),
            email: email.clone(),
            password: Some(password.clone()),
        })
        .await
        .unwrap();

        activate_user_by_email(&s.pool, &email).await;
    });

    bencher.bench(|| {
        let mut c = c.clone();
        s.rt.block_on(async {
            c.login(LoginRequest {
                email: email.clone(),
                password: password.clone(),
            })
            .await
            .unwrap()
        })
    });
}

#[divan::bench]
fn bench_grpc_get_jwks(bencher: divan::Bencher) {
    let s = start_server();
    let c = client(&s);

    bencher.bench(|| {
        let mut c = c.clone();
        s.rt.block_on(async { c.get_jwks(GetJwksRequest {}).await.unwrap() })
    });
}

fn main() {
    divan::main();
}
