use std::sync::Arc;

use spiffe::x509_source::X509SourceError;
use spiffe_rustls::authorizer;

/// Errors from SPIFFE context initialization.
#[derive(Debug, thiserror::Error)]
pub enum SpiffeError {
    #[error("failed to connect to SPIFFE Workload API: {0}")]
    Source(#[from] X509SourceError),

    #[error("failed to build TLS config: {0}")]
    Tls(#[from] spiffe_rustls::Error),
}

/// Centralised SPIFFE context: holds the X509Source and pre-built TLS configs.
///
/// Both `server_tls` and `client_tls` use `spiffe_rustls` dynamic cert
/// resolvers, so SVID rotation is handled automatically — no need to rebuild
/// configs when certificates change.
pub struct SpiffeContext {
    pub source: spiffe::X509Source,
    pub server_tls: Arc<rustls::ServerConfig>,
    pub client_tls: Arc<rustls::ClientConfig>,
}

impl SpiffeContext {
    /// Connect to the SPIFFE Workload API and build TLS configs.
    ///
    /// `endpoint` overrides the default `SPIFFE_ENDPOINT_SOCKET` env var.
    ///
    /// The returned `client_tls` config can be used for ALL outbound mTLS
    /// connections (gRPC, Kafka, PostgreSQL) — the dynamic cert resolver
    /// ensures fresh SVID material on every TLS handshake.
    pub async fn new(endpoint: Option<&str>) -> Result<Self, SpiffeError> {
        let endpoint = endpoint
            .map(String::from)
            .or_else(|| std::env::var("SPIFFE_ENDPOINT_SOCKET").ok())
            .unwrap_or_else(|| "/tmp/spire-agent/public/api.sock".into());

        tracing::info!(%endpoint, "connecting to SPIFFE Workload API");

        let source = spiffe::X509Source::builder()
            .endpoint(endpoint)
            .build()
            .await?;

        // TODO: scope to specific SPIFFE IDs in production
        let server_config = spiffe_rustls::mtls_server(source.clone())
            .authorize(authorizer::any())
            .with_alpn_protocols([b"h2".as_slice()])
            .build()?;

        // TODO: scope to specific SPIFFE IDs in production
        let client_config = spiffe_rustls::mtls_client(source.clone())
            .authorize(authorizer::any())
            .build()?;

        tracing::info!(
            spiffe_id = %source.svid().map_err(SpiffeError::Source)?.spiffe_id(),
            "SPIFFE identity loaded"
        );

        Ok(Self {
            source,
            server_tls: Arc::new(server_config),
            client_tls: Arc::new(client_config),
        })
    }
}
