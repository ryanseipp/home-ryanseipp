use std::sync::Arc;

use tokio::net::TcpListener;
use tokio_stream::wrappers::TcpListenerStream;
use tokio_util::sync::CancellationToken;
use tonic::transport::server::Server;

use crate::config::AppConfig;
use crate::crypto::Kek;
use crate::db::DatabasePool;
use crate::outbox;
use crate::proto::identity_service_server::IdentityServiceServer;
use crate::services::IdentityServiceImpl;

/// Build and run the gRPC server.
///
/// Accepts a `TcpListener` and `DatabasePool` for testability —
/// tests bind port 0 and pass their own dependencies, while production
/// builds them from config in `main`.
pub async fn run(
    listener: TcpListener,
    config: &AppConfig,
    db: DatabasePool,
    kek: Arc<Kek>,
) -> Result<(), Box<dyn std::error::Error>> {
    let addr = listener.local_addr()?;
    tracing::info!(%addr, "starting identity gRPC server");

    let cancel = CancellationToken::new();

    // Spawn outbox publisher as a background task
    let publisher_pool = db.writer().clone();
    let publisher_cancel = cancel.clone();
    let publisher_config = config.kafka.clone();
    let publisher_handle = tokio::spawn(async move {
        if let Err(e) =
            outbox::publisher::run(publisher_pool, &publisher_config, publisher_cancel).await
        {
            tracing::error!(error = %e, "outbox publisher failed");
        }
    });

    let service = IdentityServiceImpl::new(db, config.web_base_url.clone(), kek);
    let svc = IdentityServiceServer::new(service);
    let incoming = TcpListenerStream::new(listener);

    let shutdown_cancel = cancel.clone();
    let shutdown = async move {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
        tracing::info!("received SIGTERM, shutting down");
        shutdown_cancel.cancel();
    };

    if config.tls_available() {
        let tls_config = load_tls_config(&config.tls_cert_file, &config.tls_key_file).await?;
        tracing::info!(
            cert = %config.tls_cert_file,
            key = %config.tls_key_file,
            "TLS enabled"
        );

        Server::builder()
            .tls_config(tls_config)?
            .add_service(svc)
            .serve_with_incoming_shutdown(incoming, shutdown)
            .await?;
    } else {
        tracing::warn!("TLS disabled — cert/key files not found");

        Server::builder()
            .add_service(svc)
            .serve_with_incoming_shutdown(incoming, shutdown)
            .await?;
    }

    // Signal the publisher to stop (in case shutdown wasn't triggered by SIGTERM)
    cancel.cancel();

    // Wait for the publisher to finish
    if let Err(e) = publisher_handle.await {
        tracing::error!(error = %e, "outbox publisher task panicked");
    }

    Ok(())
}

/// Load TLS configuration from PEM files on disk.
///
/// Uses aws-lc-rs as the crypto provider via tonic's `tls-aws-lc` feature.
async fn load_tls_config(
    cert_path: &str,
    key_path: &str,
) -> Result<tonic::transport::ServerTlsConfig, Box<dyn std::error::Error>> {
    let cert_pem = tokio::fs::read_to_string(cert_path)
        .await
        .map_err(|e| format!("failed to read TLS cert at {cert_path}: {e}"))?;
    let key_pem = tokio::fs::read_to_string(key_path)
        .await
        .map_err(|e| format!("failed to read TLS key at {key_path}: {e}"))?;

    let identity = tonic::transport::Identity::from_pem(cert_pem, key_pem);
    let tls_config = tonic::transport::ServerTlsConfig::new().identity(identity);

    Ok(tls_config)
}
