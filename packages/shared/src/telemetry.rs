//! Shared telemetry helpers for regelrecht binaries.
//!
//! Each binary previously hand-rolled the same `tracing_subscriber::fmt()`
//! initialization. [`init_subscriber`] consolidates that pattern.

use std::env::VarError;

use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::EnvFilter;

/// Install the default `tracing_subscriber::fmt` global subscriber, honoring
/// `RUST_LOG` and falling back to `default_level` when it is unset.
///
/// Uses `try_init()` so a second call (e.g. from an integration test that
/// already set up a subscriber) leaves the existing subscriber intact rather
/// than panicking. The error is reported on stderr because the very subscriber
/// we tried to install is unavailable at that moment.
///
/// Span events are off (`FmtSpan::NONE`) — the pre-existing behaviour every
/// caller had. See [`init_subscriber_with_spans`] to turn them on.
pub fn init_subscriber(default_level: &str) {
    init_subscriber_with_spans(default_level, false);
}

/// Like [`init_subscriber`], but `default_span_events` chooses the fallback
/// when `LOG_SPAN_EVENTS` is unset.
///
/// ## Span-close timing logs
///
/// With span events on (`FmtSpan::CLOSE`), a log line is emitted when each
/// `tracing` span closes, carrying its `time.busy`/`time.idle` duration.
/// This turns the `#[tracing::instrument]` spans on the editor's
/// write/build path into a per-step latency breakdown in `zad logs`.
///
/// It is **opt-in per service** on purpose: `init_subscriber` is shared by
/// hot-path services (the harvest/enrich workers, the pipeline API), where
/// a close event per span would multiply log volume for no benefit — they
/// keep the `default_span_events = false` default. Only services that want
/// the breakdown (editor-api) call this with `true`.
///
/// `LOG_SPAN_EVENTS`, when set, always wins over `default_span_events` — so
/// an operator can force spans on (`close`/`new`/`active`/`full`) or off
/// (`none`, or any unrecognised value) without a redeploy. Passing the
/// default as an argument rather than mutating the process environment
/// avoids the `set_var`-under-a-running-runtime data-race caveat and keeps
/// the setting from leaking into child processes (git subprocesses, etc.).
pub fn init_subscriber_with_spans(default_level: &str, default_span_events: bool) {
    if let Err(e) = tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_level)),
        )
        .with_span_events(resolve_span_events(default_span_events))
        .try_init()
    {
        eprintln!("warning: tracing subscriber already initialized: {e}");
    }
}

/// Resolve the [`FmtSpan`] mask: a *set* `LOG_SPAN_EVENTS` always wins;
/// only a genuinely absent variable falls back to the caller's `default_on`
/// (CLOSE when on, NONE when off).
fn resolve_span_events(default_on: bool) -> FmtSpan {
    match std::env::var("LOG_SPAN_EVENTS") {
        Ok(raw) => match raw.to_ascii_lowercase().as_str() {
            "close" => FmtSpan::CLOSE,
            "new" => FmtSpan::NEW,
            "active" => FmtSpan::ACTIVE,
            "full" => FmtSpan::FULL,
            // Explicit off (or any unrecognised value) — never span events.
            _ => FmtSpan::NONE,
        },
        // The variable is set but not valid UTF-8: it is still an explicit
        // (if garbled) override, so treat it as "unrecognised" → off, never
        // silently fall back to `default_on`.
        Err(VarError::NotUnicode(_)) => FmtSpan::NONE,
        // Genuinely unset: honour the per-service default.
        Err(VarError::NotPresent) if default_on => FmtSpan::CLOSE,
        Err(VarError::NotPresent) => FmtSpan::NONE,
    }
}
