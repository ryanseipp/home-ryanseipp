use opentelemetry::trace::TracerProvider;
use opentelemetry::{InstrumentationScope, KeyValue};
use opentelemetry_sdk::{Resource, metrics::SdkMeterProvider, trace::SdkTracerProvider};
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

/// Guard that flushes and shuts down OTEL providers on drop.
///
/// Hold this in `main()` for the lifetime of the server.
pub struct TelemetryGuard {
    tracer_provider: SdkTracerProvider,
    meter_provider: SdkMeterProvider,
}

impl Drop for TelemetryGuard {
    fn drop(&mut self) {
        if let Err(e) = self.tracer_provider.shutdown() {
            eprintln!("failed to shutdown tracer provider: {e}");
        }
        if let Err(e) = self.meter_provider.shutdown() {
            eprintln!("failed to shutdown meter provider: {e}");
        }
    }
}

/// Initialize OpenTelemetry telemetry.
///
/// - **Traces**: exported via OTLP gRPC (configured by `OTEL_EXPORTER_OTLP_ENDPOINT` et al.)
/// - **Metrics**: exported via OTLP gRPC at 60s intervals (configurable via `OTEL_METRIC_EXPORT_INTERVAL`)
/// - **Logs**: structured JSON to stdout with trace correlation (`traceId`, `spanId`)
/// - **Filter**: controlled by `RUST_LOG` env var (default: `info`)
///
/// # Errors
///
/// Returns an error if OTLP exporter or provider initialization fails.
pub fn init() -> Result<TelemetryGuard, Box<dyn std::error::Error>> {
    // Resource identifying this service. Merges with attributes from
    // OTEL_RESOURCE_ATTRIBUTES (e.g. k8s.pod.name injected by the OTEL operator).
    let resource = Resource::builder()
        .with_attributes([
            KeyValue::new("service.name", env!("CARGO_PKG_NAME")),
            KeyValue::new("service.version", env!("CARGO_PKG_VERSION")),
        ])
        .build();

    let scope = InstrumentationScope::builder(env!("CARGO_PKG_NAME"))
        .with_version(env!("CARGO_PKG_VERSION"))
        .build();

    // Traces
    let span_exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_tonic()
        .build()?;

    let tracer_provider = SdkTracerProvider::builder()
        .with_resource(resource.clone())
        .with_batch_exporter(span_exporter)
        .build();

    let otel_trace_layer =
        tracing_opentelemetry::layer().with_tracer(tracer_provider.tracer_with_scope(scope));

    // Metrics
    let metric_exporter = opentelemetry_otlp::MetricExporter::builder()
        .with_tonic()
        .build()?;

    let meter_provider = SdkMeterProvider::builder()
        .with_resource(resource)
        .with_periodic_exporter(metric_exporter)
        .build();

    let otel_metrics_layer = tracing_opentelemetry::MetricsLayer::new(meter_provider.clone());

    // Structured JSON logs to stdout with OTEL trace_id/span_id correlation.
    // json-subscriber is a drop-in replacement for tracing_subscriber::fmt().json()
    // that natively injects OTEL context into each log line.
    let json_log_layer = json_subscriber::layer()
        .with_target(true)
        .with_span_list(true)
        .with_opentelemetry_ids(true);

    // Filter from RUST_LOG, default to info
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::registry()
        .with(filter)
        .with(otel_trace_layer)
        .with(otel_metrics_layer)
        .with(json_log_layer)
        .init();

    Ok(TelemetryGuard {
        tracer_provider,
        meter_provider,
    })
}
