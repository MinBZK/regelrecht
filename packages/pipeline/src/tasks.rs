//! Persoonlijke review-taken voor async jobs.
//!
//! Een taak koppelt een afgeronde (of terminaal gefaalde) job aan het account
//! dat hem aanvroeg. De job-input en het job-resultaat rijden als transiënte
//! rijen in `job_blobs` (patroon `document_uploads`): de worker raakt GitHub
//! in de taak-flow nooit aan. Taakrijen blijven na afhandeling bestaan
//! (audit-spoor); alleen de blobs worden opgeruimd.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::Result;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "task_status", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum TaskStatus {
    Open,
    Approved,
    Rejected,
    Dismissed,
}

/// Taaktype als TEXT + CHECK i.p.v. PG-enum: een nieuw type is een
/// CHECK-wijziging in een gewone transactionele migratie, geen
/// ALTER TYPE-dans (zie migraties 0007/0018/0025).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskType {
    JobReview,
    JobFailed,
}

impl TaskType {
    pub fn as_str(&self) -> &'static str {
        match self {
            TaskType::JobReview => "job_review",
            TaskType::JobFailed => "job_failed",
        }
    }
}

#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct Task {
    pub id: Uuid,
    pub task_type: String,
    pub status: TaskStatus,
    pub assignee_account_id: Option<Uuid>,
    pub traject_id: Option<Uuid>,
    pub job_id: Option<Uuid>,
    pub title: String,
    pub payload: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub resolved_by: Option<Uuid>,
}

pub struct NewTask {
    pub task_type: TaskType,
    pub assignee_account_id: Option<Uuid>,
    pub traject_id: Option<Uuid>,
    pub job_id: Option<Uuid>,
    pub title: String,
    pub payload: Option<serde_json::Value>,
}

const RETURNING: &str = "id, task_type, status, assignee_account_id, traject_id, job_id, \
                         title, payload, created_at, resolved_at, resolved_by";

/// Maak een taak aan. Generiek over de executor zodat de worker dit binnen
/// dezelfde transactie als `complete_job`/`fail_job_terminal` kan doen.
pub async fn create_task<'e, E>(executor: E, new: NewTask) -> Result<Task>
where
    E: sqlx::PgExecutor<'e>,
{
    let query = format!(
        "INSERT INTO tasks (task_type, assignee_account_id, traject_id, job_id, title, payload) \
         VALUES ($1, $2, $3, $4, $5, $6) RETURNING {RETURNING}",
    );
    let task = sqlx::query_as::<_, Task>(&query)
        .bind(new.task_type.as_str())
        .bind(new.assignee_account_id)
        .bind(new.traject_id)
        .bind(new.job_id)
        .bind(&new.title)
        .bind(&new.payload)
        .fetch_one(executor)
        .await?;
    tracing::info!(task_id = %task.id, task_type = %task.task_type, "task created");
    Ok(task)
}

/// Open taken van een account, nieuwste eerst. Gebonden LIMIT zodat de
/// takenlijst nooit onbegrensd groeit in één response.
pub async fn list_open_tasks_for_account(pool: &PgPool, account_id: Uuid) -> Result<Vec<Task>> {
    let query = format!(
        "SELECT {RETURNING} FROM tasks \
         WHERE assignee_account_id = $1 AND status = 'open' \
         ORDER BY created_at DESC LIMIT 100",
    );
    let tasks = sqlx::query_as::<_, Task>(&query)
        .bind(account_id)
        .fetch_all(pool)
        .await?;
    Ok(tasks)
}

/// Eén taak op id, ongeacht status (detail-view toont ook net-geresolvede).
pub async fn get_task_for_account(
    pool: &PgPool,
    task_id: Uuid,
    account_id: Uuid,
) -> Result<Option<Task>> {
    let query =
        format!("SELECT {RETURNING} FROM tasks WHERE id = $1 AND assignee_account_id = $2",);
    let task = sqlx::query_as::<_, Task>(&query)
        .bind(task_id)
        .bind(account_id)
        .fetch_optional(pool)
        .await?;
    Ok(task)
}

/// Handel een taak af. Alleen de assignee mag dat, en alleen vanuit 'open':
/// beide voorwaarden zitten in de WHERE zodat een race of vreemd account
/// simpelweg `None` oplevert (geen aparte foutklasse nodig).
pub async fn resolve_task(
    pool: &PgPool,
    task_id: Uuid,
    account_id: Uuid,
    new_status: TaskStatus,
) -> Result<Option<Task>> {
    if new_status == TaskStatus::Open {
        // 'open' is geen afhandeling; user input mag audit-velden niet
        // kunnen stempelen zonder de taak echt te sluiten.
        return Ok(None);
    }
    let query = format!(
        "UPDATE tasks SET status = $3, resolved_at = now(), resolved_by = $2 \
         WHERE id = $1 AND assignee_account_id = $2 AND status = 'open' \
         RETURNING {RETURNING}",
    );
    let task = sqlx::query_as::<_, Task>(&query)
        .bind(task_id)
        .bind(account_id)
        .bind(new_status)
        .fetch_optional(pool)
        .await?;
    if let Some(ref t) = task {
        tracing::info!(task_id = %t.id, status = ?t.status, "task resolved");
    }
    Ok(task)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlobKind {
    Input,
    Result,
}

impl BlobKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            BlobKind::Input => "input",
            BlobKind::Result => "result",
        }
    }
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct JobBlob {
    pub id: Uuid,
    pub job_id: Uuid,
    pub kind: String,
    pub path: String,
    pub content: String,
}

pub async fn insert_blob<'e, E>(
    executor: E,
    job_id: Uuid,
    kind: BlobKind,
    path: &str,
    content: &str,
) -> Result<()>
where
    E: sqlx::PgExecutor<'e>,
{
    sqlx::query("INSERT INTO job_blobs (job_id, kind, path, content) VALUES ($1, $2, $3, $4)")
        .bind(job_id)
        .bind(kind.as_str())
        .bind(path)
        .bind(content)
        .execute(executor)
        .await?;
    Ok(())
}

/// Blobs van een job, per soort. Geen account-gating hier: de autorisatie
/// ligt bij de taak-laag — handlers mogen blobs uitsluitend ophalen via een
/// taak die al met `get_task_for_account` voor dit account is geverifieerd.
pub async fn load_blobs(pool: &PgPool, job_id: Uuid, kind: BlobKind) -> Result<Vec<JobBlob>> {
    let blobs = sqlx::query_as::<_, JobBlob>(
        "SELECT id, job_id, kind, path, content FROM job_blobs \
         WHERE job_id = $1 AND kind = $2 ORDER BY path",
    )
    .bind(job_id)
    .bind(kind.as_str())
    .fetch_all(pool)
    .await?;
    Ok(blobs)
}

pub async fn delete_blobs_for_job<'e, E>(executor: E, job_id: Uuid) -> Result<()>
where
    E: sqlx::PgExecutor<'e>,
{
    sqlx::query("DELETE FROM job_blobs WHERE job_id = $1")
        .bind(job_id)
        .execute(executor)
        .await?;
    Ok(())
}

/// GC: verwijder blobs ouder dan 7 dagen waarvan de job geen open taak meer
/// heeft (patroon `cleanup_orphaned_uploads`). De grace is ruim: een open
/// taak houdt zijn blobs onbeperkt vast — dit vangt alleen wezen af van
/// gecrashte workers of nooit-geresolvede fouten.
pub async fn cleanup_orphaned_blobs(pool: &PgPool) -> Result<u64> {
    let result = sqlx::query(
        r#"
        DELETE FROM job_blobs jb
        WHERE jb.created_at < now() - interval '7 days'
          AND NOT EXISTS (
              SELECT 1 FROM tasks t
              WHERE t.job_id = jb.job_id AND t.status = 'open'
          )
        "#,
    )
    .execute(pool)
    .await?;
    Ok(result.rows_affected())
}
