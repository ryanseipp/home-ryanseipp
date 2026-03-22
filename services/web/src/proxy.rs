use axum::{
    Router,
    body::Body,
    extract::{ConnectInfo, State},
    http::{
        HeaderMap, HeaderName, HeaderValue, Method, StatusCode, Uri,
        header::{CONNECTION, CONTENT_LENGTH, HOST, TE, TRAILER, TRANSFER_ENCODING, UPGRADE},
    },
    response::{IntoResponse, Response},
    routing::any,
};
use reqwest_middleware::ClientWithMiddleware;
use std::net::SocketAddr;

/// Maximum request body size for proxied requests (1 MiB).
const MAX_BODY_SIZE: usize = 1024 * 1024;

/// Returns true if a request header is hop-by-hop (RFC 2616 §13.5.1)
/// or set by the proxy itself (to prevent spoofing).
fn is_dropped_request_header(name: &HeaderName) -> bool {
    // Hop-by-hop headers
    name == CONNECTION
        || name == TE
        || name == TRAILER
        || name == TRANSFER_ENCODING
        || name == UPGRADE
        || name == "keep-alive"
        || name == "proxy-authenticate"
        || name == "proxy-authorization"
        // Headers the proxy sets itself
        || name == CONTENT_LENGTH
        || name == HOST
        || name == "x-forwarded-for"
        || name == "x-forwarded-proto"
        || name == "x-forwarded-host"
}

/// Returns true if a response header is hop-by-hop.
fn is_hop_by_hop_response(name: &HeaderName) -> bool {
    name == CONNECTION
        || name == TE
        || name == TRAILER
        || name == TRANSFER_ENCODING
        || name == UPGRADE
        || name == "keep-alive"
}

#[derive(Clone)]
pub struct ProxyState {
    client: ClientWithMiddleware,
    gateway_url: String,
}

/// Build the API proxy router.
///
/// All requests to `/api/v1/*` are forwarded to the Gateway using reqwest
/// wrapped with `reqwest-tracing` for automatic OTel context propagation.
///
/// The proxy:
/// - Forwards all headers except hop-by-hop and proxy-set headers
/// - Adds `X-Forwarded-For`, `X-Forwarded-Proto`, `X-Forwarded-Host`
///   (appends to existing values if set by upstream load balancers)
/// - Forwards all upstream response headers (we trust our own Gateway)
/// - Logs any dropped request headers for visibility
/// - Automatically injects `traceparent`/`tracestate` via reqwest-tracing
/// - Limits request body size to 1 MiB
pub fn api_proxy(client: ClientWithMiddleware, gateway_url: &str) -> Router {
    let state = ProxyState {
        client,
        gateway_url: gateway_url.trim_end_matches('/').to_owned(),
    };

    Router::new()
        .route("/{*path}", any(proxy_handler))
        .with_state(state)
}

async fn proxy_handler(
    State(state): State<ProxyState>,
    ConnectInfo(client_addr): ConnectInfo<SocketAddr>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Body,
) -> Response {
    let path = uri.path();
    let query = uri.query().map(|q| format!("?{q}")).unwrap_or_default();
    let url = format!("{}/api/v1{path}{query}", state.gateway_url);

    // Build the outgoing request.
    // reqwest-tracing middleware automatically injects traceparent/tracestate.
    let mut req = state.client.request(method, &url);

    // Forward all safe headers (everything except hop-by-hop and proxy-set)
    for (name, value) in &headers {
        if is_dropped_request_header(name) {
            tracing::trace!(header = %name, "dropped hop-by-hop or proxy-set request header");
        } else {
            req = req.header(name, value.as_bytes());
        }
    }

    // Add reverse proxy headers, appending to any existing values
    // (this service may sit behind load balancers that already set these)
    let client_ip = client_addr.ip().to_string();
    let forwarded_for = if let Some(existing) = headers.get("x-forwarded-for") {
        format!("{}, {client_ip}", existing.to_str().unwrap_or(""))
    } else {
        client_ip
    };
    req = req.header("x-forwarded-for", &forwarded_for);

    // Preserve upstream proto if set, otherwise default to https
    if let Some(proto) = headers.get("x-forwarded-proto") {
        req = req.header("x-forwarded-proto", proto.as_bytes());
    } else {
        req = req.header("x-forwarded-proto", "https");
    }

    if let Some(host) = headers.get("x-forwarded-host") {
        req = req.header("x-forwarded-host", host.as_bytes());
    } else if let Some(host) = headers.get(HOST) {
        req = req.header("x-forwarded-host", host.as_bytes());
    }

    // Forward request body
    let body_bytes = match axum::body::to_bytes(body, MAX_BODY_SIZE).await {
        Ok(b) => b,
        Err(e) => {
            tracing::warn!(error = %e, "proxy request body too large or unreadable");
            return StatusCode::PAYLOAD_TOO_LARGE.into_response();
        }
    };

    if !body_bytes.is_empty() {
        req = req.body(body_bytes);
    }

    // Send to Gateway
    match req.send().await {
        Ok(resp) => {
            let status =
                StatusCode::from_u16(resp.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
            let upstream_headers = resp.headers().clone();
            let resp_body = resp.bytes().await.unwrap_or_default();

            let mut response = (status, resp_body).into_response();

            // Forward all upstream response headers — we trust our own Gateway.
            // Only strip hop-by-hop headers per HTTP spec.
            for (name, value) in &upstream_headers {
                if is_hop_by_hop_response(name) {
                    continue;
                }
                if let Ok(v) = HeaderValue::from_bytes(value.as_bytes()) {
                    response.headers_mut().append(name.clone(), v);
                }
            }

            response
        }
        Err(e) => {
            tracing::error!(error = %e, url = %url, "gateway proxy request failed");
            StatusCode::BAD_GATEWAY.into_response()
        }
    }
}
