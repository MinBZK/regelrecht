//! Persoonlijke notities: private per-user notes on a law.
//!
//! Unlike traject annotations (shared sidecar YAML committed to git,
//! RFC-018 Decision 1), these notes are private working context for one
//! user and live only in Postgres, keyed by `accounts.id`. Rows mirror
//! the W3C Web Annotation model of RFC-005: each note serializes as an
//! `Annotation` with a law-level target and a `TextualBody` whose
//! `format` is `text/markdown` by default.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::{Extension, Json};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::accounts::AccountRecord;
use crate::state::AppState;

/// Motivations accepted for a personal note. Deliberately narrower than
/// the full W3C vocabulary: personal notes carry free-form context, not
/// linking/tagging semantics (those belong to shared traject notes).
const ALLOWED_MOTIVATIONS: &[&str] = &["commenting", "questioning"];

/// Body formats accepted for a personal note. Markdown is the default
/// authoring format; plain text is kept for parity with RFC-005 examples.
const ALLOWED_FORMATS: &[&str] = &["text/markdown", "text/plain"];

/// Upper bound on the note body in bytes. Generous for hand-written
/// context, small enough that the table cannot be used as blob storage.
const MAX_BODY_VALUE_BYTES: usize = 64 * 1024;

/// Upper bound on notes per user per law. Far above real use, so the
/// silent `LIMIT` in `list` is unreachable and an account cannot grow
/// unbounded rows (each up to 64 KiB).
const MAX_NOTES_PER_LAW: i64 = 200;

/// Request body for creating or updating a note. `format` and
/// `motivation` fall back to the markdown/commenting defaults so the
/// minimal client payload is just `{"value": "..."}`.
#[derive(Debug, Deserialize)]
pub struct NoteRequest {
    pub value: String,
    #[serde(default)]
    pub format: Option<String>,
    #[serde(default)]
    pub motivation: Option<String>,
}

/// A personal note in W3C Web Annotation shape (RFC-005), so clients can
/// treat it like any other note. `id`, `created` and `modified` are
/// server-managed extras the sidecar format does not carry.
#[derive(Debug, Serialize)]
pub struct UserNote {
    pub id: Uuid,
    #[serde(rename = "type")]
    pub note_type: &'static str,
    pub motivation: String,
    pub target: NoteTarget,
    pub body: NoteBody,
    pub created: DateTime<Utc>,
    pub modified: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct NoteTarget {
    pub source: String,
}

#[derive(Debug, Serialize)]
pub struct NoteBody {
    #[serde(rename = "type")]
    pub body_type: &'static str,
    pub value: String,
    pub purpose: String,
    pub format: String,
}

type NoteRow = (Uuid, String, String, String, DateTime<Utc>, DateTime<Utc>);

impl UserNote {
    /// Build the annotation view from a
    /// `(id, motivation, body_value, body_format, created_at, updated_at)`
    /// row for the given law.
    fn from_row(law_id: &str, row: NoteRow) -> Self {
        let (id, motivation, body_value, body_format, created_at, updated_at) = row;
        UserNote {
            id,
            note_type: "Annotation",
            target: NoteTarget {
                source: format!("regelrecht://{law_id}"),
            },
            body: NoteBody {
                body_type: "TextualBody",
                value: body_value,
                // W3C `purpose` mirrors the annotation-level motivation
                // for a single-body note (same convention as the editor's
                // NoteCreator).
                purpose: motivation.clone(),
                format: body_format,
            },
            motivation,
            created: created_at,
            modified: updated_at,
        }
    }
}

fn get_pool(state: &AppState) -> Result<&sqlx::PgPool, StatusCode> {
    state.pool.as_ref().ok_or(StatusCode::SERVICE_UNAVAILABLE)
}

fn validate_law_id(law_id: &str) -> Result<(), StatusCode> {
    // .len() returns bytes, which equals character count for ASCII-only law IDs.
    if law_id.is_empty() || law_id.len() > 256 {
        return Err(StatusCode::BAD_REQUEST);
    }
    Ok(())
}

/// Validate a create/update request. `format`/`motivation` stay `None`
/// when absent: `create` fills in the markdown/commenting defaults,
/// `update` keeps the stored values (absent = keep, so a client that
/// only sends `{"value": ...}` cannot silently reset metadata).
fn validate_request(
    req: NoteRequest,
) -> Result<(String, Option<String>, Option<String>), StatusCode> {
    if req.value.trim().is_empty() || req.value.len() > MAX_BODY_VALUE_BYTES {
        return Err(StatusCode::BAD_REQUEST);
    }
    if let Some(format) = &req.format {
        if !ALLOWED_FORMATS.contains(&format.as_str()) {
            return Err(StatusCode::BAD_REQUEST);
        }
    }
    if let Some(motivation) = &req.motivation {
        if !ALLOWED_MOTIVATIONS.contains(&motivation.as_str()) {
            return Err(StatusCode::BAD_REQUEST);
        }
    }
    Ok((req.value, req.format, req.motivation))
}

/// GET /api/user/notes/{law_id} — the authenticated user's notes for a law,
/// oldest first.
pub async fn list(
    State(state): State<AppState>,
    Extension(account): Extension<AccountRecord>,
    Path(law_id): Path<String>,
) -> Result<Json<Vec<UserNote>>, StatusCode> {
    validate_law_id(&law_id)?;
    let pool = get_pool(&state)?;

    let rows: Vec<NoteRow> = sqlx::query_as(
        "SELECT id, motivation, body_value, body_format, created_at, updated_at \
         FROM user_notes WHERE account_id = $1 AND law_id = $2 \
         ORDER BY created_at LIMIT 1000",
    )
    .bind(account.id)
    .bind(&law_id)
    .fetch_all(pool)
    .await
    .map_err(|e| {
        tracing::error!(error = %e, "failed to fetch user notes");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(
        rows.into_iter()
            .map(|row| UserNote::from_row(&law_id, row))
            .collect(),
    ))
}

/// POST /api/user/notes/{law_id} — create a note, returns it as 201.
/// 409 when the per-law cap is reached.
pub async fn create(
    State(state): State<AppState>,
    Extension(account): Extension<AccountRecord>,
    Path(law_id): Path<String>,
    Json(req): Json<NoteRequest>,
) -> Result<(StatusCode, Json<UserNote>), StatusCode> {
    validate_law_id(&law_id)?;
    let (value, format, motivation) = validate_request(req)?;
    let format = format.unwrap_or_else(|| "text/markdown".to_string());
    let motivation = motivation.unwrap_or_else(|| "commenting".to_string());
    let pool = get_pool(&state)?;

    let (count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM user_notes WHERE account_id = $1 AND law_id = $2")
            .bind(account.id)
            .bind(&law_id)
            .fetch_one(pool)
            .await
            .map_err(|e| {
                tracing::error!(error = %e, "failed to count user notes");
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
    if count >= MAX_NOTES_PER_LAW {
        return Err(StatusCode::CONFLICT);
    }

    let row: NoteRow = sqlx::query_as(
        "INSERT INTO user_notes (account_id, law_id, motivation, body_value, body_format) \
         VALUES ($1, $2, $3, $4, $5) \
         RETURNING id, motivation, body_value, body_format, created_at, updated_at",
    )
    .bind(account.id)
    .bind(&law_id)
    .bind(&motivation)
    .bind(&value)
    .bind(&format)
    .fetch_one(pool)
    .await
    .map_err(|e| {
        tracing::error!(error = %e, "failed to create user note");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok((StatusCode::CREATED, Json(UserNote::from_row(&law_id, row))))
}

/// PUT /api/user/notes/{law_id}/{note_id} — update a note's body;
/// `format`/`motivation` are only changed when present in the request
/// (absent = keep). 404 for a note that does not exist or belongs to
/// another account, so foreign note ids are indistinguishable from
/// absent ones.
pub async fn update(
    State(state): State<AppState>,
    Extension(account): Extension<AccountRecord>,
    Path((law_id, note_id)): Path<(String, Uuid)>,
    Json(req): Json<NoteRequest>,
) -> Result<Json<UserNote>, StatusCode> {
    validate_law_id(&law_id)?;
    let (value, format, motivation) = validate_request(req)?;
    let pool = get_pool(&state)?;

    let row: Option<NoteRow> = sqlx::query_as(
        "UPDATE user_notes SET motivation = COALESCE($4, motivation), body_value = $5, \
         body_format = COALESCE($6, body_format) \
         WHERE id = $1 AND account_id = $2 AND law_id = $3 \
         RETURNING id, motivation, body_value, body_format, created_at, updated_at",
    )
    .bind(note_id)
    .bind(account.id)
    .bind(&law_id)
    .bind(&motivation)
    .bind(&value)
    .bind(&format)
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        tracing::error!(error = %e, "failed to update user note");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    match row {
        Some(row) => Ok(Json(UserNote::from_row(&law_id, row))),
        None => Err(StatusCode::NOT_FOUND),
    }
}

/// DELETE /api/user/notes/{law_id}/{note_id} — remove a note. 404 for a
/// note that does not exist or belongs to another account.
pub async fn remove(
    State(state): State<AppState>,
    Extension(account): Extension<AccountRecord>,
    Path((law_id, note_id)): Path<(String, Uuid)>,
) -> Result<StatusCode, StatusCode> {
    validate_law_id(&law_id)?;
    let pool = get_pool(&state)?;

    let result =
        sqlx::query("DELETE FROM user_notes WHERE id = $1 AND account_id = $2 AND law_id = $3")
            .bind(note_id)
            .bind(account.id)
            .bind(&law_id)
            .execute(pool)
            .await
            .map_err(|e| {
                tracing::error!(error = %e, "failed to delete user note");
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

    if result.rows_affected() > 0 {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}
