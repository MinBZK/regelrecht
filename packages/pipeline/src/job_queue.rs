use uuid::Uuid;

use crate::error::{PipelineError, Result};
use crate::models::{Job, JobStatus, JobType, Priority};

/// Internal row type for the reaper CTE result.
#[derive(sqlx::FromRow)]
struct ReapedRow {
    #[allow(dead_code)]
    id: Uuid,
    #[allow(dead_code)]
    law_id: String,
    #[allow(dead_code)]
    job_type: JobType,
    #[allow(dead_code)]
    status: JobStatus,
}

pub struct CreateJobRequest {
    pub job_type: JobType,
    pub law_id: String,
    pub priority: Priority,
    pub payload: Option<serde_json::Value>,
    pub max_attempts: i32,
}

impl CreateJobRequest {
    pub fn new(job_type: JobType, law_id: impl Into<String>) -> Self {
        Self {
            job_type,
            law_id: law_id.into(),
            priority: Priority::default(),
            payload: None,
            max_attempts: 3,
        }
    }

    pub fn with_priority(mut self, priority: Priority) -> Self {
        self.priority = priority;
        self
    }

    pub fn with_payload(mut self, payload: serde_json::Value) -> Self {
        self.payload = Some(payload);
        self
    }

    pub fn with_max_attempts(mut self, max_attempts: i32) -> Self {
        self.max_attempts = max_attempts.max(1);
        self
    }
}

/// Create a new job in the queue.
#[tracing::instrument(skip(executor, req), fields(job_type = ?req.job_type, law_id = %req.law_id, priority = req.priority.value()))]
pub async fn create_job<'e, E>(executor: E, req: CreateJobRequest) -> Result<Job>
where
    E: sqlx::PgExecutor<'e>,
{
    let job = sqlx::query_as::<_, Job>(
        r#"
        INSERT INTO jobs (job_type, law_id, priority, payload, max_attempts)
        VALUES ($1, $2, $3, $4, $5)
        RETURNING *
        "#,
    )
    .bind(req.job_type)
    .bind(&req.law_id)
    .bind(req.priority.value())
    .bind(&req.payload)
    .bind(req.max_attempts)
    .fetch_one(executor)
    .await?;

    tracing::info!(job_id = %job.id, "job created");
    Ok(job)
}

/// Claim the highest-priority pending job using FOR UPDATE SKIP LOCKED.
/// Returns None if no jobs are available.
#[tracing::instrument(skip(executor))]
pub async fn claim_job<'e, E>(executor: E, job_type: Option<JobType>) -> Result<Option<Job>>
where
    E: sqlx::PgExecutor<'e>,
{
    let job = match job_type {
        Some(jt) => {
            sqlx::query_as::<_, Job>(
                r#"
                UPDATE jobs
                SET status = 'processing', started_at = now(), attempts = attempts + 1
                WHERE id = (
                    SELECT id FROM jobs
                    WHERE status = 'pending' AND job_type = $1
                    ORDER BY priority DESC, created_at ASC
                    LIMIT 1
                    FOR UPDATE SKIP LOCKED
                )
                RETURNING *
                "#,
            )
            .bind(jt)
            .fetch_optional(executor)
            .await?
        }
        None => {
            sqlx::query_as::<_, Job>(
                r#"
                UPDATE jobs
                SET status = 'processing', started_at = now(), attempts = attempts + 1
                WHERE id = (
                    SELECT id FROM jobs
                    WHERE status = 'pending'
                    ORDER BY priority DESC, created_at ASC
                    LIMIT 1
                    FOR UPDATE SKIP LOCKED
                )
                RETURNING *
                "#,
            )
            .fetch_optional(executor)
            .await?
        }
    };

    if let Some(ref j) = job {
        tracing::info!(job_id = %j.id, law_id = %j.law_id, attempt = j.attempts, "job claimed");
    }
    Ok(job)
}

/// Mark a job as completed with an optional result payload.
#[tracing::instrument(skip(executor, result))]
pub async fn complete_job<'e, E>(
    executor: E,
    job_id: Uuid,
    result: Option<serde_json::Value>,
) -> Result<Job>
where
    E: sqlx::PgExecutor<'e>,
{
    let job = sqlx::query_as::<_, Job>(
        r#"
        UPDATE jobs
        SET status = 'completed', completed_at = now(), result = $2
        WHERE id = $1 AND status = 'processing'
        RETURNING *
        "#,
    )
    .bind(job_id)
    .bind(&result)
    .fetch_optional(executor)
    .await?
    .ok_or(PipelineError::JobNotProcessing(job_id))?;

    tracing::info!(job_id = %job.id, law_id = %job.law_id, "job completed");
    Ok(job)
}

/// Mark a job as failed. If attempts < max_attempts, reset to pending for retry.
#[tracing::instrument(skip(executor, error_result))]
pub async fn fail_job<'e, E>(
    executor: E,
    job_id: Uuid,
    error_result: Option<serde_json::Value>,
) -> Result<Job>
where
    E: sqlx::PgExecutor<'e>,
{
    let job = sqlx::query_as::<_, Job>(
        r#"
        UPDATE jobs
        SET status = CASE
                WHEN attempts < max_attempts THEN 'pending'::job_status
                ELSE 'failed'::job_status
            END,
            result = $2,
            completed_at = CASE
                WHEN attempts >= max_attempts THEN now()
                ELSE NULL
            END
        WHERE id = $1 AND status = 'processing'
        RETURNING *
        "#,
    )
    .bind(job_id)
    .bind(&error_result)
    .fetch_optional(executor)
    .await?
    .ok_or(PipelineError::JobNotProcessing(job_id))?;

    match job.status {
        JobStatus::Pending => {
            tracing::info!(job_id = %job.id, attempt = job.attempts, max = job.max_attempts, "job failed, will retry");
        }
        JobStatus::Failed => {
            tracing::warn!(job_id = %job.id, attempts = job.attempts, "job permanently failed after exhausting retries");
        }
        _ => {}
    }
    Ok(job)
}

/// Reap orphaned jobs stuck in 'processing' for longer than `timeout`.
///
/// Jobs that remain in 'processing' beyond the timeout are assumed orphaned
/// (e.g., the worker crashed). If the job still has retries left, it is reset
/// to 'pending'; otherwise it is marked 'failed'.
///
/// Returns the number of reaped jobs.
#[tracing::instrument(skip(executor))]
pub async fn reap_orphaned_jobs<'e, E>(executor: E, timeout: std::time::Duration) -> Result<u64>
where
    E: sqlx::PgExecutor<'e>,
{
    let timeout_interval = sqlx::postgres::types::PgInterval::try_from(timeout)
        .map_err(|_| PipelineError::InvalidInput(format!("invalid reaper timeout: {timeout:?}")))?;

    let reaped_rows = sqlx::query_as::<_, ReapedRow>(
        r#"
        WITH reaped AS (
            UPDATE jobs
            SET status = CASE
                    WHEN attempts < max_attempts THEN 'pending'::job_status
                    ELSE 'failed'::job_status
                END,
                result = jsonb_build_object('error', 'reaped: job stuck in processing'),
                completed_at = CASE
                    WHEN attempts >= max_attempts THEN now()
                    ELSE NULL
                END
            WHERE status = 'processing'
              AND started_at < now() - $1::interval
            RETURNING id, law_id, job_type, status
        )
        SELECT id, law_id, job_type, status FROM reaped
        "#,
    )
    .bind(timeout_interval)
    .fetch_all(executor)
    .await?;

    let count = reaped_rows.len() as u64;
    if count > 0 {
        tracing::warn!(count, "reaped orphaned jobs stuck in processing");
    }
    Ok(count)
}

/// Create a harvest job only if no non-failed harvest job exists
/// for the same (law_id, date) combination.
///
/// Uses `INSERT ... WHERE NOT EXISTS` to reduce duplicates compared to a
/// separate check + insert. Note: under READ COMMITTED isolation, concurrent
/// transactions can still both insert if they evaluate the subquery before
/// either commits. This is acceptable for the single-worker MVP — duplicates
/// only cause redundant work, not data corruption.
///
/// Returns `Some(Job)` if a new job was created, `None` if a matching job already exists.
pub async fn create_harvest_job_if_not_exists<'e, E>(
    executor: E,
    req: CreateJobRequest,
    date: &str,
) -> Result<Option<Job>>
where
    E: sqlx::PgExecutor<'e>,
{
    let job = sqlx::query_as::<_, Job>(
        r#"
        INSERT INTO jobs (job_type, law_id, priority, payload, max_attempts)
        SELECT $1, $2, $3, $4, $5
        WHERE NOT EXISTS (
            SELECT 1 FROM jobs
            WHERE job_type = 'harvest'
              AND law_id = $2
              AND (payload->>'date') = $6
              AND status != 'failed'
        )
        RETURNING *
        "#,
    )
    .bind(req.job_type)
    .bind(&req.law_id)
    .bind(req.priority.value())
    .bind(&req.payload)
    .bind(req.max_attempts)
    .bind(date)
    .fetch_optional(executor)
    .await?;

    if let Some(ref j) = job {
        tracing::info!(job_id = %j.id, law_id = %j.law_id, "follow-up harvest job created");
    }

    Ok(job)
}

/// Get a job by ID.
pub async fn get_job<'e, E>(executor: E, job_id: Uuid) -> Result<Job>
where
    E: sqlx::PgExecutor<'e>,
{
    let job = sqlx::query_as::<_, Job>(r#"SELECT * FROM jobs WHERE id = $1"#)
        .bind(job_id)
        .fetch_optional(executor)
        .await?
        .ok_or(PipelineError::JobNotFound(job_id))?;

    Ok(job)
}

/// Create an enrich job if no active (pending/processing) enrich job exists
/// for this law_id + provider combination.
///
/// Uses `INSERT ... ON CONFLICT DO NOTHING` against the
/// `idx_unique_active_enrich_job` partial unique index to atomically
/// prevent duplicates — no TOCTOU race regardless of isolation level.
///
/// Returns `Some(job)` if created, `None` if a duplicate already existed.
pub async fn create_enrich_job_if_not_exists<'e, E>(
    executor: E,
    req: CreateJobRequest,
) -> Result<Option<Job>>
where
    E: sqlx::PgExecutor<'e>,
{
    let job = sqlx::query_as::<_, Job>(
        r#"
        INSERT INTO jobs (job_type, law_id, priority, payload, max_attempts)
        VALUES ($1, $2, $3, $4, $5)
        ON CONFLICT (law_id, job_type, (payload->>'provider'))
            WHERE job_type = 'enrich' AND status IN ('pending', 'processing')
        DO NOTHING
        RETURNING *
        "#,
    )
    .bind(req.job_type)
    .bind(&req.law_id)
    .bind(req.priority.value())
    .bind(&req.payload)
    .bind(req.max_attempts)
    .fetch_optional(executor)
    .await?;

    if let Some(ref j) = job {
        tracing::info!(job_id = %j.id, "enrich job created");
    }
    Ok(job)
}

/// List jobs with optional status filter.
pub async fn list_jobs<'e, E>(executor: E, status: Option<JobStatus>) -> Result<Vec<Job>>
where
    E: sqlx::PgExecutor<'e>,
{
    let jobs = match status {
        Some(s) => {
            sqlx::query_as::<_, Job>(
                r#"SELECT * FROM jobs WHERE status = $1 ORDER BY priority DESC, created_at ASC"#,
            )
            .bind(s)
            .fetch_all(executor)
            .await?
        }
        None => {
            sqlx::query_as::<_, Job>(r#"SELECT * FROM jobs ORDER BY priority DESC, created_at ASC"#)
                .fetch_all(executor)
                .await?
        }
    };

    Ok(jobs)
}
