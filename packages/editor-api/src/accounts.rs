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
    let session_read_failed = |key: &'static str| {
        move |e: tower_sessions::session::Error| {
            tracing::error!(
                error = %e,
                key = %key,
                "session read failed in account middleware"
            );
            StatusCode::INTERNAL_SERVER_ERROR
        }
    };

    let sub: Option<String> = session
        .get(SESSION_KEY_SUB)
        .await
        .map_err(session_read_failed(SESSION_KEY_SUB))?;
    let Some(sub) = sub else { return Ok(None) };

    let email: String = session
        .get(SESSION_KEY_EMAIL)
        .await
        .map_err(session_read_failed(SESSION_KEY_EMAIL))?
        .unwrap_or_default();
    // The `email` column is UNIQUE NOT NULL — letting a missing email
    // claim default to "" would let the first such user create a row
    // with empty email and then every subsequent emailless user would
    // 500 on the constraint violation. Refuse the account creation up
    // front with a clear log line so a misconfigured IdP is visible
    // instead of presenting as a sporadic 500.
    if email.trim().is_empty() {
        tracing::error!(
            sub = %sub,
            "OIDC subject has no email claim — refusing to create account"
        );
        return Err(StatusCode::FORBIDDEN);
    }
    let name: String = session
        .get(SESSION_KEY_NAME)
        .await
        .map_err(session_read_failed(SESSION_KEY_NAME))?
        .unwrap_or_default();

    // The middleware runs on every request under the traject route layer,
    // so this query is on the hot path. The DO UPDATE … WHERE clause
    // skips the write (and the `updated_at = now()` bump) when nothing
    // actually changed; the trailing UNION ALL covers the case where the
    // WHERE filter held — `RETURNING` is empty for skipped updates so we
    // fall back to a plain SELECT against the same key. The combined
    // statement still hits the same `person_sub` index twice in the worst
    // case, but the common path is read-only after the first request.
    let row: (Uuid,) = sqlx::query_as(
        "WITH upserted AS (
             INSERT INTO accounts (person_sub, email, name)
             VALUES ($1, $2, $3)
             ON CONFLICT (person_sub) DO UPDATE
                SET email = EXCLUDED.email,
                    name  = EXCLUDED.name,
                    updated_at = now()
                WHERE accounts.email IS DISTINCT FROM EXCLUDED.email
                   OR accounts.name  IS DISTINCT FROM EXCLUDED.name
             RETURNING id
         )
         SELECT id FROM upserted
         UNION ALL
         SELECT id FROM accounts WHERE person_sub = $1
         LIMIT 1",
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

/// Promote any pending `traject_invites` rows matching the user's email
/// into real `traject_members` rows, then clear those invites.
///
/// Called on every authenticated request after the account upsert. The
/// happy path (no invites for this email) costs one indexed lookup that
/// returns zero rows and a no-op DELETE — sub-millisecond.
///
/// The INSERT … SELECT uses `ON CONFLICT DO NOTHING` so a user who's
/// somehow already a member of the matched traject (concurrent invite +
/// direct add, account email change races) just keeps their existing
/// row. The unconditional DELETE is intentional: after the INSERT runs,
/// every invite for this email is by definition obsolete — the account
/// exists and is now a member of each matching traject.
///
/// Failures are logged but not propagated to the caller: the user has
/// already been authenticated and their account upserted, so a transient
/// promotion failure just delays access to a freshly-invited traject
/// until the next request. Failing the entire request would be worse UX
/// than the lag.
pub async fn promote_pending_invites(pool: &PgPool, account_id: Uuid, email: &str) {
    let email_lower = email.trim().to_lowercase();
    if email_lower.is_empty() {
        return;
    }
    let result = sqlx::query(
        "WITH _ins AS (
             INSERT INTO traject_members (traject_id, account_id, role)
             SELECT i.traject_id, $1, i.role
               FROM traject_invites i
              WHERE i.email = $2
             ON CONFLICT (traject_id, account_id) DO NOTHING
         )
         DELETE FROM traject_invites WHERE email = $2",
    )
    .bind(account_id)
    .bind(&email_lower)
    .execute(pool)
    .await;
    if let Err(e) = result {
        tracing::error!(
            error = %e,
            account_id = %account_id,
            "promote_pending_invites failed; user will retry on next request"
        );
    }
}

/// Axum middleware that ensures the authenticated user has a row in
/// `accounts` and exposes it to downstream handlers via
/// `axum::Extension<AccountRecord>`.
///
/// Must be mounted **after** `require_session_auth` so that the session
/// claims are guaranteed present (or auth is disabled). When no account
/// can be resolved — either because the pool isn't configured (auth-off
/// dev mode) or because the session has no `sub` claim — the middleware
/// returns 503; downstream handlers all extract `Extension<AccountRecord>`
/// and would otherwise crash with a generic 500 ("missing request
/// extension"), which obscures the real reason ("feature is unavailable
/// without auth/database").
pub async fn account_middleware(
    State(state): State<AppState>,
    session: Session,
    mut request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let pool = state.pool.as_ref().ok_or_else(|| {
        tracing::warn!(
            "account_middleware: returning 503 — state.pool is None (auth disabled or DATABASE_URL missing)"
        );
        StatusCode::SERVICE_UNAVAILABLE
    })?;
    let account = ensure_account(pool, &session).await?.ok_or_else(|| {
        tracing::warn!(
            "account_middleware: returning 503 — ensure_account returned None (no SESSION_KEY_SUB in session, caller is not OIDC-authenticated)"
        );
        StatusCode::SERVICE_UNAVAILABLE
    })?;
    promote_pending_invites(pool, account.id, &account.email).await;
    request.extensions_mut().insert(account);
    Ok(next.run(request).await)
}
