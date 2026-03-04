use std::sync::atomic::{AtomicI64, AtomicU64};

use axum::extract::State;
use axum::http::{header, StatusCode};
use axum::response::IntoResponse;
use prometheus_client::encoding::text::encode;
use prometheus_client::metrics::family::Family;
use prometheus_client::metrics::gauge::Gauge;
use prometheus_client::registry::Registry;

use crate::state::AppState;

#[derive(Clone, Debug, Hash, PartialEq, Eq, prometheus_client::encoding::EncodeLabelSet)]
struct StatusLabel {
    status: String,
}

pub async fn metrics_handler(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, StatusCode> {
    let mut registry = Registry::default();

    let jobs_total = Family::<StatusLabel, Gauge<i64, AtomicI64>>::default();
    registry.register(
        "regelrecht_jobs",
        "Number of jobs per status",
        jobs_total.clone(),
    );

    let jobs_completed_total = Gauge::<i64, AtomicI64>::default();
    registry.register(
        "regelrecht_jobs_completed",
        "Total completed jobs",
        jobs_completed_total.clone(),
    );

    let jobs_failed_total = Gauge::<i64, AtomicI64>::default();
    registry.register(
        "regelrecht_jobs_failed",
        "Total failed jobs",
        jobs_failed_total.clone(),
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

    // Jobs per status
    let rows = sqlx::query_as::<_, (String, i64)>(
        "SELECT status::text, COUNT(*) FROM jobs GROUP BY status",
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|e| {
        tracing::error!(error = %e, "failed to query job counts");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    for (status, count) in rows {
        jobs_total.get_or_create(&StatusLabel { status }).set(count);
    }

    // Completed jobs total
    let completed: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM jobs WHERE status = 'completed'")
        .fetch_one(&state.pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to query completed jobs");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    jobs_completed_total.set(completed.0);

    // Failed jobs total
    let failed: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM jobs WHERE status = 'failed'")
        .fetch_one(&state.pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to query failed jobs");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    jobs_failed_total.set(failed.0);

    // Laws per status
    let law_rows = sqlx::query_as::<_, (String, i64)>(
        "SELECT status::text, COUNT(*) FROM law_entries GROUP BY status",
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|e| {
        tracing::error!(error = %e, "failed to query law counts");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    for (status, count) in law_rows {
        laws_total.get_or_create(&StatusLabel { status }).set(count);
    }

    // Average job duration (last 24h, completed jobs)
    let avg_duration: (Option<f64>,) = sqlx::query_as(
        "SELECT AVG(EXTRACT(EPOCH FROM (completed_at - started_at))) \
         FROM jobs WHERE status = 'completed' \
         AND completed_at > NOW() - INTERVAL '24 hours'",
    )
    .fetch_one(&state.pool)
    .await
    .map_err(|e| {
        tracing::error!(error = %e, "failed to query avg job duration");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    if let Some(avg) = avg_duration.0 {
        job_duration_avg.set(avg);
    }

    // Encode to Prometheus text format
    let mut buffer = String::new();
    encode(&mut buffer, &registry).map_err(|e| {
        tracing::error!(error = %e, "failed to encode metrics");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok((
        [(
            header::CONTENT_TYPE,
            "application/openmetrics-text; version=1.0.0; charset=utf-8",
        )],
        buffer,
    ))
}
