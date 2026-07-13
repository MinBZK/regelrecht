//! Taken-API: de persoonlijke taken van de ingelogde gebruiker.
//!
//! Leest/muteert uitsluitend taken waarvan het account de assignee is; de
//! store-laag (`regelrecht_pipeline::tasks`) bakt die eis in de queries, dus
//! vreemde taak-ids zijn niet te onderscheiden van afwezige (zelfde 404-
//! filosofie als user_notes).

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::{Extension, Json};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use regelrecht_pipeline::tasks::{self, BlobKind, Task, TaskStatus};

use crate::accounts::AccountRecord;
use crate::state::AppState;

fn get_pool(state: &AppState) -> Result<&sqlx::PgPool, StatusCode> {
    state.pool.as_ref().ok_or(StatusCode::SERVICE_UNAVAILABLE)
}

fn db_error<E: std::fmt::Display>(e: E) -> StatusCode {
    tracing::error!(error = %e, "taken-query mislukt");
    StatusCode::INTERNAL_SERVER_ERROR
}

#[derive(Serialize)]
pub struct TaskListResponse {
    pub tasks: Vec<Task>,
    pub open_count: usize,
}

/// GET /api/tasks — open taken van het ingelogde account, nieuwste eerst.
pub async fn list(
    State(state): State<AppState>,
    Extension(account): Extension<AccountRecord>,
) -> Result<Json<TaskListResponse>, StatusCode> {
    let pool = get_pool(&state)?;
    let tasks = tasks::list_open_tasks_for_account(pool, account.id)
        .await
        .map_err(db_error)?;
    let open_count = tasks.len();
    Ok(Json(TaskListResponse { tasks, open_count }))
}

#[derive(Serialize)]
pub struct ResultFile {
    pub path: String,
    pub content: String,
}

#[derive(Serialize)]
pub struct TaskDetailResponse {
    #[serde(flatten)]
    pub task: Task,
    /// Result-bestanden van de job (leeg voor job_failed-taken).
    pub results: Vec<ResultFile>,
}

/// GET /api/tasks/{task_id} — taak + result-content. De staleness-check
/// gebeurt client-side: de frontend vergelijkt payload.source_etag met de
/// ETag van de actuele law-GET die hij toch al doet.
pub async fn detail(
    State(state): State<AppState>,
    Extension(account): Extension<AccountRecord>,
    Path(task_id): Path<Uuid>,
) -> Result<Json<TaskDetailResponse>, StatusCode> {
    let pool = get_pool(&state)?;
    let task = tasks::get_task_for_account(pool, task_id, account.id)
        .await
        .map_err(db_error)?
        .ok_or(StatusCode::NOT_FOUND)?;
    let results = match task.job_id {
        Some(job_id) => tasks::load_blobs(pool, job_id, BlobKind::Result)
            .await
            .map_err(db_error)?
            .into_iter()
            .map(|b| ResultFile {
                path: b.path,
                content: b.content,
            })
            .collect(),
        None => Vec::new(),
    };
    Ok(Json(TaskDetailResponse { task, results }))
}

#[derive(Deserialize)]
pub struct ResolveRequest {
    pub action: ResolveAction,
}

#[derive(Deserialize, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum ResolveAction {
    Approved,
    Rejected,
    Dismissed,
}

impl From<ResolveAction> for TaskStatus {
    fn from(a: ResolveAction) -> Self {
        match a {
            ResolveAction::Approved => TaskStatus::Approved,
            ResolveAction::Rejected => TaskStatus::Rejected,
            ResolveAction::Dismissed => TaskStatus::Dismissed,
        }
    }
}

/// POST /api/tasks/{task_id}/resolve — handel een open taak af en ruim de
/// blobs op (idempotent: bij job_failed zijn ze al weg). Doet zelf géén
/// git-write; approve-na-save is de verantwoordelijkheid van de client
/// (spec §5.3).
pub async fn resolve(
    State(state): State<AppState>,
    Extension(account): Extension<AccountRecord>,
    Path(task_id): Path<Uuid>,
    Json(req): Json<ResolveRequest>,
) -> Result<Json<Task>, StatusCode> {
    let pool = get_pool(&state)?;
    let task = tasks::resolve_task(pool, task_id, account.id, req.action.into())
        .await
        .map_err(db_error)?
        .ok_or(StatusCode::NOT_FOUND)?;
    if let Some(job_id) = task.job_id {
        // Best-effort: een cleanup-fout mag de afhandeling niet terugdraaien;
        // de 7-dagen-GC vangt het restje.
        if let Err(e) = tasks::delete_blobs_for_job(pool, job_id).await {
            tracing::warn!(error = %e, task_id = %task.id, "blob-cleanup na resolve mislukt");
        }
    }
    Ok(Json(task))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_action_deserializes_lowercase_and_rejects_open() {
        let a: ResolveAction = serde_json::from_str("\"approved\"").unwrap();
        assert!(matches!(a, ResolveAction::Approved));
        let r: ResolveAction = serde_json::from_str("\"rejected\"").unwrap();
        assert!(matches!(r, ResolveAction::Rejected));
        let d: ResolveAction = serde_json::from_str("\"dismissed\"").unwrap();
        assert!(matches!(d, ResolveAction::Dismissed));
        // 'open' is geen geldige actie — deserialisatie faalt.
        assert!(serde_json::from_str::<ResolveAction>("\"open\"").is_err());
    }
}
