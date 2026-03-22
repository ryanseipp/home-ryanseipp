use reqwest_middleware::ClientWithMiddleware;
use reqwest_tracing::TracingMiddleware;

/// Client for communicating with the API Gateway during SSR.
///
/// Wraps `reqwest` with `reqwest-tracing` middleware that automatically
/// injects W3C `traceparent`/`tracestate` headers into every outbound
/// request — no manual trace propagation needed.
#[derive(Clone)]
pub struct GatewayClient {
    client: ClientWithMiddleware,
    base_url: String,
}

impl GatewayClient {
    /// Create a new `GatewayClient` pointing at the given base URL.
    ///
    /// The underlying reqwest client is wrapped with `TracingMiddleware`
    /// for automatic OpenTelemetry context propagation.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying reqwest client cannot be built.
    pub fn new(base_url: &str) -> Result<Self, reqwest::Error> {
        let client = reqwest_middleware::ClientBuilder::new(reqwest::Client::builder().build()?)
            .with(TracingMiddleware::default())
            .build();

        Ok(Self {
            client,
            base_url: base_url.trim_end_matches('/').to_owned(),
        })
    }

    /// Returns the middleware-wrapped HTTP client.
    #[must_use]
    pub fn client(&self) -> &ClientWithMiddleware {
        &self.client
    }

    /// Returns the base URL of the API Gateway.
    #[must_use]
    pub fn base_url(&self) -> &str {
        &self.base_url
    }
}
