use std::time::Duration;

use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;

use crate::config::PipelineConfig;
use crate::error::Result;

pub async fn create_pool(config: &PipelineConfig) -> Result<PgPool> {
    let pool = PgPoolOptions::new()
        .max_connections(config.max_connections)
        .connect(&config.database_url)
        .await?;

    Ok(pool)
}

/// Wait for the database schema to be ready (migrations run by admin).
pub async fn wait_for_schema(pool: &PgPool) -> Result<()> {
    let max_attempts = 30;
    for attempt in 1..=max_attempts {
        let ready: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM information_schema.tables WHERE table_name = 'jobs')",
        )
        .fetch_one(pool)
        .await
        .unwrap_or(false);

        if ready {
            tracing::info!("database schema ready");
            return Ok(());
        }

        tracing::info!(attempt, max_attempts, "waiting for database schema...");
        tokio::time::sleep(Duration::from_secs(2)).await;
    }

    Err(crate::error::PipelineError::Worker(
        "timed out waiting for database schema".to_string(),
    ))
}

pub async fn run_migrations(pool: &PgPool) -> Result<()> {
    // One-time schema reset: if stale seed migrations (versions > 1) are
    // detected, wipe all tables and migration records so 0001 re-runs cleanly.
    let needs_reset = sqlx::query_scalar::<_, bool>(
        "SELECT count(*) > 0 FROM _sqlx_migrations WHERE version > 1",
    )
    .fetch_one(pool)
    .await
    .unwrap_or(false);

    if needs_reset {
        tracing::warn!("stale migrations detected, resetting database...");
        for stmt in [
            "DROP TABLE IF EXISTS law_entries CASCADE",
            "DROP TABLE IF EXISTS jobs CASCADE",
            "DROP TYPE IF EXISTS job_type CASCADE",
            "DROP TYPE IF EXISTS job_status CASCADE",
            "DROP TYPE IF EXISTS law_status CASCADE",
            "DROP FUNCTION IF EXISTS update_updated_at CASCADE",
            "DELETE FROM _sqlx_migrations",
        ] {
            sqlx::query(stmt).execute(pool).await.ok();
        }
        tracing::info!("database reset complete");
    }

    sqlx::migrate!("./migrations").run(pool).await?;
    Ok(())
}
