use std::collections::HashMap;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;
use tower_sessions::Session;

use regelrecht_auth::SESSION_KEY_SUB;

use crate::state::AppState;

/// Keys the editor accepts as user settings. Unknown keys are rejected so the
/// table cannot be used as a catch-all key/value store for client-supplied data.
const ALLOWED_KEYS: &[&str] = &["theme"];

fn validate(key: &str, value: &str) -> Result<(), StatusCode> {
    if !ALLOWED_KEYS.contains(&key) {
        return Err(StatusCode::BAD_REQUEST);
    }
    match key {
        "theme" if !matches!(value, "light" | "dark") => Err(StatusCode::BAD_REQUEST),
        _ => Ok(()),
    }
}

async fn get_person_sub(session: &Session) -> Result<String, StatusCode> {
    session
        .get::<String>(SESSION_KEY_SUB)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::UNAUTHORIZED)
}

fn get_pool(state: &AppState) -> Result<&sqlx::PgPool, StatusCode> {
    state.pool.as_ref().ok_or(StatusCode::SERVICE_UNAVAILABLE)
}

/// GET /api/user/settings — return the authenticated user's settings.
/// An empty map is returned for a user who has never written a setting;
/// the frontend merges this with its client-side defaults.
pub async fn list(
    State(state): State<AppState>,
    session: Session,
) -> Result<Json<HashMap<String, String>>, StatusCode> {
    let person_sub = get_person_sub(&session).await?;
    let pool = get_pool(&state)?;

    let rows: Vec<(String, String)> =
        sqlx::query_as("SELECT key, value FROM user_settings WHERE person_sub = $1")
            .bind(&person_sub)
            .fetch_all(pool)
            .await
            .map_err(|e| {
                tracing::error!(error = %e, "failed to fetch user settings");
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

    Ok(Json(rows.into_iter().collect()))
}

#[derive(Deserialize)]
pub struct SetBody {
    pub value: String,
}

/// PUT /api/user/settings/{key} — idempotent upsert. The first write for a
/// user creates the row; subsequent writes update the value in place.
pub async fn set(
    State(state): State<AppState>,
    session: Session,
    Path(key): Path<String>,
    Json(body): Json<SetBody>,
) -> Result<StatusCode, StatusCode> {
    validate(&key, &body.value)?;
    let person_sub = get_person_sub(&session).await?;
    let pool = get_pool(&state)?;

    sqlx::query(
        "INSERT INTO user_settings (person_sub, key, value)
         VALUES ($1, $2, $3)
         ON CONFLICT (person_sub, key) DO UPDATE SET value = EXCLUDED.value",
    )
    .bind(&person_sub)
    .bind(&key)
    .bind(&body.value)
    .execute(pool)
    .await
    .map_err(|e| {
        tracing::error!(error = %e, "failed to upsert user setting");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_accepts_theme_light_and_dark() {
        assert!(validate("theme", "light").is_ok());
        assert!(validate("theme", "dark").is_ok());
    }

    #[test]
    fn validate_rejects_unknown_theme_value() {
        assert_eq!(validate("theme", "purple"), Err(StatusCode::BAD_REQUEST));
        assert_eq!(validate("theme", ""), Err(StatusCode::BAD_REQUEST));
    }

    #[test]
    fn validate_rejects_unknown_key() {
        assert_eq!(validate("foo", "anything"), Err(StatusCode::BAD_REQUEST));
        assert_eq!(validate("", "anything"), Err(StatusCode::BAD_REQUEST));
    }
}
