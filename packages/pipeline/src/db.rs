use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;

use crate::config::PipelineConfig;
use crate::error::Result;

/// Advisory lock key for migration coordination. All components that call
/// `ensure_schema` use this same key so only one runs migrations at a time.
pub const MIGRATION_LOCK_KEY: i64 = 0x5245_4745_4C52_4543; // "REGELREC"

pub async fn create_pool(config: &PipelineConfig) -> Result<PgPool> {
    let pool = PgPoolOptions::new()
        .max_connections(config.max_connections)
        .connect(&config.database_url)
        .await?;

    Ok(pool)
}

/// Ensure the database schema is up to date.
///
/// Uses a PostgreSQL advisory lock so that whichever component starts first
/// runs migrations while the others block. Since sqlx migrations are
/// idempotent, the second caller safely no-ops after the lock is released.
pub async fn ensure_schema(pool: &PgPool) -> Result<()> {
    // Acquire a dedicated connection so that pg_advisory_lock and
    // pg_advisory_unlock run on the same session (advisory locks are
    // session-level; routing through the pool could hit different connections).
    let mut conn = pool.acquire().await?;

    tracing::info!("acquiring migration lock...");
    sqlx::query("SELECT pg_advisory_lock($1)")
        .bind(MIGRATION_LOCK_KEY)
        .execute(conn.as_mut())
        .await?;

    // Run migrations using the pool (sqlx::migrate needs &PgPool).
    let result = run_migrations_inner(pool).await;

    // Always release the lock on the same connection, even on error.
    if let Err(e) = sqlx::query("SELECT pg_advisory_unlock($1)")
        .bind(MIGRATION_LOCK_KEY)
        .execute(conn.as_mut())
        .await
    {
        tracing::warn!(error = %e, "failed to release migration lock");
    }

    result
}

async fn run_migrations_inner(pool: &PgPool) -> Result<()> {
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

    tracing::info!("running database migrations...");
    sqlx::migrate!("./migrations").run(pool).await?;
    tracing::info!("migrations completed");
    Ok(())
}
