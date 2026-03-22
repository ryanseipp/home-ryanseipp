pub mod auth;
pub mod error;
pub mod health;
pub mod session;

use std::sync::Arc;

use axum::Router;
use tower_http::trace::TraceLayer;

use crate::pool::IdentityChannel;
use crate::session::SessionStore;

/// Shared application state available to route handlers.
#[derive(Clone)]
pub struct AppState {
    pub identity: Arc<IdentityChannel>,
    pub sessions: Arc<SessionStore>,
}

/// Build the application router with all routes and middleware.
///
/// When `state` is `Some`, auth routes are mounted. Health routes
/// are always available regardless of backend connectivity.
pub fn router(state: Option<AppState>) -> Router {
    let mut app = Router::new().merge(health::routes());

    if let Some(s) = state {
        app = app.merge(auth::routes().with_state(s));
    }

    app.layer(TraceLayer::new_for_http())
}
