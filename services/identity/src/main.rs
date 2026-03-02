#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize telemetry: traces+metrics via OTLP, JSON logs to stdout.
    // Guard must be held until shutdown to ensure flush.
    let _telemetry_guard = identity::telemetry::init()?;

    let config = identity::config::AppConfig::load()?;

    let listener = tokio::net::TcpListener::bind(config.listen_addr).await?;
    tracing::info!(addr = %config.listen_addr, "bound TCP listener");

    identity::server::run(listener, &config).await?;

    Ok(())
}
