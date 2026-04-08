//! OpenTelemetry integration for the RegelRecht engine.
//!
//! Provides OTLP trace export via the `tracing-opentelemetry` bridge.
//! All existing `tracing::debug!` / `tracing::warn!` calls in the engine
//! automatically become OpenTelemetry events when this subscriber is active.
//!
//! # Feature Gate
//!
//! This module is only available when the `otel` feature is enabled:
//!
//! ```toml
//! regelrecht-engine = { path = "packages/engine", features = ["otel"] }
//! ```
//!
//! # Example
//!
//! ```ignore
//! use regelrecht_engine::telemetry::init_otel_subscriber;
//!
//! let _guard = init_otel_subscriber("regelrecht-engine")?;
//! // Now all tracing events are exported as OTel spans/events
//! ```

use opentelemetry::trace::TracerProvider as _;
use opentelemetry_sdk::trace::SdkTracerProvider;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;

/// Initialize an OpenTelemetry subscriber with OTLP export over HTTP.
///
/// Sets up a composed `tracing_subscriber` with:
/// 1. `EnvFilter` layer (reads `RUST_LOG`, defaults to `info`)
/// 2. `tracing_opentelemetry::OpenTelemetryLayer` exporting to an OTLP endpoint
///
/// Uses a batch span processor with a dedicated background thread so spans
/// are exported asynchronously without blocking the evaluation hot path.
/// No async runtime (tokio) is required.
///
/// The OTLP endpoint is configured via `OTEL_EXPORTER_OTLP_ENDPOINT`
/// (defaults to `http://localhost:4318`).
///
/// # Errors
///
/// Returns an error if the OTLP exporter, tracer provider, or global
/// subscriber cannot be initialized.
pub fn init_otel_subscriber(
    service_name: &str,
) -> Result<OtelGuard, Box<dyn std::error::Error + Send + Sync>> {
    let exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_http()
        .build()?;

    let provider = SdkTracerProvider::builder()
        .with_batch_exporter(exporter)
        .with_resource(
            opentelemetry_sdk::Resource::builder()
                .with_service_name(service_name.to_string())
                .build(),
        )
        .build();

    let tracer = provider.tracer(service_name.to_string());

    let otel_layer = tracing_opentelemetry::layer().with_tracer(tracer);

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    // Include a stderr fmt layer so warnings (e.g., cache collision detection)
    // remain visible even when the OTLP endpoint is unreachable.
    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stderr)
        .with_target(false);

    tracing_subscriber::registry()
        .with(filter)
        .with(fmt_layer)
        .with(otel_layer)
        .try_init()
        .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { Box::new(e) })?;

    Ok(OtelGuard { provider })
}

/// Guard that shuts down the OTel provider on drop.
///
/// Hold this in your `main()` to ensure traces are flushed on exit.
pub struct OtelGuard {
    provider: SdkTracerProvider,
}

impl Drop for OtelGuard {
    fn drop(&mut self) {
        if let Err(e) = self.provider.shutdown() {
            eprintln!("Failed to shutdown OTel provider: {e}");
        }
    }
}
