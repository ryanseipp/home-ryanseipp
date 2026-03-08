use std::net::SocketAddr;

use config::ConfigError;
use serde::Deserialize;

const DEFAULT_LISTEN_ADDR: &str = "[::]:50051";
/// Default TLS certificate path (K8s cert-manager convention).
const DEFAULT_TLS_CERT_FILE: &str = "/var/run/secrets/tls/tls.crt";
/// Default TLS private key path (K8s cert-manager convention).
const DEFAULT_TLS_KEY_FILE: &str = "/var/run/secrets/tls/tls.key";

/// Default PostgreSQL TLS paths (separate cert-manager Certificate resource).
const DEFAULT_PG_CA_CERT: &str = "/var/run/secrets/pg-tls/ca.crt";
const DEFAULT_PG_CLIENT_CERT: &str = "/var/run/secrets/pg-tls/tls.crt";
const DEFAULT_PG_CLIENT_KEY: &str = "/var/run/secrets/pg-tls/tls.key";

const DEFAULT_WEB_BASE_URL: &str = "https://home.ryanseipp.com";
const DEFAULT_KAFKA_BROKERS: &str = "localhost:9092";
const DEFAULT_KAFKA_POLL_INTERVAL_MS: u64 = 5000;
const DEFAULT_KAFKA_BATCH_SIZE: i64 = 50;

const DEFAULT_DB_PORT: u16 = 5432;
const DEFAULT_SSL_MODE: &str = "verify-full";
const DEFAULT_MAX_CONNECTIONS: u32 = 5;
const DEFAULT_MIN_CONNECTIONS: u32 = 1;
/// Server configuration loaded entirely from environment variables.
///
/// All variables use the `IDENTITY_` prefix (e.g., `IDENTITY_LISTEN_ADDR`).
/// Nested structs use `__` as separator (e.g., `IDENTITY__DB__HOST`).
/// OTEL configuration is handled separately by the OpenTelemetry SDK via its
/// own standard environment variables.
#[derive(Debug, Deserialize)]
pub struct AppConfig {
    /// gRPC listen address. Default: `[::]:50051`
    #[serde(default = "default_listen_addr")]
    pub listen_addr: SocketAddr,

    /// Path to PEM-encoded TLS certificate chain.
    /// Default: `/var/run/secrets/tls/tls.crt`
    #[serde(default = "default_tls_cert_file")]
    pub tls_cert_file: String,

    /// Path to PEM-encoded TLS private key.
    /// Default: `/var/run/secrets/tls/tls.key`
    #[serde(default = "default_tls_key_file")]
    pub tls_key_file: String,

    /// Base URL for user-facing web links (e.g., email verification).
    /// Default: `https://home.ryanseipp.com`
    #[serde(default = "default_web_base_url")]
    pub web_base_url: String,

    /// Primary (writer) database configuration.
    pub db: DbConfig,

    /// Optional read-replica database configuration.
    /// When absent, reads use the primary (writer) pool.
    #[serde(default)]
    pub db_read: Option<DbConfig>,

    /// Kafka configuration for the outbox publisher.
    #[serde(default)]
    pub kafka: KafkaConfig,
}

/// Kafka configuration for the outbox publisher.
///
/// Environment variables: `IDENTITY__KAFKA__BROKERS`, etc.
#[derive(Debug, Clone, Deserialize)]
pub struct KafkaConfig {
    /// Comma-separated list of Kafka broker addresses. Default: `localhost:9092`
    #[serde(default = "default_kafka_brokers")]
    pub brokers: String,

    /// How often the publisher polls for unpublished events (ms). Default: `5000`
    #[serde(default = "default_kafka_poll_interval_ms")]
    pub poll_interval_ms: u64,

    /// Maximum number of events fetched per poll. Default: `50`
    #[serde(default = "default_kafka_batch_size")]
    pub batch_size: i64,
}

impl Default for KafkaConfig {
    fn default() -> Self {
        Self {
            brokers: default_kafka_brokers(),
            poll_interval_ms: default_kafka_poll_interval_ms(),
            batch_size: default_kafka_batch_size(),
        }
    }
}

/// Primary database connection configuration.
///
/// Environment variables: `IDENTITY__DB__HOST`, `IDENTITY__DB__PORT`, etc.
///
/// TLS defaults follow cert-manager volume conventions with paths separate from
/// the gRPC server TLS (different `Certificate` resource). When `password` is
/// absent and client cert files exist, cert-based (mTLS) auth is used.
#[derive(Debug, Deserialize)]
pub struct DbConfig {
    /// PostgreSQL host.
    pub host: String,

    /// PostgreSQL port. Default: `5432`
    #[serde(default = "default_db_port")]
    pub port: u16,

    /// Database name.
    pub database: String,

    /// Database username.
    pub username: String,

    /// Database password. Optional — prefer client certificate auth.
    #[serde(default)]
    pub password: Option<String>,

    /// SSL mode: `disable`, `prefer`, `require`, `verify-ca`, `verify-full`.
    /// Default: `verify-full`
    #[serde(default = "default_ssl_mode")]
    pub ssl_mode: String,

    /// Path to CA certificate for verifying the server.
    /// Default: `/var/run/secrets/pg-tls/ca.crt`
    #[serde(default = "default_pg_ca_cert")]
    pub ssl_root_cert: String,

    /// Path to client certificate for mTLS auth.
    /// Default: `/var/run/secrets/pg-tls/tls.crt`
    #[serde(default = "default_pg_client_cert")]
    pub ssl_client_cert: String,

    /// Path to client private key for mTLS auth.
    /// Default: `/var/run/secrets/pg-tls/tls.key`
    #[serde(default = "default_pg_client_key")]
    pub ssl_client_key: String,

    /// Maximum number of connections in the pool. Default: `5`
    #[serde(default = "default_max_connections")]
    pub max_connections: u32,

    /// Minimum number of idle connections in the pool. Default: `1`
    #[serde(default = "default_min_connections")]
    pub min_connections: u32,
}

fn default_listen_addr() -> SocketAddr {
    DEFAULT_LISTEN_ADDR
        .parse()
        .expect("valid default listen address")
}

fn default_tls_cert_file() -> String {
    DEFAULT_TLS_CERT_FILE.into()
}

fn default_tls_key_file() -> String {
    DEFAULT_TLS_KEY_FILE.into()
}

fn default_db_port() -> u16 {
    DEFAULT_DB_PORT
}

fn default_ssl_mode() -> String {
    DEFAULT_SSL_MODE.into()
}

fn default_pg_ca_cert() -> String {
    DEFAULT_PG_CA_CERT.into()
}

fn default_pg_client_cert() -> String {
    DEFAULT_PG_CLIENT_CERT.into()
}

fn default_pg_client_key() -> String {
    DEFAULT_PG_CLIENT_KEY.into()
}

fn default_max_connections() -> u32 {
    DEFAULT_MAX_CONNECTIONS
}

fn default_min_connections() -> u32 {
    DEFAULT_MIN_CONNECTIONS
}

fn default_web_base_url() -> String {
    DEFAULT_WEB_BASE_URL.into()
}

fn default_kafka_brokers() -> String {
    DEFAULT_KAFKA_BROKERS.into()
}

fn default_kafka_poll_interval_ms() -> u64 {
    DEFAULT_KAFKA_POLL_INTERVAL_MS
}

fn default_kafka_batch_size() -> i64 {
    DEFAULT_KAFKA_BATCH_SIZE
}

impl AppConfig {
    /// Load configuration from environment variables with the `IDENTITY_` prefix.
    ///
    /// Uses `__` as the separator for nested configuration
    /// (e.g., `IDENTITY__DB__HOST`).
    pub fn load() -> Result<Self, ConfigError> {
        let cfg = config::Config::builder()
            .set_default("listen_addr", DEFAULT_LISTEN_ADDR)?
            .set_default("tls_cert_file", DEFAULT_TLS_CERT_FILE)?
            .set_default("tls_key_file", DEFAULT_TLS_KEY_FILE)?
            .add_source(config::Environment::with_prefix("IDENTITY").separator("__"))
            .build()?;

        cfg.try_deserialize()
    }

    /// Returns `true` when both TLS cert and key files exist on disk.
    ///
    /// TLS paths always have defaults (K8s cert-manager convention), so TLS
    /// is enabled by the *presence* of the files, not by the config values.
    /// This lets cert-manager and external-secrets inject certs without any
    /// config changes — mount them at the default paths and TLS activates.
    pub fn tls_available(&self) -> bool {
        std::path::Path::new(&self.tls_cert_file).exists()
            && std::path::Path::new(&self.tls_key_file).exists()
    }
}
