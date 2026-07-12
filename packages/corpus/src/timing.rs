//! Request-scoped phase timing.
//!
//! The editor's write and build paths are dominated by GitHub round-trips
//! (Contents API GET/PUT, branch create, `git clone`). To see *where* a
//! request spends its time we record named phase durations from deep in
//! the corpus code and surface them two ways:
//!
//! * as a `Server-Timing` response header (editor-api middleware), and
//! * implicitly in the logs via `tracing` spans on the same call sites.
//!
//! Mechanism: a `tokio` task-local holding a shared [`Recorder`]. The
//! editor-api middleware installs one per request via [`scope`]; deep code
//! calls [`measure`] / [`record`], which are cheap no-ops when no recorder
//! is installed (a worker process, a unit test, a non-instrumented
//! endpoint). Nothing has to be threaded through the call graph.
//!
//! This lives in `regelrecht-corpus` — not `regelrecht-shared` — on
//! purpose: both the corpus backends *and* editor-api already depend on
//! this crate, so no new crate dependency edge is introduced (and
//! `regelrecht-shared` stays free of a `tokio` dependency).

use std::collections::HashMap;
use std::future::Future;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Collects the phase durations recorded during a single request.
#[derive(Default)]
pub struct Recorder {
    /// `(phase name, wall-clock duration)` in the order phases first fire.
    /// A phase name may repeat (e.g. two Contents GETs in one save); the
    /// formatter accumulates by name.
    phases: Mutex<Vec<(&'static str, Duration)>>,
}

impl Recorder {
    /// A fresh shared recorder, ready to hand to [`scope`].
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// Append a phase measurement. Poisoned-lock tolerant: instrumentation
    /// must never take down the request it is measuring.
    fn push(&self, name: &'static str, dur: Duration) {
        if let Ok(mut phases) = self.phases.lock() {
            phases.push((name, dur));
        }
    }

    /// Render the recorded phases plus `total` as a `Server-Timing` header
    /// value, e.g. `lock;dur=40.0, gh_get;dur=210.3, total;dur=850.0`.
    pub fn server_timing_header(&self, total: Duration) -> String {
        let phases = self.phases.lock().map(|p| p.clone()).unwrap_or_default();
        format_server_timing(&phases, total)
    }
}

tokio::task_local! {
    static RECORDER: Arc<Recorder>;
}

/// Run `fut` with `recorder` installed as the request-scoped recorder, so
/// any [`measure`]/[`record`] call reached from within `fut` (on the same
/// task) lands in it. Calls from tasks spawned inside `fut` are NOT
/// captured — task-locals do not cross `tokio::spawn`.
pub async fn scope<F>(recorder: Arc<Recorder>, fut: F) -> F::Output
where
    F: Future,
{
    RECORDER.scope(recorder, fut).await
}

/// Record a phase against the current request's recorder, if one is
/// installed. A no-op otherwise — safe to call from any context.
pub fn record(name: &'static str, dur: Duration) {
    let _ = RECORDER.try_with(|recorder| recorder.push(name, dur));
}

/// Await `fut`, recording its wall-clock duration under `name` against the
/// current request's recorder (no-op when none is installed). Returns the
/// future's output unchanged.
pub async fn measure<F>(name: &'static str, fut: F) -> F::Output
where
    F: Future,
{
    let start = Instant::now();
    let out = fut.await;
    record(name, start.elapsed());
    out
}

/// Sum durations per phase name (first-seen order preserved) and render a
/// `Server-Timing` header value, always ending in `total`.
fn format_server_timing(phases: &[(&'static str, Duration)], total: Duration) -> String {
    let mut order: Vec<&'static str> = Vec::new();
    let mut sums: HashMap<&'static str, Duration> = HashMap::new();
    for (name, dur) in phases {
        if !sums.contains_key(name) {
            order.push(name);
        }
        *sums.entry(name).or_default() += *dur;
    }

    let mut parts: Vec<String> = order
        .iter()
        .map(|name| format!("{};dur={:.1}", name, millis(sums[name])))
        .collect();
    parts.push(format!("total;dur={:.1}", millis(total)));
    parts.join(", ")
}

fn millis(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_phases_and_appends_total_last() {
        let phases = vec![
            ("lock", Duration::from_millis(40)),
            ("gh_get", Duration::from_millis(210)),
            ("gh_put", Duration::from_millis(580)),
        ];
        let header = format_server_timing(&phases, Duration::from_millis(850));
        assert_eq!(
            header,
            "lock;dur=40.0, gh_get;dur=210.0, gh_put;dur=580.0, total;dur=850.0"
        );
    }

    #[test]
    fn accumulates_repeated_phase_names_in_first_seen_order() {
        let phases = vec![
            ("gh_get", Duration::from_millis(100)),
            ("gh_put", Duration::from_millis(300)),
            ("gh_get", Duration::from_millis(50)),
        ];
        let header = format_server_timing(&phases, Duration::from_millis(500));
        // gh_get keeps its first position and sums to 150.
        assert_eq!(
            header,
            "gh_get;dur=150.0, gh_put;dur=300.0, total;dur=500.0"
        );
    }

    #[test]
    fn header_with_no_phases_is_just_total() {
        let header = format_server_timing(&[], Duration::from_millis(12));
        assert_eq!(header, "total;dur=12.0");
    }

    #[tokio::test]
    async fn record_outside_scope_is_a_noop() {
        // Must not panic when no recorder is installed.
        record("gh_get", Duration::from_millis(5));
        let out = measure("gh_put", async { 7 }).await;
        assert_eq!(out, 7);
    }

    #[tokio::test]
    async fn measure_and_record_land_in_the_scoped_recorder() {
        let recorder = Recorder::new();
        let captured = recorder.clone();
        scope(recorder, async {
            record("lock", Duration::from_millis(10));
            measure("gh_get", async {}).await;
        })
        .await;
        let header = captured.server_timing_header(Duration::from_millis(20));
        assert!(
            header.starts_with("lock;dur=10.0, gh_get;dur="),
            "got: {header}"
        );
        assert!(header.ends_with("total;dur=20.0"), "got: {header}");
    }
}
