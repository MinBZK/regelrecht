//! Traject CRUD endpoints.
//!
//! Trajects group law edits under a named project with its own federated
//! corpus config and a stable branch on the writable source. Edits made
//! while a traject is active are routed through the traject's corpus
//! ([`crate::traject_corpus`]) instead of the globally configured one.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::{Extension, Json};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use tower_sessions::Session;
use uuid::Uuid;

use crate::accounts::AccountRecord;
use crate::state::AppState;

/// Session key for the active traject id. The editor stores this in the
/// user's session so save handlers can resolve the traject without a
/// round-trip to the frontend.
pub const SESSION_KEY_ACTIVE_TRAJECT: &str = "active_traject_id";

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct TrajectSummary {
    pub id: Uuid,
    pub name: String,
    pub description: String,
    pub scope: String,
    pub status: String,
    pub role: String,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct TrajectMember {
    pub account_id: Uuid,
    pub email: String,
    pub name: String,
    pub role: String,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct TrajectSourceDto {
    pub source_id: String,
    pub name: String,
    pub source_type: String,
    pub gh_owner: Option<String>,
    pub gh_repo: Option<String>,
    pub gh_branch: Option<String>,
    pub gh_base_branch: Option<String>,
    pub gh_path: Option<String>,
    pub gh_ref: Option<String>,
    pub local_path: Option<String>,
    pub priority: i32,
    pub auth_ref: Option<String>,
    pub is_writable_own: bool,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct TrajectInvite {
    pub email: String,
    pub role: String,
}

#[derive(Debug, Serialize)]
pub struct TrajectDetail {
    #[serde(flatten)]
    pub summary: TrajectSummary,
    pub members: Vec<TrajectMember>,
    pub pending_invites: Vec<TrajectInvite>,
    pub sources: Vec<TrajectSourceDto>,
}

#[derive(Debug, Deserialize)]
pub struct CreateTrajectRequest {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub scope: String,
}

// Phase-1: trajects always push to a single, app-wide writable source —
// the central MinBZK corpus repo on its `development` branch, using the
// app-wide CORPUS_AUTH_MINBZK_CENTRAL_TOKEN. No per-user / per-traject
// auth yet; that's phase 2 (probably GitHub-App OAuth so we don't have
// to store PATs in the database at all). When phase 2 lands these
// constants become a fallback default for the request body.
const CENTRAL_WRITABLE_OWNER: &str = "MinBZK";
const CENTRAL_WRITABLE_REPO: &str = "regelrecht-corpus";
const CENTRAL_WRITABLE_PATH: &str = "regulation/nl";
const CENTRAL_WRITABLE_BASE_BRANCH: &str = "development";
const CENTRAL_WRITABLE_AUTH_REF: &str = "minbzk-central";
const CENTRAL_WRITABLE_NAME: &str = "MinBZK/regelrecht-corpus";

#[derive(Debug, Deserialize)]
pub struct UpdateTrajectRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub scope: Option<String>,
    /// Either `"bezig"` or `"afgerond"`.
    pub status: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SetActiveTrajectRequest {
    pub traject_id: Option<Uuid>,
}

#[derive(Debug, Serialize)]
pub struct ActiveTrajectResponse {
    pub traject_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct AddMemberRequest {
    /// Email of the user to invite. If an account already exists for
    /// this email the row lands in `traject_members` (active). If not,
    /// it goes into `traject_invites` (pending) and is promoted to a
    /// real membership the next time someone with that email claim hits
    /// any authenticated endpoint — see `accounts::ensure_account`.
    pub email: String,
    pub role: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateMemberRequest {
    pub role: String,
}

#[derive(Debug, Serialize)]
pub struct AddMemberResponse {
    /// `"active"` when a `traject_members` row was created (account
    /// existed), `"pending"` when only a `traject_invites` row was
    /// created (no account yet for this email).
    pub status: &'static str,
    /// Normalised (lowercased + trimmed) email used as the key.
    pub email: String,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn get_pool(state: &AppState) -> Result<&PgPool, StatusCode> {
    state.pool.as_ref().ok_or(StatusCode::SERVICE_UNAVAILABLE)
}

fn db_err<E: std::fmt::Display>(context: &'static str) -> impl FnOnce(E) -> StatusCode {
    move |e| {
        tracing::error!(error = %e, "{context}");
        StatusCode::INTERNAL_SERVER_ERROR
    }
}

/// Builds a session-error mapper that logs the underlying error with the
/// key being read/written so operators can diagnose tower-sessions /
/// session-store failures from logs instead of seeing a bare 500.
fn session_err(context: &'static str) -> impl FnOnce(tower_sessions::session::Error) -> StatusCode {
    move |e| {
        tracing::error!(error = %e, "{context}");
        StatusCode::INTERNAL_SERVER_ERROR
    }
}

/// Slugify a traject name for the branch suffix: lowercase, allowed chars
/// `[a-z0-9-]`, runs of disallowed chars collapsed to a single dash,
/// trimmed and capped at 32 characters.
fn slugify(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut prev_dash = true; // suppress leading dashes
    for ch in name.chars() {
        let mapped = if ch.is_ascii_alphanumeric() {
            Some(ch.to_ascii_lowercase())
        } else if ch.is_whitespace() || ch == '-' || ch == '_' {
            Some('-')
        } else {
            None
        };
        match mapped {
            Some('-') => {
                if !prev_dash {
                    out.push('-');
                    prev_dash = true;
                }
            }
            Some(c) => {
                out.push(c);
                prev_dash = false;
            }
            None => {}
        }
        if out.len() >= 32 {
            break;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        out.push_str("traject");
    }
    out
}

fn derive_branch_name(name: &str, traject_id: Uuid) -> String {
    let slug = slugify(name);
    let short = traject_id.simple().to_string()[..8].to_string();
    format!("traject/{slug}-{short}")
}

/// Look up the role of an account in a traject. Returns `None` when the
/// account is not a member.
async fn member_role(
    pool: &PgPool,
    traject_id: Uuid,
    account_id: Uuid,
) -> sqlx::Result<Option<String>> {
    let row: Option<(String,)> = sqlx::query_as(
        "SELECT role::text FROM traject_members
         WHERE traject_id = $1 AND account_id = $2",
    )
    .bind(traject_id)
    .bind(account_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|r| r.0))
}

async fn require_membership(
    pool: &PgPool,
    traject_id: Uuid,
    account_id: Uuid,
) -> Result<String, StatusCode> {
    member_role(pool, traject_id, account_id)
        .await
        .map_err(db_err("member lookup failed"))?
        .ok_or(StatusCode::FORBIDDEN)
}

async fn require_owner(
    pool: &PgPool,
    traject_id: Uuid,
    account_id: Uuid,
) -> Result<(), StatusCode> {
    let role = require_membership(pool, traject_id, account_id).await?;
    if role == "owner" {
        Ok(())
    } else {
        Err(StatusCode::FORBIDDEN)
    }
}

fn validate_role(role: &str) -> Result<(), StatusCode> {
    if role == "owner" || role == "contributor" {
        Ok(())
    } else {
        Err(StatusCode::BAD_REQUEST)
    }
}

/// Normalise an email for storage and comparison: trim whitespace and
/// lowercase. Empty results become `None` so callers can return 400
/// instead of inserting an empty key.
fn normalize_email(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_lowercase())
    }
}

fn validate_status(status: &str) -> Result<(), StatusCode> {
    if status == "bezig" || status == "afgerond" {
        Ok(())
    } else {
        Err(StatusCode::BAD_REQUEST)
    }
}

// ---------------------------------------------------------------------------
// Endpoints
// ---------------------------------------------------------------------------

/// GET /api/trajects — list trajects the caller is a member of.
pub async fn list(
    State(state): State<AppState>,
    Extension(account): Extension<AccountRecord>,
) -> Result<Json<Vec<TrajectSummary>>, StatusCode> {
    let pool = get_pool(&state)?;
    let rows: Vec<TrajectSummary> = sqlx::query_as(
        "SELECT t.id, t.name, t.description, t.scope,
                t.status::text AS status,
                tm.role::text  AS role
         FROM trajects t
         JOIN traject_members tm ON tm.traject_id = t.id
         WHERE tm.account_id = $1
         ORDER BY t.updated_at DESC",
    )
    .bind(account.id)
    .fetch_all(pool)
    .await
    .map_err(db_err("list trajects failed"))?;
    Ok(Json(rows))
}

/// GET /api/trajects/:id — details (members + sources).
pub async fn get(
    State(state): State<AppState>,
    Extension(account): Extension<AccountRecord>,
    Path(id): Path<Uuid>,
) -> Result<Json<TrajectDetail>, StatusCode> {
    let pool = get_pool(&state)?;
    let role = require_membership(pool, id, account.id).await?;

    let summary: TrajectSummary = sqlx::query_as(
        "SELECT id, name, description, scope,
                status::text AS status,
                $2           AS role
         FROM trajects WHERE id = $1",
    )
    .bind(id)
    .bind(&role)
    .fetch_one(pool)
    .await
    .map_err(db_err("traject summary fetch failed"))?;

    let members: Vec<TrajectMember> = sqlx::query_as(
        "SELECT a.id AS account_id, a.email, a.name, tm.role::text AS role
         FROM traject_members tm
         JOIN accounts a ON a.id = tm.account_id
         WHERE tm.traject_id = $1
         ORDER BY tm.added_at",
    )
    .bind(id)
    .fetch_all(pool)
    .await
    .map_err(db_err("traject members fetch failed"))?;

    let pending_invites: Vec<TrajectInvite> = sqlx::query_as(
        "SELECT email, role::text AS role
         FROM traject_invites
         WHERE traject_id = $1
         ORDER BY invited_at",
    )
    .bind(id)
    .fetch_all(pool)
    .await
    .map_err(db_err("traject invites fetch failed"))?;

    let sources: Vec<TrajectSourceDto> = sqlx::query_as(
        "SELECT source_id, name, source_type::text AS source_type,
                gh_owner, gh_repo, gh_branch, gh_base_branch, gh_path, gh_ref,
                local_path, priority, auth_ref, is_writable_own
         FROM traject_corpus_sources
         WHERE traject_id = $1
         ORDER BY priority",
    )
    .bind(id)
    .fetch_all(pool)
    .await
    .map_err(db_err("traject sources fetch failed"))?;

    Ok(Json(TrajectDetail {
        summary,
        members,
        pending_invites,
        sources,
    }))
}

/// POST /api/trajects — create a new traject.
///
/// Seeds the federated config by copying the global registry's sources
/// (with their original priorities) and then attaching the writable own
/// source at priority 0. Branch creation on the writable source is
/// handled by `GitBackend` on first use, which falls back to the
/// configured base branch when the traject branch doesn't yet exist.
pub async fn create(
    State(state): State<AppState>,
    Extension(account): Extension<AccountRecord>,
    Json(req): Json<CreateTrajectRequest>,
) -> Result<(StatusCode, Json<TrajectSummary>), StatusCode> {
    let name = req.name.trim();
    if name.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    let pool = get_pool(&state)?;
    let mut tx = pool.begin().await.map_err(db_err("begin tx"))?;

    let traject_id: Uuid = sqlx::query_scalar(
        "INSERT INTO trajects (name, description, scope, created_by)
         VALUES ($1, $2, $3, $4) RETURNING id",
    )
    .bind(name)
    .bind(&req.description)
    .bind(&req.scope)
    .bind(account.id)
    .fetch_one(&mut *tx)
    .await
    .map_err(db_err("insert traject"))?;

    sqlx::query(
        "INSERT INTO traject_members (traject_id, account_id, role)
         VALUES ($1, $2, 'owner')",
    )
    .bind(traject_id)
    .bind(account.id)
    .execute(&mut *tx)
    .await
    .map_err(db_err("insert member"))?;

    // Seed federated config from the global registry. The global corpus
    // read guard is dropped before the next await so we don't hold it
    // across the database transaction.
    let seeded: Vec<SeedSource> = {
        let corpus = state.corpus.read().await;
        corpus
            .registry
            .sources()
            .iter()
            .map(SeedSource::from_source)
            .collect()
    };

    for seed in seeded {
        sqlx::query(
            "INSERT INTO traject_corpus_sources
             (traject_id, source_id, name, source_type,
              gh_owner, gh_repo, gh_branch, gh_path, gh_ref,
              local_path, priority, auth_ref, scopes, is_writable_own)
             VALUES ($1, $2, $3, $4::corpus_source_type,
                     $5, $6, $7, $8, $9,
                     $10, $11, $12, $13, FALSE)",
        )
        .bind(traject_id)
        .bind(&seed.source_id)
        .bind(&seed.name)
        .bind(&seed.source_type)
        .bind(seed.gh_owner)
        .bind(seed.gh_repo)
        .bind(seed.gh_branch)
        .bind(seed.gh_path)
        .bind(seed.gh_ref)
        .bind(seed.local_path)
        .bind(seed.priority as i32)
        .bind(seed.auth_ref)
        .bind(seed.scopes)
        .execute(&mut *tx)
        .await
        .map_err(db_err("seed traject source"))?;
    }

    // Writable-own source: hardcoded to the central MinBZK corpus on
    // `development` for phase 1. The branch name is derived from the
    // traject name + id; auth flows through `CORPUS_AUTH_MINBZK_CENTRAL_TOKEN`
    // via `auth_ref = "minbzk-central"`. Columns stay populated as a
    // record — phase 2 (per-user auth, per-traject fork choice) can swap
    // these constants for request-body fields without touching the DB.
    let writable_branch = derive_branch_name(name, traject_id);
    let writable_source_id = format!("traject-own-{}", traject_id.simple());
    sqlx::query(
        "INSERT INTO traject_corpus_sources
         (traject_id, source_id, name, source_type,
          gh_owner, gh_repo, gh_branch, gh_base_branch, gh_path,
          priority, auth_ref, is_writable_own)
         VALUES ($1, $2, $3, 'github',
                 $4, $5, $6, $7, $8,
                 0, $9, TRUE)",
    )
    .bind(traject_id)
    .bind(&writable_source_id)
    .bind(CENTRAL_WRITABLE_NAME)
    .bind(CENTRAL_WRITABLE_OWNER)
    .bind(CENTRAL_WRITABLE_REPO)
    .bind(&writable_branch)
    .bind(CENTRAL_WRITABLE_BASE_BRANCH)
    .bind(CENTRAL_WRITABLE_PATH)
    .bind(CENTRAL_WRITABLE_AUTH_REF)
    .execute(&mut *tx)
    .await
    .map_err(db_err("insert writable source"))?;

    tx.commit().await.map_err(db_err("commit traject create"))?;

    state.trajects.invalidate(traject_id).await;

    let summary = TrajectSummary {
        id: traject_id,
        name: name.to_string(),
        description: req.description,
        scope: req.scope,
        status: "bezig".to_string(),
        role: "owner".to_string(),
    };
    Ok((StatusCode::CREATED, Json(summary)))
}

/// PATCH /api/trajects/:id — owner-only update of metadata fields.
pub async fn update(
    State(state): State<AppState>,
    Extension(account): Extension<AccountRecord>,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateTrajectRequest>,
) -> Result<StatusCode, StatusCode> {
    let pool = get_pool(&state)?;
    require_owner(pool, id, account.id).await?;
    if let Some(ref s) = req.status {
        validate_status(s)?;
    }
    // Mirror `create`'s non-empty check so a PATCH can't blank-out the
    // name with whitespace.
    if let Some(ref n) = req.name {
        if n.trim().is_empty() {
            return Err(StatusCode::BAD_REQUEST);
        }
    }

    sqlx::query(
        "UPDATE trajects SET
            name        = COALESCE($2, name),
            description = COALESCE($3, description),
            scope       = COALESCE($4, scope),
            status      = COALESCE($5::traject_status, status)
         WHERE id = $1",
    )
    .bind(id)
    .bind(req.name.as_deref().map(str::trim))
    .bind(req.description.as_deref())
    .bind(req.scope.as_deref())
    .bind(req.status.as_deref())
    .execute(pool)
    .await
    .map_err(db_err("update traject"))?;

    Ok(StatusCode::NO_CONTENT)
}

/// DELETE /api/trajects/:id — owner-only hard delete.
///
/// FK cascades on `traject_members` and `traject_corpus_sources` clean
/// up the dependent rows. The cached `TrajectCorpus` is invalidated so
/// any in-flight reads rebuild against an empty source set and surface
/// `NotFound`. The upstream branch on the writable source is **not**
/// touched — that's a manual cleanup decision and there's no way to
/// know whether the user still wants the in-flight edits preserved
/// elsewhere.
pub async fn delete(
    State(state): State<AppState>,
    Extension(account): Extension<AccountRecord>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, StatusCode> {
    let pool = get_pool(&state)?;
    require_owner(pool, id, account.id).await?;

    let affected = sqlx::query("DELETE FROM trajects WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await
        .map_err(db_err("delete traject"))?
        .rows_affected();

    if affected == 0 {
        return Err(StatusCode::NOT_FOUND);
    }

    state.trajects.invalidate(id).await;
    Ok(StatusCode::NO_CONTENT)
}

/// POST /api/trajects/:id/members — invite by email.
///
/// If the email already maps to an account, a `traject_members` row is
/// created (or updated, idempotently) and the response status is
/// `"active"`. If not, a `traject_invites` row is created and the
/// response status is `"pending"`; that row is promoted to a real
/// membership the next time someone with the matching email claim hits
/// any authenticated endpoint (see `accounts::ensure_account`).
pub async fn add_member(
    State(state): State<AppState>,
    Extension(account): Extension<AccountRecord>,
    Path(id): Path<Uuid>,
    Json(req): Json<AddMemberRequest>,
) -> Result<Json<AddMemberResponse>, StatusCode> {
    let pool = get_pool(&state)?;
    require_owner(pool, id, account.id).await?;
    validate_role(&req.role)?;
    let email = normalize_email(&req.email).ok_or(StatusCode::BAD_REQUEST)?;

    // `accounts.email` keeps the IdP-supplied casing, so the lookup
    // lowercases on the DB side to match our normalised key.
    let target: Option<(Uuid,)> = sqlx::query_as("SELECT id FROM accounts WHERE lower(email) = $1")
        .bind(&email)
        .fetch_optional(pool)
        .await
        .map_err(db_err("lookup account"))?;

    if let Some((target_id,)) = target {
        // Known account → write traject_members. Guard the "would demote
        // the last owner" case atomically in the same statement so it
        // can't race with a concurrent leave/remove.
        //
        // The WHERE evaluates to true when any of:
        //   * the new role is `owner` (promotion/no-op, never reduces count)
        //   * the target isn't currently an owner (no demotion happening)
        //   * another owner exists for this traject (demotion is safe)
        // When all three are false (target is the sole owner being
        // demoted to contributor) the SELECT returns no row, the INSERT
        // runs zero times, no conflict triggers, and `rows_affected` is
        // 0 → CONFLICT.
        let affected = sqlx::query(
            "INSERT INTO traject_members (traject_id, account_id, role)
             SELECT $1, $2, $3::traject_role
             WHERE
                 $3::traject_role = 'owner'
                 OR NOT EXISTS (
                     SELECT 1 FROM traject_members tm
                     WHERE tm.traject_id = $1
                       AND tm.account_id = $2
                       AND tm.role = 'owner'
                 )
                 OR EXISTS (
                     SELECT 1 FROM traject_members tm2
                     WHERE tm2.traject_id = $1
                       AND tm2.role = 'owner'
                       AND tm2.account_id <> $2
                 )
             ON CONFLICT (traject_id, account_id) DO UPDATE
                 SET role = EXCLUDED.role",
        )
        .bind(id)
        .bind(target_id)
        .bind(&req.role)
        .execute(pool)
        .await
        .map_err(db_err("add member"))?
        .rows_affected();

        if affected == 0 {
            return Err(StatusCode::CONFLICT);
        }
        return Ok(Json(AddMemberResponse {
            status: "active",
            email,
        }));
    }

    // Unknown account → park the invite. Re-inviting the same email
    // with a different role just updates the role (last write wins).
    sqlx::query(
        "INSERT INTO traject_invites (traject_id, email, role, invited_by)
         VALUES ($1, $2, $3::traject_role, $4)
         ON CONFLICT (traject_id, email) DO UPDATE
             SET role = EXCLUDED.role,
                 invited_by = EXCLUDED.invited_by,
                 invited_at = now()",
    )
    .bind(id)
    .bind(&email)
    .bind(&req.role)
    .bind(account.id)
    .execute(pool)
    .await
    .map_err(db_err("add invite"))?;

    Ok(Json(AddMemberResponse {
        status: "pending",
        email,
    }))
}

/// DELETE /api/trajects/:id/invites/:email — owner-only removal of a
/// pending invite. Returns 404 when no invite exists for the (traject,
/// email) pair, so the operation is idempotent against repeated
/// cancellations.
pub async fn remove_invite(
    State(state): State<AppState>,
    Extension(account): Extension<AccountRecord>,
    Path((id, email)): Path<(Uuid, String)>,
) -> Result<StatusCode, StatusCode> {
    let pool = get_pool(&state)?;
    require_owner(pool, id, account.id).await?;
    let email = normalize_email(&email).ok_or(StatusCode::BAD_REQUEST)?;

    let affected = sqlx::query("DELETE FROM traject_invites WHERE traject_id = $1 AND email = $2")
        .bind(id)
        .bind(&email)
        .execute(pool)
        .await
        .map_err(db_err("remove invite"))?
        .rows_affected();

    if affected == 0 {
        return Err(StatusCode::NOT_FOUND);
    }
    Ok(StatusCode::NO_CONTENT)
}

/// PATCH /api/trajects/:id/members/:account_id — change a member's role.
pub async fn update_member(
    State(state): State<AppState>,
    Extension(account): Extension<AccountRecord>,
    Path((id, account_id)): Path<(Uuid, Uuid)>,
    Json(req): Json<UpdateMemberRequest>,
) -> Result<StatusCode, StatusCode> {
    let pool = get_pool(&state)?;
    require_owner(pool, id, account.id).await?;
    validate_role(&req.role)?;

    // Atomic guard against demoting the last owner. The UPDATE only
    // fires when at least one of these holds:
    //   * the new role is `owner` (no demote)
    //   * the row's current role isn't `owner` (no demote)
    //   * another owner exists for this traject (demote is safe)
    // Otherwise the row stays untouched, `rows_affected` is 0, and we
    // disambiguate "row missing" (NOT_FOUND) from "guard blocked"
    // (CONFLICT) via a follow-up read on the cold path.
    let affected = sqlx::query(
        "UPDATE traject_members SET role = $3::traject_role
         WHERE traject_id = $1 AND account_id = $2
           AND (
               $3::traject_role = 'owner'
               OR role <> 'owner'
               OR EXISTS (
                   SELECT 1 FROM traject_members tm2
                   WHERE tm2.traject_id = $1
                     AND tm2.role = 'owner'
                     AND tm2.account_id <> $2
               )
           )",
    )
    .bind(id)
    .bind(account_id)
    .bind(&req.role)
    .execute(pool)
    .await
    .map_err(db_err("update member"))?
    .rows_affected();

    if affected == 0 {
        return distinguish_member_missing_or_conflict(pool, id, account_id).await;
    }
    Ok(StatusCode::NO_CONTENT)
}

/// DELETE /api/trajects/:id/members/:account_id — remove a member.
pub async fn remove_member(
    State(state): State<AppState>,
    Extension(account): Extension<AccountRecord>,
    Path((id, account_id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, StatusCode> {
    let pool = get_pool(&state)?;
    require_owner(pool, id, account.id).await?;

    // Atomic guard: DELETE only succeeds when the target isn't the sole
    // owner. The EXISTS condition is part of the same statement so
    // two concurrent removes can't both pass the check and then both
    // commit a delete.
    let affected = sqlx::query(
        "DELETE FROM traject_members
         WHERE traject_id = $1 AND account_id = $2
           AND (
               role <> 'owner'
               OR EXISTS (
                   SELECT 1 FROM traject_members tm2
                   WHERE tm2.traject_id = $1
                     AND tm2.role = 'owner'
                     AND tm2.account_id <> $2
               )
           )",
    )
    .bind(id)
    .bind(account_id)
    .execute(pool)
    .await
    .map_err(db_err("remove member"))?
    .rows_affected();

    if affected == 0 {
        return distinguish_member_missing_or_conflict(pool, id, account_id).await;
    }
    Ok(StatusCode::NO_CONTENT)
}

/// POST /api/trajects/:id/leave — caller removes themselves from the
/// traject.
///
/// A `contributor` can always leave. An `owner` cannot leave when they are
/// the last owner — they must hand over the role or delete the
/// traject. When the caller leaves the traject they currently have
/// active in their session, that session pointer is cleared so the
/// next save handler doesn't try to resolve a traject they no longer
/// belong to.
pub async fn leave(
    State(state): State<AppState>,
    Extension(account): Extension<AccountRecord>,
    session: Session,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, StatusCode> {
    let pool = get_pool(&state)?;
    require_membership(pool, id, account.id).await?;

    // Atomic guard, same shape as remove_member: the DELETE only runs
    // when the caller is a contributor or another owner remains. Two
    // owners calling `leave` concurrently can't both pass a separate
    // count then both delete — the second one's DELETE re-evaluates the
    // EXISTS under Postgres' READ COMMITTED snapshot semantics and ends
    // up matching zero rows.
    let affected = sqlx::query(
        "DELETE FROM traject_members
         WHERE traject_id = $1 AND account_id = $2
           AND (
               role <> 'owner'
               OR EXISTS (
                   SELECT 1 FROM traject_members tm2
                   WHERE tm2.traject_id = $1
                     AND tm2.role = 'owner'
                     AND tm2.account_id <> $2
               )
           )",
    )
    .bind(id)
    .bind(account.id)
    .execute(pool)
    .await
    .map_err(db_err("leave traject"))?
    .rows_affected();

    if affected == 0 {
        // require_membership passed above, so the most likely cause is
        // the guard. The narrow window where the row was concurrently
        // removed between the membership check and the DELETE would
        // also yield 0 — distinguish with one more read.
        return distinguish_member_missing_or_conflict(pool, id, account.id).await;
    }

    let active: Option<Uuid> = session
        .get(SESSION_KEY_ACTIVE_TRAJECT)
        .await
        .map_err(session_err("session read active-traject in leave"))?;
    if active == Some(id) {
        let _: Option<Uuid> = session
            .remove(SESSION_KEY_ACTIVE_TRAJECT)
            .await
            .map_err(session_err("session clear active-traject in leave"))?;
    }

    Ok(StatusCode::NO_CONTENT)
}

/// Disambiguate "row not found" (404) from "guard blocked the write"
/// (409) after a DELETE/UPDATE on `traject_members` came back with zero
/// rows affected. The atomic guards embedded in the write statements
/// can't tell these apart by themselves; this lookup runs on the cold
/// path only.
async fn distinguish_member_missing_or_conflict(
    pool: &PgPool,
    traject_id: Uuid,
    account_id: Uuid,
) -> Result<StatusCode, StatusCode> {
    if member_role(pool, traject_id, account_id)
        .await
        .map_err(db_err("post-write membership lookup"))?
        .is_none()
    {
        Err(StatusCode::NOT_FOUND)
    } else {
        Err(StatusCode::CONFLICT)
    }
}

/// GET /api/session/active-traject — return the current active traject id
/// from the session (or `null` when none).
pub async fn get_active(session: Session) -> Result<Json<ActiveTrajectResponse>, StatusCode> {
    let traject_id: Option<Uuid> = session
        .get(SESSION_KEY_ACTIVE_TRAJECT)
        .await
        .map_err(session_err("session read active-traject"))?;
    Ok(Json(ActiveTrajectResponse { traject_id }))
}

/// PUT /api/session/active-traject — set or clear the active traject for
/// this session. Membership is verified against `traject_members` before
/// persisting so a user cannot point themselves at a traject they don't
/// belong to.
pub async fn set_active(
    State(state): State<AppState>,
    Extension(account): Extension<AccountRecord>,
    session: Session,
    Json(req): Json<SetActiveTrajectRequest>,
) -> Result<Json<ActiveTrajectResponse>, StatusCode> {
    let pool = get_pool(&state)?;
    match req.traject_id {
        Some(id) => {
            let _role = require_membership(pool, id, account.id).await?;
            session
                .insert(SESSION_KEY_ACTIVE_TRAJECT, id)
                .await
                .map_err(session_err("session set active-traject"))?;
            Ok(Json(ActiveTrajectResponse {
                traject_id: Some(id),
            }))
        }
        None => {
            let _: Option<Uuid> = session
                .remove(SESSION_KEY_ACTIVE_TRAJECT)
                .await
                .map_err(session_err("session clear active-traject"))?;
            Ok(Json(ActiveTrajectResponse { traject_id: None }))
        }
    }
}

/// Flattened snapshot of a [`regelrecht_corpus::models::Source`] used to
/// seed `traject_corpus_sources` rows without holding the corpus read
/// guard across the database transaction.
struct SeedSource {
    source_id: String,
    name: String,
    source_type: String,
    gh_owner: Option<String>,
    gh_repo: Option<String>,
    gh_branch: Option<String>,
    gh_path: Option<String>,
    gh_ref: Option<String>,
    local_path: Option<String>,
    priority: u32,
    auth_ref: Option<String>,
    scopes: serde_json::Value,
}

impl SeedSource {
    fn from_source(s: &regelrecht_corpus::models::Source) -> Self {
        let (source_type, gh_owner, gh_repo, gh_branch, gh_path, gh_ref, local_path) =
            match &s.source_type {
                regelrecht_corpus::models::SourceType::GitHub { github } => (
                    "github".to_string(),
                    Some(github.owner.clone()),
                    Some(github.repo.clone()),
                    Some(github.branch.clone()),
                    github.path.clone(),
                    github.git_ref.clone(),
                    None,
                ),
                regelrecht_corpus::models::SourceType::Local { local } => (
                    "local".to_string(),
                    None,
                    None,
                    None,
                    None,
                    None,
                    Some(local.path.to_string_lossy().to_string()),
                ),
            };
        let scopes = serde_json::to_value(&s.scopes).unwrap_or(serde_json::json!([]));
        SeedSource {
            source_id: s.id.clone(),
            name: s.name.clone(),
            source_type,
            gh_owner,
            gh_repo,
            gh_branch,
            gh_path,
            gh_ref,
            local_path,
            priority: s.priority,
            auth_ref: s.auth_ref.clone(),
            scopes,
        }
    }
}

/// Read the active traject id from a session — used by save handlers in
/// `corpus_handlers.rs`. Returns `Ok(None)` when no traject is selected.
pub async fn read_active_from_session(session: &Session) -> Result<Option<Uuid>, StatusCode> {
    session
        .get::<Uuid>(SESSION_KEY_ACTIVE_TRAJECT)
        .await
        .map_err(session_err("session read active-traject in save"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugify_strips_specials() {
        assert_eq!(slugify("Hello World!"), "hello-world");
        assert_eq!(slugify("---hello---"), "hello");
        assert_eq!(slugify("Wet op de Zorgtoeslag"), "wet-op-de-zorgtoeslag");
    }

    #[test]
    fn slugify_caps_length() {
        let s = slugify(&"a".repeat(100));
        assert!(s.len() <= 32);
    }

    #[test]
    fn slugify_empty_falls_back() {
        assert_eq!(slugify(""), "traject");
        assert_eq!(slugify("---"), "traject");
    }

    #[test]
    fn branch_name_format() {
        let id = Uuid::nil();
        let branch = derive_branch_name("Tarief", id);
        assert!(branch.starts_with("traject/tarief-"));
        assert_eq!(branch.len(), "traject/tarief-".len() + 8);
    }
}
