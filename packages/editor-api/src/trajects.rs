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

#[derive(Debug, Serialize)]
pub struct TrajectDetail {
    #[serde(flatten)]
    pub summary: TrajectSummary,
    pub members: Vec<TrajectMember>,
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
    /// Email of an existing account to invite. Returns 404 when no
    /// matching account exists — we deliberately do not create stub
    /// accounts on invite so OIDC stays the only path that produces them.
    pub email: String,
    pub role: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateMemberRequest {
    pub role: String,
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

async fn require_beheerder(
    pool: &PgPool,
    traject_id: Uuid,
    account_id: Uuid,
) -> Result<(), StatusCode> {
    let role = require_membership(pool, traject_id, account_id).await?;
    if role == "beheerder" {
        Ok(())
    } else {
        Err(StatusCode::FORBIDDEN)
    }
}

fn validate_role(role: &str) -> Result<(), StatusCode> {
    if role == "beheerder" || role == "lid" {
        Ok(())
    } else {
        Err(StatusCode::BAD_REQUEST)
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
         VALUES ($1, $2, 'beheerder')",
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
        role: "beheerder".to_string(),
    };
    Ok((StatusCode::CREATED, Json(summary)))
}

/// PATCH /api/trajects/:id — beheerder-only update of metadata fields.
pub async fn update(
    State(state): State<AppState>,
    Extension(account): Extension<AccountRecord>,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateTrajectRequest>,
) -> Result<StatusCode, StatusCode> {
    let pool = get_pool(&state)?;
    require_beheerder(pool, id, account.id).await?;
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

/// DELETE /api/trajects/:id — beheerder-only hard delete.
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
    require_beheerder(pool, id, account.id).await?;

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

/// POST /api/trajects/:id/members — invite an existing account by email.
pub async fn add_member(
    State(state): State<AppState>,
    Extension(account): Extension<AccountRecord>,
    Path(id): Path<Uuid>,
    Json(req): Json<AddMemberRequest>,
) -> Result<StatusCode, StatusCode> {
    let pool = get_pool(&state)?;
    require_beheerder(pool, id, account.id).await?;
    validate_role(&req.role)?;

    let target: Option<(Uuid,)> = sqlx::query_as("SELECT id FROM accounts WHERE email = $1")
        .bind(&req.email)
        .fetch_optional(pool)
        .await
        .map_err(db_err("lookup account"))?;
    let target_id = target.ok_or(StatusCode::NOT_FOUND)?.0;

    // If this upsert would convert an existing beheerder to lid, make sure
    // at least one other beheerder remains — otherwise add_member would be
    // a back-door around the same guard that update_member and remove_member
    // already enforce.
    if req.role != "beheerder" {
        ensure_other_beheerder(pool, id, target_id).await?;
    }

    sqlx::query(
        "INSERT INTO traject_members (traject_id, account_id, role)
         VALUES ($1, $2, $3::traject_role)
         ON CONFLICT (traject_id, account_id) DO UPDATE SET role = EXCLUDED.role",
    )
    .bind(id)
    .bind(target_id)
    .bind(&req.role)
    .execute(pool)
    .await
    .map_err(db_err("add member"))?;

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
    require_beheerder(pool, id, account.id).await?;
    validate_role(&req.role)?;

    if req.role != "beheerder" {
        ensure_other_beheerder(pool, id, account_id).await?;
    }

    let affected = sqlx::query(
        "UPDATE traject_members SET role = $3::traject_role
         WHERE traject_id = $1 AND account_id = $2",
    )
    .bind(id)
    .bind(account_id)
    .bind(&req.role)
    .execute(pool)
    .await
    .map_err(db_err("update member"))?
    .rows_affected();

    if affected == 0 {
        Err(StatusCode::NOT_FOUND)
    } else {
        Ok(StatusCode::NO_CONTENT)
    }
}

/// DELETE /api/trajects/:id/members/:account_id — remove a member.
pub async fn remove_member(
    State(state): State<AppState>,
    Extension(account): Extension<AccountRecord>,
    Path((id, account_id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, StatusCode> {
    let pool = get_pool(&state)?;
    require_beheerder(pool, id, account.id).await?;
    ensure_other_beheerder(pool, id, account_id).await?;

    let affected =
        sqlx::query("DELETE FROM traject_members WHERE traject_id = $1 AND account_id = $2")
            .bind(id)
            .bind(account_id)
            .execute(pool)
            .await
            .map_err(db_err("remove member"))?
            .rows_affected();

    if affected == 0 {
        Err(StatusCode::NOT_FOUND)
    } else {
        Ok(StatusCode::NO_CONTENT)
    }
}

/// POST /api/trajects/:id/leave — caller removes themselves from the
/// traject.
///
/// A `lid` can always leave. A `beheerder` cannot leave when they are
/// the last beheerder — they must hand over the role or delete the
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
    ensure_other_beheerder(pool, id, account.id).await?;

    let affected =
        sqlx::query("DELETE FROM traject_members WHERE traject_id = $1 AND account_id = $2")
            .bind(id)
            .bind(account.id)
            .execute(pool)
            .await
            .map_err(db_err("leave traject"))?
            .rows_affected();

    if affected == 0 {
        return Err(StatusCode::NOT_FOUND);
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

/// Guard that prevents removing the last beheerder of a traject.
async fn ensure_other_beheerder(
    pool: &PgPool,
    traject_id: Uuid,
    candidate: Uuid,
) -> Result<(), StatusCode> {
    let row: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM traject_members
         WHERE traject_id = $1 AND role = 'beheerder' AND account_id <> $2",
    )
    .bind(traject_id)
    .bind(candidate)
    .fetch_one(pool)
    .await
    .map_err(db_err("count beheerders"))?;
    if row.0 == 0 {
        Err(StatusCode::CONFLICT)
    } else {
        Ok(())
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
