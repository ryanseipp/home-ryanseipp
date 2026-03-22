use std::sync::Arc;

use identity::config::AppConfig;
use identity::crypto::kek::load_kek;
use identity::db::DatabasePool;
use identity::outbox::publisher;
use identity::server::run;
use identity::services::ensure_signing_key;
use identity::spiffe::SpiffeContext;
use identity::telemetry;
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize telemetry: traces+metrics via OTLP, JSON logs to stdout.
    // Guard must be held until shutdown to ensure flush.
    let _telemetry_guard = telemetry::init()?;

    let config = AppConfig::load()?;

    let kek = Arc::new(load_kek()?);
    tracing::info!("KEK loaded");

    let spiffe = SpiffeContext::new(config.spiffe_endpoint_socket.as_deref()).await?;

    let db = DatabasePool::connect(&config.db, config.db_read.as_ref(), &spiffe.client_tls)?;
    db.migrate().await?;
    tracing::info!("database migrations applied");

    // Ensure at least one signing key exists for JWT operations
    ensure_signing_key(db.writer(), &kek).await?;

    let publisher = publisher::spawn(
        db.writer().clone(),
        &config.kafka,
        Some(spiffe.client_tls.clone()),
    );

    let listener = TcpListener::bind(config.listen_addr).await?;
    tracing::info!(addr = %config.listen_addr, "bound TCP listener");

    run(
        listener,
        &config.web_base_url,
        db,
        kek,
        Some(spiffe.server_tls),
    )
    .await?;

    publisher.shutdown().await;

    Ok(())
}
