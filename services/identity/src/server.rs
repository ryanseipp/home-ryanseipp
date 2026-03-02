use tokio::net::TcpListener;
use tokio_stream::wrappers::TcpListenerStream;
use tonic::transport::server::Server;

use crate::config::AppConfig;
use crate::proto::identity_service_server::IdentityServiceServer;
use crate::service::IdentityServiceImpl;

/// Build and run the gRPC server.
///
/// Accepts a `TcpListener` for testability — tests bind port 0 and pass the
/// listener in, while production binds the configured address in `main`.
pub async fn run(
    listener: TcpListener,
    config: &AppConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    let addr = listener.local_addr()?;
    tracing::info!(%addr, "starting identity gRPC server");

    let service = IdentityServiceImpl::new();
    let svc = IdentityServiceServer::new(service);
    let incoming = TcpListenerStream::new(listener);

    let shutdown = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
        tracing::info!("received SIGTERM, shutting down");
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
