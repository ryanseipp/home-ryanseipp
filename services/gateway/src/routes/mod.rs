pub mod auth;
pub mod error;
pub mod health;
pub mod session;

use std::sync::Arc;

use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::http::{HeaderName, HeaderValue, header};
use tower_http::set_header::SetResponseHeaderLayer;
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
        .layer(DefaultBodyLimit::max(1_048_576))
        .layer(SetResponseHeaderLayer::overriding(
            header::STRICT_TRANSPORT_SECURITY,
            HeaderValue::from_static("max-age=63072000; includeSubDomains; preload"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            header::X_CONTENT_TYPE_OPTIONS,
            HeaderValue::from_static("nosniff"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            header::X_FRAME_OPTIONS,
            HeaderValue::from_static("DENY"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            HeaderName::from_static("referrer-policy"),
            HeaderValue::from_static("strict-origin-when-cross-origin"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            HeaderName::from_static("permissions-policy"),
            HeaderValue::from_static(
                "accelerometer=(), camera=(), geolocation=(), gyroscope=(), \
                 magnetometer=(), microphone=(), payment=(), usb=()",
            ),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            HeaderName::from_static("content-security-policy"),
            HeaderValue::from_static("default-src 'none'; frame-ancestors 'none'; base-uri 'none'"),
        ))
}

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    use super::*;

    #[tokio::test]
    async fn security_headers_present_on_health() {
        let app = router(None);
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/healthz")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(
            resp.headers().get("strict-transport-security").unwrap(),
            "max-age=63072000; includeSubDomains; preload"
        );
        assert_eq!(
            resp.headers().get("x-content-type-options").unwrap(),
            "nosniff"
        );
        assert_eq!(resp.headers().get("x-frame-options").unwrap(), "DENY");
        assert_eq!(
            resp.headers().get("referrer-policy").unwrap(),
            "strict-origin-when-cross-origin"
        );
        assert!(resp.headers().get("permissions-policy").is_some());
        assert_eq!(
            resp.headers().get("content-security-policy").unwrap(),
            "default-src 'none'; frame-ancestors 'none'; base-uri 'none'"
        );
    }
}
