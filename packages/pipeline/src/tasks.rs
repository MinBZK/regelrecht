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
use crate::models::{JobStatus, JobType};

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

/// Aantal open taken van een account. Losse COUNT naast de gelimiteerde
/// lijst-query, zodat de teller ook boven de lijst-LIMIT klopt; gedekt door
/// de partial index idx_tasks_open_assignee.
pub async fn count_open_tasks_for_account(pool: &PgPool, account_id: Uuid) -> Result<i64> {
    let (count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM tasks WHERE assignee_account_id = $1 AND status = 'open'",
    )
    .bind(account_id)
    .fetch_one(pool)
    .await?;
    Ok(count)
}

/// Eén lopende taak-flow-job voor de "Bezig"-sectie: een enrich-,
/// document_convert-, law_convert- of traject_harvest-job die deze gebruiker
/// via `deliver: "task"` heeft aangevraagd en die nog niet is afgerond
/// (job_review/job_failed-taak bestaat pas na completion/failure).
#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct RunningTaskJob {
    pub job_id: Uuid,
    pub job_type: JobType,
    pub law_id: String,
    pub traject_ref: Option<String>,
    /// Weergavenaam voor conversie-jobs: het doelbestand van een
    /// document_convert-job (payload `target_path`) of de bestandsnaam van
    /// een law_convert-upload (payload `filename`); None voor enrich-jobs.
    /// Het `law_id`-veld draagt voor conversies een synthetische sleutel
    /// (`doc:`/`lawdoc:`), dus de weergave leest dit veld.
    pub target_path: Option<String>,
    pub status: JobStatus,
    pub created_at: DateTime<Utc>,
}

/// Lopende (pending/processing) taak-flow-jobs van een account, nieuwste
/// eerst — de "Bezig"-sectie van de takenlijst. Leest de jobs-tabel op
/// payload-velden; de tabel is klein genoeg dat dit zonder eigen index kan
/// (zelfde afweging als de cap-check in editor-api, zie task_requests.rs).
pub async fn list_running_task_jobs_for_account(
    pool: &PgPool,
    account_id: Uuid,
) -> Result<Vec<RunningTaskJob>> {
    let jobs = sqlx::query_as::<_, RunningTaskJob>(
        "SELECT id AS job_id, job_type, law_id, traject_ref, \
                COALESCE(payload->>'target_path', payload->>'filename') AS target_path, \
                status, created_at \
         FROM jobs \
         WHERE job_type IN ('enrich', 'document_convert', 'law_convert', 'traject_harvest') \
           AND status IN ('pending', 'processing') \
           AND payload->>'deliver' = 'task' \
           AND payload->>'requested_by' = $1 \
         ORDER BY created_at DESC LIMIT 50",
    )
    .bind(account_id.to_string())
    .fetch_all(pool)
    .await?;
    Ok(jobs)
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

/// Target paths reserved by OPEN document-review tasks of a traject: a
/// task-delivered conversion is already `completed` on the underlying job
/// (see `finish_document_convert_task_job`) while the review itself is still
/// unresolved and the `.md` doesn't exist on the branch yet. The upload
/// collision check (`pending_target_paths`, which only looks at
/// pending/processing jobs) misses this window — a second upload with the
/// same derived name would collide with a name that's really still "in use"
/// by the open task. Approving/rejecting the task (or letting it lapse) frees
/// the name again, since the task's status then stops matching `= 'open'`.
pub async fn open_document_task_target_paths(
    pool: &PgPool,
    traject_id: Uuid,
) -> Result<Vec<String>> {
    let rows = sqlx::query_as::<_, (Option<String>,)>(
        "SELECT payload->>'target_path' FROM tasks \
         WHERE traject_id = $1 AND status = 'open' AND task_type = 'job_review' \
           AND payload->>'kind' = 'document' AND payload->>'target_path' IS NOT NULL",
    )
    .bind(traject_id)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().filter_map(|(p,)| p).collect())
}

/// Maak `job_failed`-taken voor door de reaper TERMINAAL gefaalde
/// taak-flow-jobs (payload `deliver: "task"` met een `requested_by`).
///
/// Zonder deze nazorg verdwijnt zo'n job stil: de "Bezig"-regel in het
/// takenpaneel verdwijnt en de aanvrager krijgt nooit te zien dat er iets
/// misging (waargenomen bij trage law-convert-runs, maar het gat gold voor
/// álle taak-flow-jobtypen). Her-pogingen (status terug naar pending) slaan
/// we over: die komen gewoon opnieuw langs. Ruimt per gefaalde job ook de
/// job-blobs op (patroon `finalize_failed_task_job_tx`). Best-effort per
/// job: één mislukte taak-insert mag de reaper-loop niet stoppen.
pub async fn notify_reaped_task_jobs(
    pool: &PgPool,
    reaped: &[crate::job_queue::ReapedJob],
) -> Result<()> {
    for job in reaped {
        if job.status != JobStatus::Failed {
            continue;
        }
        let Some(payload) = job.payload.as_ref() else {
            continue;
        };
        if payload.get("deliver").and_then(|v| v.as_str()) != Some("task") {
            continue;
        }
        let Some(assignee) = payload
            .get("requested_by")
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::parse_str(s).ok())
        else {
            continue;
        };
        let traject_id = payload
            .get("traject_id")
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::parse_str(s).ok());
        let str_field = |key: &str| payload.get(key).and_then(|v| v.as_str());

        let title = match job.job_type {
            // Harvest-jobs zijn nooit taak-flow; de deliver-check hierboven
            // filtert ze al, dit is een expliciet vangnet.
            JobType::Harvest => continue,
            JobType::Enrich => {
                let law_id = str_field("law_id").unwrap_or(&job.law_id);
                if payload.get("new_law").and_then(|v| v.as_bool()) == Some(true) {
                    format!("Wet aanmaken mislukt: {law_id}")
                } else {
                    format!("Verrijking mislukt: {law_id}")
                }
            }
            JobType::DocumentConvert => format!(
                "Conversie mislukt: {}",
                str_field("target_path").unwrap_or("werkdocument")
            ),
            JobType::LawConvert => format!(
                "Conversie naar wet mislukt: {}",
                str_field("filename").unwrap_or("document")
            ),
            JobType::TrajectHarvest => format!(
                "Wet ophalen mislukt: {}",
                str_field("law_name")
                    .or_else(|| str_field("bwb_id"))
                    .unwrap_or(&job.law_id)
            ),
        };

        let mut task_payload = serde_json::json!({
            "error": "De verwerking duurde te lang of de worker is herstart; \
                      de job is afgebroken.",
        });
        for key in ["traject_ref", "law_id", "target_path", "filename", "bwb_id"] {
            if let Some(v) = str_field(key) {
                task_payload[key] = serde_json::json!(v);
            }
        }

        let result: Result<()> = async {
            let mut tx = pool.begin().await?;
            delete_blobs_for_job(&mut *tx, job.id).await?;
            create_task(
                &mut *tx,
                NewTask {
                    task_type: TaskType::JobFailed,
                    assignee_account_id: Some(assignee),
                    traject_id,
                    job_id: Some(job.id),
                    title,
                    payload: Some(task_payload),
                },
            )
            .await?;
            tx.commit().await?;
            Ok(())
        }
        .await;
        if let Err(e) = result {
            tracing::warn!(
                job_id = %job.id,
                error = %e,
                "job_failed-taak voor gereapte taak-flow-job aanmaken mislukt"
            );
        }
    }
    Ok(())
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
