pub mod health;

use axum::Router;
use tower_http::trace::TraceLayer;

use crate::config::AppConfig;

/// Build the application router with all routes and middleware.
pub async fn router(_config: &AppConfig) -> Result<Router, Box<dyn std::error::Error>> {
    let app = Router::new()
        .merge(health::routes())
        .layer(TraceLayer::new_for_http());

    Ok(app)
}
