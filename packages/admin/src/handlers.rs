use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::Json;
use regelrecht_pipeline::harvest_request::{
    request_harvest, HarvestRequestOptions, HarvestRequestOutcome,
};
use regelrecht_pipeline::job_queue::{create_enrich_job_if_not_exists, CreateJobRequest};
use regelrecht_pipeline::law_status::set_enrich_job;
use regelrecht_pipeline::{EnrichPayload, JobType, Priority, ENRICH_PROVIDERS};
use serde::{Deserialize, Serialize};

use crate::error::ApiError;
use crate::models::{Job, LawEntry, PaginatedResponse, Untranslatable};
use crate::state::AppState;

/// Map a sqlx error to a 500 ApiError, logging the cause with `op` so the log
/// names which query failed. Centralises ~17 copy-pasted `.map_err(|e| {
/// tracing::error!(error = %e, "<op>"); ApiError::Internal("internal server
/// error".into()) })` blocks. Internal-only — the user always sees the
/// generic "internal server error" message.
fn db_err(op: &'static str) -> impl FnOnce(sqlx::Error) -> ApiError {
    move |e: sqlx::Error| {
        tracing::error!(error = %e, "{op}");
        ApiError::Internal("internal server error".to_string())
    }
}

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

/// SQL ORDER BY expression for a validated job sort column. For `status` we
/// substitute a CASE expression that orders by relevance (failed → pending →
/// processing → completed) instead of alphabetical, since that's the order
/// operators actually scan for issues. Higher number = higher relevance, so a
/// DESC sort surfaces failed first.
fn job_sort_expression(col: &str) -> String {
    match col {
        "status" => "CASE status::text \
            WHEN 'failed' THEN 4 \
            WHEN 'pending' THEN 3 \
            WHEN 'processing' THEN 2 \
            WHEN 'completed' THEN 1 \
            ELSE 0 END"
            .to_string(),
        other => other.to_string(),
    }
}

/// SQL ORDER BY expression for the law_entries query. Treats NULL
/// `coverage_score` as the lowest value (via COALESCE) so empty coverage sorts
/// after 0% rather than at the top with DESC default NULLS FIRST.
fn law_sort_expression(col: &str) -> String {
    match col {
        "coverage_score" => "COALESCE(coverage_score, -1)".to_string(),
        other => other.to_string(),
    }
}

/// Escape LIKE/ILIKE wildcard metacharacters so user input matches literally.
/// Postgres uses `\` as the default escape character; we escape `\` first so
/// the subsequent `\%` / `\_` insertions aren't themselves re-escaped.
fn like_escape(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
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
    let sort_expr = law_sort_expression(sort_column);

    // Count query
    let total: i64 = if let Some(ref status) = params.status {
        sqlx::query_scalar("SELECT COUNT(*) FROM law_entries WHERE status::text = $1")
            .bind(status)
            .fetch_one(pool)
            .await
            .map_err(db_err("count query failed"))?
    } else {
        sqlx::query_scalar("SELECT COUNT(*) FROM law_entries")
            .fetch_one(pool)
            .await
            .map_err(db_err("count query failed"))?
    };

    // Data query — sort column is validated against an allowlist above, so
    // interpolating it into the query string is safe.
    let query_str = if params.status.is_some() {
        format!(
            "SELECT law_id, law_name, slug, status, coverage_score, \
             harvest_job_id, enrich_job_id, harvest_fail_count, enrich_fail_count, \
             created_at, updated_at \
             FROM law_entries WHERE status::text = $1 \
             ORDER BY {sort_expr} {order} LIMIT $2 OFFSET $3"
        )
    } else {
        format!(
            "SELECT law_id, law_name, slug, status, coverage_score, \
             harvest_job_id, enrich_job_id, harvest_fail_count, enrich_fail_count, \
             created_at, updated_at \
             FROM law_entries \
             ORDER BY {sort_expr} {order} LIMIT $1 OFFSET $2"
        )
    };

    let data: Vec<LawEntry> = if let Some(ref status) = params.status {
        sqlx::query_as::<_, LawEntry>(&query_str)
            .bind(status)
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await
            .map_err(db_err("data query failed"))?
    } else {
        sqlx::query_as::<_, LawEntry>(&query_str)
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await
            .map_err(db_err("data query failed"))?
    };

    Ok(Json(PaginatedResponse {
        data,
        total,
        limit,
        offset,
    }))
}

// --- Untranslatables ---

#[derive(Deserialize)]
pub struct UntranslatablesQuery {
    pub law_id: Option<String>,
    pub provider: Option<String>,
    pub accepted: Option<bool>,
    pub construct: Option<String>,
    pub sort: Option<String>,
    pub order: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

const ALLOWED_SORT_COLUMNS_UNTRANSLATABLE: &[&str] = &[
    "id",
    "law_id",
    "provider",
    "article",
    "construct",
    "accepted",
    "created_at",
];

pub async fn list_untranslatables(
    State(state): State<AppState>,
    Query(params): Query<UntranslatablesQuery>,
) -> Result<Json<PaginatedResponse<Untranslatable>>, ApiError> {
    let pool = &state.pool;
    let limit = clamped_limit(params.limit);
    let offset = clamped_offset(params.offset);

    let sort_column = validated_sort_column(
        params.sort.as_deref(),
        ALLOWED_SORT_COLUMNS_UNTRANSLATABLE,
        "created_at",
    )
    .ok_or(ApiError::BadRequest("invalid sort column".to_string()))?;

    let order = normalized_order(params.order.as_deref());

    // Build dynamic WHERE clause for multi-filter support. Columns are qualified
    // with `u.` so they stay unambiguous under the law_entries join.
    let mut where_clauses = Vec::new();
    let mut bind_index: usize = 1;

    if params.law_id.is_some() {
        // Partial / case-insensitive match, matching the jobs/law search fields.
        where_clauses.push(format!("u.law_id ILIKE ${bind_index}"));
        bind_index += 1;
    }
    if params.provider.is_some() {
        where_clauses.push(format!("u.provider = ${bind_index}"));
        bind_index += 1;
    }
    if params.accepted.is_some() {
        where_clauses.push(format!("u.accepted = ${bind_index}"));
        bind_index += 1;
    }
    if params.construct.is_some() {
        where_clauses.push(format!("u.construct ILIKE ${bind_index}"));
        bind_index += 1;
    }

    let where_sql = if where_clauses.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", where_clauses.join(" AND "))
    };

    // Count query (no join needed — filters are all on the untranslatables table).
    // Filter values are bound in the same order for the count and data queries.
    let count_sql = format!("SELECT COUNT(*) FROM untranslatables u {where_sql}");
    let mut count_query = sqlx::query_scalar::<_, i64>(&count_sql);
    if let Some(ref law_id) = params.law_id {
        count_query = count_query.bind(format!("%{}%", like_escape(law_id)));
    }
    if let Some(ref provider) = params.provider {
        count_query = count_query.bind(provider);
    }
    if let Some(accepted) = params.accepted {
        count_query = count_query.bind(accepted);
    }
    if let Some(ref construct) = params.construct {
        count_query = count_query.bind(format!("%{}%", like_escape(construct)));
    }

    let total: i64 = count_query
        .fetch_one(pool)
        .await
        .map_err(db_err("count query failed"))?;

    // Data query — sort column is validated against an allowlist above, so
    // interpolating it into the query string is safe. LEFT JOIN so an
    // untranslatable whose law_entry is missing still appears (law_name = NULL).
    let limit_idx = bind_index;
    let offset_idx = bind_index + 1;
    let data_sql = format!(
        "SELECT u.id, u.law_id, le.law_name, u.enrich_job_id, u.provider, \
         u.article, u.construct, u.reason, u.suggestion, u.legal_text_excerpt, \
         u.accepted, u.created_at \
         FROM untranslatables u \
         LEFT JOIN law_entries le ON le.law_id = u.law_id \
         {where_sql} \
         ORDER BY u.{sort_column} {order} LIMIT ${limit_idx} OFFSET ${offset_idx}"
    );

    let mut data_query = sqlx::query_as::<_, Untranslatable>(&data_sql);
    if let Some(ref law_id) = params.law_id {
        data_query = data_query.bind(format!("%{}%", like_escape(law_id)));
    }
    if let Some(ref provider) = params.provider {
        data_query = data_query.bind(provider);
    }
    if let Some(accepted) = params.accepted {
        data_query = data_query.bind(accepted);
    }
    if let Some(ref construct) = params.construct {
        data_query = data_query.bind(format!("%{}%", like_escape(construct)));
    }
    data_query = data_query.bind(limit).bind(offset);

    let data: Vec<Untranslatable> = data_query
        .fetch_all(pool)
        .await
        .map_err(db_err("data query failed"))?;

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

const ALLOWED_SORT_COLUMNS_JOB_SUMMARY: &[&str] =
    &["law_id", "total_jobs", "latest_created_at", "status"];

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
        // Partial / case-insensitive match so the search field finds e.g.
        // "18" inside "BWBR0018451" or "cvdr" inside "CVDR681386".
        where_clauses.push(format!("law_id ILIKE ${bind_index}"));
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
        count_query = count_query.bind(format!("%{}%", like_escape(law_id)));
    }

    let total: i64 = count_query
        .fetch_one(pool)
        .await
        .map_err(db_err("count query failed"))?;

    // Data query — sort column is validated against an allowlist above, so
    // interpolating it into the query string is safe.
    let limit_idx = bind_index;
    let offset_idx = bind_index + 1;

    let sort_expr = job_sort_expression(sort_column);
    let data_sql = format!(
        "SELECT id, job_type, law_id, status, \
         priority, payload, result, progress, attempts, max_attempts, created_at, updated_at, started_at, completed_at, scheduled_at \
         FROM jobs {where_sql} \
         ORDER BY {sort_expr} {order} LIMIT ${limit_idx} OFFSET ${offset_idx}"
    );

    let mut data_query = sqlx::query_as::<_, Job>(&data_sql);
    if let Some(ref status) = params.status {
        data_query = data_query.bind(status);
    }
    if let Some(ref job_type) = params.job_type {
        data_query = data_query.bind(job_type);
    }
    if let Some(ref law_id) = params.law_id {
        data_query = data_query.bind(format!("%{}%", like_escape(law_id)));
    }
    data_query = data_query.bind(limit).bind(offset);

    let data: Vec<Job> = data_query
        .fetch_all(pool)
        .await
        .map_err(db_err("data query failed"))?;

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

    let total: i64 = count_query
        .fetch_one(pool)
        .await
        .map_err(db_err("count query failed"))?;

    // Data query — sort column is validated against an allowlist above, so
    // interpolating it into the query string is safe.
    let limit_idx = bind_index;
    let offset_idx = bind_index + 1;

    // Build ORDER BY clause. Status uses a multi-key sort by percentage
    // (failed% → pending% → processing% → completed%, all DESC) so rows
    // ladder from "most broken" at the top to "fully completed" at the
    // bottom, with predictable in-group ordering. The frontend's
    // GROUPED_SORT_OPTIONS deliberately omits directionLabels for status,
    // so `order` is ignored here — there is no meaningful ascending
    // equivalent of "least-broken-first" beyond reversing the existing
    // ladder, which we don't expose. Other columns use the generic
    // {expr} {order} shape.
    let order_by_clause = if sort_column == "status" {
        "failed::float / NULLIF(total_jobs, 0) DESC, \
         pending::float / NULLIF(total_jobs, 0) DESC, \
         processing::float / NULLIF(total_jobs, 0) DESC, \
         completed::float / NULLIF(total_jobs, 0) ASC"
            .to_string()
    } else {
        format!("{sort_column} {order}")
    };
    // Wrap the GROUP BY in a subquery so the ORDER BY can reference the
    // aggregate aliases (e.g. `failed`, `pending`) which Postgres won't
    // resolve when they're combined inside expressions like CASE/GREATEST
    // directly on the grouping query.
    let data_sql = format!(
        "SELECT * FROM ( \
            SELECT law_id, \
            COUNT(*) as total_jobs, \
            COUNT(*) FILTER (WHERE status = 'pending') as pending, \
            COUNT(*) FILTER (WHERE status = 'processing') as processing, \
            COUNT(*) FILTER (WHERE status = 'completed') as completed, \
            COUNT(*) FILTER (WHERE status = 'failed') as failed, \
            MAX(created_at) as latest_created_at \
            FROM jobs {where_sql} \
            GROUP BY law_id \
         ) sub \
         ORDER BY {order_by_clause} LIMIT ${limit_idx} OFFSET ${offset_idx}"
    );

    let mut data_query = sqlx::query_as::<_, JobSummary>(&data_sql);
    if let Some(ref status) = params.status {
        data_query = data_query.bind(status);
    }
    if let Some(ref job_type) = params.job_type {
        data_query = data_query.bind(job_type);
    }
    data_query = data_query.bind(limit).bind(offset);

    let data: Vec<JobSummary> = data_query
        .fetch_all(pool)
        .await
        .map_err(db_err("data query failed"))?;

    Ok(Json(PaginatedResponse {
        data,
        total,
        limit,
        offset,
    }))
}

// --- Dashboard stats ---

/// Job counts per status. The four fields are the full `job_status` enum; an
/// unrecognised status (e.g. a future migration) is logged and dropped rather
/// than surfaced, so the dashboard never shows a mystery bucket.
#[derive(Serialize, Default)]
pub struct StatusCounts {
    pub pending: i64,
    pub processing: i64,
    pub completed: i64,
    pub failed: i64,
}

impl StatusCounts {
    fn add(&mut self, status: &str, count: i64) {
        match status {
            "pending" => self.pending += count,
            "processing" => self.processing += count,
            "completed" => self.completed += count,
            "failed" => self.failed += count,
            other => tracing::warn!(status = other, "unknown job status in dashboard stats"),
        }
    }

    fn total(&self) -> i64 {
        self.pending + self.processing + self.completed + self.failed
    }

    fn merge(a: &Self, b: &Self) -> Self {
        Self {
            pending: a.pending + b.pending,
            processing: a.processing + b.processing,
            completed: a.completed + b.completed,
            failed: a.failed + b.failed,
        }
    }
}

/// Job counts per type. The two fields are the full `job_type` enum.
#[derive(Serialize, Default)]
pub struct TypeCounts {
    pub harvest: i64,
    pub enrich: i64,
}

/// Per-type breakdown of the status counts.
#[derive(Serialize, Default)]
pub struct TypeStatus {
    pub harvest: StatusCounts,
    pub enrich: StatusCounts,
}

#[derive(Serialize)]
pub struct JobsBlock {
    pub total: i64,
    pub by_type: TypeCounts,
    pub by_status: StatusCounts,
    pub by_type_status: TypeStatus,
}

/// Count of jobs "executed" in a time window, split by type. A job's effective
/// timestamp is `COALESCE(completed_at, created_at)`: terminal jobs
/// (completed/failed) count on their completion date, non-terminal jobs
/// (pending/processing) on their creation date.
#[derive(Serialize, Default)]
pub struct WindowCounts {
    pub total: i64,
    pub harvest: i64,
    pub enrich: i64,
}

#[derive(Serialize)]
pub struct ExecutedBlock {
    pub today: WindowCounts,
    pub last_7d: WindowCounts,
}

/// One failed job for the "recent failures" list, with its failure reason.
#[derive(Serialize, sqlx::FromRow)]
pub struct RecentFailure {
    pub id: uuid::Uuid,
    pub law_id: String,
    pub job_type: String,
    /// `COALESCE(completed_at, created_at)`; `created_at` is NOT NULL so this is
    /// never null.
    pub failed_at: chrono::DateTime<chrono::Utc>,
    pub error: String,
}

/// Per-day counts for one job type: jobs created that day (`added`) and jobs
/// that reached a terminal status that day (`succeeded`/`failed`).
#[derive(Serialize, Default)]
pub struct DailyCounts {
    pub added: i64,
    pub succeeded: i64,
    pub failed: i64,
}

/// One Europe/Amsterdam calendar day (`YYYY-MM-DD`) in the 14-day window.
#[derive(Serialize)]
pub struct DailyEntry {
    pub date: String,
    pub harvest: DailyCounts,
    pub enrich: DailyCounts,
}

#[derive(Serialize)]
pub struct DashboardStats {
    pub jobs: JobsBlock,
    pub executed: ExecutedBlock,
    pub open_untranslatables: i64,
    pub recent_failures: Vec<RecentFailure>,
    pub daily: Vec<DailyEntry>,
}

/// Aggregate snapshot for the harvester "Overzicht" dashboard: job counts by
/// type and status, jobs executed today / in the last 7 days, the number of
/// open (unaccepted) untranslatables, and the most recent failed jobs with
/// their reasons.
pub async fn dashboard_stats(
    State(state): State<AppState>,
) -> Result<Json<DashboardStats>, ApiError> {
    let pool = &state.pool;

    // 1. Jobs by type x status — fold into the per-type breakdown; totals and
    //    the type-agnostic status counts are derived from it.
    let type_status_rows = sqlx::query_as::<_, (String, String, i64)>(
        "SELECT job_type::text, status::text, COUNT(*) FROM jobs GROUP BY job_type, status",
    )
    .fetch_all(pool)
    .await
    .map_err(db_err("dashboard jobs-by-type-status query failed"))?;

    let mut by_type_status = TypeStatus::default();
    for (job_type, status, count) in type_status_rows {
        match job_type.as_str() {
            "harvest" => by_type_status.harvest.add(&status, count),
            "enrich" => by_type_status.enrich.add(&status, count),
            other => tracing::warn!(job_type = other, "unknown job type in dashboard stats"),
        }
    }
    let by_type = TypeCounts {
        harvest: by_type_status.harvest.total(),
        enrich: by_type_status.enrich.total(),
    };
    let by_status = StatusCounts::merge(&by_type_status.harvest, &by_type_status.enrich);
    let jobs = JobsBlock {
        total: by_type.harvest + by_type.enrich,
        by_type,
        by_status,
        by_type_status,
    };

    // 2. Executed today / last 7 days, per type. "Today" is the Europe/Amsterdam
    //    calendar day (same tz convention as the pipeline's hourly rate limiter).
    let executed_rows = sqlx::query_as::<_, (String, i64, i64)>(
        "SELECT job_type::text, \
             COUNT(*) FILTER (WHERE COALESCE(completed_at, created_at) \
                 >= (date_trunc('day', now() AT TIME ZONE 'Europe/Amsterdam') \
                     AT TIME ZONE 'Europe/Amsterdam')), \
             COUNT(*) FILTER (WHERE COALESCE(completed_at, created_at) \
                 >= now() - INTERVAL '7 days') \
         FROM jobs GROUP BY job_type",
    )
    .fetch_all(pool)
    .await
    .map_err(db_err("dashboard executed-windows query failed"))?;

    let mut today = WindowCounts::default();
    let mut last_7d = WindowCounts::default();
    for (job_type, today_count, week_count) in executed_rows {
        match job_type.as_str() {
            "harvest" => {
                today.harvest = today_count;
                last_7d.harvest = week_count;
            }
            "enrich" => {
                today.enrich = today_count;
                last_7d.enrich = week_count;
            }
            other => tracing::warn!(job_type = other, "unknown job type in dashboard stats"),
        }
    }
    today.total = today.harvest + today.enrich;
    last_7d.total = last_7d.harvest + last_7d.enrich;
    let executed = ExecutedBlock { today, last_7d };

    // 3. Open (unaccepted) untranslatables.
    let open_untranslatables: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM untranslatables WHERE accepted = false")
            .fetch_one(pool)
            .await
            .map_err(db_err("dashboard untranslatables count failed"))?;

    // 4. Recent failures with reason. Failed jobs store the reason in
    //    `result->>'error'` (no dedicated column); the placeholder only guards
    //    the theoretical case of a failed job whose result lacks that key.
    let recent_failures = sqlx::query_as::<_, RecentFailure>(
        "SELECT id, law_id, job_type::text AS job_type, \
             COALESCE(completed_at, created_at) AS failed_at, \
             COALESCE(result->>'error', 'onbekend') AS error \
         FROM jobs WHERE status = 'failed' \
         ORDER BY COALESCE(completed_at, created_at) DESC LIMIT 50",
    )
    .fetch_all(pool)
    .await
    .map_err(db_err("dashboard recent-failures query failed"))?;

    // 5. Daily series for the last 14 Europe/Amsterdam days: jobs added
    //    (created_at) and jobs finished (completed_at, split by outcome), per
    //    type. Days without activity come back as explicit zero rows so the
    //    frontend never fills gaps. Pre-aggregate per (day, type, kind) first —
    //    a day-skeleton joined directly against the jobs rows degrades into an
    //    O(days × rows) nested loop. The 15-day cutoffs give slack for the
    //    UTC↔Amsterdam offset; at most 6 aggregate rows join per day, so the
    //    SUM FILTER never double-counts.
    let daily_rows = sqlx::query_as::<_, (String, i64, i64, i64, i64, i64, i64)>(
        "WITH days AS ( \
             SELECT ((now() AT TIME ZONE 'Europe/Amsterdam')::date - offs) AS day \
             FROM generate_series(13, 0, -1) AS offs \
         ), events AS ( \
             SELECT (created_at AT TIME ZONE 'Europe/Amsterdam')::date AS day, \
                    job_type::text AS job_type, 'added' AS kind, COUNT(*) AS n \
             FROM jobs \
             WHERE created_at >= now() - INTERVAL '15 days' \
             GROUP BY 1, 2 \
             UNION ALL \
             SELECT (completed_at AT TIME ZONE 'Europe/Amsterdam')::date, \
                    job_type::text, \
                    CASE status::text WHEN 'completed' THEN 'succeeded' ELSE 'failed' END, \
                    COUNT(*) \
             FROM jobs \
             WHERE status IN ('completed', 'failed') \
               AND completed_at >= now() - INTERVAL '15 days' \
             GROUP BY 1, 2, 3 \
         ) \
         SELECT to_char(days.day, 'YYYY-MM-DD'), \
             COALESCE(SUM(e.n) FILTER (WHERE e.job_type = 'harvest' AND e.kind = 'added'), 0)::bigint, \
             COALESCE(SUM(e.n) FILTER (WHERE e.job_type = 'harvest' AND e.kind = 'succeeded'), 0)::bigint, \
             COALESCE(SUM(e.n) FILTER (WHERE e.job_type = 'harvest' AND e.kind = 'failed'), 0)::bigint, \
             COALESCE(SUM(e.n) FILTER (WHERE e.job_type = 'enrich' AND e.kind = 'added'), 0)::bigint, \
             COALESCE(SUM(e.n) FILTER (WHERE e.job_type = 'enrich' AND e.kind = 'succeeded'), 0)::bigint, \
             COALESCE(SUM(e.n) FILTER (WHERE e.job_type = 'enrich' AND e.kind = 'failed'), 0)::bigint \
         FROM days \
         LEFT JOIN events e ON e.day = days.day \
         GROUP BY days.day \
         ORDER BY days.day",
    )
    .fetch_all(pool)
    .await
    .map_err(db_err("dashboard daily-series query failed"))?;

    let daily: Vec<DailyEntry> = daily_rows
        .into_iter()
        .map(
            |(date, h_added, h_succeeded, h_failed, e_added, e_succeeded, e_failed)| DailyEntry {
                date,
                harvest: DailyCounts {
                    added: h_added,
                    succeeded: h_succeeded,
                    failed: h_failed,
                },
                enrich: DailyCounts {
                    added: e_added,
                    succeeded: e_succeeded,
                    failed: e_failed,
                },
            },
        )
        .collect();

    Ok(Json(DashboardStats {
        jobs,
        executed,
        open_untranslatables,
        recent_failures,
        daily,
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

    // detect_source validates the ID format (prefix + digit count)
    regelrecht_harvester::detect_source(&raw_id).map_err(|e| {
        tracing::debug!(law_id = %raw_id, error = %e, "rejected invalid law ID");
        ApiError::BadRequest(format!("invalid law ID: {e}"))
    })?;

    let law_id = raw_id;

    // All harvest-request semantics (advisory lock, dedup, exhausted check,
    // date validation, law upsert + status + job link) live in the canonical
    // pipeline function; this handler only parses input and maps the outcome.
    let opts = HarvestRequestOptions {
        priority: Priority::new(body.priority.unwrap_or(50)),
        date: body.date,
        law_name: None,
        slug: None,
    };

    match request_harvest(&state.pool, &law_id, opts).await {
        Ok(HarvestRequestOutcome::Created(job)) => {
            tracing::info!(job_id = %job.id, law_id = %law_id, "created harvest job");
            Ok((
                StatusCode::CREATED,
                Json(CreateJobResponse {
                    job_id: job.id.to_string(),
                    law_id,
                }),
            ))
        }
        Ok(HarvestRequestOutcome::AlreadyQueued { existing_job_id }) => {
            Err(ApiError::Conflict(format!(
                "a pending or processing harvest job already exists: {existing_job_id}"
            )))
        }
        Ok(HarvestRequestOutcome::Exhausted) => Err(ApiError::Conflict(format!(
            "{law_id} is harvest_exhausted — reset via /api/law_entries/{law_id}/reset-exhausted first"
        ))),
        Ok(HarvestRequestOutcome::InvalidDate { reason }) => {
            Err(ApiError::BadRequest(format!("invalid date: {reason}")))
        }
        Err(e) => {
            tracing::error!(error = %e, law_id = %law_id, "failed to create harvest job");
            Err(ApiError::Internal("failed to create harvest job".to_string()))
        }
    }
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

    let mut tx = pool
        .begin()
        .await
        .map_err(db_err("failed to begin transaction"))?;

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
            // Admin-requested enrichments are roots of the related-harvest chain.
            depth: None,
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

    tx.commit()
        .await
        .map_err(db_err("failed to commit transaction"))?;

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
         created_at, updated_at, started_at, completed_at, scheduled_at \
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

    let mut tx = pool
        .begin()
        .await
        .map_err(db_err("failed to begin transaction"))?;

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

    // --- detect_source (via harvester crate) ---

    #[test]
    fn detect_bwb_source() {
        let source = regelrecht_harvester::detect_source("BWBR0018451").unwrap();
        assert_eq!(source.name(), "BWB");
    }

    #[test]
    fn detect_cvdr_source() {
        let source = regelrecht_harvester::detect_source("CVDR681386").unwrap();
        assert_eq!(source.name(), "CVDR");
    }

    #[test]
    fn detect_invalid_source() {
        assert!(regelrecht_harvester::detect_source("INVALID").is_err());
        assert!(regelrecht_harvester::detect_source("").is_err());
    }

    #[test]
    fn validate_bwb_id_too_few_digits() {
        // detect_source validates internally — short BWB IDs are rejected
        assert!(regelrecht_harvester::detect_source("BWBR123").is_err());
    }

    #[test]
    fn validate_cvdr_id_too_few_digits() {
        // detect_source validates internally — short CVDR IDs are rejected
        assert!(regelrecht_harvester::detect_source("CVDR12").is_err());
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
