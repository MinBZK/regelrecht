//! Shared telemetry helpers for regelrecht binaries.
//!
//! Each binary previously hand-rolled the same `tracing_subscriber::fmt()`
//! initialization. [`init_subscriber`] consolidates that pattern.

use tracing_subscriber::EnvFilter;

/// Install the default `tracing_subscriber::fmt` global subscriber, honoring
/// `RUST_LOG` and falling back to `default_level` when it is unset.
///
/// Uses `try_init()` so a second call (e.g. from an integration test that
/// already set up a subscriber) leaves the existing subscriber intact rather
/// than panicking. The error is reported on stderr because the very subscriber
/// we tried to install is unavailable at that moment.
pub fn init_subscriber(default_level: &str) {
    if let Err(e) = tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_level)),
        )
        .try_init()
    {
        eprintln!("warning: tracing subscriber already initialized: {e}");
    }
}
