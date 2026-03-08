use std::io::BufReader;
use std::sync::Arc;

use axum::Router;
use hyper::Request;
use hyper::body::Incoming;
use rustls::ServerConfig;
use rustls_pemfile::{certs, private_key};
use tokio::net::TcpListener;
use tokio_rustls::TlsAcceptor;
use tokio_util::sync::CancellationToken;
use tower::ServiceExt;

use crate::config::AppConfig;
use crate::routes;

/// Build and run the HTTP server.
///
/// Accepts a `TcpListener` for testability — tests bind port 0 and pass
/// their own dependencies, while production builds them from config in `main`.
pub async fn run(
    listener: TcpListener,
    config: &AppConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    let addr = listener.local_addr()?;
    tracing::info!(%addr, "starting gateway HTTP server");

    let cancel = CancellationToken::new();

    let app = routes::router(config).await?;

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
        serve_tls(listener, app, config, shutdown).await?;
    } else {
        tracing::warn!("TLS disabled — cert/key files not found");
        axum::serve(listener, app)
            .with_graceful_shutdown(shutdown)
            .await?;
    }

    cancel.cancel();
    Ok(())
}

/// Serve HTTPS using tokio-rustls + hyper-util.
///
/// Accepts TCP connections, performs TLS handshake, and feeds encrypted
/// streams into hyper's auto-detecting HTTP/1+HTTP/2 connection builder.
async fn serve_tls(
    listener: TcpListener,
    app: Router,
    config: &AppConfig,
    shutdown: impl std::future::Future<Output = ()>,
) -> Result<(), Box<dyn std::error::Error>> {
    let tls_config = load_rustls_config(&config.tls_cert_file, &config.tls_key_file).await?;

    tracing::info!(
        cert = %config.tls_cert_file,
        key = %config.tls_key_file,
        "TLS enabled"
    );

    let tls_acceptor = TlsAcceptor::from(Arc::new(tls_config));

    tokio::pin!(shutdown);

    loop {
        tokio::select! {
            result = listener.accept() => {
                let (tcp_stream, remote_addr) = result?;
                let acceptor = tls_acceptor.clone();
                let app = app.clone();

                tokio::spawn(async move {
                    let tls_stream = match acceptor.accept(tcp_stream).await {
                        Ok(s) => s,
                        Err(e) => {
                            tracing::debug!(error = %e, %remote_addr, "TLS handshake failed");
                            return;
                        }
                    };

                    let service = hyper::service::service_fn(
                        move |req: Request<Incoming>| {
                            let app = app.clone();
                            async move { app.oneshot(req.map(axum::body::Body::new)).await }
                        },
                    );

                    let builder = hyper_util::server::conn::auto::Builder::new(
                        hyper_util::rt::TokioExecutor::new(),
                    );

                    if let Err(e) = builder
                        .serve_connection(
                            hyper_util::rt::TokioIo::new(tls_stream),
                            service,
                        )
                        .await
                    {
                        tracing::debug!(error = %e, %remote_addr, "connection error");
                    }
                });
            }
            () = &mut shutdown => {
                tracing::info!("shutting down TLS server");
                break;
            }
        }
    }

    Ok(())
}

/// Load TLS configuration from PEM files on disk.
///
/// Uses aws-lc-rs as the crypto provider via rustls.
async fn load_rustls_config(
    cert_path: &str,
    key_path: &str,
) -> Result<ServerConfig, Box<dyn std::error::Error>> {
    let cert_data = tokio::fs::read(cert_path)
        .await
        .map_err(|e| format!("failed to read TLS cert at {cert_path}: {e}"))?;
    let key_data = tokio::fs::read(key_path)
        .await
        .map_err(|e| format!("failed to read TLS key at {key_path}: {e}"))?;

    let cert_chain: Vec<_> = certs(&mut BufReader::new(&*cert_data)).collect::<Result<_, _>>()?;
    let key =
        private_key(&mut BufReader::new(&*key_data))?.ok_or("no private key found in PEM file")?;

    let mut tls_config = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(cert_chain, key)?;

    tls_config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];

    Ok(tls_config)
}
