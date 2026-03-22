use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use rustls::pki_types::ServerName;
use spiffe::X509Source;
use spiffe::x509_source::X509SourceError;
use spiffe_rustls::authorizer;
use tokio::sync::RwLock;
use tonic::transport::{self, Channel, Endpoint};
use tonic_tls::rustls::TlsConnector;

use crate::proto::identity::v1::identity_service_client::IdentityServiceClient;

/// Errors from channel creation.
#[derive(Debug, thiserror::Error)]
pub enum ChannelError {
    #[error("SPIFFE source error: {0}")]
    Source(#[from] X509SourceError),

    #[error("failed to build TLS config: {0}")]
    Tls(#[from] spiffe_rustls::Error),

    #[error("failed to connect: {0}")]
    Transport(#[from] transport::Error),
}

/// A SPIFFE mTLS channel to the identity service that lazily reconnects
/// when the X.509 SVID rotates.
///
/// TLS is handled by `tonic-tls` with the SPIFFE `ClientConfig`, which
/// installs a dynamic cert resolver so new handshakes automatically pick
/// up rotated SVIDs. The sequence check provides defense-in-depth by
/// forcing a reconnect after rotation, since long-lived HTTP/2 connections
/// don't re-handshake on their own.
pub struct IdentityChannel {
    channel: RwLock<Channel>,
    source: X509Source,
    addr: String,
    tls_config: Arc<rustls::ClientConfig>,
    server_name: ServerName<'static>,
    created_seq: AtomicU64,
}

impl IdentityChannel {
    /// Create a new mTLS channel to the identity service.
    ///
    /// # Errors
    ///
    /// Returns `ChannelError` if SPIFFE source initialization, TLS config
    /// building, or the initial connection fails.
    pub async fn new(
        addr: String,
        source: X509Source,
        server_name: ServerName<'static>,
    ) -> Result<Self, ChannelError> {
        // TODO: scope to specific SPIFFE IDs in production
        let tls_config = spiffe_rustls::mtls_client(source.clone())
            .authorize(authorizer::any())
            .with_alpn_protocols([b"h2".as_slice()])
            .build()?;
        let tls_config = Arc::new(tls_config);

        let channel = Self::connect(&addr, &tls_config, &server_name).await?;
        let created_seq = AtomicU64::new(source.updated().last());

        Ok(Self {
            channel: RwLock::new(channel),
            source,
            addr,
            tls_config,
            server_name,
            created_seq,
        })
    }

    /// Returns a strongly-typed identity service client.
    ///
    /// Checks whether the SVID has rotated since the channel was created.
    /// If so, rebuilds the channel (new TLS handshake picks up the fresh
    /// SVID) before returning the client.
    ///
    /// # Errors
    ///
    /// Returns `ChannelError` if reconnection after SVID rotation fails.
    pub async fn client(&self) -> Result<IdentityServiceClient<Channel>, ChannelError> {
        let current_seq = self.source.updated().last();
        let prev_seq = self.created_seq.load(Ordering::Relaxed);

        if current_seq != prev_seq {
            let mut ch = self.channel.write().await;

            // Double-check: another task may have rebuilt already.
            let recheck = self.created_seq.load(Ordering::Relaxed);
            if recheck != current_seq {
                tracing::debug!(
                    old_seq = recheck,
                    new_seq = current_seq,
                    "SVID rotated, rebuilding channel"
                );
                *ch = Self::connect(&self.addr, &self.tls_config, &self.server_name).await?;
                self.created_seq.store(current_seq, Ordering::Relaxed);
            }

            return Ok(IdentityServiceClient::new(ch.clone()));
        }

        let ch = self.channel.read().await;
        Ok(IdentityServiceClient::new(ch.clone()))
    }

    async fn connect(
        addr: &str,
        tls_config: &Arc<rustls::ClientConfig>,
        server_name: &ServerName<'static>,
    ) -> Result<Channel, ChannelError> {
        let endpoint = Endpoint::from_shared(addr.to_string())?;
        let transport = tonic_tls::TcpTransport::from_endpoint(&endpoint);
        let connector = TlsConnector::new(transport, Arc::clone(tls_config), server_name.clone());

        let channel = endpoint.connect_with_connector(connector).await?;
        Ok(channel)
    }
}
