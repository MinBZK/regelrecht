//! Pipeline-API endpoints for the editor-suggestion pipeline.
//!
//! `POST /suggest` enqueues one job per kind for a saved law on its traject
//! branch; the editor-api calls this fire-and-forget after a successful save.
//! `GET /suggest/status` reports the latest suggest-job state per kind so the
//! editor can poll until the suggestions are ready.

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::job_queue::{self, CreateJobRequest};
use crate::models::{JobType, Priority};
use crate::suggest::{SuggestKind, SuggestPayload};
use crate::ApiState;

/// Editor suggestions run at a higher priority than batch enrich (50) so a user
/// waiting in the editor isn't stuck behind bulk enrichment, but below
/// editor-requested harvest (80).
const SUGGEST_PRIORITY: i32 = 70;

/// Suggest jobs are advisory; a single attempt is enough (the user can re-save
/// to retry). Avoids spending repeated LLM runs on a flaky law.
const SUGGEST_MAX_ATTEMPTS: i32 = 1;

#[derive(Deserialize)]
pub struct SuggestRequest {
    pub law_id: String,
    pub yaml_path: String,
    pub traject_ref: String,
    pub traject_branch: String,
    /// Optional single article to scope to. None = whole law.
    #[serde(default)]
    pub article_number: Option<String>,
}

#[derive(Serialize)]
pub struct SuggestResponse {
    /// Which kinds were enqueued this call (a kind already pending/processing
    /// for this law+traject is skipped, so it won't appear here).
    pub enqueued: Vec<String>,
}

/// POST /suggest
///
/// Enqueues a guidelines job and a machine_readable job for the saved law. A
/// kind that already has an active (pending/processing) job for this
/// law+traject is skipped (the partial unique index makes this atomic).
pub async fn request_suggest(
    State(state): State<ApiState>,
    Json(body): Json<SuggestRequest>,
) -> Result<Json<SuggestResponse>, (StatusCode, String)> {
    if body.law_id.is_empty() || body.yaml_path.is_empty() || body.traject_branch.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "law_id, yaml_path and traject_branch are required".to_string(),
        ));
    }

    let mut enqueued = Vec::new();
    for kind in [SuggestKind::Guidelines, SuggestKind::MachineReadable] {
        let payload = SuggestPayload {
            law_id: body.law_id.clone(),
            yaml_path: body.yaml_path.clone(),
            traject_ref: body.traject_ref.clone(),
            traject_branch: body.traject_branch.clone(),
            kind,
            article_number: body.article_number.clone(),
        };
        let payload_json = match serde_json::to_value(&payload) {
            Ok(v) => v,
            Err(e) => {
                tracing::error!(error = %e, "failed to serialize suggest payload");
                continue;
            }
        };
        let job_type = match kind {
            SuggestKind::Guidelines => JobType::SuggestGuidelines,
            SuggestKind::MachineReadable => JobType::SuggestMachineReadable,
        };
        let req = CreateJobRequest::new(job_type, body.law_id.clone())
            .with_priority(Priority::new(SUGGEST_PRIORITY))
            .with_payload(payload_json)
            .with_max_attempts(SUGGEST_MAX_ATTEMPTS);

        match job_queue::create_suggest_job_if_not_exists(&state.pool, req, &body.traject_ref).await
        {
            Ok(Some(_job)) => enqueued.push(kind.slug().to_string()),
            Ok(None) => {
                tracing::debug!(law_id = %body.law_id, kind = kind.slug(), "suggest job already active, skipped");
            }
            Err(e) => {
                tracing::error!(error = %e, kind = kind.slug(), "failed to enqueue suggest job");
            }
        }
    }

    Ok(Json(SuggestResponse { enqueued }))
}

#[derive(Deserialize)]
pub struct SuggestStatusQuery {
    pub law_id: String,
    pub traject_ref: String,
}

#[derive(Serialize)]
pub struct SuggestStatusEntry {
    pub kind: String,
    pub status: String,
}

#[derive(Serialize)]
pub struct SuggestStatusResponse {
    /// Latest job state per kind for this law+traject. Absent kinds have never
    /// been requested.
    pub results: Vec<SuggestStatusEntry>,
}

/// GET /suggest/status?law_id=...&traject_ref=...
///
/// Returns the latest job status per suggest kind for a law within a traject,
/// so the editor can poll until both runs reach a terminal state.
pub async fn suggest_status(
    State(state): State<ApiState>,
    Query(query): Query<SuggestStatusQuery>,
) -> Result<Json<SuggestStatusResponse>, (StatusCode, String)> {
    if query.law_id.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "law_id is required".to_string()));
    }

    // Latest job per suggest kind for this law + traject (matched on the
    // payload's traject_ref). DISTINCT ON keeps the most recent per job_type.
    let rows: Vec<(String, String)> = sqlx::query_as(
        r#"
        SELECT DISTINCT ON (job_type) job_type::text, status::text
        FROM jobs
        WHERE law_id = $1
          AND job_type IN ('suggest_guidelines', 'suggest_machine_readable')
          AND payload->>'traject_ref' = $2
        ORDER BY job_type, created_at DESC
        "#,
    )
    .bind(&query.law_id)
    .bind(&query.traject_ref)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| {
        tracing::error!(error = %e, "failed to query suggest status");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to query suggest status".to_string(),
        )
    })?;

    let results = rows
        .into_iter()
        .map(|(job_type, status)| SuggestStatusEntry {
            // Map the DB job_type back to the editor-facing kind slug.
            kind: match job_type.as_str() {
                "suggest_guidelines" => "guidelines".to_string(),
                "suggest_machine_readable" => "machine_readable".to_string(),
                other => other.to_string(),
            },
            status,
        })
        .collect();

    Ok(Json(SuggestStatusResponse { results }))
}
