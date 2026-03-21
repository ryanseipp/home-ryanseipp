use std::net::SocketAddr;

use config::ConfigError;
use serde::Deserialize;

const DEFAULT_LISTEN_ADDR: &str = "[::]:50051";

const DEFAULT_WEB_BASE_URL: &str = "https://home.ryanseipp.com";
const DEFAULT_KAFKA_BROKERS: &str = "localhost:9092";
const DEFAULT_KAFKA_POLL_INTERVAL_MS: u64 = 5000;
const DEFAULT_KAFKA_BATCH_SIZE: i64 = 50;

const DEFAULT_DB_PORT: u16 = 5432;
const DEFAULT_MAX_CONNECTIONS: u32 = 5;

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

    /// Override for the SPIFFE Workload API socket path.
    /// When absent, `X509Source` reads `SPIFFE_ENDPOINT_SOCKET` from the env.
    #[serde(default)]
    pub spiffe_endpoint_socket: Option<String>,
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
/// TLS cert material (CA cert, client cert, client key) is provided at runtime
/// from the SPIFFE X509Source rather than file paths. When no SPIFFE source is
/// available (dev/test), connections fall back to `PgSslMode::Prefer`.
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

    /// Maximum number of connections in the pool. Default: `5`
    #[serde(default = "default_max_connections")]
    pub max_connections: u32,
}

fn default_listen_addr() -> SocketAddr {
    DEFAULT_LISTEN_ADDR
        .parse()
        .expect("valid default listen address")
}

fn default_db_port() -> u16 {
    DEFAULT_DB_PORT
}

fn default_max_connections() -> u32 {
    DEFAULT_MAX_CONNECTIONS
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
            .add_source(config::Environment::with_prefix("IDENTITY").separator("__"))
            .build()?;

        cfg.try_deserialize()
    }
}
