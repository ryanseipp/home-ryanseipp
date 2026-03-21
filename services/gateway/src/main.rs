use std::sync::Arc;

use gateway::config::AppConfig;
use gateway::pool::IdentityChannel;
use gateway::routes::AppState;
use gateway::server::run;
use gateway::telemetry;
use rustls::pki_types::ServerName;
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize telemetry: traces+metrics via OTLP, JSON logs to stdout.
    // Guard must be held until shutdown to ensure flush.
    let _telemetry_guard = telemetry::init()?;

    let config = AppConfig::load()?;

    let source = spiffe::X509Source::new().await?;

    tracing::info!(
        spiffe_id = %source.svid()?.spiffe_id(),
        "SPIFFE identity loaded for backend mTLS"
    );

    let identity = Arc::new(
        IdentityChannel::new(
            config.identity.addr.clone(),
            source,
            ServerName::try_from("identity")?,
        )
        .await?,
    );

    let state = AppState { identity };

    let listener = TcpListener::bind(config.listen_addr).await?;
    tracing::info!(addr = %config.listen_addr, "bound TCP listener");

    run(listener, &config, Some(state)).await?;

    Ok(())
}
