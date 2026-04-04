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
) -> Result<Json<PaginatedResponse<LawEntry>>, (StatusCode, String)> {
    let pool = &state.pool;
    let limit = clamped_limit(params.limit);
    let offset = clamped_offset(params.offset);

    let sort_column = validated_sort_column(
        params.sort.as_deref(),
        ALLOWED_SORT_COLUMNS_LAW,
        "updated_at",
    )
    .ok_or((StatusCode::BAD_REQUEST, "invalid sort column".to_string()))?;

    let order = normalized_order(params.order.as_deref());

    // Count query
    let total: i64 = if let Some(ref status) = params.status {
        sqlx::query_scalar("SELECT COUNT(*) FROM law_entries WHERE status::text = $1")
            .bind(status)
            .fetch_one(pool)
            .await
            .map_err(|e| {
                tracing::error!(error = %e, "count query failed");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal server error".to_string(),
                )
            })?
    } else {
        sqlx::query_scalar("SELECT COUNT(*) FROM law_entries")
            .fetch_one(pool)
            .await
            .map_err(|e| {
                tracing::error!(error = %e, "count query failed");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal server error".to_string(),
                )
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
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal server error".to_string(),
                )
            })?
    } else {
        sqlx::query_as::<_, LawEntry>(&query_str)
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await
            .map_err(|e| {
                tracing::error!(error = %e, "data query failed");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal server error".to_string(),
                )
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
) -> Result<Json<PaginatedResponse<Job>>, (StatusCode, String)> {
    let pool = &state.pool;
    let limit = clamped_limit(params.limit);
    let offset = clamped_offset(params.offset);

    let sort_column = validated_sort_column(
        params.sort.as_deref(),
        ALLOWED_SORT_COLUMNS_JOB,
        "created_at",
    )
    .ok_or((StatusCode::BAD_REQUEST, "invalid sort column".to_string()))?;

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
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal server error".to_string(),
        )
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
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal server error".to_string(),
        )
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
) -> Result<Json<PaginatedResponse<JobSummary>>, (StatusCode, String)> {
    let pool = &state.pool;
    let limit = clamped_limit(params.limit);
    let offset = clamped_offset(params.offset);

    let sort_column = validated_sort_column(
        params.sort.as_deref(),
        ALLOWED_SORT_COLUMNS_JOB_SUMMARY,
        "latest_created_at",
    )
    .ok_or((StatusCode::BAD_REQUEST, "invalid sort column".to_string()))?;

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
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal server error".to_string(),
        )
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
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal server error".to_string(),
        )
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
    pub bwb_id: String,
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
) -> Result<(StatusCode, Json<CreateJobResponse>), (StatusCode, String)> {
    let bwb_id = body.bwb_id.trim().to_string();
    if bwb_id.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "bwb_id must not be empty".to_string(),
        ));
    }

    if !BWB_ID_PATTERN.is_match(&bwb_id) {
        tracing::debug!(bwb_id, "rejected invalid BWB ID");
        return Err((
            StatusCode::BAD_REQUEST,
            "invalid BWB ID format: expected BWBR followed by 7 digits".to_string(),
        ));
    }

    if let Some(ref date) = body.date {
        if chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d").is_err() {
            tracing::debug!(date, "rejected invalid date");
            return Err((
                StatusCode::BAD_REQUEST,
                "invalid date format: expected YYYY-MM-DD".to_string(),
            ));
        }
    }

    let pool = &state.pool;

    let mut tx = pool.begin().await.map_err(|e| {
        tracing::error!(error = %e, "failed to begin transaction");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal server error".to_string(),
        )
    })?;

    // Acquire an advisory lock keyed on the bwb_id to serialize concurrent requests
    // for the same law. This prevents the TOCTOU race where two requests both see
    // no existing job and both create one. The lock is released when the transaction
    // commits or rolls back.
    sqlx::query("SELECT pg_advisory_xact_lock(hashtext($1))")
        .bind(&bwb_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, law_id = %bwb_id, "failed to acquire advisory lock");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal server error".to_string(),
            )
        })?;

    // Check for existing pending or processing harvest job to prevent duplicates.
    let existing: Option<(sqlx::types::Uuid,)> = sqlx::query_as(
        "SELECT id FROM jobs \
         WHERE law_id = $1 AND job_type = 'harvest' AND status IN ('pending', 'processing') \
         LIMIT 1",
    )
    .bind(&bwb_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| {
        tracing::error!(error = %e, law_id = %bwb_id, "failed to check for existing jobs");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "failed to check for existing jobs".to_string(),
        )
    })?;

    if let Some((existing_id,)) = existing {
        return Err((
            StatusCode::CONFLICT,
            format!("a pending or processing harvest job already exists: {existing_id}"),
        ));
    }

    // Check if law is exhausted for harvest.
    // RowNotFound is fine (new law, can't be exhausted); other errors should propagate.
    match regelrecht_pipeline::law_status::get_law(&mut *tx, &bwb_id).await {
        Ok(law) if law.status == regelrecht_pipeline::LawStatusValue::HarvestExhausted => {
            return Err((
                StatusCode::CONFLICT,
                format!("{bwb_id} is harvest_exhausted — reset via /api/law_entries/{bwb_id}/reset-exhausted first"),
            ));
        }
        Err(regelrecht_pipeline::PipelineError::LawNotFound(_)) => {}
        Err(e) => {
            tracing::error!(error = %e, "failed to check exhausted status");
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
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
    .bind(&bwb_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        tracing::error!(error = %e, law_id = %bwb_id, "failed to upsert law entry");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "failed to upsert law entry".to_string(),
        )
    })?;

    let payload = HarvestPayload {
        bwb_id: bwb_id.clone(),
        date: body.date,
        max_size_mb: None,
        depth: None,
    };

    let priority = Priority::new(body.priority.unwrap_or(50));

    let req = CreateJobRequest::new(JobType::Harvest, &bwb_id)
        .with_priority(priority)
        .with_payload(serde_json::to_value(&payload).map_err(|e| {
            tracing::error!(error = %e, "failed to serialize payload");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to serialize payload".to_string(),
            )
        })?);

    let job = create_job(&mut *tx, req).await.map_err(|e| {
        tracing::error!(error = %e, law_id = %bwb_id, "failed to create harvest job");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "failed to create harvest job".to_string(),
        )
    })?;

    // Link the harvest job to the law entry.
    set_harvest_job(&mut *tx, &bwb_id, job.id).await.map_err(|e| {
        tracing::error!(error = %e, law_id = %bwb_id, job_id = %job.id, "failed to link harvest job to law entry");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "failed to link harvest job to law entry".to_string(),
        )
    })?;

    tx.commit().await.map_err(|e| {
        tracing::error!(error = %e, "failed to commit transaction");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal server error".to_string(),
        )
    })?;

    tracing::info!(job_id = %job.id, law_id = %bwb_id, "created harvest job");

    Ok((
        StatusCode::CREATED,
        Json(CreateJobResponse {
            job_id: job.id.to_string(),
            law_id: bwb_id,
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
) -> Result<(StatusCode, Json<CreateEnrichResponse>), (StatusCode, String)> {
    let law_id = body.law_id.trim().to_string();
    if law_id.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "law_id must not be empty".to_string(),
        ));
    }

    let pool = &state.pool;

    let mut tx = pool.begin().await.map_err(|e| {
        tracing::error!(error = %e, "failed to begin transaction");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal server error".to_string(),
        )
    })?;

    // Advisory lock to serialize concurrent requests for the same law.
    sqlx::query("SELECT pg_advisory_xact_lock(hashtext($1))")
        .bind(&law_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, law_id = %law_id, "failed to acquire advisory lock");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal server error".to_string(),
            )
        })?;

    // Check if law is exhausted for enrich.
    match regelrecht_pipeline::law_status::get_law(&mut *tx, &law_id).await {
        Ok(law) if law.status == regelrecht_pipeline::LawStatusValue::EnrichExhausted => {
            return Err((
                StatusCode::CONFLICT,
                format!("{law_id} is enrich_exhausted — reset via /api/law_entries/{law_id}/reset-exhausted first"),
            ));
        }
        Err(regelrecht_pipeline::PipelineError::LawNotFound(_)) => {}
        Err(e) => {
            tracing::error!(error = %e, "failed to check exhausted status");
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
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
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "failed to look up harvest result".to_string(),
        )
    })?;

    let yaml_path = harvest_result
        .as_ref()
        .and_then(|(result,)| result.get("file_path"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                format!("no completed harvest found for {law_id} — harvest the law first"),
            )
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
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to serialize enrich payload".to_string(),
            )
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
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("failed to create enrich job for provider {provider_name} (transaction rolled back, no jobs were created)"),
                ));
            }
        }
    }

    if job_ids.is_empty() {
        return Err((
            StatusCode::CONFLICT,
            format!("enrich jobs already pending or processing for {law_id}"),
        ));
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
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "failed to link enrich job".to_string(),
                )
            })?;
    }

    tx.commit().await.map_err(|e| {
        tracing::error!(error = %e, "failed to commit transaction");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal server error".to_string(),
        )
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
) -> Result<Json<Job>, (StatusCode, String)> {
    let pool = &state.pool;

    let uuid: sqlx::types::Uuid = job_id
        .parse()
        .map_err(|_| (StatusCode::BAD_REQUEST, format!("invalid job id: {job_id}")))?;

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
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal server error".to_string(),
        )
    })?
    .ok_or_else(|| (StatusCode::NOT_FOUND, format!("job not found: {job_id}")))?;

    Ok(Json(job))
}

// --- Delete Jobs ---

#[derive(Serialize)]
pub struct DeleteJobsResponse {
    pub deleted: i64,
}

pub async fn delete_all_jobs(
    State(state): State<AppState>,
) -> Result<Json<DeleteJobsResponse>, (StatusCode, String)> {
    let pool = &state.pool;

    let result = sqlx::query("DELETE FROM jobs WHERE status != 'processing'")
        .execute(pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to delete jobs");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to delete jobs".to_string(),
            )
        })?;

    let deleted = result.rows_affected() as i64;
    tracing::info!(deleted, "deleted non-processing jobs");

    Ok(Json(DeleteJobsResponse { deleted }))
}

// --- Reset exhausted ---

pub async fn reset_exhausted(
    State(state): State<AppState>,
    axum::extract::Path(law_id): axum::extract::Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    let pool = &state.pool;

    let mut tx = pool.begin().await.map_err(|e| {
        tracing::error!(error = %e, "failed to begin transaction");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal server error".to_string(),
        )
    })?;

    // Read status inside the transaction to prevent TOCTOU race.
    let law = match regelrecht_pipeline::law_status::get_law(&mut *tx, &law_id).await {
        Ok(law) => law,
        Err(regelrecht_pipeline::PipelineError::LawNotFound(_)) => {
            return Err((StatusCode::NOT_FOUND, format!("law not found: {law_id}")));
        }
        Err(e) => {
            tracing::error!(error = %e, "failed to get law");
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal server error".to_string(),
            ));
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
            return Err((
                StatusCode::BAD_REQUEST,
                format!("law is not exhausted (status: {})", law.status),
            ))
        }
    };

    regelrecht_pipeline::law_status::reset_fail_count(&mut *tx, &law_id, job_type)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to reset fail count");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to reset fail count".to_string(),
            )
        })?;

    // Use update_status_if to only update when status is still exhausted,
    // preventing regression if the law was reset concurrently.
    regelrecht_pipeline::law_status::update_status_if(&mut *tx, &law_id, law.status, new_status)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to update status");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to update status".to_string(),
            )
        })?;

    tx.commit().await.map_err(|e| {
        tracing::error!(error = %e, "failed to commit transaction");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "failed to commit transaction".to_string(),
        )
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
    fn create_job_body_full() {
        let json = r#"{"bwb_id": "BWBR0018451", "priority": 80, "date": "2026-01-01"}"#;
        let body: CreateJobBody = serde_json::from_str(json).unwrap();
        assert_eq!(body.bwb_id, "BWBR0018451");
        assert_eq!(body.priority, Some(80));
        assert_eq!(body.date.as_deref(), Some("2026-01-01"));
    }

    #[test]
    fn create_job_body_minimal() {
        let json = r#"{"bwb_id": "BWBR0018451"}"#;
        let body: CreateJobBody = serde_json::from_str(json).unwrap();
        assert_eq!(body.bwb_id, "BWBR0018451");
        assert_eq!(body.priority, None);
        assert_eq!(body.date, None);
    }

    #[test]
    fn create_job_body_missing_bwb_id() {
        let json = r#"{"priority": 50}"#;
        let result = serde_json::from_str::<CreateJobBody>(json);
        assert!(result.is_err());
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
