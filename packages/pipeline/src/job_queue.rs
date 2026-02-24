use uuid::Uuid;

use crate::error::{PipelineError, Result};
use crate::models::{Job, JobStatus, JobType, Priority};

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
