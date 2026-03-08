use std::path::Path;

use sqlx::PgPool;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions, PgSslMode};

use crate::config::DbConfig;

/// Database connection pool with write-primary / read-replica routing.
///
/// When no read replica is configured, `reader()` returns the writer pool.
#[derive(Clone)]
pub struct DatabasePool {
    writer: PgPool,
    reader: PgPool,
}

impl DatabasePool {
    /// Wrap an existing `PgPool` as both writer and reader.
    ///
    /// Useful for tests where the pool is created from a testcontainers URL.
    pub fn from_pool(pool: PgPool) -> Self {
        Self {
            writer: pool.clone(),
            reader: pool,
        }
    }

    /// Connect to the database(s) and return a pool pair.
    ///
    /// Runs TLS configuration when cert files exist on disk; falls back to
    /// `PgSslMode::Prefer` for development/testing environments without TLS.
    pub async fn connect(db: &DbConfig, db_read: Option<&DbConfig>) -> Result<Self, sqlx::Error> {
        let writer_opts = build_connect_options(db);

        let writer = PgPoolOptions::new()
            .max_connections(db.max_connections)
            .min_connections(db.min_connections)
            .connect_with(writer_opts)
            .await?;

        let reader = match db_read {
            Some(read_cfg) => {
                let reader_opts = build_connect_options(read_cfg);

                PgPoolOptions::new()
                    .max_connections(read_cfg.max_connections)
                    .min_connections(read_cfg.min_connections)
                    .connect_with(reader_opts)
                    .await?
            }
            None => writer.clone(),
        };

        Ok(Self { writer, reader })
    }

    /// Writer pool — use for INSERT / UPDATE / DELETE.
    pub fn writer(&self) -> &PgPool {
        &self.writer
    }

    /// Reader pool — use for SELECT. Routes to replica if configured,
    /// otherwise returns the writer pool.
    pub fn reader(&self) -> &PgPool {
        &self.reader
    }

    /// Run embedded migrations on the writer (primary) database.
    pub async fn migrate(&self) -> Result<(), sqlx::migrate::MigrateError> {
        sqlx::migrate!("./migrations").run(&self.writer).await
    }

    /// Gracefully close both connection pools.
    pub async fn close(&self) {
        self.writer.close().await;
        self.reader.close().await;
    }
}

/// Parse an SSL mode string into `PgSslMode`.
fn parse_ssl_mode(mode: &str) -> PgSslMode {
    match mode {
        "disable" => PgSslMode::Disable,
        "allow" => PgSslMode::Allow,
        "prefer" => PgSslMode::Prefer,
        "require" => PgSslMode::Require,
        "verify-ca" => PgSslMode::VerifyCa,
        "verify-full" => PgSslMode::VerifyFull,
        other => {
            tracing::warn!(
                ssl_mode = other,
                "unknown ssl_mode, falling back to verify-full"
            );
            PgSslMode::VerifyFull
        }
    }
}

/// Build `PgConnectOptions` from config values.
///
/// Applies TLS settings only when the CA cert file exists on disk. This lets
/// the same binary work in TLS-enabled production (cert-manager mounts certs)
/// and in plain development/testing environments (no certs mounted).
fn build_connect_options(cfg: &DbConfig) -> PgConnectOptions {
    let mut opts = PgConnectOptions::new()
        .host(&cfg.host)
        .port(cfg.port)
        .database(&cfg.database)
        .username(&cfg.username);

    if let Some(pw) = &cfg.password {
        opts = opts.password(pw);
    }

    // When TLS cert files exist, use the configured SSL mode with full cert chain.
    // Otherwise fall back to Prefer for dev/test (e.g. testcontainers without TLS).
    if Path::new(&cfg.ssl_root_cert).exists() {
        opts = opts
            .ssl_mode(parse_ssl_mode(&cfg.ssl_mode))
            .ssl_root_cert(&cfg.ssl_root_cert);

        // Client certs for mTLS auth — only set if files exist.
        if Path::new(&cfg.ssl_client_cert).exists() && Path::new(&cfg.ssl_client_key).exists() {
            opts = opts
                .ssl_client_cert(&cfg.ssl_client_cert)
                .ssl_client_key(&cfg.ssl_client_key);
        }
    } else {
        opts = opts.ssl_mode(PgSslMode::Prefer);
    }

    opts
}
