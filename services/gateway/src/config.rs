use std::net::SocketAddr;

use config::ConfigError;
use serde::Deserialize;

const DEFAULT_LISTEN_ADDR: &str = "[::]:8080";
/// Default TLS certificate path (K8s cert-manager convention).
const DEFAULT_TLS_CERT_FILE: &str = "/var/run/secrets/tls/tls.crt";
/// Default TLS private key path (K8s cert-manager convention).
const DEFAULT_TLS_KEY_FILE: &str = "/var/run/secrets/tls/tls.key";

const DEFAULT_IDENTITY_ADDR: &str = "http://[::1]:50051";

const DEFAULT_SCYLLA_CONTACT_POINT: &str = "[::1]:9042";
const DEFAULT_SCYLLA_KEYSPACE: &str = "gateway";

/// Server configuration loaded entirely from environment variables.
///
/// All variables use the `GATEWAY_` prefix (e.g., `GATEWAY_LISTEN_ADDR`).
/// Nested structs use `__` as separator (e.g., `GATEWAY__IDENTITY__ADDR`).
/// OTEL configuration is handled separately by the OpenTelemetry SDK via its
/// own standard environment variables.
#[derive(Debug, Deserialize)]
pub struct AppConfig {
    /// HTTP listen address. Default: `[::]:8080`
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

    /// Identity gRPC backend configuration.
    #[serde(default)]
    pub identity: IdentityConfig,

    /// `ScyllaDB` session store configuration.
    #[serde(default)]
    pub scylla: ScyllaConfig,
}

/// Configuration for connecting to the Identity gRPC service.
///
/// Environment variables: `GATEWAY__IDENTITY__ADDR`.
#[derive(Debug, Deserialize)]
pub struct IdentityConfig {
    /// gRPC endpoint for the identity service.
    /// Default: `http://[::1]:50051`
    #[serde(default = "default_identity_addr")]
    pub addr: String,
}

impl Default for IdentityConfig {
    fn default() -> Self {
        Self {
            addr: default_identity_addr(),
        }
    }
}

/// Configuration for connecting to `ScyllaDB`.
///
/// Environment variables: `GATEWAY__SCYLLA__CONTACT_POINTS`,
/// `GATEWAY__SCYLLA__KEYSPACE`.
#[derive(Debug, Deserialize)]
pub struct ScyllaConfig {
    /// `ScyllaDB` contact points (host:port).
    /// Default: `["[::1]:9042"]`
    #[serde(default = "default_scylla_contact_points")]
    pub contact_points: Vec<String>,

    /// `ScyllaDB` keyspace for session storage.
    /// Default: `"gateway"`
    #[serde(default = "default_scylla_keyspace")]
    pub keyspace: String,
}

impl Default for ScyllaConfig {
    fn default() -> Self {
        Self {
            contact_points: default_scylla_contact_points(),
            keyspace: default_scylla_keyspace(),
        }
    }
}

fn default_scylla_contact_points() -> Vec<String> {
    vec![DEFAULT_SCYLLA_CONTACT_POINT.into()]
}

fn default_scylla_keyspace() -> String {
    DEFAULT_SCYLLA_KEYSPACE.into()
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

fn default_identity_addr() -> String {
    DEFAULT_IDENTITY_ADDR.into()
}

impl AppConfig {
    /// Load configuration from environment variables with the `GATEWAY_` prefix.
    ///
    /// Uses `__` as the separator for nested configuration
    /// (e.g., `GATEWAY__IDENTITY__ADDR`).
    ///
    /// # Errors
    ///
    /// Returns `ConfigError` if environment variables cannot be parsed or
    /// deserialized into the configuration struct.
    pub fn load() -> Result<Self, ConfigError> {
        let cfg = config::Config::builder()
            .set_default("listen_addr", DEFAULT_LISTEN_ADDR)?
            .set_default("tls_cert_file", DEFAULT_TLS_CERT_FILE)?
            .set_default("tls_key_file", DEFAULT_TLS_KEY_FILE)?
            .add_source(config::Environment::with_prefix("GATEWAY").separator("__"))
            .build()?;

        cfg.try_deserialize()
    }

    /// Returns `true` when both TLS cert and key files exist on disk.
    ///
    /// TLS paths always have defaults (K8s cert-manager convention), so TLS
    /// is enabled by the *presence* of the files, not by the config values.
    /// This lets cert-manager and external-secrets inject certs without any
    /// config changes — mount them at the default paths and TLS activates.
    #[must_use]
    pub fn tls_available(&self) -> bool {
        std::path::Path::new(&self.tls_cert_file).exists()
            && std::path::Path::new(&self.tls_key_file).exists()
    }
}
