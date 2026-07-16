//! DB-backed tests for `cancel_traject_document_job`.
//!
//! Cancelling is destructive (it deletes the job row and its source upload) and
//! the two things that keep it safe live entirely in the SQL: the `traject_ref`
//! scoping, which stops a member cancelling another traject's job by id, and the
//! `status <> 'completed'` guard. Neither is visible from the call site, so a
//! refactor could drop either without any caller noticing - hence these.

use serde_json::json;

use regelrecht_pipeline::document_convert::cancel_traject_document_job;
use regelrecht_pipeline::job_queue::{self, CreateJobRequest};
use regelrecht_pipeline::models::JobType;
use regelrecht_pipeline::test_utils::TestDb;

/// A document-convert job with its source upload, the way the upload handler
/// enqueues one. Returns (job_id, upload_id).
async fn seed_convert_job(
    db: &TestDb,
    traject_ref: &str,
    target_path: &str,
) -> (uuid::Uuid, uuid::Uuid) {
    let (upload_id,): (uuid::Uuid,) = sqlx::query_as(
        "INSERT INTO document_uploads (traject_ref, filename, content_type, bytes) \
         VALUES ($1, 'bron.pdf', 'application/pdf', $2) RETURNING id",
    )
    .bind(traject_ref)
    .bind(vec![1u8, 2, 3])
    .fetch_one(&db.pool)
    .await
    .expect("insert upload");

    let job = job_queue::create_job(
        &db.pool,
        CreateJobRequest::new(
            JobType::DocumentConvert,
            format!("doc:{traject_ref}/{target_path}"),
        )
        .with_traject_ref(traject_ref)
        .with_payload(json!({ "upload_id": upload_id, "target_path": target_path })),
    )
    .await
    .expect("create job");

    (job.id, upload_id)
}

async fn job_exists(db: &TestDb, job_id: uuid::Uuid) -> bool {
    let row: Option<(uuid::Uuid,)> = sqlx::query_as("SELECT id FROM jobs WHERE id = $1")
        .bind(job_id)
        .fetch_optional(&db.pool)
        .await
        .expect("select job");
    row.is_some()
}

async fn upload_exists(db: &TestDb, upload_id: uuid::Uuid) -> bool {
    let row: Option<(uuid::Uuid,)> =
        sqlx::query_as("SELECT id FROM document_uploads WHERE id = $1")
            .bind(upload_id)
            .fetch_optional(&db.pool)
            .await
            .expect("select upload");
    row.is_some()
}

#[tokio::test]
async fn cancels_a_pending_job_and_removes_its_source_upload() {
    let db = TestDb::new().await;
    let (job_id, upload_id) = seed_convert_job(&db, "traject-a", "verslag.md").await;

    let cancelled = cancel_traject_document_job(&db.pool, "traject-a", job_id)
        .await
        .expect("cancel");

    assert!(cancelled, "cancelling a pending job of its own traject");
    assert!(!job_exists(&db, job_id).await, "job row is gone");
    assert!(
        !upload_exists(&db, upload_id).await,
        "the now-orphaned upload is gone too"
    );
}

#[tokio::test]
async fn refuses_a_job_belonging_to_another_traject() {
    let db = TestDb::new().await;
    let (job_id, upload_id) = seed_convert_job(&db, "traject-a", "verslag.md").await;

    // Same job id, a traject the caller is a member of. Membership is checked in
    // the handler; this scoping is what makes that check mean anything.
    let cancelled = cancel_traject_document_job(&db.pool, "traject-b", job_id)
        .await
        .expect("cancel");

    assert!(!cancelled, "another traject's job is not cancellable");
    assert!(job_exists(&db, job_id).await, "job row survives");
    assert!(upload_exists(&db, upload_id).await, "upload survives");
}

#[tokio::test]
async fn refuses_a_completed_job() {
    let db = TestDb::new().await;
    let (job_id, upload_id) = seed_convert_job(&db, "traject-a", "verslag.md").await;
    sqlx::query("UPDATE jobs SET status = 'completed' WHERE id = $1")
        .bind(job_id)
        .execute(&db.pool)
        .await
        .expect("complete job");

    let cancelled = cancel_traject_document_job(&db.pool, "traject-a", job_id)
        .await
        .expect("cancel");

    // A completed conversion already wrote its .md; deleting the job (and its
    // upload) would be cancelling something that already happened.
    assert!(!cancelled, "a completed job is not cancellable");
    assert!(job_exists(&db, job_id).await, "job row survives");
    assert!(upload_exists(&db, upload_id).await, "upload survives");
}

#[tokio::test]
async fn reports_no_match_for_an_unknown_job() {
    let db = TestDb::new().await;

    // The handler answers 204 either way - a double-click must not 500 or
    // report a second cancel that didn't happen.
    let cancelled = cancel_traject_document_job(&db.pool, "traject-a", uuid::Uuid::new_v4())
        .await
        .expect("cancel");

    assert!(!cancelled, "nothing matched, nothing removed");
}
