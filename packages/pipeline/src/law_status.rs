use uuid::Uuid;

use crate::error::{PipelineError, Result};
use crate::models::{LawEntry, LawStatusValue};

/// Upsert a law entry. Creates it if it doesn't exist, updates name if it does.
#[tracing::instrument(skip(executor))]
pub async fn upsert_law<'e, E>(
    executor: E,
    law_id: &str,
    law_name: Option<&str>,
) -> Result<LawEntry>
where
    E: sqlx::PgExecutor<'e>,
{
    let entry = sqlx::query_as::<_, LawEntry>(
        r#"
        INSERT INTO law_entries (law_id, law_name)
        VALUES ($1, $2)
        ON CONFLICT (law_id) DO UPDATE SET law_name = COALESCE($2, law_entries.law_name)
        RETURNING *
        "#,
    )
    .bind(law_id)
    .bind(law_name)
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

/// Set the quality score for a law entry. Score must be finite and between 0.0 and 1.0.
#[tracing::instrument(skip(executor))]
pub async fn set_quality_score<'e, E>(executor: E, law_id: &str, score: f64) -> Result<LawEntry>
where
    E: sqlx::PgExecutor<'e>,
{
    if !score.is_finite() || !(0.0..=1.0).contains(&score) {
        return Err(PipelineError::InvalidInput(format!(
            "quality_score must be between 0.0 and 1.0, got {score}"
        )));
    }

    let entry = sqlx::query_as::<_, LawEntry>(
        r#"
        UPDATE law_entries SET quality_score = $2
        WHERE law_id = $1
        RETURNING *
        "#,
    )
    .bind(law_id)
    .bind(score)
    .fetch_optional(executor)
    .await?
    .ok_or_else(|| PipelineError::LawNotFound(law_id.to_string()))?;

    tracing::info!(law_id = %entry.law_id, score, "quality score updated");
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
