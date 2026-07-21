//! DB-integration tests for the chunked-enrichment completion contract:
//! `worker::complete_enrich_success_tx` must complete the job and create the
//! continuation job in ONE transaction, respecting the partial unique index
//! `idx_unique_active_enrich_job`, and drive the `enriching` → `enriched`
//! status machine without ever stranding a law in `enriching` without an
//! active/pending job.

use serde_json::json;

use regelrecht_pipeline::enrich::{EnrichPayload, EnrichResult};
use regelrecht_pipeline::job_queue::{self, CreateJobRequest};
use regelrecht_pipeline::models::{Job, JobStatus, JobType, LawStatusValue, Priority};
use regelrecht_pipeline::test_utils::TestDb;
use regelrecht_pipeline::{law_status, worker};

const LAW_ID: &str = "BWBR0099999";
const YAML_PATH: &str = "regulation/nl/wet/test_wet/2025-01-01.yaml";

fn payload(provider: &str) -> EnrichPayload {
    EnrichPayload {
        law_id: LAW_ID.into(),
        yaml_path: YAML_PATH.into(),
        provider: Some(provider.into()),
        depth: Some(1),
        requested_by: None,
        deliver: None,
        traject_id: None,
        traject_ref: None,
        source_etag: None,
        new_law: None,
        chunk_articles: None,
        skip_mvt: None,
    }
}

fn chunk_result(provider: &str, law_complete: bool, enrich_cursor: usize) -> EnrichResult {
    EnrichResult {
        law_id: LAW_ID.into(),
        yaml_path: YAML_PATH.into(),
        articles_total: 30,
        articles_with_machine_readable: enrich_cursor.min(30),
        coverage_score: 0.5,
        provider: provider.into(),
        branch: format!("enrich/{provider}"),
        related_legislation: Vec::new(),
        untranslatables: Vec::new(),
        law_complete,
        enrich_cursor,
    }
}

/// Create + claim an enrich job for `provider`, with the law in `enriching`.
async fn setup_processing_enrich_job(db: &TestDb, provider: &str) -> Job {
    law_status::upsert_law(&db.pool, LAW_ID, Some("Testwet"), None)
        .await
        .unwrap();
    law_status::update_status(&db.pool, LAW_ID, LawStatusValue::Enriching)
        .await
        .unwrap();

    let req = CreateJobRequest::new(JobType::Enrich, LAW_ID)
        .with_priority(Priority::new(70))
        .with_payload(serde_json::to_value(payload(provider)).unwrap());
    let job = job_queue::create_enrich_job_if_not_exists(&db.pool, req)
        .await
        .unwrap()
        .expect("job created");
    let claimed = job_queue::claim_job(&db.pool, Some(JobType::Enrich))
        .await
        .unwrap()
        .expect("job claimable");
    assert_eq!(claimed.id, job.id);
    claimed
}

#[tokio::test]
async fn incomplete_chunk_creates_continuation_in_same_tx_and_keeps_enriching() {
    let db = TestDb::new().await;
    let job = setup_processing_enrich_job(&db, "opencode").await;

    let result = chunk_result("opencode", false, 15);
    let continuation = worker::complete_enrich_success_tx(
        &db.pool,
        &job,
        &payload("opencode"),
        &result,
        Some(serde_json::to_value(&result).unwrap()),
    )
    .await
    .unwrap()
    .expect("continuation job created");

    // The original job is completed…
    let done = job_queue::get_job(&db.pool, job.id).await.unwrap();
    assert_eq!(done.status, JobStatus::Completed);

    // …the law stays `enriching` (NOT enriched)…
    let law = law_status::get_law(&db.pool, LAW_ID).await.unwrap();
    assert_eq!(law.status, LawStatusValue::Enriching);

    // …and the continuation is pending in the same scope with the same
    // provider/priority/depth and a bare payload (no chunk fields — the next
    // window is recomputed from the cursor on the branch).
    assert_eq!(continuation.status, JobStatus::Pending);
    assert_eq!(continuation.law_id, LAW_ID);
    assert_eq!(continuation.priority, job.priority);
    let cont_payload: EnrichPayload =
        serde_json::from_value(continuation.payload.clone().unwrap()).unwrap();
    assert_eq!(cont_payload.provider.as_deref(), Some("opencode"));
    assert_eq!(cont_payload.depth, Some(1));
    assert!(cont_payload.chunk_articles.is_none());
    assert!(cont_payload.skip_mvt.is_none());

    // The unique-active index still holds: a second enrich request for the
    // same law/provider/scope is refused while the continuation is pending.
    let dup = CreateJobRequest::new(JobType::Enrich, LAW_ID)
        .with_payload(serde_json::to_value(payload("opencode")).unwrap());
    let blocked = job_queue::create_enrich_job_if_not_exists(&db.pool, dup)
        .await
        .unwrap();
    assert!(
        blocked.is_none(),
        "active continuation must block duplicates"
    );
}

#[tokio::test]
async fn final_chunk_marks_law_enriched_without_continuation() {
    let db = TestDb::new().await;
    let job = setup_processing_enrich_job(&db, "opencode").await;

    let result = chunk_result("opencode", true, 30);
    let continuation = worker::complete_enrich_success_tx(
        &db.pool,
        &job,
        &payload("opencode"),
        &result,
        Some(serde_json::to_value(&result).unwrap()),
    )
    .await
    .unwrap();
    assert!(continuation.is_none(), "complete law needs no continuation");

    let done = job_queue::get_job(&db.pool, job.id).await.unwrap();
    assert_eq!(done.status, JobStatus::Completed);
    let law = law_status::get_law(&db.pool, LAW_ID).await.unwrap();
    assert_eq!(law.status, LawStatusValue::Enriched);
}

#[tokio::test]
async fn chunk_loop_transitions_enriching_to_enriched_across_runs() {
    // Full loop over the queue: chunk 1 (incomplete) → continuation claimable
    // → chunk 2 (complete) → `enriched`. No moment leaves the law `enriching`
    // without an active/pending job.
    let db = TestDb::new().await;
    let job1 = setup_processing_enrich_job(&db, "claude").await;

    let result1 = chunk_result("claude", false, 15);
    let continuation = worker::complete_enrich_success_tx(
        &db.pool,
        &job1,
        &payload("claude"),
        &result1,
        Some(serde_json::to_value(&result1).unwrap()),
    )
    .await
    .unwrap()
    .expect("continuation created");
    assert_eq!(
        law_status::get_law(&db.pool, LAW_ID).await.unwrap().status,
        LawStatusValue::Enriching
    );

    // The continuation is the next claimable job for this law.
    let job2 = job_queue::claim_job(&db.pool, Some(JobType::Enrich))
        .await
        .unwrap()
        .expect("continuation claimable");
    assert_eq!(job2.id, continuation.id);

    let result2 = chunk_result("claude", true, 30);
    let none = worker::complete_enrich_success_tx(
        &db.pool,
        &job2,
        &payload("claude"),
        &result2,
        Some(serde_json::to_value(&result2).unwrap()),
    )
    .await
    .unwrap();
    assert!(none.is_none());

    assert_eq!(
        law_status::get_law(&db.pool, LAW_ID).await.unwrap().status,
        LawStatusValue::Enriched
    );
    assert_eq!(
        job_queue::get_job(&db.pool, job2.id).await.unwrap().status,
        JobStatus::Completed
    );
}

#[tokio::test]
async fn continuation_inherits_traject_scope() {
    // A traject-scoped enrich (non-task-flow) continues within the same
    // uniqueness scope of `idx_unique_active_enrich_job`.
    let db = TestDb::new().await;
    law_status::upsert_law(&db.pool, LAW_ID, Some("Testwet"), None)
        .await
        .unwrap();
    law_status::update_status(&db.pool, LAW_ID, LawStatusValue::Enriching)
        .await
        .unwrap();

    let mut traject_payload = payload("opencode");
    traject_payload.traject_ref = Some("testtraject-abcd1234".into());
    let req = CreateJobRequest::new(JobType::Enrich, LAW_ID)
        .with_traject_ref("testtraject-abcd1234")
        .with_payload(serde_json::to_value(&traject_payload).unwrap());
    job_queue::create_enrich_job_if_not_exists(&db.pool, req)
        .await
        .unwrap()
        .expect("job created");
    let job = job_queue::claim_job(&db.pool, Some(JobType::Enrich))
        .await
        .unwrap()
        .expect("claimable");

    let result = chunk_result("opencode", false, 15);
    let continuation =
        worker::complete_enrich_success_tx(&db.pool, &job, &traject_payload, &result, None)
            .await
            .unwrap()
            .expect("continuation created");
    let cont_payload: EnrichPayload =
        serde_json::from_value(continuation.payload.clone().unwrap()).unwrap();
    assert_eq!(
        cont_payload.traject_ref.as_deref(),
        Some("testtraject-abcd1234")
    );

    // Same scope is blocked; the corpus-wide scope (traject_ref NULL) is not.
    let same_scope = CreateJobRequest::new(JobType::Enrich, LAW_ID)
        .with_traject_ref("testtraject-abcd1234")
        .with_payload(serde_json::to_value(&traject_payload).unwrap());
    assert!(
        job_queue::create_enrich_job_if_not_exists(&db.pool, same_scope)
            .await
            .unwrap()
            .is_none()
    );
    let corpus_scope = CreateJobRequest::new(JobType::Enrich, LAW_ID)
        .with_payload(serde_json::to_value(payload("opencode")).unwrap());
    assert!(
        job_queue::create_enrich_job_if_not_exists(&db.pool, corpus_scope)
            .await
            .unwrap()
            .is_some()
    );
}

#[tokio::test]
async fn legacy_result_json_counts_as_complete() {
    // A `jobs.result` JSON without `law_complete` (pre-chunking) deserializes
    // as complete: replaying it through the completion path marks the law
    // `enriched` and creates no continuation.
    let db = TestDb::new().await;
    let job = setup_processing_enrich_job(&db, "opencode").await;

    let legacy_json = json!({
        "law_id": LAW_ID,
        "yaml_path": YAML_PATH,
        "articles_total": 10,
        "articles_with_machine_readable": 10,
        "coverage_score": 1.0,
        "provider": "opencode",
        "branch": "enrich/opencode",
    });
    let result: EnrichResult = serde_json::from_value(legacy_json.clone()).unwrap();
    assert!(result.law_complete);

    let continuation = worker::complete_enrich_success_tx(
        &db.pool,
        &job,
        &payload("opencode"),
        &result,
        Some(legacy_json),
    )
    .await
    .unwrap();
    assert!(continuation.is_none());
    assert_eq!(
        law_status::get_law(&db.pool, LAW_ID).await.unwrap().status,
        LawStatusValue::Enriched
    );
}
