use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use tower_sessions::Session;

use regelrecht_auth::SESSION_KEY_SUB;

use crate::state::AppState;

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

/// GET /api/favorites — list the authenticated user's favorites.
pub async fn list(
    State(state): State<AppState>,
    session: Session,
) -> Result<Json<Vec<String>>, StatusCode> {
    let person_sub = get_person_sub(&session).await?;
    let pool = get_pool(&state)?;

    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT law_id FROM user_favorites WHERE person_sub = $1 ORDER BY created_at",
    )
    .bind(&person_sub)
    .fetch_all(pool)
    .await
    .map_err(|e| {
        tracing::error!(error = %e, "failed to fetch favorites");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(rows.into_iter().map(|(id,)| id).collect()))
}

fn validate_law_id(law_id: &str) -> Result<(), StatusCode> {
    if law_id.is_empty() || law_id.len() > 256 {
        return Err(StatusCode::BAD_REQUEST);
    }
    Ok(())
}

/// PUT /api/favorites/{law_id} — add a law to the user's favorites.
pub async fn add(
    State(state): State<AppState>,
    session: Session,
    Path(law_id): Path<String>,
) -> Result<StatusCode, StatusCode> {
    validate_law_id(&law_id)?;
    let person_sub = get_person_sub(&session).await?;
    let pool = get_pool(&state)?;

    sqlx::query(
        "INSERT INTO user_favorites (person_sub, law_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
    )
    .bind(&person_sub)
    .bind(&law_id)
    .execute(pool)
    .await
    .map_err(|e| {
        tracing::error!(error = %e, "failed to add favorite");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(StatusCode::CREATED)
}

/// DELETE /api/favorites/{law_id} — remove a law from the user's favorites.
pub async fn remove(
    State(state): State<AppState>,
    session: Session,
    Path(law_id): Path<String>,
) -> Result<StatusCode, StatusCode> {
    validate_law_id(&law_id)?;
    let person_sub = get_person_sub(&session).await?;
    let pool = get_pool(&state)?;

    sqlx::query("DELETE FROM user_favorites WHERE person_sub = $1 AND law_id = $2")
        .bind(&person_sub)
        .bind(&law_id)
        .execute(pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to remove favorite");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(StatusCode::NO_CONTENT)
}
