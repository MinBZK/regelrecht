use serde_json::json;

use regelrecht_pipeline::job_queue::{self, CreateJobRequest};
use regelrecht_pipeline::models::JobType;
use regelrecht_pipeline::tasks::{self, BlobKind, NewTask, TaskStatus, TaskType};
use regelrecht_pipeline::test_utils::TestDb;

/// Maak een account + traject om FK's te vullen. tasks.assignee_account_id
/// verwijst naar accounts(id); trajects(created_by) idem.
async fn seed_account_and_traject(db: &TestDb) -> (uuid::Uuid, uuid::Uuid) {
    let (account_id,): (uuid::Uuid,) = sqlx::query_as(
        "INSERT INTO accounts (person_sub, email, name) \
         VALUES ('sub-test', 'tester@example.org', 'Test Persoon') RETURNING id",
    )
    .fetch_one(&db.pool)
    .await
    .unwrap();
    let (traject_id,): (uuid::Uuid,) = sqlx::query_as(
        "INSERT INTO trajects (name, created_by) VALUES ('Testtraject', $1) RETURNING id",
    )
    .bind(account_id)
    .fetch_one(&db.pool)
    .await
    .unwrap();
    (account_id, traject_id)
}

#[tokio::test]
async fn test_create_and_list_open_tasks() {
    let db = TestDb::new().await;
    let (account_id, traject_id) = seed_account_and_traject(&db).await;
    let job = job_queue::create_job(&db.pool, CreateJobRequest::new(JobType::Enrich, "test_wet"))
        .await
        .unwrap();

    let task = tasks::create_task(
        &db.pool,
        NewTask {
            task_type: TaskType::JobReview,
            assignee_account_id: Some(account_id),
            traject_id: Some(traject_id),
            job_id: Some(job.id),
            title: "Verrijking beoordelen: test_wet".into(),
            payload: Some(json!({"law_id": "test_wet"})),
        },
    )
    .await
    .unwrap();
    assert_eq!(task.status, TaskStatus::Open);

    let open = tasks::list_open_tasks_for_account(&db.pool, account_id)
        .await
        .unwrap();
    assert_eq!(open.len(), 1);
    assert_eq!(open[0].id, task.id);
    assert_eq!(open[0].title, "Verrijking beoordelen: test_wet");
}

#[tokio::test]
async fn test_resolve_task_only_by_assignee_and_only_once() {
    let db = TestDb::new().await;
    let (account_id, traject_id) = seed_account_and_traject(&db).await;
    let task = tasks::create_task(
        &db.pool,
        NewTask {
            task_type: TaskType::JobFailed,
            assignee_account_id: Some(account_id),
            traject_id: Some(traject_id),
            job_id: None,
            title: "Conversie mislukt".into(),
            payload: None,
        },
    )
    .await
    .unwrap();

    // Vreemd account mag niet resolven → None.
    let stranger = uuid::Uuid::new_v4();
    let denied = tasks::resolve_task(&db.pool, task.id, stranger, TaskStatus::Dismissed)
        .await
        .unwrap();
    assert!(denied.is_none());

    // Assignee wel.
    let resolved = tasks::resolve_task(&db.pool, task.id, account_id, TaskStatus::Dismissed)
        .await
        .unwrap()
        .expect("assignee mag resolven");
    assert_eq!(resolved.status, TaskStatus::Dismissed);
    assert_eq!(resolved.resolved_by, Some(account_id));
    assert!(resolved.resolved_at.is_some());

    // Tweede keer resolven → None (status is niet meer 'open').
    let again = tasks::resolve_task(&db.pool, task.id, account_id, TaskStatus::Approved)
        .await
        .unwrap();
    assert!(again.is_none());
}

#[tokio::test]
async fn test_blob_roundtrip_and_delete() {
    let db = TestDb::new().await;
    let job = job_queue::create_job(&db.pool, CreateJobRequest::new(JobType::Enrich, "test_wet"))
        .await
        .unwrap();

    tasks::insert_blob(
        &db.pool,
        job.id,
        BlobKind::Input,
        "laws/test_wet/law.yaml",
        "a: 1",
    )
    .await
    .unwrap();
    tasks::insert_blob(
        &db.pool,
        job.id,
        BlobKind::Result,
        "laws/test_wet/law.yaml",
        "a: 2",
    )
    .await
    .unwrap();

    let results = tasks::load_blobs(&db.pool, job.id, BlobKind::Result)
        .await
        .unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].content, "a: 2");

    tasks::delete_blobs_for_job(&db.pool, job.id).await.unwrap();
    let after = tasks::load_blobs(&db.pool, job.id, BlobKind::Result)
        .await
        .unwrap();
    assert!(after.is_empty());
}

#[tokio::test]
async fn test_cleanup_orphaned_blobs_keeps_open_task_blobs() {
    let db = TestDb::new().await;
    let (account_id, traject_id) = seed_account_and_traject(&db).await;
    let kept_job = job_queue::create_job(&db.pool, CreateJobRequest::new(JobType::Enrich, "wet_a"))
        .await
        .unwrap();
    let orphan_job =
        job_queue::create_job(&db.pool, CreateJobRequest::new(JobType::Enrich, "wet_b"))
            .await
            .unwrap();
    tasks::insert_blob(&db.pool, kept_job.id, BlobKind::Result, "p", "kept")
        .await
        .unwrap();
    tasks::insert_blob(&db.pool, orphan_job.id, BlobKind::Result, "p", "orphan")
        .await
        .unwrap();
    tasks::create_task(
        &db.pool,
        NewTask {
            task_type: TaskType::JobReview,
            assignee_account_id: Some(account_id),
            traject_id: Some(traject_id),
            job_id: Some(kept_job.id),
            title: "t".into(),
            payload: None,
        },
    )
    .await
    .unwrap();
    // Backdate beide blobs voorbij de 7-dagen grace zodat alleen het
    // open-taak-criterium onderscheidt.
    sqlx::query("UPDATE job_blobs SET created_at = now() - interval '8 days'")
        .execute(&db.pool)
        .await
        .unwrap();

    let removed = tasks::cleanup_orphaned_blobs(&db.pool).await.unwrap();
    assert_eq!(removed, 1);
    assert_eq!(
        tasks::load_blobs(&db.pool, kept_job.id, BlobKind::Result)
            .await
            .unwrap()
            .len(),
        1
    );
}
