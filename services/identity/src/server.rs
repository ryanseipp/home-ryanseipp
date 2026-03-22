use std::sync::Arc;

use tokio::net::TcpListener;
use tokio::signal::unix::{self, SignalKind};
use tonic::transport::Error as TransportError;
use tonic::transport::server::{Server, TcpIncoming};
use tonic_tls::rustls::TlsIncoming;

use crate::crypto::Kek;
use crate::db::DatabasePool;
use crate::proto::identity_service_server::IdentityServiceServer;
use crate::services::IdentityServiceImpl;

/// Errors from the gRPC server.
#[derive(Debug, thiserror::Error)]
pub enum ServerError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("gRPC transport error: {0}")]
    Transport(#[from] TransportError),
}

/// Build and run the gRPC server.
///
/// Accepts a `TcpListener` and `DatabasePool` for testability —
/// tests bind port 0 and pass their own dependencies, while production
/// builds them from config in `main`.
///
/// When `server_tls` is `Some`, the server performs TLS via `tonic-tls`
/// on each incoming TCP connection.
///
/// # Errors
///
/// Returns `ServerError` if binding or serving fails.
///
/// # Panics
///
/// Panics if the SIGTERM signal handler cannot be installed.
pub async fn run(
    listener: TcpListener,
    web_base_url: &str,
    db: DatabasePool,
    kek: Arc<Kek>,
    server_tls: Option<Arc<rustls::ServerConfig>>,
) -> Result<(), ServerError> {
    let addr = listener.local_addr()?;
    tracing::info!(%addr, "starting identity gRPC server");

    let service = IdentityServiceImpl::new(db, web_base_url.to_owned(), kek);
    let svc = IdentityServiceServer::new(service);

    let shutdown = async {
        unix::signal(SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
        tracing::info!("received SIGTERM, shutting down");
    };

    let incoming = TcpIncoming::from(listener);

    if let Some(tls_config) = server_tls {
        tracing::info!("TLS enabled (SPIFFE)");
        let tls_incoming = TlsIncoming::new(incoming, tls_config);

        Server::builder()
            .add_service(svc)
            .serve_with_incoming_shutdown(tls_incoming, shutdown)
            .await?;
    } else {
        tracing::warn!("TLS disabled — no SPIFFE source available");

        Server::builder()
            .add_service(svc)
            .serve_with_incoming_shutdown(incoming, shutdown)
            .await?;
    }

    Ok(())
}
