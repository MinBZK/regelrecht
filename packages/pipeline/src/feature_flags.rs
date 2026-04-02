use crate::error::Result;
use crate::models::FeatureFlag;

/// List all feature flags.
pub async fn list_flags<'e, E>(executor: E) -> Result<Vec<FeatureFlag>>
where
    E: sqlx::PgExecutor<'e>,
{
    let flags = sqlx::query_as::<_, FeatureFlag>("SELECT * FROM feature_flags ORDER BY key")
        .fetch_all(executor)
        .await?;
    Ok(flags)
}

/// Get a single feature flag by key.
pub async fn get_flag<'e, E>(executor: E, key: &str) -> Result<Option<FeatureFlag>>
where
    E: sqlx::PgExecutor<'e>,
{
    let flag = sqlx::query_as::<_, FeatureFlag>("SELECT * FROM feature_flags WHERE key = $1")
        .bind(key)
        .fetch_optional(executor)
        .await?;
    Ok(flag)
}

/// Update the enabled status of a feature flag. Returns the updated flag.
pub async fn set_flag<'e, E>(executor: E, key: &str, enabled: bool) -> Result<Option<FeatureFlag>>
where
    E: sqlx::PgExecutor<'e>,
{
    let flag = sqlx::query_as::<_, FeatureFlag>(
        "UPDATE feature_flags SET enabled = $2 WHERE key = $1 RETURNING *",
    )
    .bind(key)
    .bind(enabled)
    .fetch_optional(executor)
    .await?;
    Ok(flag)
}

/// Create or update a feature flag.
pub async fn upsert_flag<'e, E>(
    executor: E,
    key: &str,
    enabled: bool,
    description: Option<&str>,
) -> Result<FeatureFlag>
where
    E: sqlx::PgExecutor<'e>,
{
    let flag = sqlx::query_as::<_, FeatureFlag>(
        r#"
        INSERT INTO feature_flags (key, enabled, description)
        VALUES ($1, $2, $3)
        ON CONFLICT (key) DO UPDATE SET enabled = $2, description = COALESCE($3, feature_flags.description)
        RETURNING *
        "#,
    )
    .bind(key)
    .bind(enabled)
    .bind(description)
    .fetch_one(executor)
    .await?;
    Ok(flag)
}
