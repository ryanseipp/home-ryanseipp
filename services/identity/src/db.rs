use std::str::FromStr;
use std::sync::Arc;

use deadpool_postgres::{Manager, ManagerConfig, Pool, RecyclingMethod};
use tokio_postgres::NoTls;
use tokio_postgres_rustls::MakeRustlsConnect;

use crate::config::DbConfig;

mod embedded {
    use refinery::embed_migrations;
    embed_migrations!("./migrations");
}

/// Errors from database pool creation and migration.
#[derive(Debug, thiserror::Error)]
pub enum DatabaseError {
    #[error("invalid connection config: {0}")]
    Config(#[from] tokio_postgres::Error),

    #[error("failed to build pool: {0}")]
    Build(#[from] deadpool_postgres::BuildError),

    #[error("failed to get connection: {0}")]
    Pool(#[from] deadpool_postgres::PoolError),

    #[error("migration failed: {0}")]
    Migration(Box<refinery::Error>),
}

/// Database connection pool with write-primary / read-replica routing.
///
/// When no read replica is configured, `reader()` returns the writer pool.
#[derive(Clone)]
pub struct DatabasePool {
    writer: Pool,
    reader: Pool,
}

impl DatabasePool {
    /// Wrap an existing `Pool` as both writer and reader.
    ///
    /// Useful for tests where the pool is created from a testcontainers URL.
    pub fn from_pool(pool: Pool) -> Self {
        Self {
            writer: pool.clone(),
            reader: pool,
        }
    }

    /// Connect to the database(s) and return a pool pair.
    ///
    /// The `tls` config should be built via `spiffe_rustls::mtls_client()` so
    /// that its dynamic cert resolver handles SVID rotation automatically.
    pub fn connect(
        db: &DbConfig,
        db_read: Option<&DbConfig>,
        tls: Arc<rustls::ClientConfig>,
    ) -> Result<Self, DatabaseError> {
        let writer = build_pool(db, Arc::clone(&tls))?;

        let reader = match db_read {
            Some(read_cfg) => build_pool(read_cfg, Arc::clone(&tls))?,
            None => writer.clone(),
        };

        Ok(Self { writer, reader })
    }

    /// Create a pool from a connection URL without TLS (for tests).
    pub fn from_url(url: &str, max_size: usize) -> Result<Self, DatabaseError> {
        let pg_config = tokio_postgres::Config::from_str(url)?;
        let mgr = Manager::from_config(
            pg_config,
            NoTls,
            ManagerConfig {
                recycling_method: RecyclingMethod::Fast,
            },
        );
        let pool = Pool::builder(mgr).max_size(max_size).build()?;
        Ok(Self::from_pool(pool))
    }

    /// Writer pool — use for INSERT / UPDATE / DELETE.
    pub fn writer(&self) -> &Pool {
        &self.writer
    }

    /// Reader pool — use for SELECT. Routes to replica if configured,
    /// otherwise returns the writer pool.
    pub fn reader(&self) -> &Pool {
        &self.reader
    }

    /// Run embedded migrations on the writer (primary) database.
    pub async fn migrate(&self) -> Result<(), DatabaseError> {
        let mut client = self.writer.get().await?;
        embedded::migrations::runner()
            .run_async(&mut **client)
            .await
            .map_err(|e| DatabaseError::Migration(Box::new(e)))?;
        Ok(())
    }
}

/// Build a deadpool-postgres pool from config with TLS.
fn build_pool(cfg: &DbConfig, tls: Arc<rustls::ClientConfig>) -> Result<Pool, DatabaseError> {
    let mut pg_config = tokio_postgres::Config::new();
    pg_config
        .host(&cfg.host)
        .port(cfg.port)
        .dbname(&cfg.database)
        .user(&cfg.username);

    if let Some(pw) = &cfg.password {
        pg_config.password(pw);
    }

    let tls_connector = MakeRustlsConnect::new(Arc::unwrap_or_clone(tls));
    let mgr = Manager::from_config(
        pg_config,
        tls_connector,
        ManagerConfig {
            recycling_method: RecyclingMethod::Fast,
        },
    );

    let pool = Pool::builder(mgr)
        .max_size(cfg.max_connections as usize)
        .build()?;

    Ok(pool)
}
