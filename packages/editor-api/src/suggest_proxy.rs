//! Proxy for the editor's suggestion-status polling.
//!
//! The editor polls `/api/trajects/{ref}/corpus/laws/{law_id}/suggestions/status`
//! after a save; this forwards it to the pipeline-api `/suggest/status` endpoint
//! (in-cluster) and returns the per-kind job state. Mirrors `harvest_proxy` but
//! maps the traject-scoped path to the pipeline's flat query interface.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

use crate::state::AppState;

/// GET /api/trajects/{traject_ref}/corpus/laws/{law_id}/suggestions/status
pub async fn proxy_suggest_status(
    State(state): State<AppState>,
    Path((traject_ref, law_id)): Path<(String, String)>,
) -> Result<Response, (StatusCode, String)> {
    let pipeline_url = state.pipeline_api_url.as_deref().ok_or((
        StatusCode::SERVICE_UNAVAILABLE,
        "Pipeline API not configured".to_string(),
    ))?;

    let resp = state
        .http_client
        .get(format!("{pipeline_url}/suggest/status"))
        .query(&[
            ("law_id", law_id.as_str()),
            ("traject_ref", traject_ref.as_str()),
        ])
        .send()
        .await
        .map_err(|e| {
            tracing::warn!(error = %e, "suggest status proxy request failed");
            (
                StatusCode::BAD_GATEWAY,
                "Failed to reach pipeline API".to_string(),
            )
        })?;

    let status =
        StatusCode::from_u16(resp.status().as_u16()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    let body = resp.text().await.unwrap_or_default();

    Ok((
        status,
        [(axum::http::header::CONTENT_TYPE, "application/json")],
        body,
    )
        .into_response())
}
