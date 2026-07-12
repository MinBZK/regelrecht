#![allow(clippy::unwrap_used, clippy::expect_used)]

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
use regelrecht_pipeline::test_utils::TestDb;
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
        .route("/api/untranslatables", get(handlers::list_untranslatables))
        .route("/api/dashboard-stats", get(handlers::dashboard_stats))
        .route("/api/harvest-jobs", post(handlers::create_harvest_job))
        .with_state(state)
}

/// Seed one untranslatable row for `law_id`/`provider`. Creates the law_entry
/// (so the join has a `law_name`) and an enrich job (to satisfy the FK) on first
/// use, then inserts the row. Returns nothing — tests query via the handler.
async fn seed_untranslatable(
    pool: &sqlx::PgPool,
    law_id: &str,
    law_name: &str,
    provider: &str,
    construct: &str,
    accepted: bool,
) {
    regelrecht_pipeline::law_status::upsert_law(pool, law_id, Some(law_name), None)
        .await
        .unwrap();
    let job = job_queue::create_job(pool, CreateJobRequest::new(JobType::Enrich, law_id))
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO untranslatables \
         (law_id, enrich_job_id, provider, article, construct, reason, accepted) \
         VALUES ($1, $2, $3, '1', $4, 'reason', $5)",
    )
    .bind(law_id)
    .bind(job.id)
    .bind(provider)
    .bind(construct)
    .bind(accepted)
    .execute(pool)
    .await
    .unwrap();
}

async fn get_untranslatables(pool: &sqlx::PgPool, query: &str) -> Value {
    let app = test_app(pool.clone());
    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/untranslatables{query}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    body_json(response).await
}

// --- list_untranslatables ---

#[tokio::test]
async fn list_untranslatables_empty() {
    let db = TestDb::new().await;
    let json = get_untranslatables(&db.pool, "").await;
    assert_eq!(json["total"], 0);
    assert_eq!(json["data"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn list_untranslatables_returns_rows_with_law_name() {
    let db = TestDb::new().await;
    seed_untranslatable(
        &db.pool, "test_law", "Test Law", "opencode", "rounding", false,
    )
    .await;
    seed_untranslatable(
        &db.pool,
        "test_law",
        "Test Law",
        "opencode",
        "table_lookup",
        true,
    )
    .await;

    let json = get_untranslatables(&db.pool, "").await;
    assert_eq!(json["total"], 2);
    // law_name comes from the LEFT JOIN on law_entries.
    assert_eq!(json["data"][0]["law_name"], "Test Law");
}

#[tokio::test]
async fn list_untranslatables_filters_by_accepted() {
    let db = TestDb::new().await;
    seed_untranslatable(
        &db.pool, "test_law", "Test Law", "opencode", "rounding", false,
    )
    .await;
    seed_untranslatable(
        &db.pool,
        "test_law",
        "Test Law",
        "opencode",
        "table_lookup",
        true,
    )
    .await;

    let json = get_untranslatables(&db.pool, "?accepted=true").await;
    assert_eq!(json["total"], 1);
    assert_eq!(json["data"][0]["construct"], "table_lookup");
}

#[tokio::test]
async fn list_untranslatables_filters_by_law_id_partial() {
    let db = TestDb::new().await;
    seed_untranslatable(
        &db.pool,
        "wet_alpha",
        "Wet Alpha",
        "opencode",
        "rounding",
        false,
    )
    .await;
    seed_untranslatable(
        &db.pool, "wet_beta", "Wet Beta", "opencode", "rounding", false,
    )
    .await;

    let json = get_untranslatables(&db.pool, "?law_id=alpha").await;
    assert_eq!(json["total"], 1);
    assert_eq!(json["data"][0]["law_id"], "wet_alpha");
}

#[tokio::test]
async fn list_untranslatables_filters_by_provider() {
    let db = TestDb::new().await;
    seed_untranslatable(
        &db.pool, "test_law", "Test Law", "opencode", "rounding", false,
    )
    .await;
    seed_untranslatable(
        &db.pool, "test_law", "Test Law", "claude", "rounding", false,
    )
    .await;

    let json = get_untranslatables(&db.pool, "?provider=claude").await;
    assert_eq!(json["total"], 1);
    assert_eq!(json["data"][0]["provider"], "claude");
}

#[tokio::test]
async fn list_untranslatables_rejects_invalid_sort() {
    let db = TestDb::new().await;
    let app = test_app(db.pool.clone());
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/untranslatables?sort=bogus")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn list_untranslatables_pagination() {
    let db = TestDb::new().await;
    for construct in ["a", "b", "c"] {
        seed_untranslatable(
            &db.pool, "test_law", "Test Law", "opencode", construct, false,
        )
        .await;
    }

    let json = get_untranslatables(&db.pool, "?limit=1&offset=1").await;
    assert_eq!(json["total"], 3);
    assert_eq!(json["data"].as_array().unwrap().len(), 1);
}

async fn body_json(response: axum::http::Response<Body>) -> Value {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

// --- create_harvest_job ---

#[tokio::test]
async fn create_harvest_job_returns_created() {
    let db = TestDb::new().await;
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
    let db = TestDb::new().await;
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
    let db = TestDb::new().await;
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
    let db = TestDb::new().await;
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
    let db = TestDb::new().await;
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
    let db = TestDb::new().await;
    let app = test_app(db.pool.clone());

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/harvest-jobs")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"bwb_id": "BWBR0018451", "priority": 80, "date": "2024-01-01"}"#,
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
    let db = TestDb::new().await;
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
    let db = TestDb::new().await;
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
    let db = TestDb::new().await;
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

#[tokio::test]
async fn create_harvest_job_rejects_exhausted_law() {
    let db = TestDb::new().await;
    let app = test_app(db.pool.clone());

    regelrecht_pipeline::law_status::upsert_law(&db.pool, "BWBR0018451", None, None)
        .await
        .unwrap();
    regelrecht_pipeline::law_status::update_status(
        &db.pool,
        "BWBR0018451",
        regelrecht_pipeline::LawStatusValue::HarvestExhausted,
    )
    .await
    .unwrap();

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

    assert_eq!(response.status(), StatusCode::CONFLICT);
}

// --- list endpoints ---

#[tokio::test]
async fn list_jobs_empty() {
    let db = TestDb::new().await;
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
    let db = TestDb::new().await;
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
    let db = TestDb::new().await;
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
    let db = TestDb::new().await;
    let snapshot = fetch_metrics(&db.pool).await.unwrap();

    assert!(snapshot.jobs_by_status.is_empty());
    assert!(snapshot.laws_by_status.is_empty());
    assert_eq!(snapshot.avg_job_duration_secs, None);
}

#[tokio::test]
async fn fetch_metrics_with_only_pending_jobs() {
    let db = TestDb::new().await;
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
    let db = TestDb::new().await;
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

// --- dashboard_stats ---

/// Fetch the dashboard stats through the handler and return the parsed JSON.
async fn get_dashboard_stats(pool: &sqlx::PgPool) -> Value {
    let app = test_app(pool.clone());
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/dashboard-stats")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    body_json(response).await
}

/// Force a job into `failed` with a given result JSON and completion time.
async fn mark_job_failed(pool: &sqlx::PgPool, job_id: uuid::Uuid, result: Value, completed: &str) {
    sqlx::query(
        "UPDATE jobs SET status = 'failed', result = $2, \
         completed_at = $3::timestamptz WHERE id = $1",
    )
    .bind(job_id)
    .bind(result)
    .bind(completed)
    .execute(pool)
    .await
    .unwrap();
}

#[tokio::test]
async fn dashboard_stats_on_empty_db() {
    let db = TestDb::new().await;
    let json = get_dashboard_stats(&db.pool).await;

    assert_eq!(json["jobs"]["total"], 0);
    assert_eq!(json["jobs"]["by_status"]["pending"], 0);
    assert_eq!(json["jobs"]["by_type"]["harvest"], 0);
    assert_eq!(json["executed"]["today"]["total"], 0);
    assert_eq!(json["open_untranslatables"], 0);
    assert_eq!(json["recent_failures"].as_array().unwrap().len(), 0);
    // The daily series always spans 14 days, all zeros on an empty DB.
    let daily = json["daily"].as_array().unwrap();
    assert_eq!(daily.len(), 14);
    assert!(daily.iter().all(|d| {
        ["harvest", "enrich"].iter().all(|t| {
            ["added", "succeeded", "failed"]
                .iter()
                .all(|k| d[t][k] == 0)
        })
    }));
}

#[tokio::test]
async fn dashboard_stats_counts_by_type_and_status() {
    let db = TestDb::new().await;
    let pool = db.pool.clone();

    // Two pending harvest jobs, one pending enrich job.
    job_queue::create_job(
        &pool,
        CreateJobRequest::new(JobType::Harvest, "BWBR0000001"),
    )
    .await
    .unwrap();
    job_queue::create_job(
        &pool,
        CreateJobRequest::new(JobType::Harvest, "BWBR0000002"),
    )
    .await
    .unwrap();
    job_queue::create_job(&pool, CreateJobRequest::new(JobType::Enrich, "BWBR0000003"))
        .await
        .unwrap();

    // Complete the enrich one.
    let claimed = job_queue::claim_job(&pool, Some(JobType::Enrich))
        .await
        .unwrap()
        .unwrap();
    job_queue::complete_job(&pool, claimed.id, None)
        .await
        .unwrap();

    let json = get_dashboard_stats(&pool).await;

    assert_eq!(json["jobs"]["total"], 3);
    assert_eq!(json["jobs"]["by_type"]["harvest"], 2);
    assert_eq!(json["jobs"]["by_type"]["enrich"], 1);
    assert_eq!(json["jobs"]["by_status"]["pending"], 2);
    assert_eq!(json["jobs"]["by_status"]["completed"], 1);
    assert_eq!(json["jobs"]["by_type_status"]["harvest"]["pending"], 2);
    assert_eq!(json["jobs"]["by_type_status"]["enrich"]["completed"], 1);
    assert_eq!(json["jobs"]["by_type_status"]["enrich"]["pending"], 0);
}

#[tokio::test]
async fn dashboard_stats_executed_windows_use_effective_timestamp() {
    let db = TestDb::new().await;
    let pool = db.pool.clone();

    // A pending job that was created today counts via created_at (it has no
    // completed_at yet).
    job_queue::create_job(
        &pool,
        CreateJobRequest::new(JobType::Harvest, "BWBR0000010"),
    )
    .await
    .unwrap();

    // A completed enrich job finished today.
    let enrich =
        job_queue::create_job(&pool, CreateJobRequest::new(JobType::Enrich, "BWBR0000011"))
            .await
            .unwrap();
    let claimed = job_queue::claim_job(&pool, Some(JobType::Enrich))
        .await
        .unwrap()
        .unwrap();
    job_queue::complete_job(&pool, claimed.id, None)
        .await
        .unwrap();
    assert_eq!(enrich.job_type, JobType::Enrich);

    // A completed harvest job created AND completed 10 days ago: outside both
    // "today" and "last 7 days".
    let old = job_queue::create_job(
        &pool,
        CreateJobRequest::new(JobType::Harvest, "BWBR0000012"),
    )
    .await
    .unwrap();
    sqlx::query(
        "UPDATE jobs SET status = 'completed', \
         created_at = now() - interval '10 days', \
         completed_at = now() - interval '10 days' WHERE id = $1",
    )
    .bind(old.id)
    .execute(&pool)
    .await
    .unwrap();

    let json = get_dashboard_stats(&pool).await;

    // today: the pending harvest (via created_at) + the completed enrich.
    assert_eq!(json["executed"]["today"]["total"], 2);
    assert_eq!(json["executed"]["today"]["harvest"], 1);
    assert_eq!(json["executed"]["today"]["enrich"], 1);
    // last 7 days: same two; the 10-day-old harvest is excluded.
    assert_eq!(json["executed"]["last_7d"]["total"], 2);
    assert_eq!(json["executed"]["last_7d"]["harvest"], 1);
}

#[tokio::test]
async fn dashboard_stats_open_untranslatables_counts_only_unaccepted() {
    let db = TestDb::new().await;

    seed_untranslatable(&db.pool, "wet_a", "Wet A", "opencode", "rounding", false).await;
    seed_untranslatable(&db.pool, "wet_b", "Wet B", "opencode", "rounding", false).await;
    seed_untranslatable(&db.pool, "wet_c", "Wet C", "opencode", "rounding", true).await;

    let json = get_dashboard_stats(&db.pool).await;

    assert_eq!(json["open_untranslatables"], 2);
}

#[tokio::test]
async fn dashboard_stats_recent_failures_carry_reason() {
    let db = TestDb::new().await;
    let pool = db.pool.clone();

    let older = job_queue::create_job(
        &pool,
        CreateJobRequest::new(JobType::Harvest, "BWBR0000020"),
    )
    .await
    .unwrap();
    let newer = job_queue::create_job(&pool, CreateJobRequest::new(JobType::Enrich, "BWBR0000021"))
        .await
        .unwrap();

    mark_job_failed(
        &pool,
        older.id,
        serde_json::json!({ "error": "harvest boom" }),
        "2026-07-01T10:00:00Z",
    )
    .await;
    mark_job_failed(
        &pool,
        newer.id,
        serde_json::json!({ "error": "job timed out after 300s" }),
        "2026-07-03T10:00:00Z",
    )
    .await;

    let json = get_dashboard_stats(&pool).await;

    assert_eq!(json["jobs"]["by_status"]["failed"], 2);
    let failures = json["recent_failures"].as_array().unwrap();
    assert_eq!(failures.len(), 2);
    // Ordered by failed_at DESC — the newer failure comes first.
    assert_eq!(failures[0]["law_id"], "BWBR0000021");
    assert_eq!(failures[0]["job_type"], "enrich");
    assert_eq!(failures[0]["error"], "job timed out after 300s");
    assert_eq!(failures[1]["law_id"], "BWBR0000020");
    assert_eq!(failures[1]["error"], "harvest boom");
}

#[tokio::test]
async fn dashboard_stats_failure_reason_falls_back_when_no_error_key() {
    let db = TestDb::new().await;
    let pool = db.pool.clone();

    // A failed job whose result JSON lacks an `error` key falls back to the
    // placeholder rather than serialising null.
    let job = job_queue::create_job(
        &pool,
        CreateJobRequest::new(JobType::Harvest, "BWBR0000030"),
    )
    .await
    .unwrap();
    mark_job_failed(
        &pool,
        job.id,
        serde_json::json!({ "note": "something else" }),
        "2026-07-02T10:00:00Z",
    )
    .await;

    let json = get_dashboard_stats(&pool).await;
    let failures = json["recent_failures"].as_array().unwrap();
    assert_eq!(failures.len(), 1);
    assert_eq!(failures[0]["error"], "onbekend");
}

#[tokio::test]
async fn dashboard_stats_daily_series_fourteen_days() {
    let db = TestDb::new().await;
    let pool = db.pool.clone();

    // Harvest job created today, still pending → counts as added only.
    job_queue::create_job(
        &pool,
        CreateJobRequest::new(JobType::Harvest, "BWBR0000040"),
    )
    .await
    .unwrap();

    // Enrich job created and completed today → added + succeeded today.
    job_queue::create_job(&pool, CreateJobRequest::new(JobType::Enrich, "BWBR0000041"))
        .await
        .unwrap();
    let claimed = job_queue::claim_job(&pool, Some(JobType::Enrich))
        .await
        .unwrap()
        .unwrap();
    job_queue::complete_job(&pool, claimed.id, None)
        .await
        .unwrap();

    // Harvest job created 5 days ago, failed 3 days ago: added counts on the
    // creation day, failed on the completion day.
    let failed = job_queue::create_job(
        &pool,
        CreateJobRequest::new(JobType::Harvest, "BWBR0000042"),
    )
    .await
    .unwrap();
    sqlx::query(
        "UPDATE jobs SET status = 'failed', \
         created_at = now() - interval '5 days', \
         completed_at = now() - interval '3 days' WHERE id = $1",
    )
    .bind(failed.id)
    .execute(&pool)
    .await
    .unwrap();

    // Harvest job created and completed 20 days ago → outside the window.
    let old = job_queue::create_job(
        &pool,
        CreateJobRequest::new(JobType::Harvest, "BWBR0000043"),
    )
    .await
    .unwrap();
    sqlx::query(
        "UPDATE jobs SET status = 'completed', \
         created_at = now() - interval '20 days', \
         completed_at = now() - interval '20 days' WHERE id = $1",
    )
    .bind(old.id)
    .execute(&pool)
    .await
    .unwrap();

    let json = get_dashboard_stats(&pool).await;
    let daily = json["daily"].as_array().unwrap();
    assert_eq!(daily.len(), 14);

    // Dates ascend and the last entry is today's Europe/Amsterdam date (same
    // source of truth as the query: Postgres).
    let today: String = sqlx::query_scalar(
        "SELECT to_char((now() AT TIME ZONE 'Europe/Amsterdam')::date, 'YYYY-MM-DD')",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(daily[13]["date"], today.as_str());
    for pair in daily.windows(2) {
        assert!(pair[0]["date"].as_str().unwrap() < pair[1]["date"].as_str().unwrap());
    }

    // Today (index 13): 1 harvest added, 1 enrich added + succeeded.
    assert_eq!(daily[13]["harvest"]["added"], 1);
    assert_eq!(daily[13]["harvest"]["succeeded"], 0);
    assert_eq!(daily[13]["harvest"]["failed"], 0);
    assert_eq!(daily[13]["enrich"]["added"], 1);
    assert_eq!(daily[13]["enrich"]["succeeded"], 1);

    // 5 days ago (index 8): the failed harvest counts as added there;
    // 3 days ago (index 10): it counts as failed there.
    assert_eq!(daily[8]["harvest"]["added"], 1);
    assert_eq!(daily[10]["harvest"]["failed"], 1);

    // The 20-day-old job appears nowhere in the window.
    let total_harvest_added: i64 = daily
        .iter()
        .map(|d| d["harvest"]["added"].as_i64().unwrap())
        .sum();
    assert_eq!(total_harvest_added, 2);
}
