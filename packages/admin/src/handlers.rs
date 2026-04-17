use std::sync::LazyLock;

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::Json;
use regelrecht_pipeline::job_queue::{
    create_enrich_job_if_not_exists, create_job, CreateJobRequest,
};
use regelrecht_pipeline::law_status::{set_enrich_job, set_harvest_job};
use regelrecht_pipeline::{EnrichPayload, HarvestPayload, JobType, Priority, ENRICH_PROVIDERS};
use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::error::ApiError;
use crate::models::{Job, LawEntry, PaginatedResponse};
use crate::state::AppState;

// --- Platform info ---

#[derive(Serialize)]
pub struct PlatformInfo {
    pub deployment_name: String,
    pub component_name: String,
}

pub async fn platform_info() -> Json<PlatformInfo> {
    Json(PlatformInfo {
        deployment_name: std::env::var("DEPLOYMENT_NAME").unwrap_or_default(),
        component_name: std::env::var("COMPONENT_NAME").unwrap_or_default(),
    })
}

#[allow(clippy::expect_used)]
static BWB_ID_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^BWBR\d{7}$").expect("valid regex"));

#[allow(clippy::expect_used)]
static CVDR_ID_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^CVDR\d{3,}$").expect("valid regex"));

/// Identifies the source type of a law identifier.
enum LawIdType {
    Bwb(String),
    Cvdr(String),
}

/// Validate and classify a law identifier as BWB or CVDR.
fn classify_law_id(id: &str) -> Option<LawIdType> {
    if BWB_ID_PATTERN.is_match(id) {
        Some(LawIdType::Bwb(id.to_string()))
    } else if CVDR_ID_PATTERN.is_match(id) {
        Some(LawIdType::Cvdr(id.to_string()))
    } else {
        None
    }
}

/// Validate a sort column against an allowlist. Returns `None` if not allowed.
fn validated_sort_column<'a>(
    sort: Option<&'a str>,
    allowed: &[&str],
    default: &'a str,
) -> Option<&'a str> {
    let col = sort.unwrap_or(default);
    if allowed.contains(&col) {
        Some(col)
    } else {
        None
    }
}

/// Normalize an order parameter to "ASC" or "DESC" (default).
fn normalized_order(order: Option<&str>) -> &'static str {
    match order {
        Some("ASC" | "asc") => "ASC",
        _ => "DESC",
    }
}

/// Clamp a limit value: default 50, range 1..=200.
fn clamped_limit(limit: Option<i64>) -> i64 {
    limit.unwrap_or(50).clamp(1, 200)
}

/// Clamp an offset value: default 0, minimum 0.
fn clamped_offset(offset: Option<i64>) -> i64 {
    offset.unwrap_or(0).max(0)
}

#[derive(Deserialize)]
pub struct LawEntriesQuery {
    pub status: Option<String>,
    pub sort: Option<String>,
    pub order: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

const ALLOWED_SORT_COLUMNS_LAW: &[&str] = &[
    "law_id",
    "law_name",
    "status",
    "coverage_score",
    "created_at",
    "updated_at",
];

pub async fn list_law_entries(
    State(state): State<AppState>,
    Query(params): Query<LawEntriesQuery>,
) -> Result<Json<PaginatedResponse<LawEntry>>, ApiError> {
    let pool = &state.pool;
    let limit = clamped_limit(params.limit);
    let offset = clamped_offset(params.offset);

    let sort_column = validated_sort_column(
        params.sort.as_deref(),
        ALLOWED_SORT_COLUMNS_LAW,
        "updated_at",
    )
    .ok_or(ApiError::BadRequest("invalid sort column".to_string()))?;

    let order = normalized_order(params.order.as_deref());

    // Count query
    let total: i64 = if let Some(ref status) = params.status {
        sqlx::query_scalar("SELECT COUNT(*) FROM law_entries WHERE status::text = $1")
            .bind(status)
            .fetch_one(pool)
            .await
            .map_err(|e| {
                tracing::error!(error = %e, "count query failed");
                ApiError::Internal("internal server error".to_string())
            })?
    } else {
        sqlx::query_scalar("SELECT COUNT(*) FROM law_entries")
            .fetch_one(pool)
            .await
            .map_err(|e| {
                tracing::error!(error = %e, "count query failed");
                ApiError::Internal("internal server error".to_string())
            })?
    };

    // Data query — sort column is validated against an allowlist above, so
    // interpolating it into the query string is safe.
    let query_str = if params.status.is_some() {
        format!(
            "SELECT law_id, law_name, status, coverage_score, \
             harvest_job_id, enrich_job_id, harvest_fail_count, enrich_fail_count, \
             created_at, updated_at \
             FROM law_entries WHERE status::text = $1 \
             ORDER BY {sort_column} {order} LIMIT $2 OFFSET $3"
        )
    } else {
        format!(
            "SELECT law_id, law_name, status, coverage_score, \
             harvest_job_id, enrich_job_id, harvest_fail_count, enrich_fail_count, \
             created_at, updated_at \
             FROM law_entries \
             ORDER BY {sort_column} {order} LIMIT $1 OFFSET $2"
        )
    };

    let data: Vec<LawEntry> = if let Some(ref status) = params.status {
        sqlx::query_as::<_, LawEntry>(&query_str)
            .bind(status)
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await
            .map_err(|e| {
                tracing::error!(error = %e, "data query failed");
                ApiError::Internal("internal server error".to_string())
            })?
    } else {
        sqlx::query_as::<_, LawEntry>(&query_str)
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await
            .map_err(|e| {
                tracing::error!(error = %e, "data query failed");
                ApiError::Internal("internal server error".to_string())
            })?
    };

    Ok(Json(PaginatedResponse {
        data,
        total,
        limit,
        offset,
    }))
}

// --- Jobs ---

#[derive(Deserialize)]
pub struct JobsQuery {
    pub status: Option<String>,
    pub job_type: Option<String>,
    pub law_id: Option<String>,
    pub sort: Option<String>,
    pub order: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Deserialize)]
pub struct JobsSummaryQuery {
    pub status: Option<String>,
    pub job_type: Option<String>,
    pub sort: Option<String>,
    pub order: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct JobSummary {
    pub law_id: String,
    pub total_jobs: i64,
    pub pending: i64,
    pub processing: i64,
    pub completed: i64,
    pub failed: i64,
    pub latest_created_at: chrono::DateTime<chrono::Utc>,
}

const ALLOWED_SORT_COLUMNS_JOB_SUMMARY: &[&str] = &["law_id", "total_jobs", "latest_created_at"];

const ALLOWED_SORT_COLUMNS_JOB: &[&str] = &[
    "id",
    "job_type",
    "law_id",
    "status",
    "priority",
    "attempts",
    "created_at",
    "updated_at",
    "started_at",
    "completed_at",
];

pub async fn list_jobs(
    State(state): State<AppState>,
    Query(params): Query<JobsQuery>,
) -> Result<Json<PaginatedResponse<Job>>, ApiError> {
    let pool = &state.pool;
    let limit = clamped_limit(params.limit);
    let offset = clamped_offset(params.offset);

    let sort_column = validated_sort_column(
        params.sort.as_deref(),
        ALLOWED_SORT_COLUMNS_JOB,
        "created_at",
    )
    .ok_or(ApiError::BadRequest("invalid sort column".to_string()))?;

    let order = normalized_order(params.order.as_deref());

    // Build dynamic WHERE clause for multi-filter support.
    let mut where_clauses = Vec::new();
    let mut bind_index: usize = 1;

    if params.status.is_some() {
        where_clauses.push(format!("status::text = ${bind_index}"));
        bind_index += 1;
    }

    if params.job_type.is_some() {
        where_clauses.push(format!("job_type::text = ${bind_index}"));
        bind_index += 1;
    }

    if params.law_id.is_some() {
        where_clauses.push(format!("law_id = ${bind_index}"));
        bind_index += 1;
    }

    let where_sql = if where_clauses.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", where_clauses.join(" AND "))
    };

    // Count query
    let count_sql = format!("SELECT COUNT(*) FROM jobs {where_sql}");

    let mut count_query = sqlx::query_scalar::<_, i64>(&count_sql);
    if let Some(ref status) = params.status {
        count_query = count_query.bind(status);
    }
    if let Some(ref job_type) = params.job_type {
        count_query = count_query.bind(job_type);
    }
    if let Some(ref law_id) = params.law_id {
        count_query = count_query.bind(law_id);
    }

    let total: i64 = count_query.fetch_one(pool).await.map_err(|e| {
        tracing::error!(error = %e, "count query failed");
        ApiError::Internal("internal server error".to_string())
    })?;

    // Data query — sort column is validated against an allowlist above, so
    // interpolating it into the query string is safe.
    let limit_idx = bind_index;
    let offset_idx = bind_index + 1;

    let data_sql = format!(
        "SELECT id, job_type, law_id, status, \
         priority, payload, result, progress, attempts, max_attempts, created_at, updated_at, started_at, completed_at \
         FROM jobs {where_sql} \
         ORDER BY {sort_column} {order} LIMIT ${limit_idx} OFFSET ${offset_idx}"
    );

    let mut data_query = sqlx::query_as::<_, Job>(&data_sql);
    if let Some(ref status) = params.status {
        data_query = data_query.bind(status);
    }
    if let Some(ref job_type) = params.job_type {
        data_query = data_query.bind(job_type);
    }
    if let Some(ref law_id) = params.law_id {
        data_query = data_query.bind(law_id);
    }
    data_query = data_query.bind(limit).bind(offset);

    let data: Vec<Job> = data_query.fetch_all(pool).await.map_err(|e| {
        tracing::error!(error = %e, "data query failed");
        ApiError::Internal("internal server error".to_string())
    })?;

    Ok(Json(PaginatedResponse {
        data,
        total,
        limit,
        offset,
    }))
}

pub async fn list_jobs_summary(
    State(state): State<AppState>,
    Query(params): Query<JobsSummaryQuery>,
) -> Result<Json<PaginatedResponse<JobSummary>>, ApiError> {
    let pool = &state.pool;
    let limit = clamped_limit(params.limit);
    let offset = clamped_offset(params.offset);

    let sort_column = validated_sort_column(
        params.sort.as_deref(),
        ALLOWED_SORT_COLUMNS_JOB_SUMMARY,
        "latest_created_at",
    )
    .ok_or(ApiError::BadRequest("invalid sort column".to_string()))?;

    let order = normalized_order(params.order.as_deref());

    // Build dynamic WHERE clause for multi-filter support.
    let mut where_clauses = Vec::new();
    let mut bind_index: usize = 1;

    if params.status.is_some() {
        where_clauses.push(format!("status::text = ${bind_index}"));
        bind_index += 1;
    }

    if params.job_type.is_some() {
        where_clauses.push(format!("job_type::text = ${bind_index}"));
        bind_index += 1;
    }

    let where_sql = if where_clauses.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", where_clauses.join(" AND "))
    };

    // Count query (distinct law_ids matching filters)
    let count_sql = format!("SELECT COUNT(DISTINCT law_id) FROM jobs {where_sql}");

    let mut count_query = sqlx::query_scalar::<_, i64>(&count_sql);
    if let Some(ref status) = params.status {
        count_query = count_query.bind(status);
    }
    if let Some(ref job_type) = params.job_type {
        count_query = count_query.bind(job_type);
    }

    let total: i64 = count_query.fetch_one(pool).await.map_err(|e| {
        tracing::error!(error = %e, "count query failed");
        ApiError::Internal("internal server error".to_string())
    })?;

    // Data query — sort column is validated against an allowlist above, so
    // interpolating it into the query string is safe.
    let limit_idx = bind_index;
    let offset_idx = bind_index + 1;

    let data_sql = format!(
        "SELECT law_id, \
         COUNT(*) as total_jobs, \
         COUNT(*) FILTER (WHERE status = 'pending') as pending, \
         COUNT(*) FILTER (WHERE status = 'processing') as processing, \
         COUNT(*) FILTER (WHERE status = 'completed') as completed, \
         COUNT(*) FILTER (WHERE status = 'failed') as failed, \
         MAX(created_at) as latest_created_at \
         FROM jobs {where_sql} \
         GROUP BY law_id \
         ORDER BY {sort_column} {order} LIMIT ${limit_idx} OFFSET ${offset_idx}"
    );

    let mut data_query = sqlx::query_as::<_, JobSummary>(&data_sql);
    if let Some(ref status) = params.status {
        data_query = data_query.bind(status);
    }
    if let Some(ref job_type) = params.job_type {
        data_query = data_query.bind(job_type);
    }
    data_query = data_query.bind(limit).bind(offset);

    let data: Vec<JobSummary> = data_query.fetch_all(pool).await.map_err(|e| {
        tracing::error!(error = %e, "data query failed");
        ApiError::Internal("internal server error".to_string())
    })?;

    Ok(Json(PaginatedResponse {
        data,
        total,
        limit,
        offset,
    }))
}

#[derive(Deserialize)]
pub struct CreateJobBody {
    /// Law identifier — BWB (e.g. "BWBR0018451") or CVDR (e.g. "CVDR681386").
    /// Also accepts the legacy `bwb_id` field for backward compatibility.
    pub law_id: Option<String>,
    /// Legacy field — use `law_id` instead. If both are set, `law_id` takes precedence.
    pub bwb_id: Option<String>,
    pub priority: Option<i32>,
    pub date: Option<String>,
}

#[derive(Serialize)]
pub struct CreateJobResponse {
    pub job_id: String,
    pub law_id: String,
}

pub async fn create_harvest_job(
    State(state): State<AppState>,
    Json(body): Json<CreateJobBody>,
) -> Result<(StatusCode, Json<CreateJobResponse>), ApiError> {
    // Accept `law_id` with `bwb_id` as fallback for backward compatibility.
    let raw_id = body
        .law_id
        .or(body.bwb_id)
        .map(|s| s.trim().to_string())
        .unwrap_or_default();

    if raw_id.is_empty() {
        return Err(ApiError::BadRequest(
            "law_id must not be empty (BWB or CVDR identifier)".to_string(),
        ));
    }

    let law_id_type = classify_law_id(&raw_id).ok_or_else(|| {
        tracing::debug!(law_id = %raw_id, "rejected invalid law ID");
        ApiError::BadRequest(
            "invalid law ID format: expected BWBR followed by 7 digits, or CVDR followed by 3+ digits".to_string(),
        )
    })?;

    let law_id = match &law_id_type {
        LawIdType::Bwb(id) | LawIdType::Cvdr(id) => id.clone(),
    };

    if let Some(ref date) = body.date {
        if chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d").is_err() {
            tracing::debug!(date, "rejected invalid date");
            return Err(ApiError::BadRequest(
                "invalid date format: expected YYYY-MM-DD".to_string(),
            ));
        }
    }

    let pool = &state.pool;

    let mut tx = pool.begin().await.map_err(|e| {
        tracing::error!(error = %e, "failed to begin transaction");
        ApiError::Internal("internal server error".to_string())
    })?;

    // Acquire an advisory lock keyed on the law_id to serialize concurrent requests
    // for the same law. This prevents the TOCTOU race where two requests both see
    // no existing job and both create one. The lock is released when the transaction
    // commits or rolls back.
    sqlx::query("SELECT pg_advisory_xact_lock(hashtext($1))")
        .bind(&law_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, law_id = %law_id, "failed to acquire advisory lock");
            ApiError::Internal("internal server error".to_string())
        })?;

    // Check for existing pending or processing harvest job to prevent duplicates.
    let existing: Option<(sqlx::types::Uuid,)> = sqlx::query_as(
        "SELECT id FROM jobs \
         WHERE law_id = $1 AND job_type = 'harvest' AND status IN ('pending', 'processing') \
         LIMIT 1",
    )
    .bind(&law_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| {
        tracing::error!(error = %e, law_id = %law_id, "failed to check for existing jobs");
        ApiError::Internal("failed to check for existing jobs".to_string())
    })?;

    if let Some((existing_id,)) = existing {
        return Err(ApiError::Conflict(format!(
            "a pending or processing harvest job already exists: {existing_id}"
        )));
    }

    // Check if law is exhausted for harvest.
    // RowNotFound is fine (new law, can't be exhausted); other errors should propagate.
    match regelrecht_pipeline::law_status::get_law(&mut *tx, &law_id).await {
        Ok(law) if law.status == regelrecht_pipeline::LawStatusValue::HarvestExhausted => {
            return Err(ApiError::Conflict(format!("{law_id} is harvest_exhausted — reset via /api/law_entries/{law_id}/reset-exhausted first")));
        }
        Err(regelrecht_pipeline::PipelineError::LawNotFound(_)) => {}
        Err(e) => {
            tracing::error!(error = %e, "failed to check exhausted status");
            return Err(ApiError::Internal(
                "failed to check exhausted status".to_string(),
            ));
        }
        Ok(_) => {}
    }

    sqlx::query(
        "INSERT INTO law_entries (law_id, status) \
         VALUES ($1, 'queued') \
         ON CONFLICT (law_id) DO UPDATE SET status = 'queued', updated_at = NOW() \
         WHERE law_entries.status NOT IN ('harvesting', 'enriching')",
    )
    .bind(&law_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        tracing::error!(error = %e, law_id = %law_id, "failed to upsert law entry");
        ApiError::Internal("failed to upsert law entry".to_string())
    })?;

    let payload = match law_id_type {
        LawIdType::Bwb(ref id) => HarvestPayload {
            bwb_id: Some(id.clone()),
            cvdr_id: None,
            date: body.date,
            max_size_mb: None,
            depth: None,
        },
        LawIdType::Cvdr(ref id) => HarvestPayload {
            bwb_id: None,
            cvdr_id: Some(id.clone()),
            date: body.date,
            max_size_mb: None,
            depth: None,
        },
    };

    let priority = Priority::new(body.priority.unwrap_or(50));

    let req = CreateJobRequest::new(JobType::Harvest, &law_id)
        .with_priority(priority)
        .with_payload(serde_json::to_value(&payload).map_err(|e| {
            tracing::error!(error = %e, "failed to serialize payload");
            ApiError::Internal("failed to serialize payload".to_string())
        })?);

    let job = create_job(&mut *tx, req).await.map_err(|e| {
        tracing::error!(error = %e, law_id = %law_id, "failed to create harvest job");
        ApiError::Internal("failed to create harvest job".to_string())
    })?;

    // Link the harvest job to the law entry.
    set_harvest_job(&mut *tx, &law_id, job.id).await.map_err(|e| {
        tracing::error!(error = %e, law_id = %law_id, job_id = %job.id, "failed to link harvest job to law entry");
        ApiError::Internal("failed to link harvest job to law entry".to_string())
    })?;

    tx.commit().await.map_err(|e| {
        tracing::error!(error = %e, "failed to commit transaction");
        ApiError::Internal("internal server error".to_string())
    })?;

    tracing::info!(job_id = %job.id, law_id = %law_id, "created harvest job");

    Ok((
        StatusCode::CREATED,
        Json(CreateJobResponse {
            job_id: job.id.to_string(),
            law_id,
        }),
    ))
}

// --- Enrich Jobs ---

#[derive(Deserialize)]
pub struct CreateEnrichBody {
    pub law_id: String,
    pub priority: Option<i32>,
}

#[derive(Serialize)]
pub struct CreateEnrichResponse {
    pub job_ids: Vec<String>,
    pub law_id: String,
    pub providers: Vec<String>,
}

pub async fn create_enrich_jobs(
    State(state): State<AppState>,
    Json(body): Json<CreateEnrichBody>,
) -> Result<(StatusCode, Json<CreateEnrichResponse>), ApiError> {
    let law_id = body.law_id.trim().to_string();
    if law_id.is_empty() {
        return Err(ApiError::BadRequest("law_id must not be empty".to_string()));
    }

    let pool = &state.pool;

    let mut tx = pool.begin().await.map_err(|e| {
        tracing::error!(error = %e, "failed to begin transaction");
        ApiError::Internal("internal server error".to_string())
    })?;

    // Advisory lock to serialize concurrent requests for the same law.
    sqlx::query("SELECT pg_advisory_xact_lock(hashtext($1))")
        .bind(&law_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, law_id = %law_id, "failed to acquire advisory lock");
            ApiError::Internal("internal server error".to_string())
        })?;

    // Check if law is exhausted for enrich.
    match regelrecht_pipeline::law_status::get_law(&mut *tx, &law_id).await {
        Ok(law) if law.status == regelrecht_pipeline::LawStatusValue::EnrichExhausted => {
            return Err(ApiError::Conflict(format!("{law_id} is enrich_exhausted — reset via /api/law_entries/{law_id}/reset-exhausted first")));
        }
        Err(regelrecht_pipeline::PipelineError::LawNotFound(_)) => {}
        Err(e) => {
            tracing::error!(error = %e, "failed to check exhausted status");
            return Err(ApiError::Internal(
                "failed to check exhausted status".to_string(),
            ));
        }
        Ok(_) => {}
    }

    // Look up the law to find its yaml_path from the most recent completed harvest job.
    let harvest_result: Option<(serde_json::Value,)> = sqlx::query_as(
        "SELECT result FROM jobs \
         WHERE law_id = $1 AND job_type = 'harvest' AND status = 'completed' \
         ORDER BY completed_at DESC LIMIT 1",
    )
    .bind(&law_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| {
        tracing::error!(error = %e, law_id = %law_id, "failed to look up harvest result");
        ApiError::Internal("failed to look up harvest result".to_string())
    })?;

    let yaml_path = harvest_result
        .as_ref()
        .and_then(|(result,)| result.get("file_path"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            ApiError::BadRequest(format!(
                "no completed harvest found for {law_id} — harvest the law first"
            ))
        })?
        .to_string();

    let priority = Priority::new(body.priority.unwrap_or(50));
    let mut job_ids = Vec::new();
    let mut providers = Vec::new();
    let mut last_job_id = None;

    for provider_name in ENRICH_PROVIDERS {
        let enrich_payload = EnrichPayload {
            law_id: law_id.clone(),
            yaml_path: yaml_path.clone(),
            provider: Some((*provider_name).to_string()),
        };

        let payload_json = serde_json::to_value(&enrich_payload).map_err(|e| {
            tracing::error!(error = %e, "failed to serialize enrich payload");
            ApiError::Internal("failed to serialize enrich payload".to_string())
        })?;

        let enrich_req = CreateJobRequest::new(JobType::Enrich, &law_id)
            .with_priority(priority)
            .with_payload(payload_json);

        match create_enrich_job_if_not_exists(&mut *tx, enrich_req).await {
            Ok(Some(enrich_job)) => {
                last_job_id = Some(enrich_job.id);
                job_ids.push(enrich_job.id.to_string());
                providers.push(provider_name.to_string());
            }
            Ok(None) => {
                tracing::info!(
                    law_id = %law_id,
                    provider = %provider_name,
                    "skipping: active enrich job already exists"
                );
            }
            Err(e) => {
                tracing::error!(error = %e, law_id = %law_id, provider = %provider_name, "failed to create enrich job");
                return Err(ApiError::Internal(format!("failed to create enrich job for provider {provider_name} (transaction rolled back, no jobs were created)")));
            }
        }
    }

    if job_ids.is_empty() {
        return Err(ApiError::Conflict(format!(
            "enrich jobs already pending or processing for {law_id}"
        )));
    }

    // Link the last created enrich job to the law entry.
    // enrich_job_id is a single UUID column, so we store the most recent one.
    if let Some(job_id) = last_job_id {
        set_enrich_job(&mut *tx, &law_id, job_id)
            .await
            .map_err(|e| {
                tracing::error!(
                    error = %e,
                    law_id = %law_id,
                    "failed to link enrich job to law entry"
                );
                ApiError::Internal("failed to link enrich job".to_string())
            })?;
    }

    tx.commit().await.map_err(|e| {
        tracing::error!(error = %e, "failed to commit transaction");
        ApiError::Internal("internal server error".to_string())
    })?;

    tracing::info!(law_id = %law_id, jobs = ?job_ids, "created enrich jobs");

    Ok((
        StatusCode::CREATED,
        Json(CreateEnrichResponse {
            job_ids,
            law_id,
            providers,
        }),
    ))
}

// --- Get single Job ---

pub async fn get_job(
    State(state): State<AppState>,
    axum::extract::Path(job_id): axum::extract::Path<String>,
) -> Result<Json<Job>, ApiError> {
    let pool = &state.pool;

    let uuid: sqlx::types::Uuid = job_id
        .parse()
        .map_err(|_| ApiError::BadRequest(format!("invalid job id: {job_id}")))?;

    let job = sqlx::query_as::<_, Job>(
        "SELECT id, job_type, law_id, status, \
         priority, payload, result, progress, attempts, max_attempts, \
         created_at, updated_at, started_at, completed_at \
         FROM jobs WHERE id = $1",
    )
    .bind(uuid)
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        tracing::error!(error = %e, "get_job query failed");
        ApiError::Internal("internal server error".to_string())
    })?
    .ok_or_else(|| ApiError::NotFound(format!("job not found: {job_id}")))?;

    Ok(Json(job))
}

// --- Delete Jobs ---

#[derive(Deserialize)]
pub struct DeleteJobsRequest {
    pub job_ids: Vec<uuid::Uuid>,
}

#[derive(Serialize)]
pub struct DeleteJobsResponse {
    pub deleted: i64,
}

pub async fn delete_jobs(
    State(state): State<AppState>,
    body: axum::body::Bytes,
) -> Result<Json<DeleteJobsResponse>, ApiError> {
    let pool = &state.pool;

    if body.is_empty() {
        return Err(ApiError::BadRequest(
            "request body with job_ids is required".to_string(),
        ));
    }

    let req = serde_json::from_slice::<DeleteJobsRequest>(&body)
        .map_err(|e| ApiError::BadRequest(format!("invalid request body: {e}")))?;

    if req.job_ids.is_empty() {
        return Ok(Json(DeleteJobsResponse { deleted: 0 }));
    }

    if req.job_ids.len() > 1000 {
        return Err(ApiError::BadRequest(
            "job_ids array exceeds maximum size of 1000".to_string(),
        ));
    }

    let result = sqlx::query("DELETE FROM jobs WHERE id = ANY($1) AND status != 'processing'")
        .bind(&req.job_ids)
        .execute(pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to delete jobs");
            ApiError::Internal("failed to delete jobs".to_string())
        })?;

    let deleted = i64::try_from(result.rows_affected()).unwrap_or(i64::MAX);
    tracing::info!(deleted, "deleted jobs");

    Ok(Json(DeleteJobsResponse { deleted }))
}

// --- Reset exhausted ---

pub async fn reset_exhausted(
    State(state): State<AppState>,
    axum::extract::Path(law_id): axum::extract::Path<String>,
) -> Result<StatusCode, ApiError> {
    let pool = &state.pool;

    let mut tx = pool.begin().await.map_err(|e| {
        tracing::error!(error = %e, "failed to begin transaction");
        ApiError::Internal("internal server error".to_string())
    })?;

    // Read status inside the transaction to prevent TOCTOU race.
    let law = match regelrecht_pipeline::law_status::get_law(&mut *tx, &law_id).await {
        Ok(law) => law,
        Err(regelrecht_pipeline::PipelineError::LawNotFound(_)) => {
            return Err(ApiError::NotFound(format!("law not found: {law_id}")));
        }
        Err(e) => {
            tracing::error!(error = %e, "failed to get law");
            return Err(ApiError::Internal("internal server error".to_string()));
        }
    };

    let (job_type, new_status) = match law.status {
        regelrecht_pipeline::LawStatusValue::HarvestExhausted => (
            regelrecht_pipeline::JobType::Harvest,
            regelrecht_pipeline::LawStatusValue::HarvestFailed,
        ),
        regelrecht_pipeline::LawStatusValue::EnrichExhausted => (
            regelrecht_pipeline::JobType::Enrich,
            regelrecht_pipeline::LawStatusValue::EnrichFailed,
        ),
        _ => {
            return Err(ApiError::BadRequest(format!(
                "law is not exhausted (status: {})",
                law.status
            )))
        }
    };

    regelrecht_pipeline::law_status::reset_fail_count(&mut *tx, &law_id, job_type)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to reset fail count");
            ApiError::Internal("failed to reset fail count".to_string())
        })?;

    // Use update_status_if to only update when status is still exhausted,
    // preventing regression if the law was reset concurrently.
    regelrecht_pipeline::law_status::update_status_if(&mut *tx, &law_id, law.status, new_status)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to update status");
            ApiError::Internal("failed to update status".to_string())
        })?;

    tx.commit().await.map_err(|e| {
        tracing::error!(error = %e, "failed to commit transaction");
        ApiError::Internal("failed to commit transaction".to_string())
    })?;

    tracing::info!(law_id = %law_id, job_type = ?job_type, "exhausted status reset");
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    // --- validated_sort_column ---

    #[test]
    fn sort_column_valid() {
        let allowed = &["name", "date", "id"];
        assert_eq!(
            validated_sort_column(Some("name"), allowed, "id"),
            Some("name")
        );
    }

    #[test]
    fn sort_column_invalid_returns_none() {
        let allowed = &["name", "date"];
        assert_eq!(
            validated_sort_column(Some("injection"), allowed, "name"),
            None
        );
    }

    #[test]
    fn sort_column_none_uses_default() {
        let allowed = &["name", "date"];
        assert_eq!(validated_sort_column(None, allowed, "date"), Some("date"));
    }

    #[test]
    fn sort_column_default_not_in_allowed() {
        let allowed = &["name"];
        assert_eq!(validated_sort_column(None, allowed, "missing"), None);
    }

    // --- normalized_order ---

    #[test]
    fn order_asc_uppercase() {
        assert_eq!(normalized_order(Some("ASC")), "ASC");
    }

    #[test]
    fn order_asc_lowercase() {
        assert_eq!(normalized_order(Some("asc")), "ASC");
    }

    #[test]
    fn order_desc_uppercase() {
        assert_eq!(normalized_order(Some("DESC")), "DESC");
    }

    #[test]
    fn order_desc_lowercase() {
        assert_eq!(normalized_order(Some("desc")), "DESC");
    }

    #[test]
    fn order_none_defaults_to_desc() {
        assert_eq!(normalized_order(None), "DESC");
    }

    #[test]
    fn order_garbage_defaults_to_desc() {
        assert_eq!(normalized_order(Some("RANDOM")), "DESC");
    }

    // --- clamped_limit ---

    #[test]
    fn limit_default() {
        assert_eq!(clamped_limit(None), 50);
    }

    #[test]
    fn limit_below_min() {
        assert_eq!(clamped_limit(Some(0)), 1);
        assert_eq!(clamped_limit(Some(-10)), 1);
    }

    #[test]
    fn limit_above_max() {
        assert_eq!(clamped_limit(Some(500)), 200);
    }

    #[test]
    fn limit_normal() {
        assert_eq!(clamped_limit(Some(25)), 25);
    }

    // --- clamped_offset ---

    #[test]
    fn offset_default() {
        assert_eq!(clamped_offset(None), 0);
    }

    #[test]
    fn offset_negative() {
        assert_eq!(clamped_offset(Some(-5)), 0);
    }

    #[test]
    fn offset_normal() {
        assert_eq!(clamped_offset(Some(100)), 100);
    }

    // --- Allowlist constants ---

    #[test]
    fn law_allowlist_contains_expected_columns() {
        for col in &[
            "law_id",
            "law_name",
            "status",
            "coverage_score",
            "created_at",
            "updated_at",
        ] {
            assert!(
                ALLOWED_SORT_COLUMNS_LAW.contains(col),
                "missing law column: {col}"
            );
        }
    }

    // --- CreateJobBody deserialization ---

    #[test]
    fn create_job_body_with_law_id() {
        let json = r#"{"law_id": "BWBR0018451", "priority": 80, "date": "2026-01-01"}"#;
        let body: CreateJobBody = serde_json::from_str(json).unwrap();
        assert_eq!(body.law_id.as_deref(), Some("BWBR0018451"));
        assert_eq!(body.priority, Some(80));
        assert_eq!(body.date.as_deref(), Some("2026-01-01"));
    }

    #[test]
    fn create_job_body_with_cvdr_id() {
        let json = r#"{"law_id": "CVDR681386"}"#;
        let body: CreateJobBody = serde_json::from_str(json).unwrap();
        assert_eq!(body.law_id.as_deref(), Some("CVDR681386"));
    }

    #[test]
    fn create_job_body_legacy_bwb_id() {
        // Backward compatibility: old clients sending bwb_id
        let json = r#"{"bwb_id": "BWBR0018451"}"#;
        let body: CreateJobBody = serde_json::from_str(json).unwrap();
        assert!(body.law_id.is_none());
        assert_eq!(body.bwb_id.as_deref(), Some("BWBR0018451"));
    }

    #[test]
    fn create_job_body_minimal_empty() {
        let json = r#"{}"#;
        let body: CreateJobBody = serde_json::from_str(json).unwrap();
        assert!(body.law_id.is_none());
        assert!(body.bwb_id.is_none());
        assert!(body.priority.is_none());
        assert!(body.date.is_none());
    }

    // --- classify_law_id ---

    #[test]
    fn classify_bwb_id() {
        assert!(matches!(
            classify_law_id("BWBR0018451"),
            Some(LawIdType::Bwb(_))
        ));
    }

    #[test]
    fn classify_cvdr_id() {
        assert!(matches!(
            classify_law_id("CVDR681386"),
            Some(LawIdType::Cvdr(_))
        ));
    }

    #[test]
    fn classify_invalid_id() {
        assert!(classify_law_id("INVALID").is_none());
        assert!(classify_law_id("").is_none());
        assert!(classify_law_id("BWBR123").is_none()); // too few digits
        assert!(classify_law_id("CVDR12").is_none()); // too few digits
    }

    #[test]
    fn job_allowlist_contains_expected_columns() {
        for col in &[
            "id",
            "job_type",
            "law_id",
            "status",
            "priority",
            "attempts",
            "created_at",
            "updated_at",
            "started_at",
            "completed_at",
        ] {
            assert!(
                ALLOWED_SORT_COLUMNS_JOB.contains(col),
                "missing job column: {col}"
            );
        }
    }
}
