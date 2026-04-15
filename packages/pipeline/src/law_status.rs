use uuid::Uuid;

use crate::error::{PipelineError, Result};
use crate::models::{LawEntry, LawStatusValue};

/// Upsert a law entry. Creates it if it doesn't exist, updates name/slug if it does.
#[tracing::instrument(skip(executor))]
pub async fn upsert_law<'e, E>(
    executor: E,
    law_id: &str,
    law_name: Option<&str>,
    slug: Option<&str>,
) -> Result<LawEntry>
where
    E: sqlx::PgExecutor<'e>,
{
    let entry = sqlx::query_as::<_, LawEntry>(
        r#"
        INSERT INTO law_entries (law_id, law_name, slug)
        VALUES ($1, $2, $3)
        ON CONFLICT (law_id) DO UPDATE SET
            law_name = COALESCE($2, law_entries.law_name),
            slug = COALESCE($3, law_entries.slug)
        RETURNING *
        "#,
    )
    .bind(law_id)
    .bind(law_name)
    .bind(slug)
    .fetch_one(executor)
    .await?;

    tracing::info!(law_id = %entry.law_id, "law upserted");
    Ok(entry)
}

/// Update the status of a law entry.
#[tracing::instrument(skip(executor))]
pub async fn update_status<'e, E>(
    executor: E,
    law_id: &str,
    status: LawStatusValue,
) -> Result<LawEntry>
where
    E: sqlx::PgExecutor<'e>,
{
    let entry = sqlx::query_as::<_, LawEntry>(
        r#"
        UPDATE law_entries SET status = $2
        WHERE law_id = $1
        RETURNING *
        "#,
    )
    .bind(law_id)
    .bind(status)
    .fetch_optional(executor)
    .await?
    .ok_or_else(|| PipelineError::LawNotFound(law_id.to_string()))?;

    tracing::info!(law_id = %entry.law_id, status = ?status, "law status updated");
    Ok(entry)
}

/// Link a harvest job to a law entry.
#[tracing::instrument(skip(executor))]
pub async fn set_harvest_job<'e, E>(executor: E, law_id: &str, job_id: Uuid) -> Result<LawEntry>
where
    E: sqlx::PgExecutor<'e>,
{
    let entry = sqlx::query_as::<_, LawEntry>(
        r#"
        UPDATE law_entries SET harvest_job_id = $2
        WHERE law_id = $1
        RETURNING *
        "#,
    )
    .bind(law_id)
    .bind(job_id)
    .fetch_optional(executor)
    .await?
    .ok_or_else(|| PipelineError::LawNotFound(law_id.to_string()))?;

    Ok(entry)
}

/// Link an enrich job to a law entry.
#[tracing::instrument(skip(executor))]
pub async fn set_enrich_job<'e, E>(executor: E, law_id: &str, job_id: Uuid) -> Result<LawEntry>
where
    E: sqlx::PgExecutor<'e>,
{
    let entry = sqlx::query_as::<_, LawEntry>(
        r#"
        UPDATE law_entries SET enrich_job_id = $2
        WHERE law_id = $1
        RETURNING *
        "#,
    )
    .bind(law_id)
    .bind(job_id)
    .fetch_optional(executor)
    .await?
    .ok_or_else(|| PipelineError::LawNotFound(law_id.to_string()))?;

    Ok(entry)
}

/// Set the coverage score for a law entry. Score must be finite and between 0.0 and 1.0.
///
/// Coverage score measures what fraction of articles have a `machine_readable`
/// section — it does NOT measure correctness or quality of that section.
#[tracing::instrument(skip(executor))]
pub async fn set_coverage_score<'e, E>(executor: E, law_id: &str, score: f64) -> Result<LawEntry>
where
    E: sqlx::PgExecutor<'e>,
{
    if !score.is_finite() || !(0.0..=1.0).contains(&score) {
        return Err(PipelineError::InvalidInput(format!(
            "coverage_score must be between 0.0 and 1.0, got {score}"
        )));
    }

    let entry = sqlx::query_as::<_, LawEntry>(
        r#"
        UPDATE law_entries SET coverage_score = $2
        WHERE law_id = $1
        RETURNING *
        "#,
    )
    .bind(law_id)
    .bind(score)
    .fetch_optional(executor)
    .await?
    .ok_or_else(|| PipelineError::LawNotFound(law_id.to_string()))?;

    tracing::info!(law_id = %entry.law_id, score, "coverage score updated");
    Ok(entry)
}

/// Atomically update status only if the current status matches `expected`.
///
/// Returns `Ok(Some(entry))` if the row was updated, `Ok(None)` if the current
/// status didn't match (no row modified). This avoids TOCTOU races that occur
/// with separate get_law + update_status calls.
#[tracing::instrument(skip(executor))]
pub async fn update_status_if<'e, E>(
    executor: E,
    law_id: &str,
    expected: LawStatusValue,
    new_status: LawStatusValue,
) -> Result<Option<LawEntry>>
where
    E: sqlx::PgExecutor<'e>,
{
    let entry = sqlx::query_as::<_, LawEntry>(
        r#"
        UPDATE law_entries SET status = $3
        WHERE law_id = $1 AND status = $2
        RETURNING *
        "#,
    )
    .bind(law_id)
    .bind(expected)
    .bind(new_status)
    .fetch_optional(executor)
    .await?;

    if let Some(ref e) = entry {
        tracing::info!(law_id = %e.law_id, from = ?expected, to = ?new_status, "law status conditionally updated");
    } else {
        tracing::debug!(law_id = %law_id, expected = ?expected, to = ?new_status, "conditional status update skipped (status mismatch)");
    }
    Ok(entry)
}

/// Atomically update status only if the current status is NOT the given value.
///
/// Returns `Ok(Some(entry))` if the row was updated, `Ok(None)` if the current
/// status matched `not_status` (no row modified).
#[tracing::instrument(skip(executor))]
pub async fn update_status_unless<'e, E>(
    executor: E,
    law_id: &str,
    not_status: LawStatusValue,
    new_status: LawStatusValue,
) -> Result<Option<LawEntry>>
where
    E: sqlx::PgExecutor<'e>,
{
    let entry = sqlx::query_as::<_, LawEntry>(
        r#"
        UPDATE law_entries SET status = $3
        WHERE law_id = $1 AND status != $2
        RETURNING *
        "#,
    )
    .bind(law_id)
    .bind(not_status)
    .bind(new_status)
    .fetch_optional(executor)
    .await?;

    if let Some(ref e) = entry {
        tracing::info!(law_id = %e.law_id, to = ?new_status, "law status updated (was not {:?})", not_status);
    } else {
        tracing::debug!(law_id = %law_id, not_status = ?not_status, to = ?new_status, "status update skipped (status is {:?})", not_status);
    }
    Ok(entry)
}

/// Get a law entry by ID.
pub async fn get_law<'e, E>(executor: E, law_id: &str) -> Result<LawEntry>
where
    E: sqlx::PgExecutor<'e>,
{
    let entry = sqlx::query_as::<_, LawEntry>(r#"SELECT * FROM law_entries WHERE law_id = $1"#)
        .bind(law_id)
        .fetch_optional(executor)
        .await?
        .ok_or_else(|| PipelineError::LawNotFound(law_id.to_string()))?;

    Ok(entry)
}

/// List all law entries, optionally filtered by status.
pub async fn list_laws<'e, E>(executor: E, status: Option<LawStatusValue>) -> Result<Vec<LawEntry>>
where
    E: sqlx::PgExecutor<'e>,
{
    let entries = match status {
        Some(s) => {
            sqlx::query_as::<_, LawEntry>(
                r#"SELECT * FROM law_entries WHERE status = $1 ORDER BY law_id"#,
            )
            .bind(s)
            .fetch_all(executor)
            .await?
        }
        None => {
            sqlx::query_as::<_, LawEntry>(r#"SELECT * FROM law_entries ORDER BY law_id"#)
                .fetch_all(executor)
                .await?
        }
    };

    Ok(entries)
}

/// Increment the consecutive fail count for a job type. Returns the new count.
#[tracing::instrument(skip(executor))]
pub async fn increment_fail_count<'e, E>(
    executor: E,
    law_id: &str,
    job_type: crate::models::JobType,
) -> Result<i32>
where
    E: sqlx::PgExecutor<'e>,
{
    let column = match job_type {
        crate::models::JobType::Harvest => "harvest_fail_count",
        crate::models::JobType::Enrich => "enrich_fail_count",
    };
    // Column name is from a match on an enum, not user input — safe to interpolate.
    let sql = format!(
        "UPDATE law_entries SET {column} = {column} + 1, updated_at = now() \
         WHERE law_id = $1 RETURNING {column}"
    );
    let count: (i32,) = sqlx::query_as(&sql)
        .bind(law_id)
        .fetch_one(executor)
        .await?;

    tracing::info!(law_id = %law_id, job_type = ?job_type, fail_count = count.0, "fail count incremented");
    Ok(count.0)
}

/// Reset the consecutive fail count for a job type to zero.
#[tracing::instrument(skip(executor))]
pub async fn reset_fail_count<'e, E>(
    executor: E,
    law_id: &str,
    job_type: crate::models::JobType,
) -> Result<()>
where
    E: sqlx::PgExecutor<'e>,
{
    let column = match job_type {
        crate::models::JobType::Harvest => "harvest_fail_count",
        crate::models::JobType::Enrich => "enrich_fail_count",
    };
    let sql = format!("UPDATE law_entries SET {column} = 0, updated_at = now() WHERE law_id = $1");
    sqlx::query(&sql).bind(law_id).execute(executor).await?;

    tracing::info!(law_id = %law_id, job_type = ?job_type, "fail count reset");
    Ok(())
}

/// Mark a law as exhausted for a given job type.
#[tracing::instrument(skip(executor))]
pub async fn exhaust_law<'e, E>(
    executor: E,
    law_id: &str,
    job_type: crate::models::JobType,
) -> Result<()>
where
    E: sqlx::PgExecutor<'e>,
{
    let (expected, new_status) = match job_type {
        crate::models::JobType::Harvest => (
            LawStatusValue::HarvestFailed,
            LawStatusValue::HarvestExhausted,
        ),
        crate::models::JobType::Enrich => (
            LawStatusValue::EnrichFailed,
            LawStatusValue::EnrichExhausted,
        ),
    };
    // Only exhaust if status is still the corresponding failed state,
    // preventing a race with admin reset.
    let result = sqlx::query(
        "UPDATE law_entries SET status = $2, updated_at = now() WHERE law_id = $1 AND status = $3",
    )
    .bind(law_id)
    .bind(new_status)
    .bind(expected)
    .execute(executor)
    .await?;

    if result.rows_affected() > 0 {
        tracing::warn!(law_id = %law_id, job_type = ?job_type, "law marked as exhausted");
    } else {
        tracing::debug!(law_id = %law_id, job_type = ?job_type, "exhaust_law skipped: status was not in expected failed state");
    }
    Ok(())
}
