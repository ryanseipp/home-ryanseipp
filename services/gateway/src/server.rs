use std::io::BufReader;
use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use hyper::Request;
use hyper::body::Incoming;
use hyper::service::service_fn;
use hyper_util::rt::{TokioExecutor, TokioIo};
use hyper_util::server::conn::auto::Builder;
use rustls::ServerConfig;
use rustls_pemfile::{certs, private_key};
use tokio::fs;
use tokio::net::TcpListener;
use tokio::signal::unix::{SignalKind, signal};
use tokio_rustls::TlsAcceptor;
use tower::ServiceExt;

use crate::config::AppConfig;
use crate::routes;
use crate::routes::AppState;

/// Errors from the HTTP server.
#[derive(Debug, thiserror::Error)]
pub enum ServerError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("TLS error: {0}")]
    Tls(#[from] rustls::Error),

    #[error("failed to read TLS cert/key: {0}")]
    CertLoad(String),

    #[error("no private key found in PEM file")]
    MissingKey,
}

/// Build and run the HTTP server.
///
/// Accepts a `TcpListener` for testability — tests bind port 0 and pass
/// their own dependencies, while production builds them from config in `main`.
///
/// External HTTPS uses file-based certs (publicly-trusted, e.g. Let's Encrypt).
/// Internal mTLS to backends uses SPIFFE — that's handled in `identity_client`.
///
/// # Errors
///
/// Returns `ServerError` if binding, TLS setup, or serving fails.
///
/// # Panics
///
/// Panics if the SIGTERM signal handler cannot be installed.
pub async fn run(
    listener: TcpListener,
    config: &AppConfig,
    state: Option<AppState>,
) -> Result<(), ServerError> {
    let addr = listener.local_addr()?;
    tracing::info!(%addr, "starting gateway HTTP server");

    let app = routes::router(state);

    let shutdown = async {
        signal(SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
        tracing::info!("received SIGTERM, shutting down");
    };

    if config.tls_available() {
        serve_tls(listener, app, config, shutdown).await?;
    } else {
        tracing::warn!("TLS disabled — cert/key files not found");
        axum::serve(listener, app)
            .with_graceful_shutdown(shutdown)
            .await?;
    }

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
) -> Result<(), ServerError> {
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

                    let service = service_fn(
                        move |req: Request<Incoming>| {
                            let app = app.clone();
                            async move { app.oneshot(req.map(Body::new)).await }
                        },
                    );

                    let builder = Builder::new(
                        TokioExecutor::new(),
                    );

                    if let Err(e) = builder
                        .serve_connection(
                            TokioIo::new(tls_stream),
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
async fn load_rustls_config(cert_path: &str, key_path: &str) -> Result<ServerConfig, ServerError> {
    let cert_data = fs::read(cert_path)
        .await
        .map_err(|e| ServerError::CertLoad(format!("failed to read cert at {cert_path}: {e}")))?;
    let key_data = fs::read(key_path)
        .await
        .map_err(|e| ServerError::CertLoad(format!("failed to read key at {key_path}: {e}")))?;

    let cert_chain: Vec<_> = certs(&mut BufReader::new(&*cert_data))
        .collect::<Result<_, _>>()
        .map_err(|e| ServerError::CertLoad(format!("failed to parse cert PEM: {e}")))?;
    let key = private_key(&mut BufReader::new(&*key_data))
        .map_err(|e| ServerError::CertLoad(format!("failed to parse key PEM: {e}")))?
        .ok_or(ServerError::MissingKey)?;

    let mut tls_config = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(cert_chain, key)?;

    tls_config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];

    Ok(tls_config)
}
