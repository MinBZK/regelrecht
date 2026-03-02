use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;

use crate::models::{Job, LawEntry, PaginatedResponse};
use crate::state::AppState;

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
    "quality_score",
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
            "SELECT law_id, law_name, status::text as status, quality_score, \
             harvest_job_id, enrich_job_id, created_at, updated_at \
             FROM law_entries WHERE status::text = $1 \
             ORDER BY {sort_column} {order} LIMIT $2 OFFSET $3"
        )
    } else {
        format!(
            "SELECT law_id, law_name, status::text as status, quality_score, \
             harvest_job_id, enrich_job_id, created_at, updated_at \
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
    pub sort: Option<String>,
    pub order: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

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

    // Build dynamic WHERE clause for dual-filter support.
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

    // Count query
    let count_sql = format!("SELECT COUNT(*) FROM jobs {where_sql}");

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
        "SELECT id, job_type::text as job_type, law_id, status::text as status, \
         priority, attempts, max_attempts, created_at, updated_at, started_at, completed_at \
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

#[cfg(test)]
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
            "quality_score",
            "created_at",
            "updated_at",
        ] {
            assert!(
                ALLOWED_SORT_COLUMNS_LAW.contains(col),
                "missing law column: {col}"
            );
        }
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
