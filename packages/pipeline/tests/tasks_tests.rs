use serde_json::json;

use regelrecht_pipeline::job_queue::{self, CreateJobRequest};
use regelrecht_pipeline::models::JobType;
use regelrecht_pipeline::tasks::{self, BlobKind, NewTask, TaskStatus, TaskType};
use regelrecht_pipeline::test_utils::TestDb;
use regelrecht_pipeline::worker::{
    fail_enrich_task_job, fail_enrich_task_job_with_retry, finish_enrich_task_job,
};

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

    // Detail-gating: vreemd account ziet de taak niet, assignee wel.
    let stranger_view = tasks::get_task_for_account(&db.pool, task.id, uuid::Uuid::new_v4())
        .await
        .unwrap();
    assert!(stranger_view.is_none());
    let own_view = tasks::get_task_for_account(&db.pool, task.id, account_id)
        .await
        .unwrap();
    assert_eq!(own_view.expect("assignee ziet eigen taak").id, task.id);

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

    // Resolve naar 'open' is geen afhandeling en wordt geweigerd.
    let task2 = tasks::create_task(
        &db.pool,
        NewTask {
            task_type: TaskType::JobFailed,
            assignee_account_id: Some(account_id),
            traject_id: Some(traject_id),
            job_id: None,
            title: "Nog een taak".into(),
            payload: None,
        },
    )
    .await
    .unwrap();
    let reopened = tasks::resolve_task(&db.pool, task2.id, account_id, TaskStatus::Open)
        .await
        .unwrap();
    assert!(reopened.is_none());
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

#[tokio::test]
async fn test_finish_enrich_task_job_creates_task_and_result_blobs() {
    let db = TestDb::new().await;
    let (account_id, traject_id) = seed_account_and_traject(&db).await;
    let _created = job_queue::create_job(
        &db.pool,
        CreateJobRequest::new(JobType::Enrich, "test_wet").with_payload(json!({
            "law_id": "test_wet",
            "yaml_path": "laws/test_wet/law.yaml",
            "provider": "claude",
            "requested_by": account_id,
            "deliver": "task",
            "traject_id": traject_id,
            "traject_ref": "testtraject-abcd1234",
            "source_etag": "\"etag-1\""
        })),
    )
    .await
    .unwrap();
    // Claim zodat complete_job ('processing' vereist) slaagt.
    let job = job_queue::claim_job(&db.pool, Some(JobType::Enrich))
        .await
        .unwrap()
        .unwrap();

    // Simuleer een geslaagde enrichment: een werkdirectory met output.
    let dir = tempfile::tempdir().unwrap();
    let law_abs = dir.path().join("laws/test_wet/law.yaml");
    tokio::fs::create_dir_all(law_abs.parent().unwrap())
        .await
        .unwrap();
    tokio::fs::write(&law_abs, "verrijkt: ja").await.unwrap();

    finish_enrich_task_job(
        &db.pool,
        &job,
        dir.path(),
        &[law_abs.clone()],
        Some(json!({"coverage_score": 1.0})),
    )
    .await
    .unwrap();

    // Job completed, result-blob + taak aanwezig.
    let done = job_queue::get_job(&db.pool, job.id).await.unwrap();
    assert_eq!(
        done.status,
        regelrecht_pipeline::models::JobStatus::Completed
    );
    let results = tasks::load_blobs(&db.pool, job.id, BlobKind::Result)
        .await
        .unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].path, "laws/test_wet/law.yaml");
    assert_eq!(results[0].content, "verrijkt: ja");
    let open = tasks::list_open_tasks_for_account(&db.pool, account_id)
        .await
        .unwrap();
    assert_eq!(open.len(), 1);
    assert_eq!(open[0].task_type, "job_review");
    assert_eq!(open[0].job_id, Some(job.id));
}

#[tokio::test]
async fn test_fail_enrich_task_job_creates_failed_task_and_cleans_input() {
    let db = TestDb::new().await;
    let (account_id, traject_id) = seed_account_and_traject(&db).await;
    let _created = job_queue::create_job(
        &db.pool,
        CreateJobRequest::new(JobType::Enrich, "test_wet").with_payload(json!({
            "law_id": "test_wet",
            "yaml_path": "laws/test_wet/law.yaml",
            "requested_by": account_id,
            "deliver": "task",
            "traject_id": traject_id
        })),
    )
    .await
    .unwrap();
    let job = job_queue::claim_job(&db.pool, Some(JobType::Enrich))
        .await
        .unwrap()
        .unwrap();
    tasks::insert_blob(
        &db.pool,
        job.id,
        BlobKind::Input,
        "laws/test_wet/law.yaml",
        "x",
    )
    .await
    .unwrap();

    fail_enrich_task_job(&db.pool, &job, "LLM produceerde niets")
        .await
        .unwrap();

    let failed = job_queue::get_job(&db.pool, job.id).await.unwrap();
    assert_eq!(
        failed.status,
        regelrecht_pipeline::models::JobStatus::Failed
    );
    assert!(tasks::load_blobs(&db.pool, job.id, BlobKind::Input)
        .await
        .unwrap()
        .is_empty());
    let open = tasks::list_open_tasks_for_account(&db.pool, account_id)
        .await
        .unwrap();
    assert_eq!(open.len(), 1);
    assert_eq!(open[0].task_type, "job_failed");
}

#[tokio::test]
async fn test_fail_with_retry_creates_task_only_when_attempts_exhausted() {
    let db = TestDb::new().await;
    let (account_id, traject_id) = seed_account_and_traject(&db).await;
    let _created = job_queue::create_job(
        &db.pool,
        CreateJobRequest::new(JobType::Enrich, "test_wet")
            .with_max_attempts(2)
            .with_payload(json!({
                "law_id": "test_wet",
                "yaml_path": "laws/test_wet/law.yaml",
                "requested_by": account_id,
                "deliver": "task",
                "traject_id": traject_id
            })),
    )
    .await
    .unwrap();
    let job = job_queue::claim_job(&db.pool, Some(JobType::Enrich))
        .await
        .unwrap()
        .unwrap();
    tasks::insert_blob(
        &db.pool,
        job.id,
        BlobKind::Input,
        "laws/test_wet/law.yaml",
        "x",
    )
    .await
    .unwrap();

    // Eerste fout: attempts over → pending met backoff, GEEN taak, blobs blijven.
    fail_enrich_task_job_with_retry(&db.pool, &job, "transiente fout")
        .await
        .unwrap();
    let after_first = job_queue::get_job(&db.pool, job.id).await.unwrap();
    assert_eq!(
        after_first.status,
        regelrecht_pipeline::models::JobStatus::Pending
    );
    assert!(tasks::list_open_tasks_for_account(&db.pool, account_id)
        .await
        .unwrap()
        .is_empty());
    assert_eq!(
        tasks::load_blobs(&db.pool, job.id, BlobKind::Input)
            .await
            .unwrap()
            .len(),
        1
    );

    // Backoff resetten zodat de job direct claimbaar is voor de tweede poging.
    sqlx::query("UPDATE jobs SET scheduled_at = NULL WHERE id = $1")
        .bind(job.id)
        .execute(&db.pool)
        .await
        .unwrap();
    let job = job_queue::claim_job(&db.pool, Some(JobType::Enrich))
        .await
        .unwrap()
        .unwrap();

    // Tweede fout: attempts uitgeput → Failed + taak + blobs weg.
    fail_enrich_task_job_with_retry(&db.pool, &job, "transiente fout")
        .await
        .unwrap();
    let after_second = job_queue::get_job(&db.pool, job.id).await.unwrap();
    assert_eq!(
        after_second.status,
        regelrecht_pipeline::models::JobStatus::Failed
    );
    let open = tasks::list_open_tasks_for_account(&db.pool, account_id)
        .await
        .unwrap();
    assert_eq!(open.len(), 1);
    assert_eq!(open[0].task_type, "job_failed");
    assert!(tasks::load_blobs(&db.pool, job.id, BlobKind::Input)
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn test_document_jobs_list_excludes_failed() {
    let db = TestDb::new().await;
    let req = CreateJobRequest::new(JobType::DocumentConvert, "doc:testtraject-abcd1234/a.md")
        .with_traject_ref("testtraject-abcd1234")
        .with_payload(json!({
            "upload_id": uuid::Uuid::new_v4(),
            "traject_id": uuid::Uuid::new_v4(),
            "traject_ref": "testtraject-abcd1234",
            "target_path": "a.md"
        }))
        .with_max_attempts(1);
    let job = job_queue::create_job(&db.pool, req).await.unwrap();
    let claimed = job_queue::claim_job(&db.pool, Some(JobType::DocumentConvert))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(claimed.id, job.id);
    job_queue::fail_job_terminal(&db.pool, job.id, None)
        .await
        .unwrap();

    let views = regelrecht_pipeline::document_convert::list_traject_document_jobs(
        &db.pool,
        "testtraject-abcd1234",
    )
    .await
    .unwrap();
    assert!(
        views.is_empty(),
        "failed jobs horen niet meer in de documentenlijst"
    );
}
