mod spire;

pub use spire::{SpireTestCluster, SpireWorkloadEntry};

use deadpool_postgres::Pool;
use identity::db::DatabasePool;
use testcontainers_modules::postgres::Postgres;
use testcontainers_modules::testcontainers::ContainerAsync;
use testcontainers_modules::testcontainers::ImageExt;
use testcontainers_modules::testcontainers::core::Mount;
use testcontainers_modules::testcontainers::runners::AsyncRunner;

/// Start a testcontainers Postgres instance, run identity migrations, and
/// return the container handle (must be kept alive) and a connection pool.
///
/// Uses a tmpfs mount for PGDATA to avoid accumulating anonymous Docker
/// volumes from the Postgres image's VOLUME directive.
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
