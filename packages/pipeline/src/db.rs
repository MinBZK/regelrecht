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
    // Clean up migration records for removed seed migrations (0002, 0003)
    // so sqlx doesn't error on missing files.
    if let Ok(result) = sqlx::query("DELETE FROM _sqlx_migrations WHERE version > 1")
        .execute(pool)
        .await
    {
        if result.rows_affected() > 0 {
            tracing::info!(
                removed = result.rows_affected(),
                "cleaned up stale migration records"
            );
        }
    }

    sqlx::migrate!("./migrations").run(pool).await?;
    Ok(())
}
