#[cfg(feature = "ssr")]
#[tokio::main]
async fn main() {
    use std::net::SocketAddr;

    use axum::Router;
    use axum::extract::DefaultBodyLimit;
    use axum::http::{HeaderName, HeaderValue, header};
    use leptos::prelude::*;
    use leptos_axum::{LeptosRoutes, generate_route_list};
    use tower_http::set_header::SetResponseHeaderLayer;
    use tower_http::trace::TraceLayer;
    use web::app::{App, shell};
    use web::config::AppConfig;

    let _telemetry_guard = web::telemetry::init().expect("failed to initialize telemetry");

    let app_config = AppConfig::load().expect("failed to load configuration");
    tracing::info!(gateway_url = %app_config.gateway.url, "configuration loaded");

    let conf = get_configuration(None).expect("failed to load leptos configuration");
    let addr = conf.leptos_options.site_addr;
    let leptos_options = conf.leptos_options;
    let routes = generate_route_list(App);

    let gateway_client =
        web::api::GatewayClient::new(&app_config.gateway.url).expect("failed to create API client");

    // Build the router:
    // 1. Leptos SSR routes (needs LeptosOptions as state, consumed by .with_state())
    // 2. API proxy to Gateway (uses its own state with reqwest-tracing client)
    // 3. TraceLayer for automatic request/response tracing (same as gateway)
    let app = Router::new()
        .leptos_routes(&leptos_options, routes, {
            let leptos_options = leptos_options.clone();
            move || shell(leptos_options.clone())
        })
        .fallback(leptos_axum::file_and_error_handler(shell))
        .with_state(leptos_options)
        .nest(
            "/api/v1",
            web::proxy::api_proxy(gateway_client.client().clone(), &app_config.gateway.url),
        )
        .layer(TraceLayer::new_for_http())
        .layer(DefaultBodyLimit::max(262_144))
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
        ));

    tracing::info!("listening on http://{addr}");
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("failed to bind TCP listener");
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .expect("server error");
}

#[cfg(not(feature = "ssr"))]
pub fn main() {
    // Client-side entry is handled by lib.rs hydrate()
}
