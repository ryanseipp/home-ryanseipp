use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize telemetry: traces+metrics via OTLP, JSON logs to stdout.
    // Guard must be held until shutdown to ensure flush.
    let _telemetry_guard = identity::telemetry::init()?;

    let config = identity::config::AppConfig::load()?;

    let kek = Arc::new(identity::crypto::kek::load_kek()?);
    tracing::info!("KEK loaded");

    let db = identity::db::DatabasePool::connect(&config.db, config.db_read.as_ref()).await?;
    db.migrate().await?;
    tracing::info!("database migrations applied");

    // Ensure at least one signing key exists for JWT operations
    identity::services::ensure_signing_key(db.writer(), &kek).await?;

    let listener = tokio::net::TcpListener::bind(config.listen_addr).await?;
    tracing::info!(addr = %config.listen_addr, "bound TCP listener");

    identity::server::run(listener, &config, db, kek).await?;

    Ok(())
}
