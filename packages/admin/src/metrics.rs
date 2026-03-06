use std::sync::atomic::{AtomicI64, AtomicU64};
use std::time::{Duration, Instant};

use axum::extract::State;
use axum::http::{header, StatusCode};
use axum::response::IntoResponse;
use prometheus_client::encoding::text::encode;
use prometheus_client::metrics::family::Family;
use prometheus_client::metrics::gauge::Gauge;
use prometheus_client::registry::Registry;
use sqlx::PgPool;
use tokio::sync::RwLock;

use crate::state::AppState;

/// Cache TTL — avoids hitting the database on every Prometheus scrape.
const CACHE_TTL: Duration = Duration::from_secs(15);

#[derive(Clone, Debug, Hash, PartialEq, Eq, prometheus_client::encoding::EncodeLabelSet)]
pub struct StatusLabel {
    pub status: String,
}

/// Raw metrics data fetched from the database.
#[derive(Clone, Debug)]
pub struct MetricsSnapshot {
    pub jobs_by_status: Vec<(String, i64)>,
    pub laws_by_status: Vec<(String, i64)>,
    pub avg_job_duration_secs: Option<f64>,
}

/// Cached response: the encoded Prometheus text and when it was generated.
pub struct CachedMetrics {
    pub body: String,
    pub generated_at: Instant,
}

/// Global metrics cache, initialised once in [`AppState`].
pub type MetricsCache = RwLock<Option<CachedMetrics>>;

/// Create a new (empty) metrics cache.
pub fn new_cache() -> MetricsCache {
    RwLock::new(None)
}

/// Fetch all metrics from the database in as few queries as possible.
pub async fn fetch_metrics(pool: &PgPool) -> Result<MetricsSnapshot, sqlx::Error> {
    // Two queries instead of five: jobs grouped by status + laws grouped by status + avg duration.
    // We combine the avg-duration query with a CTE so it's a single round-trip for jobs.
    let jobs_by_status = sqlx::query_as::<_, (String, i64)>(
        "SELECT status::text, COUNT(*) FROM jobs GROUP BY status",
    )
    .fetch_all(pool)
    .await?;

    let laws_by_status = sqlx::query_as::<_, (String, i64)>(
        "SELECT status::text, COUNT(*) FROM law_entries GROUP BY status",
    )
    .fetch_all(pool)
    .await?;

    let avg_duration: (Option<f64>,) = sqlx::query_as(
        "SELECT AVG(EXTRACT(EPOCH FROM (completed_at - started_at))) \
         FROM jobs WHERE status = 'completed' \
         AND completed_at > NOW() - INTERVAL '24 hours'",
    )
    .fetch_one(pool)
    .await?;

    Ok(MetricsSnapshot {
        jobs_by_status,
        laws_by_status,
        avg_job_duration_secs: avg_duration.0,
    })
}

/// Encode a [`MetricsSnapshot`] into Prometheus/OpenMetrics text format.
pub fn encode_metrics(snapshot: &MetricsSnapshot) -> Result<String, std::fmt::Error> {
    let mut registry = Registry::default();

    let jobs_total = Family::<StatusLabel, Gauge<i64, AtomicI64>>::default();
    registry.register(
        "regelrecht_jobs",
        "Number of jobs per status",
        jobs_total.clone(),
    );

    let laws_total = Family::<StatusLabel, Gauge<i64, AtomicI64>>::default();
    registry.register(
        "regelrecht_laws",
        "Number of laws per status",
        laws_total.clone(),
    );

    let job_duration_avg = Gauge::<f64, AtomicU64>::default();
    registry.register(
        "regelrecht_job_duration_avg_seconds",
        "Average job duration in seconds (last 24h)",
        job_duration_avg.clone(),
    );

    for (status, count) in &snapshot.jobs_by_status {
        jobs_total
            .get_or_create(&StatusLabel {
                status: status.clone(),
            })
            .set(*count);
    }

    for (status, count) in &snapshot.laws_by_status {
        laws_total
            .get_or_create(&StatusLabel {
                status: status.clone(),
            })
            .set(*count);
    }

    if let Some(avg) = snapshot.avg_job_duration_secs {
        job_duration_avg.set(avg);
    }

    let mut buffer = String::new();
    encode(&mut buffer, &registry)?;
    Ok(buffer)
}

/// Axum handler for `GET /metrics`.
pub async fn metrics_handler(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, StatusCode> {
    // Fast path: return cached response if still fresh.
    {
        let cache = state.metrics_cache.read().await;
        if let Some(ref cached) = *cache {
            if cached.generated_at.elapsed() < CACHE_TTL {
                return Ok((
                    [(
                        header::CONTENT_TYPE,
                        "application/openmetrics-text; version=1.0.0; charset=utf-8",
                    )],
                    cached.body.clone(),
                ));
            }
        }
    }

    let snapshot = fetch_metrics(&state.pool).await.map_err(|e| {
        tracing::error!(error = %e, "failed to fetch metrics");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let body = encode_metrics(&snapshot).map_err(|e| {
        tracing::error!(error = %e, "failed to encode metrics");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Update cache.
    {
        let mut cache = state.metrics_cache.write().await;
        *cache = Some(CachedMetrics {
            body: body.clone(),
            generated_at: Instant::now(),
        });
    }

    Ok((
        [(
            header::CONTENT_TYPE,
            "application/openmetrics-text; version=1.0.0; charset=utf-8",
        )],
        body,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_empty_snapshot() {
        let snapshot = MetricsSnapshot {
            jobs_by_status: vec![],
            laws_by_status: vec![],
            avg_job_duration_secs: None,
        };
        let body = encode_metrics(&snapshot).expect("encode should succeed");
        assert!(
            body.contains("regelrecht_jobs"),
            "should contain jobs metric family"
        );
        assert!(
            body.contains("regelrecht_laws"),
            "should contain laws metric family"
        );
        assert!(
            body.contains("regelrecht_job_duration_avg_seconds"),
            "should contain duration metric"
        );
    }

    #[test]
    fn encode_snapshot_with_data() {
        let snapshot = MetricsSnapshot {
            jobs_by_status: vec![
                ("completed".to_string(), 42),
                ("failed".to_string(), 3),
                ("pending".to_string(), 7),
            ],
            laws_by_status: vec![("active".to_string(), 100), ("draft".to_string(), 5)],
            avg_job_duration_secs: Some(12.5),
        };
        let body = encode_metrics(&snapshot).expect("encode should succeed");

        // Job counts by status should appear as labeled gauges.
        assert!(
            body.contains("regelrecht_jobs{status=\"completed\"} 42"),
            "completed jobs"
        );
        assert!(
            body.contains("regelrecht_jobs{status=\"failed\"} 3"),
            "failed jobs"
        );
        assert!(
            body.contains("regelrecht_jobs{status=\"pending\"} 7"),
            "pending jobs"
        );

        // Law counts.
        assert!(
            body.contains("regelrecht_laws{status=\"active\"} 100"),
            "active laws"
        );
        assert!(
            body.contains("regelrecht_laws{status=\"draft\"} 5"),
            "draft laws"
        );

        // Average duration.
        assert!(
            body.contains("regelrecht_job_duration_avg_seconds 12.5"),
            "avg duration"
        );
    }

    #[test]
    fn encode_snapshot_without_avg_duration() {
        let snapshot = MetricsSnapshot {
            jobs_by_status: vec![("running".to_string(), 1)],
            laws_by_status: vec![],
            avg_job_duration_secs: None,
        };
        let body = encode_metrics(&snapshot).expect("encode should succeed");
        // When no avg duration, the gauge should remain at default (0).
        assert!(body.contains("regelrecht_job_duration_avg_seconds"));
    }

    #[test]
    fn cache_is_initially_empty() {
        let cache = new_cache();
        // RwLock::try_read is sync-safe in tests.
        let guard = cache.try_read().expect("should be readable");
        assert!(guard.is_none(), "cache should start empty");
    }

    #[test]
    fn redundant_completed_failed_metrics_removed() {
        // The old implementation had separate `regelrecht_jobs_completed` and
        // `regelrecht_jobs_failed` metrics. Verify they no longer appear — users
        // should use `regelrecht_jobs{status="completed"}` instead.
        let snapshot = MetricsSnapshot {
            jobs_by_status: vec![("completed".to_string(), 10), ("failed".to_string(), 2)],
            laws_by_status: vec![],
            avg_job_duration_secs: None,
        };
        let body = encode_metrics(&snapshot).expect("encode should succeed");
        assert!(
            !body.contains("regelrecht_jobs_completed"),
            "redundant completed metric should not exist"
        );
        assert!(
            !body.contains("regelrecht_jobs_failed"),
            "redundant failed metric should not exist"
        );
    }
}
