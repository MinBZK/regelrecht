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
use uuid::Uuid;

use crate::accounts::AccountRecord;
use crate::state::AppState;

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct TrajectSummary {
    pub id: Uuid,
    pub name: String,
    pub description: String,
    pub scope: String,
    pub status: String,
    pub role: String,
    /// URL-form reference: `{slug}-{8hex}`. Built from current `name`
    /// and `id`; the slug part is cosmetic, the trailing 8 hex chars of
    /// the uuid are the actual lookup key (see `resolve_traject_ref`).
    /// Populated post-fetch — sqlx::FromRow doesn't see it in the SELECT.
    ///
    /// `Option<String>` (not `String` with a default of `""`) so a future
    /// code path that fetches a `TrajectSummary` and forgets to call
    /// `fill_ref()` serializes to `"ref": null` instead of `"ref": ""`.
    /// The frontend's `t.ref === activeTrajectRef.value` comparison then
    /// fails loudly (never matches) instead of silently equating two
    /// empty strings against a missing trajectRef.
    #[serde(rename = "ref")]
    #[sqlx(default)]
    pub traject_ref: Option<String>,
}

impl TrajectSummary {
    /// Recompute `traject_ref` from the current `name` and `id`. Called
    /// right after a sqlx fetch and after any in-memory mutation that
    /// might change the slug.
    pub fn fill_ref(&mut self) {
        self.traject_ref = Some(traject_ref(&self.name, self.id));
    }
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
    /// GitHub `owner` for the writable-own source. When `None`, falls
    /// back to the central MinBZK repo (phase-1 default). When set,
    /// `repo_name` and `base_branch` must also be set; the server
    /// derives `auth_ref` deterministically and looks up the matching
    /// `CORPUS_AUTH_{AUTH_REF}_TOKEN` env var.
    #[serde(default)]
    pub repo_owner: Option<String>,
    /// GitHub `repo`. Required when `repo_owner` is set.
    #[serde(default)]
    pub repo_name: Option<String>,
    /// Branch to base the traject branch off of (and target with the
    /// session PR). Required when `repo_owner` is set; the default
    /// branch of the repo if you're unsure.
    #[serde(default)]
    pub base_branch: Option<String>,
    /// Optional sub-path within the repo where regulation YAML files
    /// live. Empty / omitted means "everything under repo root" — the
    /// right default for user repos dedicated to regulations. Set this
    /// when the YAML files sit in a subdirectory like `regulation/nl`
    /// (which is the MinBZK default).
    #[serde(default)]
    pub repo_path: Option<String>,
}

/// GitHub `owner` / `repo` segments end up in a URL path
/// (`/repos/{owner}/{repo}`), in the on-disk session-branch name, and
/// in the commit/PR body. Accept only the character set that GitHub
/// itself allows for these identifiers — alphanumerics plus `-`, `_`,
/// and `.`. Lets us refuse path-traversal-shaped input (`..`, `/`) at
/// the API boundary rather than relying on GitHub to 404 it.
///
/// GitHub additionally disallows the literal segments `.` and `..` and
/// names starting with a leading `.` (hidden/reserved). Reject those
/// here so we don't store a row that GitHub will subsequently 404 on
/// every contents/branches call.
fn valid_repo_segment(s: &str) -> bool {
    if s.is_empty() || s.len() > 100 {
        return false;
    }
    if s == "." || s == ".." || s.starts_with('.') {
        return false;
    }
    s.chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
}

/// Reject branch names that git itself would refuse as refnames.
///
/// Mirrors the subset of `git-check-ref-format` rules that we can apply
/// to a single component: no control chars, no whitespace, no
/// `~^:?*[\` or `..`, no leading dash, no leading/trailing slash, no
/// `@{` sequence. Bounded length so an oversized input can't bloat the
/// on-disk branch name.
///
/// `base_branch` flows unencoded into a GitHub Contents-API URL
/// (`/repos/{owner}/{repo}/branches/{base_branch}`) and is later
/// persisted + used as a git refname in `commit_and_push_to_branch`.
/// A non-empty trim check would let `"main?spoof=1"` through — GitHub
/// strips the query, validates `main`, and the bogus suffix lands in
/// the DB. This guard refuses any refname-illegal character at the API
/// boundary so the persisted value is always a clean refname.
fn valid_branch_name(s: &str) -> bool {
    if s.is_empty() || s.len() > 200 {
        return false;
    }
    // `git-check-ref-format` rejects the bare `@` as a refname.
    if s == "@" {
        return false;
    }
    // Leading `-`/`/`, trailing `/`, and a trailing `.` are all rejected
    // by `git-check-ref-format` (e.g. `main.` is refused).
    if s.starts_with('-') || s.starts_with('/') || s.ends_with('/') || s.ends_with('.') {
        return false;
    }
    if s.contains("..") || s.contains("@{") || s.contains("//") {
        return false;
    }
    // Per `git-check-ref-format`: no path component may start with `.`
    // (so `.hidden` and `feature/.hidden` are both refused) and none may
    // end with `.lock` (git treats `<ref>.lock` as a lockfile sentinel,
    // so `main.lock` and `feature/foo.lock` are refused). Walk the
    // components once for both rules.
    if s.split('/')
        .any(|c| c.starts_with('.') || c.ends_with(".lock"))
    {
        return false;
    }
    s.chars().all(|c| {
        !c.is_control()
            && c != ' '
            && c != '~'
            && c != '^'
            && c != ':'
            && c != '?'
            && c != '*'
            && c != '['
            && c != '\\'
            // `#` is a legal refname char in git but is the URL fragment
            // delimiter. `format!(".../branches/{base_branch}")` followed
            // by reqwest parsing would silently drop everything after
            // `#`, so a branch name `main#spoof` would 200-pass the
            // pre-flight (it matches `main`) but later git operations
            // reference the non-existent `main#spoof`. Reject up front.
            && c != '#'
    })
}

/// Validate a user-supplied `repo_path` (sub-directory within a repo).
///
/// Empty is allowed and means "repo root". Otherwise the path must be
/// relative (no leading `/`, no Windows prefix) and contain only
/// "normal" components — no `..`, no `.`, no root. Each segment is
/// further constrained to the same character set as `valid_repo_segment`
/// so it can safely flow into a GitHub Contents-API URL without
/// double-encoding or escaping surprises.
///
/// `validate_relative_path` inside `GitBackend::resolve` covers the
/// inner per-file relative path, but the *base* `repo_subpath` (stored
/// on the row as `gh_path`) never reaches that check. Without this
/// guard a caller could ship `repo_path = "../../etc"` and the backend
/// would read/write outside the per-traject clone.
fn valid_repo_path(s: &str) -> bool {
    if s.is_empty() {
        return true;
    }
    let p = std::path::Path::new(s);
    if p.is_absolute() {
        return false;
    }
    for c in p.components() {
        match c {
            std::path::Component::Normal(seg) => {
                let seg_str = seg.to_string_lossy();
                if !valid_repo_segment(&seg_str) {
                    return false;
                }
            }
            // ParentDir (`..`), CurDir (`.`), RootDir, Prefix — all
            // rejected so the stored gh_path is always a clean,
            // forward-traversal-only relative path.
            _ => return false,
        }
    }
    true
}

// Phase-1 default writable source — the central MinBZK corpus repo on
// its `development` branch, using the app-wide
// `CORPUS_AUTH_MINBZK_CENTRAL_TOKEN`. Used when the create-request omits
// the repo_* fields entirely, so existing flows keep working unchanged.
// When the request *does* supply repo coordinates, all four user-facing
// fields (owner/repo/branch/path) are taken from the request and
// `auth_ref` is derived from owner+repo (see `derive_auth_ref`).
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

/// Same as [`get_pool`] but for handlers that return `(StatusCode, String)`
/// so the body can carry a user-facing message. Used by `create` where
/// failure reasons (missing token, missing branch, …) need to reach the UI.
fn get_pool_msg(state: &AppState) -> Result<&PgPool, (StatusCode, String)> {
    state.pool.as_ref().ok_or((
        StatusCode::SERVICE_UNAVAILABLE,
        "database not configured".to_string(),
    ))
}

/// Same as [`db_err`] but returning `(StatusCode, String)`. Empty body —
/// 500s caused by a DB issue carry no information the caller can act on,
/// so we just log on the server and let the client see a bare 500.
fn db_err_msg<E: std::fmt::Display>(
    context: &'static str,
) -> impl FnOnce(E) -> (StatusCode, String) {
    move |e| {
        tracing::error!(error = %e, "{context}");
        (StatusCode::INTERNAL_SERVER_ERROR, String::new())
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

/// First 8 hex characters of a traject UUID — the suffix used in the
/// URL ref (`{slug}-{short}`). Same length as the branch-name short id so
/// users see one identifier across URL and branch.
///
/// `simple()` emits the 32-char hyphen-less form (`3f4a8b2c…`), and the
/// canonical hyphenated form (`3f4a8b2c-…`) used by Postgres `id::text`
/// places its first hyphen at position 8 — so `left(id::text, 8)` in
/// SQL and `traject_id.simple()[..8]` in Rust produce the SAME first-8
/// hex chars. `resolve_traject_ref`'s DB lookup
/// (`WHERE left(id::text, 8) = $1`) relies on this alignment.
pub fn short_id(traject_id: Uuid) -> String {
    traject_id.simple().to_string()[..8].to_string()
}

/// Build the URL-form ref for a traject from its current name and id.
/// `{slug}-{8hex}`. The slug part is cosmetic — the resolver only cares
/// about the trailing 8-hex chunk, so renaming a traject does not break
/// existing URLs.
pub fn traject_ref(name: &str, traject_id: Uuid) -> String {
    format!("{}-{}", slugify(name), short_id(traject_id))
}

/// Derive the `auth_ref` for a user-supplied `<owner>/<repo>` so the
/// existing token-resolver picks up the right
/// `CORPUS_AUTH_{AUTH_REF_UPPER}_TOKEN` env var without anyone having
/// to hand-maintain the mapping. Operators see a deterministic env-var
/// name derived from the repo coordinates.
///
/// Rule: lowercase, runs of non-alphanumeric characters collapse to a
/// single `-`, leading/trailing dashes trimmed. This is a slug, not a
/// hash — it's lossy and not injective (e.g. `("a-", "-b")` and
/// `("a", "b")` both produce `"a-b"`), so two pathological repo
/// names could share an env var. Acceptable trade-off for legibility;
/// the env-var collision just means both repos use the same token.
///
/// Examples:
///   ("Acme",   "Secret-Repo")        → "acme-secret-repo"
///   ("MinBZK", "regelrecht-corpus")  → "minbzk-regelrecht-corpus"
///   ("a.b",    "c_d")                → "a-b-c-d"
pub fn derive_auth_ref(owner: &str, repo: &str) -> String {
    let mut out = String::new();
    let mut prev_dash = true; // suppress leading dashes
    let combined = format!("{owner}/{repo}");
    for ch in combined.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            prev_dash = false;
        } else if !prev_dash {
            out.push('-');
            prev_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    out
}

/// Resolve a `{slug}-{8hex}` URL ref to a traject UUID. Returns 400 on
/// a malformed ref, 404 when no traject has a uuid starting with the
/// suffix.
///
/// The UUID prefix is uniformly distributed across 32 bits — for N=1k
/// trajects the birthday-bound collision probability is
/// N·(N−1)/(2·2³²) ≈ 1.16×10⁻⁴, which we accept for the readability
/// gain. Ambiguous prefixes surface as 409 Conflict
/// (the URL is genuinely ambiguous; we refuse to guess and the caller
/// must rebuild the ref against a fresh traject). A tracing error on
/// the duplicate branch catches the case in production before it bites
/// a user.
pub async fn resolve_traject_ref(
    pool: &PgPool,
    traject_ref: &str,
) -> Result<Uuid, (StatusCode, String)> {
    // 8-hex suffix preceded by a dash. Anything else (bare UUID, raw
    // slug without suffix, garbage) is a 400 — we don't try to fall
    // back to a bare-uuid lookup because that path no longer exists in
    // the URL contract.
    //
    // Reject non-ASCII up front. Valid refs are slug + 8 hex chars,
    // both ASCII by construction; without this guard a crafted
    // multi-byte sequence like `abcé1234567` passes the length check
    // and then panics on the byte-index slicing below (a multi-byte
    // char straddling `suffix_start` is a char-boundary mid-slice).
    if !traject_ref.is_ascii() {
        return Err((
            StatusCode::BAD_REQUEST,
            "Malformed traject reference".to_string(),
        ));
    }
    let suffix_start = traject_ref.len().checked_sub(8).ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            "Malformed traject reference".to_string(),
        )
    })?;
    // Reject empty- and missing-slug refs in one go:
    //   suffix_start == 0  → ref is exactly 8 hex chars (no dash, no slug)
    //   suffix_start == 1  → ref is `-{8hex}` (the slug part is empty)
    // The SPA router regex requires at least one alphanumeric char
    // before the dash, so the frontend can never produce these; a
    // direct HTTP request still hits the DB lookup if we don't gate
    // here. Aligns both layers on the same minimum shape.
    if suffix_start <= 1 {
        return Err((
            StatusCode::BAD_REQUEST,
            "Malformed traject reference".to_string(),
        ));
    }
    let separator = &traject_ref[suffix_start - 1..suffix_start];
    let suffix = &traject_ref[suffix_start..];
    if separator != "-" || !suffix.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err((
            StatusCode::BAD_REQUEST,
            "Malformed traject reference".to_string(),
        ));
    }
    let suffix_lower = suffix.to_ascii_lowercase();

    // UUID text format starts with the first 8 hex chars (`xxxxxxxx-...`).
    // Equality on `left(id::text, 8)` matches our short id exactly and
    // uses the functional index from migration 0017
    // (`trajects_short_id_idx`) — every traject-scoped request runs this
    // lookup, so the index avoids a seq scan on every save once the
    // table grows.
    let rows: Vec<(Uuid,)> = sqlx::query_as("SELECT id FROM trajects WHERE left(id::text, 8) = $1")
        .bind(&suffix_lower)
        .fetch_all(pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "resolve traject ref query failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to resolve traject reference".to_string(),
            )
        })?;
    match rows.len() {
        1 => Ok(rows[0].0),
        0 => Err((StatusCode::NOT_FOUND, "Traject not found".to_string())),
        _ => {
            // Two trajects whose UUIDs share the same first 8 hex
            // chars — astronomical odds, but if it happens the URL is
            // ambiguous. Log loudly and refuse rather than guess.
            tracing::error!(
                suffix = %suffix_lower,
                count = rows.len(),
                "traject ref short id collides; refusing to guess"
            );
            Err((
                StatusCode::CONFLICT,
                "Traject reference is ambiguous (id-suffix collision); contact support".to_string(),
            ))
        }
    }
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

/// Normalise an email for storage and comparison: trim whitespace,
/// lowercase, and reject obvious non-emails so junk addresses can't
/// accumulate as pending invites that will never promote.
///
/// We intentionally avoid pulling in a full RFC 5322 parser: invite
/// creation is owner-only behind OIDC, so this is correctness/data
/// hygiene rather than a security boundary. The structural check is:
/// exactly one `@`, non-empty local part, non-empty domain part, and a
/// `.` in the domain. The IdP is the source of truth for whether an
/// address is actually deliverable.
fn normalize_email(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let at_count = trimmed.bytes().filter(|b| *b == b'@').count();
    if at_count != 1 {
        return None;
    }
    let (local, domain) = trimmed.split_once('@')?;
    if local.is_empty() || domain.is_empty() || !domain.contains('.') {
        return None;
    }
    Some(trimmed.to_lowercase())
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
    let mut rows: Vec<TrajectSummary> = sqlx::query_as(
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
    for row in &mut rows {
        row.fill_ref();
    }
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

    let mut summary: TrajectSummary = sqlx::query_as(
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
    summary.fill_ref();

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

    // Pending invites carry the email addresses of people the owner has
    // invited but who haven't yet logged in. Only owners — who can act
    // on invites (cancel, re-invite, change role) — get to see them;
    // contributors get an empty list. This keeps invitee emails out of
    // a non-owner's view by default.
    let pending_invites: Vec<TrajectInvite> = if role == "owner" {
        sqlx::query_as(
            "SELECT email, role::text AS role
             FROM traject_invites
             WHERE traject_id = $1
             ORDER BY invited_at",
        )
        .bind(id)
        .fetch_all(pool)
        .await
        .map_err(db_err("traject invites fetch failed"))?
    } else {
        Vec::new()
    };

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

/// Resolved writable-own source coordinates for the create handler.
/// Either the phase-1 MinBZK defaults or the user-supplied repo after
/// validation. The handler picks once up-front and then threads this
/// through the INSERT — no `Option` juggling in the SQL bind list.
struct WritableTarget {
    owner: String,
    repo: String,
    base_branch: String,
    /// Optional sub-path within the repo. `None` means "everything
    /// under repo root" and is persisted as SQL `NULL` — consistent
    /// with how seeded sources without a subpath are stored. `Some` is
    /// always a non-empty, validated relative path (see
    /// `valid_repo_path`).
    path: Option<String>,
    auth_ref: String,
    /// Display name used as the source label in the federation config.
    /// `MinBZK/regelrecht-corpus` for the default, `<owner>/<repo>` for
    /// user-supplied repos.
    display_name: String,
}

/// Decide whether the create-request asks for the MinBZK default or a
/// user-supplied repo, then validate the latter against the configured
/// token before we open the DB transaction.
///
/// The three `repo_*` fields are all-or-nothing: any subset other than
/// "none" or "all three" is a 400 — partial submissions almost always
/// mean the frontend forgot to wire one field.
async fn resolve_writable_target(
    state: &AppState,
    req: &CreateTrajectRequest,
    account_id: Uuid,
    headers: &axum::http::HeaderMap,
) -> Result<WritableTarget, (StatusCode, String)> {
    let owner = req
        .repo_owner
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let repo = req
        .repo_name
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let base = req
        .base_branch
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());

    match (owner, repo, base) {
        // No repo coords → phase-1 default.
        (None, None, None) => Ok(WritableTarget {
            owner: CENTRAL_WRITABLE_OWNER.to_string(),
            repo: CENTRAL_WRITABLE_REPO.to_string(),
            base_branch: CENTRAL_WRITABLE_BASE_BRANCH.to_string(),
            path: Some(CENTRAL_WRITABLE_PATH.to_string()),
            auth_ref: CENTRAL_WRITABLE_AUTH_REF.to_string(),
            display_name: CENTRAL_WRITABLE_NAME.to_string(),
        }),
        // All three filled → user-supplied repo path. Validate.
        (Some(owner), Some(repo), Some(base_branch)) => {
            if !valid_repo_segment(owner) || !valid_repo_segment(repo) {
                return Err((
                    StatusCode::BAD_REQUEST,
                    "repo_owner / repo_name mogen alleen letters, cijfers, en \
                     de tekens '-', '_' en '.' bevatten"
                        .to_string(),
                ));
            }
            // `base_branch` flows unencoded into a GitHub URL and is
            // later persisted + used as a git refname; reject any
            // refname-illegal character at the boundary so neither
            // GitHub nor git ever sees a malformed ref. See
            // `valid_branch_name` for the exact rule set.
            if !valid_branch_name(base_branch) {
                return Err((
                    StatusCode::BAD_REQUEST,
                    "base_branch bevat tekens die niet zijn toegestaan in een git branch-naam"
                        .to_string(),
                ));
            }
            // Validate the optional repo sub-path at the API boundary.
            // `GitBackend::resolve` only validates the inner *relative*
            // path on each read/write; the base `gh_path` itself is
            // never re-checked downstream, so a traversal like
            // `../../etc` would let the backend read/write outside the
            // per-traject clone. Reject up-front.
            //
            // Trim + treat empty as "no subpath" → stored as SQL NULL
            // (matches how seeded sources without a subpath are stored;
            // avoids the "" vs NULL inconsistency on the row).
            let repo_path: Option<String> = req
                .repo_path
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string);
            if let Some(ref p) = repo_path {
                if !valid_repo_path(p) {
                    return Err((
                        StatusCode::BAD_REQUEST,
                        "repo_path moet een relatief pad zijn zonder '..' segmenten en \
                         mag alleen letters, cijfers, en '-', '_', '.' bevatten"
                            .to_string(),
                    ));
                }
            }
            let auth_ref = derive_auth_ref(owner, repo);
            if auth_ref.is_empty() {
                return Err((
                    StatusCode::BAD_REQUEST,
                    format!("owner/repo \"{owner}/{repo}\" produces an empty auth_ref"),
                ));
            }
            // Reject the rare collision where a user-supplied
            // owner/repo combination derives to the same slug as the
            // hardcoded central writable auth ref. Without this check,
            // a user who happens to point a traject at
            // `MinBZK/central` would silently get the central token
            // routed to their repo, violating the design principle
            // that the central token only ever reaches the central
            // repo. Surface as a 400 so the operator picks a
            // different slug (or omits the repo fields entirely to
            // use the actual MinBZK default).
            if auth_ref == CENTRAL_WRITABLE_AUTH_REF {
                return Err((
                    StatusCode::BAD_REQUEST,
                    format!(
                        "repo \"{owner}/{repo}\" botst met de gereserveerde central auth ref; \
                         gebruik een andere repo of laat de repo-velden weg om naar de centrale \
                         MinBZK-repo te wijzen"
                    ),
                ));
            }

            // Resolve the token to preflight the repo with, following the
            // same precedence rule as the write path
            // (`user_write_token_for_backend`): a configured per-repo
            // service token goes first — the eventual writes on this repo
            // run over that token too, so preflighting with the user's
            // personal token would validate an access path the traject
            // will never use. The strict context (`TokenContext::strict`)
            // is deliberate: `auth_ref` derives from user-supplied repo
            // coords, so an unknown ref must NOT fall back to
            // `CORPUS_GIT_TOKEN` (that would ship the central token to a
            // user-picked repo, a token-exfiltration vector).
            //
            // Only for a token-less ref does the acting user's OWN GitHub
            // token come into play (user-OAuth spike): the preflight then
            // validates *their* push access to the chosen repo — the
            // entitlement check GitHub gives us for free, so the editor
            // never needs an all-access credential to police repo choice
            // (the gap that #885 tracks). `user_write_token` returns 428
            // when a linked token is required but absent.
            let auth_file = {
                let corpus = state.corpus.read().await;
                corpus.auth_file.clone()
            };
            let service_token =
                regelrecht_corpus::auth::CredentialResolver::new(auth_file.as_deref())
                    .resolve(regelrecht_corpus::auth::TokenContext::strict(&auth_ref))
                    .map(regelrecht_corpus::auth::TokenDecision::into_token)
                    .map_err(|e| {
                        tracing::error!(error = %e, "auth lookup failed for new traject repo");
                        (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            "auth lookup failed".to_string(),
                        )
                    })?;
            let token = match service_token {
                Some(service_token) => service_token,
                None => crate::github_oauth::user_write_token(state, account_id, headers)
                    .await?
                    .ok_or_else(|| {
                        let env_name = regelrecht_corpus::auth::token_env_name(&auth_ref);
                        tracing::warn!(
                            auth_ref = %auth_ref,
                            env_name = %env_name,
                            "no token configured for user-supplied repo"
                        );
                        (
                            StatusCode::SERVICE_UNAVAILABLE,
                            format!(
                                "deze repo is nog niet door je beheerder geconfigureerd \
                             (verwacht env var {env_name})"
                            ),
                        )
                    })?,
            };

            // The OAuth config's `api_base` (a pub field, overridable in
            // tests with a wiremock server) wins over the real default, so
            // the preflight — including WHICH token it authenticates with —
            // is testable without touching github.com.
            let api_base = state
                .config
                .github_oauth
                .as_ref()
                .map(|o| o.api_base.as_str())
                .unwrap_or("https://api.github.com");
            let info = regelrecht_corpus::repo_access::validate_repo_access(
                api_base,
                owner,
                repo,
                base_branch,
                &token,
            )
            .await
            .map_err(|e| repo_access_error_to_status(&e, owner, repo, base_branch))?;

            tracing::info!(
                owner = %owner,
                repo = %repo,
                base_branch = %base_branch,
                default_branch = %info.default_branch,
                is_private = info.is_private,
                "validated user-supplied repo for new traject"
            );

            Ok(WritableTarget {
                owner: owner.to_string(),
                repo: repo.to_string(),
                base_branch: base_branch.to_string(),
                // `repo_path` is optional — `None` means "everything
                // under repo root" (persisted as NULL), which the
                // corpus client already handles correctly. The MinBZK
                // default explicitly sets "regulation/nl"; user repos
                // dedicated to regulations typically don't need a
                // subpath. Already validated above as a clean relative
                // path.
                path: repo_path,
                auth_ref,
                display_name: format!("{owner}/{repo}"),
            })
        }
        // Partial → caller error, refuse rather than guess.
        _ => Err((
            StatusCode::BAD_REQUEST,
            "repo_owner, repo_name en base_branch moeten alle drie worden meegegeven \
             (of alle drie weggelaten worden voor de standaard MinBZK repo)"
                .to_string(),
        )),
    }
}

/// Map a `RepoAccessError` to the appropriate HTTP status + a NL message
/// the create-sheet can show as-is. Kept verbose so each failure mode is
/// distinguishable in the UI.
fn repo_access_error_to_status(
    err: &regelrecht_corpus::repo_access::RepoAccessError,
    owner: &str,
    repo: &str,
    base_branch: &str,
) -> (StatusCode, String) {
    use regelrecht_corpus::repo_access::RepoAccessError as E;
    match err {
        // The *user* is fully authenticated (made it through OIDC + this
        // handler's middleware). What failed is the *operator's*
        // GitHub PAT — that is an upstream credential issue, not a
        // missing user credential. Use BAD_GATEWAY rather than 401 so
        // proxies / browsers don't try to inject WWW-Authenticate or
        // pop a credential dialog at the editor's user.
        E::Unauthorized => (
            StatusCode::BAD_GATEWAY,
            "het token van je beheerder wordt door GitHub geweigerd".to_string(),
        ),
        E::RepoNotFound => (
            StatusCode::NOT_FOUND,
            format!("repo {owner}/{repo} bestaat niet of het token kan 'm niet zien"),
        ),
        E::BranchNotFound => (
            StatusCode::NOT_FOUND,
            format!("branch '{base_branch}' bestaat niet op {owner}/{repo}"),
        ),
        E::NoPushAccess => (
            StatusCode::FORBIDDEN,
            "het geconfigureerde token heeft geen schrijftoegang tot deze repo".to_string(),
        ),
        E::Transport(msg) => {
            tracing::warn!(error = %msg, "transport error validating repo");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                "kon GitHub niet bereiken om de repo te valideren".to_string(),
            )
        }
        E::Other(msg) => {
            tracing::warn!(error = %msg, "unexpected GitHub response validating repo");
            (
                StatusCode::BAD_GATEWAY,
                "onverwacht antwoord van GitHub bij repo-validatie".to_string(),
            )
        }
    }
}

/// POST /api/trajects — create a new traject.
///
/// Seeds the federated config by copying the global registry's sources
/// (with their original priorities) and then attaching the writable own
/// source at priority 0. Branch creation on the writable source is
/// handled by `GitBackend` on first use, which falls back to the
/// configured base branch when the traject branch doesn't yet exist.
///
/// When the request supplies `repo_owner`/`repo_name`/`base_branch`,
/// the writable-own source points to that user repo and the
/// pre-existing `CORPUS_AUTH_*_TOKEN` env var (named after a
/// deterministic slug of `owner-repo` — see [`derive_auth_ref`])
/// authenticates it. No request omitted → phase-1 MinBZK default.
pub async fn create(
    State(state): State<AppState>,
    Extension(account): Extension<AccountRecord>,
    headers: axum::http::HeaderMap,
    Json(req): Json<CreateTrajectRequest>,
) -> Result<(StatusCode, Json<TrajectSummary>), (StatusCode, String)> {
    let name = req.name.trim();
    if name.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "name is required".to_string()));
    }

    // Resolve the writable-own GitHub target up-front. Validation may need
    // to talk to GitHub (which can fail with a helpful 4xx); we do that
    // *before* opening the DB transaction so a network blip doesn't leak a
    // half-rolled row.
    let target = resolve_writable_target(&state, &req, account.id, &headers).await?;

    let pool = get_pool_msg(&state)?;
    let mut tx = pool.begin().await.map_err(db_err_msg("begin tx"))?;

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
    .map_err(db_err_msg("insert traject"))?;

    sqlx::query(
        "INSERT INTO traject_members (traject_id, account_id, role)
         VALUES ($1, $2, 'owner')",
    )
    .bind(traject_id)
    .bind(account.id)
    .execute(&mut *tx)
    .await
    .map_err(db_err_msg("insert member"))?;

    // Seed federated read-config from the global registry. The global
    // corpus read guard is dropped before the next await so we don't hold
    // it across the database transaction.
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
        .map_err(db_err_msg("seed traject source"))?;
    }

    // Writable-own source: the phase-1 MinBZK default or the user-supplied
    // repo (already validated up-front for push access). The branch name is
    // derived from the traject name + id; auth flows through
    // `CORPUS_AUTH_{AUTH_REF_UPPER}_TOKEN` via the `auth_ref` stored on this
    // row.
    let writable_source_id = format!("traject-own-{}", traject_id.simple());
    let writable_branch = derive_branch_name(name, traject_id);
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
    .bind(&target.display_name)
    .bind(&target.owner)
    .bind(&target.repo)
    .bind(&writable_branch)
    .bind(&target.base_branch)
    .bind(&target.path)
    .bind(&target.auth_ref)
    .execute(&mut *tx)
    .await
    .map_err(db_err_msg("insert writable source"))?;

    tx.commit()
        .await
        .map_err(db_err_msg("commit traject create"))?;

    state.trajects.invalidate(traject_id).await;

    let mut summary = TrajectSummary {
        id: traject_id,
        name: name.to_string(),
        description: req.description,
        scope: req.scope,
        status: "bezig".to_string(),
        role: "owner".to_string(),
        traject_ref: None,
    };
    summary.fill_ref();
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
    // lowercases on the DB side to match our normalised key. The
    // functional unique index `idx_accounts_email_lower` (migration
    // 0016) makes this index-only and case-insensitively unique, so
    // `fetch_optional` is sound by construction.
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
        //
        // Transaction: the INSERT and the stale-invite cleanup must
        // commit together. If the cleanup failed mid-flight, GET would
        // briefly return the user in both `members` and `pending_invites`
        // until `promote_pending_invites` next ran.
        let mut tx = pool.begin().await.map_err(db_err("begin add_member tx"))?;
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
        .execute(&mut *tx)
        .await
        .map_err(db_err("add member"))?
        .rows_affected();

        if affected == 0 {
            return Err(StatusCode::CONFLICT);
        }
        sqlx::query("DELETE FROM traject_invites WHERE traject_id = $1 AND email = $2")
            .bind(id)
            .bind(&email)
            .execute(&mut *tx)
            .await
            .map_err(db_err("clean up stale invite"))?;
        tx.commit().await.map_err(db_err("commit add_member tx"))?;
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
/// traject. The next write request the caller makes against this
/// traject's URL will 403 on the membership re-check in
/// `require_traject_corpus_from_ref`.
pub async fn leave(
    State(state): State<AppState>,
    Extension(account): Extension<AccountRecord>,
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

    #[test]
    fn auth_ref_lowercases_and_collapses_separators() {
        // Owner + repo combined with `/`, all separators collapse to a
        // single dash. Operators read this back as the env-var stem
        // `CORPUS_AUTH_<UPPER>_TOKEN`.
        assert_eq!(derive_auth_ref("Acme", "Secret-Repo"), "acme-secret-repo");
        assert_eq!(
            derive_auth_ref("MinBZK", "regelrecht-corpus"),
            "minbzk-regelrecht-corpus"
        );
        assert_eq!(derive_auth_ref("a.b", "c_d"), "a-b-c-d");
    }

    #[test]
    fn auth_ref_stable_under_already_normalised_input() {
        // An already-normalised owner/repo (lowercase + dash-only) must
        // come out byte-identical — operators read this back from the
        // DB row to know the matching env-var name; any drift between
        // create-time and read-time would mean "broken token resolution
        // after a rename".
        assert_eq!(derive_auth_ref("acme", "secret-repo"), "acme-secret-repo");
        assert_eq!(
            derive_auth_ref("minbzk", "regelrecht-corpus"),
            "minbzk-regelrecht-corpus"
        );
    }

    #[test]
    fn auth_ref_trims_leading_and_trailing_dashes() {
        // Garbage in/out: only-special-chars resolves to "" rather than
        // a string of pure dashes. The caller should reject this before
        // it reaches the INSERT.
        assert_eq!(derive_auth_ref("...", "..."), "");
        assert_eq!(derive_auth_ref(".foo.", ".bar."), "foo-bar");
    }

    #[test]
    fn valid_repo_path_accepts_empty_and_simple_subdirs() {
        assert!(valid_repo_path(""));
        assert!(valid_repo_path("regulation/nl"));
        assert!(valid_repo_path("a"));
        assert!(valid_repo_path("a/b/c"));
        assert!(valid_repo_path("dir.with.dots/and_underscores"));
    }

    #[test]
    fn valid_repo_path_rejects_traversal_and_absolute() {
        // The whole point of this check: the base gh_path is never
        // re-validated by GitBackend::resolve, so any traversal or
        // absolute path that lands in the DB row will be honoured by
        // the backend on subsequent reads/writes. Reject at the API
        // boundary.
        assert!(!valid_repo_path("../etc"));
        assert!(!valid_repo_path("/etc"));
        assert!(!valid_repo_path("regulation/../../etc"));
        assert!(!valid_repo_path("./regulation"));
        assert!(!valid_repo_path(".."));
        assert!(!valid_repo_path("regulation/with space"));
        assert!(!valid_repo_path("regulation/with!bang"));
    }

    #[test]
    fn valid_repo_segment_rejects_leading_dot_and_dots_only() {
        // GitHub disallows segments that are literal `.` or `..` and
        // names starting with a leading `.`. The previous validator
        // permitted these because they're inside the `[A-Za-z0-9._-]`
        // character class; this test pins the explicit rejection.
        assert!(!valid_repo_segment("."));
        assert!(!valid_repo_segment(".."));
        assert!(!valid_repo_segment(".hidden"));
        assert!(!valid_repo_segment(".github"));
        // Dots in the middle of a segment remain valid (e.g. version
        // suffixes like `repo.v2`).
        assert!(valid_repo_segment("a.b"));
        assert!(valid_repo_segment("regelrecht-corpus"));
    }

    #[test]
    fn valid_branch_name_accepts_common_branches() {
        // Real branch names that flow through `resolve_writable_target`
        // every day: must not be rejected by the boundary guard.
        assert!(valid_branch_name("main"));
        assert!(valid_branch_name("develop"));
        assert!(valid_branch_name("feature/foo"));
        assert!(valid_branch_name("release-1.0"));
        assert!(valid_branch_name("user/feature/sub-branch"));
    }

    #[test]
    fn valid_branch_name_rejects_url_injection_and_refname_illegals() {
        // `?` would let `main?spoof=1` pass the trim+non-empty check
        // because GitHub strips the query before validating `main`;
        // refuse it at the boundary so the persisted refname is clean.
        assert!(!valid_branch_name("main?spoof=1"));
        // Trailing/embedded whitespace and control chars.
        assert!(!valid_branch_name("main "));
        assert!(!valid_branch_name("ma in"));
        assert!(!valid_branch_name("main\n"));
        assert!(!valid_branch_name("main\t"));
        // Refname-illegal characters from `git-check-ref-format`.
        assert!(!valid_branch_name("feat~1"));
        assert!(!valid_branch_name("feat^2"));
        assert!(!valid_branch_name("a:b"));
        assert!(!valid_branch_name("a*b"));
        assert!(!valid_branch_name("a[b"));
        assert!(!valid_branch_name("a\\b"));
        assert!(!valid_branch_name("feat@{1}"));
        // `#` is legal in git refnames but is the URL fragment delimiter,
        // so `main#spoof` would 200-pass the pre-flight (matches `main`)
        // and produce a non-existent refname downstream. Refuse it.
        assert!(!valid_branch_name("main#spoof"));
        assert!(!valid_branch_name("#"));
        // `..` (dot-dot), leading `-`, leading/trailing `/`, double `/`.
        assert!(!valid_branch_name(".."));
        assert!(!valid_branch_name("a..b"));
        assert!(!valid_branch_name("-x"));
        assert!(!valid_branch_name("/x"));
        assert!(!valid_branch_name("x/"));
        assert!(!valid_branch_name("feat//bar"));
        // Empty + oversized.
        assert!(!valid_branch_name(""));
        let oversized: String = "a".repeat(201);
        assert!(!valid_branch_name(&oversized));
    }

    #[test]
    fn valid_branch_name_rejects_git_specific_corner_cases() {
        // git-check-ref-format rejects refnames that end with `.`, the
        // bare `@`, or any component ending in `.lock`. Mirror those
        // here so the boundary check matches the refname grammar.
        assert!(!valid_branch_name("main."));
        assert!(!valid_branch_name("@"));
        assert!(!valid_branch_name("main.lock"));
        assert!(!valid_branch_name("feat/foo.lock"));
        // No path component may start with `.`. GitHub would accept
        // these refs but git would refuse them downstream at push time.
        assert!(!valid_branch_name(".hidden"));
        assert!(!valid_branch_name("feature/.hidden"));
        assert!(!valid_branch_name(".github"));
        // Sanity: dots / "lock" inside a name (not as a trailing
        // component / suffix) stay accepted.
        assert!(valid_branch_name("main.foo"));
        assert!(valid_branch_name("locked-down"));
    }

    #[test]
    fn minbzk_central_collides_with_reserved_auth_ref() {
        // Documents the collision the `resolve_writable_target` guard
        // exists to prevent: a user-supplied owner=MinBZK, repo=central
        // derives byte-for-byte to the hardcoded CENTRAL_WRITABLE_AUTH_REF.
        // Without the guard, that traject would silently get the central
        // token routed to a user-chosen repo. The guard rejects the
        // request before INSERT; this test pins the precondition so a
        // future rename of either constant or `derive_auth_ref` can't
        // silently break the guard's premise.
        assert_eq!(
            derive_auth_ref("MinBZK", "central"),
            CENTRAL_WRITABLE_AUTH_REF
        );
        // Sanity check: an unrelated repo doesn't collide.
        assert_ne!(
            derive_auth_ref("MinBZK", "regelrecht-corpus"),
            CENTRAL_WRITABLE_AUTH_REF
        );
    }
}
