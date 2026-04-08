use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::state::AppState;

/// Priority for editor-requested harvest jobs (higher = processed first).
/// Default pipeline priority is 50, follow-up jobs use 30.
const EDITOR_HARVEST_PRIORITY: i32 = 80;

#[derive(Deserialize)]
pub struct HarvestRequest {
    pub law_ids: Vec<String>,
}

#[derive(Serialize)]
pub struct HarvestResponse {
    pub results: Vec<HarvestSlugResult>,
}

#[derive(Serialize)]
pub struct HarvestSlugResult {
    pub law_id: String,
    pub status: HarvestStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bwb_id: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HarvestStatus {
    /// Law is already available in the editor corpus.
    AlreadyAvailable,
    /// Harvest job created with high priority.
    Queued,
    /// A pending or processing harvest job already exists.
    AlreadyQueued,
    /// No BWB ID mapping found for this slug.
    NotFound,
    /// Pipeline database not configured.
    HarvestDisabled,
}

pub async fn request_harvest(
    State(state): State<AppState>,
    Json(body): Json<HarvestRequest>,
) -> Result<Json<HarvestResponse>, (StatusCode, String)> {
    let corpus = state.corpus.read().await;
    let pool = match &state.pipeline_pool {
        Some(pool) => pool,
        None => {
            let results = body
                .law_ids
                .into_iter()
                .map(|law_id| HarvestSlugResult {
                    law_id,
                    status: HarvestStatus::HarvestDisabled,
                    bwb_id: None,
                })
                .collect();
            return Ok(Json(HarvestResponse { results }));
        }
    };

    let mut results = Vec::with_capacity(body.law_ids.len());

    for slug in &body.law_ids {
        // Check if law is already in corpus
        if corpus.source_map.get_law(slug).is_some() {
            results.push(HarvestSlugResult {
                law_id: slug.clone(),
                status: HarvestStatus::AlreadyAvailable,
                bwb_id: None,
            });
            continue;
        }

        // Look up BWB ID by slug in the pipeline database
        let result = match find_bwb_id_by_slug(pool, slug).await {
            Ok(Some(bwb_id)) => create_harvest_job(pool, slug, &bwb_id).await,
            Ok(None) => HarvestSlugResult {
                law_id: slug.clone(),
                status: HarvestStatus::NotFound,
                bwb_id: None,
            },
            Err(e) => {
                tracing::warn!(error = %e, slug = %slug, "failed to look up slug");
                HarvestSlugResult {
                    law_id: slug.clone(),
                    status: HarvestStatus::NotFound,
                    bwb_id: None,
                }
            }
        };

        results.push(result);
    }

    Ok(Json(HarvestResponse { results }))
}

/// Find a law's BWB ID by its slug in the law_entries table.
async fn find_bwb_id_by_slug(pool: &PgPool, slug: &str) -> Result<Option<String>, sqlx::Error> {
    let row: Option<(String,)> =
        sqlx::query_as("SELECT law_id FROM law_entries WHERE slug = $1 LIMIT 1")
            .bind(slug)
            .fetch_optional(pool)
            .await?;

    Ok(row.map(|(law_id,)| law_id))
}

/// Create a high-priority harvest job for a law, with deduplication.
async fn create_harvest_job(pool: &PgPool, slug: &str, bwb_id: &str) -> HarvestSlugResult {
    // Check for existing pending/processing harvest job
    let existing: Option<(uuid::Uuid,)> = match sqlx::query_as(
        "SELECT id FROM jobs \
         WHERE law_id = $1 AND job_type = 'harvest' AND status IN ('pending', 'processing') \
         LIMIT 1",
    )
    .bind(bwb_id)
    .fetch_optional(pool)
    .await
    {
        Ok(row) => row,
        Err(e) => {
            tracing::warn!(error = %e, bwb_id = %bwb_id, "failed to check existing jobs");
            return HarvestSlugResult {
                law_id: slug.to_string(),
                status: HarvestStatus::NotFound,
                bwb_id: Some(bwb_id.to_string()),
            };
        }
    };

    if existing.is_some() {
        return HarvestSlugResult {
            law_id: slug.to_string(),
            status: HarvestStatus::AlreadyQueued,
            bwb_id: Some(bwb_id.to_string()),
        };
    }

    // Create the harvest job with high priority
    let payload = serde_json::json!({
        "bwb_id": bwb_id,
    });

    let result = sqlx::query_scalar::<_, uuid::Uuid>(
        "INSERT INTO jobs (id, job_type, law_id, status, priority, payload) \
         VALUES (gen_random_uuid(), 'harvest', $1, 'pending', $2, $3) \
         RETURNING id",
    )
    .bind(bwb_id)
    .bind(EDITOR_HARVEST_PRIORITY)
    .bind(&payload)
    .fetch_one(pool)
    .await;

    match result {
        Ok(job_id) => {
            tracing::info!(
                job_id = %job_id,
                slug = %slug,
                bwb_id = %bwb_id,
                priority = EDITOR_HARVEST_PRIORITY,
                "created editor-requested harvest job"
            );

            // Best-effort: upsert law_entry to 'queued' and link job
            let _ = sqlx::query(
                "INSERT INTO law_entries (law_id, status) \
                 VALUES ($1, 'queued') \
                 ON CONFLICT (law_id) DO UPDATE SET status = 'queued', updated_at = NOW() \
                 WHERE law_entries.status NOT IN ('harvesting', 'enriching')",
            )
            .bind(bwb_id)
            .execute(pool)
            .await;

            let _ = sqlx::query("UPDATE law_entries SET harvest_job_id = $2 WHERE law_id = $1")
                .bind(bwb_id)
                .bind(job_id)
                .execute(pool)
                .await;

            HarvestSlugResult {
                law_id: slug.to_string(),
                status: HarvestStatus::Queued,
                bwb_id: Some(bwb_id.to_string()),
            }
        }
        Err(e) => {
            tracing::warn!(error = %e, bwb_id = %bwb_id, "failed to create harvest job");
            HarvestSlugResult {
                law_id: slug.to_string(),
                status: HarvestStatus::NotFound,
                bwb_id: Some(bwb_id.to_string()),
            }
        }
    }
}
