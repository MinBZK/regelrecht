mod common;

use pretty_assertions::assert_eq;
use serde_json::json;

use regelrecht_pipeline::job_queue::{self, CreateJobRequest};
use regelrecht_pipeline::models::{JobStatus, JobType, Priority};
use regelrecht_pipeline::PipelineError;

#[tokio::test]
async fn test_create_and_get_job() {
    let db = common::TestDb::new().await;

    let req = CreateJobRequest::new(JobType::Harvest, "BWBR0001840");
    let job = job_queue::create_job(&db.pool, req).await.unwrap();

    assert_eq!(job.job_type, JobType::Harvest);
    assert_eq!(job.law_id, "BWBR0001840");
    assert_eq!(job.status, JobStatus::Pending);
    assert_eq!(job.priority, 50);
    assert_eq!(job.attempts, 0);
    assert_eq!(job.max_attempts, 3);

    let fetched = job_queue::get_job(&db.pool, job.id).await.unwrap();
    assert_eq!(fetched.id, job.id);
}

#[tokio::test]
async fn test_create_job_with_payload() {
    let db = common::TestDb::new().await;

    let payload = json!({"url": "https://wetten.overheid.nl/BWBR0001840"});
    let req = CreateJobRequest::new(JobType::Harvest, "BWBR0001840").with_payload(payload.clone());
    let job = job_queue::create_job(&db.pool, req).await.unwrap();

    assert_eq!(job.payload, Some(payload));
}

#[tokio::test]
async fn test_max_attempts_clamped_to_minimum_1() {
    let db = common::TestDb::new().await;

    let req = CreateJobRequest::new(JobType::Harvest, "BWBR0001840").with_max_attempts(0);
    let job = job_queue::create_job(&db.pool, req).await.unwrap();
    assert_eq!(job.max_attempts, 1);

    let req = CreateJobRequest::new(JobType::Harvest, "BWBR0001841").with_max_attempts(-5);
    let job = job_queue::create_job(&db.pool, req).await.unwrap();
    assert_eq!(job.max_attempts, 1);
}

#[tokio::test]
async fn test_claim_job_priority_ordering() {
    let db = common::TestDb::new().await;

    let low = CreateJobRequest::new(JobType::Harvest, "low").with_priority(Priority::new(10));
    let high = CreateJobRequest::new(JobType::Harvest, "high").with_priority(Priority::new(90));
    let medium = CreateJobRequest::new(JobType::Harvest, "medium").with_priority(Priority::new(50));

    job_queue::create_job(&db.pool, low).await.unwrap();
    job_queue::create_job(&db.pool, high).await.unwrap();
    job_queue::create_job(&db.pool, medium).await.unwrap();

    let claimed = job_queue::claim_job(&db.pool, None).await.unwrap().unwrap();
    assert_eq!(claimed.law_id, "high");
    assert_eq!(claimed.status, JobStatus::Processing);
    assert_eq!(claimed.attempts, 1);

    let claimed = job_queue::claim_job(&db.pool, None).await.unwrap().unwrap();
    assert_eq!(claimed.law_id, "medium");

    let claimed = job_queue::claim_job(&db.pool, None).await.unwrap().unwrap();
    assert_eq!(claimed.law_id, "low");

    let none = job_queue::claim_job(&db.pool, None).await.unwrap();
    assert!(none.is_none());
}

#[tokio::test]
async fn test_claim_job_by_type() {
    let db = common::TestDb::new().await;

    let harvest = CreateJobRequest::new(JobType::Harvest, "law1");
    let enrich = CreateJobRequest::new(JobType::Enrich, "law2");

    job_queue::create_job(&db.pool, harvest).await.unwrap();
    job_queue::create_job(&db.pool, enrich).await.unwrap();

    let claimed = job_queue::claim_job(&db.pool, Some(JobType::Enrich))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(claimed.law_id, "law2");
    assert_eq!(claimed.job_type, JobType::Enrich);

    let none = job_queue::claim_job(&db.pool, Some(JobType::Enrich))
        .await
        .unwrap();
    assert!(none.is_none());

    let claimed = job_queue::claim_job(&db.pool, Some(JobType::Harvest))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(claimed.law_id, "law1");
}

#[tokio::test]
async fn test_complete_job() {
    let db = common::TestDb::new().await;

    let req = CreateJobRequest::new(JobType::Harvest, "BWBR0001840");
    let job = job_queue::create_job(&db.pool, req).await.unwrap();

    let claimed = job_queue::claim_job(&db.pool, None).await.unwrap().unwrap();
    assert_eq!(claimed.id, job.id);

    let result = json!({"articles": 15});
    let completed = job_queue::complete_job(&db.pool, job.id, Some(result.clone()))
        .await
        .unwrap();
    assert_eq!(completed.status, JobStatus::Completed);
    assert_eq!(completed.result, Some(result));
    assert!(completed.completed_at.is_some());
}

#[tokio::test]
async fn test_complete_job_not_processing() {
    let db = common::TestDb::new().await;

    let req = CreateJobRequest::new(JobType::Harvest, "BWBR0001840");
    let job = job_queue::create_job(&db.pool, req).await.unwrap();

    let result = job_queue::complete_job(&db.pool, job.id, None).await;
    assert!(matches!(result, Err(PipelineError::JobNotProcessing(_))));
}

#[tokio::test]
async fn test_fail_job_with_retry() {
    let db = common::TestDb::new().await;

    let req = CreateJobRequest::new(JobType::Harvest, "BWBR0001840").with_max_attempts(3);
    let job = job_queue::create_job(&db.pool, req).await.unwrap();

    let claimed = job_queue::claim_job(&db.pool, None).await.unwrap().unwrap();
    assert_eq!(claimed.attempts, 1);
    let failed = job_queue::fail_job(&db.pool, job.id, Some(json!({"error": "timeout"})))
        .await
        .unwrap();
    assert_eq!(failed.status, JobStatus::Pending);

    let claimed = job_queue::claim_job(&db.pool, None).await.unwrap().unwrap();
    assert_eq!(claimed.attempts, 2);
    let failed = job_queue::fail_job(&db.pool, job.id, Some(json!({"error": "timeout again"})))
        .await
        .unwrap();
    assert_eq!(failed.status, JobStatus::Pending);

    let claimed = job_queue::claim_job(&db.pool, None).await.unwrap().unwrap();
    assert_eq!(claimed.attempts, 3);
    let failed = job_queue::fail_job(&db.pool, job.id, Some(json!({"error": "gave up"})))
        .await
        .unwrap();
    assert_eq!(failed.status, JobStatus::Failed);
    assert!(failed.completed_at.is_some());

    let none = job_queue::claim_job(&db.pool, None).await.unwrap();
    assert!(none.is_none());
}

#[tokio::test]
async fn test_list_jobs() {
    let db = common::TestDb::new().await;

    let req1 = CreateJobRequest::new(JobType::Harvest, "law1");
    let req2 = CreateJobRequest::new(JobType::Enrich, "law2");
    job_queue::create_job(&db.pool, req1).await.unwrap();
    job_queue::create_job(&db.pool, req2).await.unwrap();

    let all = job_queue::list_jobs(&db.pool, None).await.unwrap();
    assert_eq!(all.len(), 2);

    job_queue::claim_job(&db.pool, None).await.unwrap();

    let pending = job_queue::list_jobs(&db.pool, Some(JobStatus::Pending))
        .await
        .unwrap();
    assert_eq!(pending.len(), 1);

    let processing = job_queue::list_jobs(&db.pool, Some(JobStatus::Processing))
        .await
        .unwrap();
    assert_eq!(processing.len(), 1);
}

#[tokio::test]
async fn test_concurrent_claim_safety() {
    let db = common::TestDb::new().await;

    let req = CreateJobRequest::new(JobType::Harvest, "contested");
    job_queue::create_job(&db.pool, req).await.unwrap();

    let (r1, r2) = tokio::join!(
        job_queue::claim_job(&db.pool, None),
        job_queue::claim_job(&db.pool, None)
    );

    let claimed_count = [r1.unwrap(), r2.unwrap()]
        .iter()
        .filter(|j| j.is_some())
        .count();

    assert_eq!(claimed_count, 1);
}
