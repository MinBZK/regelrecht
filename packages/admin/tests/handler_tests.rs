#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::routing::{get, post};
use axum::Router;
use http_body_util::BodyExt;
use pretty_assertions::assert_eq;
use serde_json::Value;
use tower::ServiceExt;

use regelrecht_admin::config::AppConfig;
use regelrecht_admin::handlers;
use regelrecht_admin::metrics;
use regelrecht_admin::metrics::fetch_metrics;
use regelrecht_admin::state::AppState;
use regelrecht_pipeline::job_queue::{self, CreateJobRequest};
use regelrecht_pipeline::JobType;

fn test_app(pool: sqlx::PgPool) -> Router {
    let state = AppState {
        pool,
        oidc_client: None,
        end_session_url: None,
        config: Arc::new(AppConfig {
            oidc: None,
            base_url: None,
            api_key: None,
            api_key_hash: None,
            metrics_token_hash: None,
            test_sso: false,
        }),
        metrics_cache: Arc::new(metrics::new_cache()),
        http_client: reqwest::Client::new(),
        corpus: Arc::new(tokio::sync::RwLock::new(
            regelrecht_admin::state::CorpusState::empty(),
        )),
    };
    Router::new()
        .route("/api/law_entries", get(handlers::list_law_entries))
        .route("/api/jobs", get(handlers::list_jobs))
        .route("/api/harvest-jobs", post(handlers::create_harvest_job))
        .with_state(state)
}

async fn body_json(response: axum::http::Response<Body>) -> Value {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

// --- create_harvest_job ---

#[tokio::test]
async fn create_harvest_job_returns_created() {
    let db = common::TestDb::new().await;
    let app = test_app(db.pool.clone());

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/harvest-jobs")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"bwb_id": "BWBR0018451"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);

    let json = body_json(response).await;
    assert_eq!(json["law_id"], "BWBR0018451");
    assert!(json["job_id"].as_str().is_some());
}

#[tokio::test]
async fn create_harvest_job_links_harvest_job_id() {
    let db = common::TestDb::new().await;
    let app = test_app(db.pool.clone());

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/harvest-jobs")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"bwb_id": "BWBR0018451"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
    let json = body_json(response).await;
    let job_id: uuid::Uuid = json["job_id"].as_str().unwrap().parse().unwrap();

    // Verify the law entry has the harvest_job_id linked
    let row: (Option<uuid::Uuid>,) =
        sqlx::query_as("SELECT harvest_job_id FROM law_entries WHERE law_id = $1")
            .bind("BWBR0018451")
            .fetch_one(&db.pool)
            .await
            .unwrap();

    assert_eq!(row.0, Some(job_id));
}

#[tokio::test]
async fn create_harvest_job_rejects_duplicate() {
    let db = common::TestDb::new().await;
    let pool = db.pool.clone();
    let app = test_app(pool.clone());

    // First request succeeds
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/harvest-jobs")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"bwb_id": "BWBR0018451"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    // Second request for same law_id should be rejected
    let app2 = test_app(pool.clone());
    let response = app2
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/harvest-jobs")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"bwb_id": "BWBR0018451"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn create_harvest_job_allows_after_completion() {
    let db = common::TestDb::new().await;
    let pool = db.pool.clone();
    let app = test_app(pool.clone());

    // Create first job
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/harvest-jobs")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"bwb_id": "BWBR0018451"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let json = body_json(response).await;
    let job_id: uuid::Uuid = json["job_id"].as_str().unwrap().parse().unwrap();

    // Simulate job completion: claim then complete
    let claimed = job_queue::claim_job(&pool, Some(JobType::Harvest))
        .await
        .unwrap()
        .unwrap();
    job_queue::complete_job(&pool, claimed.id, None)
        .await
        .unwrap();
    assert_eq!(claimed.id, job_id);

    // Now creating another harvest job for the same law_id should succeed
    let app2 = test_app(pool.clone());
    let response = app2
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/harvest-jobs")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"bwb_id": "BWBR0018451"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
}

#[tokio::test]
async fn create_harvest_job_rejects_empty_bwb_id() {
    let db = common::TestDb::new().await;
    let app = test_app(db.pool.clone());

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/harvest-jobs")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"bwb_id": "  "}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn create_harvest_job_with_priority_and_date() {
    let db = common::TestDb::new().await;
    let app = test_app(db.pool.clone());

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/harvest-jobs")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"bwb_id": "BWBR0018451", "priority": 80, "date": "2026-01-01"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);

    // Verify priority was set on the job
    let job: (i32,) = sqlx::query_as("SELECT priority FROM jobs WHERE law_id = $1")
        .bind("BWBR0018451")
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(job.0, 80);
}

#[tokio::test]
async fn create_harvest_job_rejects_invalid_bwb_id() {
    let db = common::TestDb::new().await;
    let app = test_app(db.pool.clone());

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/harvest-jobs")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"bwb_id": "INVALID"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn create_harvest_job_rejects_invalid_date() {
    let db = common::TestDb::new().await;
    let app = test_app(db.pool.clone());

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/harvest-jobs")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"bwb_id": "BWBR0018451", "date": "not-a-date"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn create_harvest_job_rejects_impossible_date() {
    let db = common::TestDb::new().await;
    let app = test_app(db.pool.clone());

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/harvest-jobs")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"bwb_id": "BWBR0018451", "date": "2025-13-01"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

// --- list endpoints ---

#[tokio::test]
async fn list_jobs_empty() {
    let db = common::TestDb::new().await;
    let app = test_app(db.pool.clone());

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/jobs")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = body_json(response).await;
    assert_eq!(json["total"], 0);
    assert_eq!(json["data"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn list_law_entries_empty() {
    let db = common::TestDb::new().await;
    let app = test_app(db.pool.clone());

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/law_entries")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = body_json(response).await;
    assert_eq!(json["total"], 0);
}

#[tokio::test]
async fn list_jobs_after_creation() {
    let db = common::TestDb::new().await;
    let pool = db.pool.clone();

    // Create a job via the handler
    let app = test_app(pool.clone());
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/harvest-jobs")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"bwb_id": "BWBR0018451"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    // List jobs
    let app2 = test_app(pool.clone());
    let response = app2
        .oneshot(
            Request::builder()
                .uri("/api/jobs")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = body_json(response).await;
    assert_eq!(json["total"], 1);
    assert_eq!(json["data"][0]["law_id"], "BWBR0018451");
}

// --- fetch_metrics ---

#[tokio::test]
async fn fetch_metrics_on_empty_db() {
    let db = common::TestDb::new().await;
    let snapshot = fetch_metrics(&db.pool).await.unwrap();

    assert!(snapshot.jobs_by_status.is_empty());
    assert!(snapshot.laws_by_status.is_empty());
    assert_eq!(snapshot.avg_job_duration_secs, None);
}

#[tokio::test]
async fn fetch_metrics_with_only_pending_jobs() {
    let db = common::TestDb::new().await;
    let pool = db.pool.clone();

    // Create a pending job but don't complete it - AVG query returns NULL
    // because no jobs match status='completed'. This is the scenario that
    // triggered a NUMERIC vs float8 mismatch in production.
    let req = CreateJobRequest::new(JobType::Harvest, "BWBR0018451");
    job_queue::create_job(&pool, req).await.unwrap();

    let snapshot = fetch_metrics(&pool).await.unwrap();

    assert_eq!(snapshot.avg_job_duration_secs, None);
    assert!(
        snapshot
            .jobs_by_status
            .iter()
            .any(|(s, c)| s == "pending" && *c == 1),
        "should have 1 pending job"
    );
}

#[tokio::test]
async fn fetch_metrics_avg_duration_with_completed_jobs() {
    let db = common::TestDb::new().await;
    let pool = db.pool.clone();

    // Create and complete a job so AVG(EXTRACT(EPOCH ...)) returns a value.
    let req = CreateJobRequest::new(JobType::Harvest, "BWBR0018451");
    job_queue::create_job(&pool, req).await.unwrap();
    let job = job_queue::claim_job(&pool, Some(JobType::Harvest))
        .await
        .unwrap()
        .unwrap();
    job_queue::complete_job(&pool, job.id, None).await.unwrap();

    let snapshot = fetch_metrics(&pool).await.unwrap();

    assert!(
        snapshot.avg_job_duration_secs.is_some(),
        "should have avg duration for completed jobs"
    );
    assert!(
        snapshot
            .jobs_by_status
            .iter()
            .any(|(s, c)| s == "completed" && *c == 1),
        "should have 1 completed job"
    );
}
