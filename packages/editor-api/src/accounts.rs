//! Account upsert layer.
//!
//! On every authenticated request we mirror the OIDC subject claim into a
//! local `accounts` row so downstream features (favorites, user_settings,
//! trajects) can foreign-key by `accounts.id` instead of by the opaque
//! OIDC sub string. The middleware runs after `require_session_auth`; for
//! handlers that need the resolved account the [`AccountRecord`] is made
//! available via [`axum::Extension`].

use axum::extract::{Request, State};
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::Response;
use sqlx::PgPool;
use tower_sessions::Session;
use uuid::Uuid;

use regelrecht_auth::{SESSION_KEY_EMAIL, SESSION_KEY_NAME, SESSION_KEY_SUB};

use crate::state::AppState;

#[derive(Debug, Clone)]
#[allow(dead_code)] // person_sub/email/name kept for handlers that need them (e.g. future /api/me); only `id` is read today.
pub struct AccountRecord {
    pub id: Uuid,
    pub person_sub: String,
    pub email: String,
    pub name: String,
}

/// Upsert the authenticated user into `accounts` and return the row.
///
/// Returns `Ok(None)` when the session has no OIDC sub (auth-disabled
/// mode) — callers map that to a no-op rather than an error so the
/// middleware can run uniformly across both modes.
pub async fn ensure_account(
    pool: &PgPool,
    session: &Session,
) -> Result<Option<AccountRecord>, StatusCode> {
    let sub: Option<String> = session
        .get(SESSION_KEY_SUB)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let Some(sub) = sub else { return Ok(None) };

    let email: String = session
        .get(SESSION_KEY_EMAIL)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .unwrap_or_default();
    let name: String = session
        .get(SESSION_KEY_NAME)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .unwrap_or_default();

    let row: (Uuid,) = sqlx::query_as(
        "INSERT INTO accounts (person_sub, email, name)
         VALUES ($1, $2, $3)
         ON CONFLICT (person_sub) DO UPDATE
            SET email = EXCLUDED.email,
                name  = EXCLUDED.name,
                updated_at = now()
         RETURNING id",
    )
    .bind(&sub)
    .bind(&email)
    .bind(&name)
    .fetch_one(pool)
    .await
    .map_err(|e| {
        tracing::error!(error = %e, "failed to upsert account");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Some(AccountRecord {
        id: row.0,
        person_sub: sub,
        email,
        name,
    }))
}

/// Axum middleware that ensures the authenticated user has a row in
/// `accounts` and exposes it to downstream handlers via
/// `axum::Extension<AccountRecord>`.
///
/// Must be mounted **after** `require_session_auth` so that the session
/// claims are guaranteed present (or auth is disabled).
pub async fn account_middleware(
    State(state): State<AppState>,
    session: Session,
    mut request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    if let Some(pool) = state.pool.as_ref() {
        if let Some(account) = ensure_account(pool, &session).await? {
            request.extensions_mut().insert(account);
        }
    }
    Ok(next.run(request).await)
}
