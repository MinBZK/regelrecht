//! Shared telemetry helpers for regelrecht binaries.
//!
//! Each binary previously hand-rolled the same `tracing_subscriber::fmt()`
//! initialization. [`init_subscriber`] consolidates that pattern.

use tracing_subscriber::EnvFilter;

/// Install the default `tracing_subscriber::fmt` global subscriber, honoring
/// `RUST_LOG` and falling back to `default_level` when it is unset.
///
/// Idiomatic call: `init_subscriber("info")`. Workers that prefer a quieter
/// default may pass `"warn"`.
pub fn init_subscriber(default_level: &str) {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_level)),
        )
        .init();
}
