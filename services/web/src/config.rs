use std::net::SocketAddr;

use config::ConfigError;
use serde::Deserialize;

const DEFAULT_LISTEN_ADDR: &str = "[::]:3000";
const DEFAULT_GATEWAY_URL: &str = "http://[::1]:8080";

/// Server configuration loaded from environment variables.
///
/// All variables use the `WEB_` prefix (e.g., `WEB__LISTEN_ADDR`).
/// Nested structs use `__` as separator (e.g., `WEB__GATEWAY__URL`).
/// `OTel` configuration is handled by the OpenTelemetry SDK via its own
/// standard environment variables.
#[derive(Debug, Deserialize)]
pub struct AppConfig {
    /// HTTP listen address. Default: `[::]:3000`
    #[serde(default = "default_listen_addr")]
    pub listen_addr: SocketAddr,

    /// Gateway API configuration.
    #[serde(default)]
    pub gateway: GatewayConfig,
}

/// Configuration for connecting to the API Gateway.
///
/// Environment variable: `WEB__GATEWAY__URL`.
#[derive(Debug, Deserialize)]
pub struct GatewayConfig {
    /// Base URL of the API Gateway.
    /// Default: `http://[::1]:8080`
    #[serde(default = "default_gateway_url")]
    pub url: String,
}

impl Default for GatewayConfig {
    fn default() -> Self {
        Self {
            url: default_gateway_url(),
        }
    }
}

fn default_listen_addr() -> SocketAddr {
    DEFAULT_LISTEN_ADDR
        .parse()
        .expect("valid default listen address")
}

fn default_gateway_url() -> String {
    DEFAULT_GATEWAY_URL.into()
}

impl AppConfig {
    /// Load configuration from environment variables with the `WEB_` prefix.
    ///
    /// # Errors
    ///
    /// Returns `ConfigError` if environment variables cannot be parsed.
    pub fn load() -> Result<Self, ConfigError> {
        let cfg = config::Config::builder()
            .set_default("listen_addr", DEFAULT_LISTEN_ADDR)?
            .add_source(config::Environment::with_prefix("WEB").separator("__"))
            .build()?;

        cfg.try_deserialize()
    }
}
