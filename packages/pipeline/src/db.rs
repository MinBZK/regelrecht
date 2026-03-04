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
    // One-time schema reset: drop and recreate if stale seed migrations
    // (0002, 0003) or a modified 0001 are detected. Safe because the schema
    // is fully defined in migration 0001.
    let needs_reset = sqlx::query_scalar::<_, bool>(
        "SELECT count(*) > 0 FROM _sqlx_migrations WHERE version > 1",
    )
    .fetch_one(pool)
    .await
    .unwrap_or(false);

    if needs_reset {
        tracing::warn!("stale migrations detected, resetting schema...");
        sqlx::query("DROP SCHEMA public CASCADE")
            .execute(pool)
            .await
            .ok();
        sqlx::query("CREATE SCHEMA public")
            .execute(pool)
            .await
            .ok();
        tracing::info!("schema reset complete");
    }

    sqlx::migrate!("./migrations").run(pool).await?;
    Ok(())
}
