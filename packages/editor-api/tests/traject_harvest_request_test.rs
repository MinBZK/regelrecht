//! Integration tests for `task_requests::request_traject_harvest`: de
//! traject-scoped harvest-aanvraag (taken-mechanisme). Pint vast dat een
//! aanvraag een `traject_harvest`-job enqueue't met het juiste
//! taak-flow-payload (deliver=task, requested_by, prioriteit 80), dat een
//! tweede aanvraag voor dezelfde wet dedupliceert (409) en dat een ongeldig
//! BWB-id met 400 wordt geweigerd.
//!
//! Zelfde hermetische opzet als `promote_law_test.rs` / `traject_reads_test.rs`.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use axum::extract::{Extension, Path, State};
use axum::http::StatusCode;
use pretty_assertions::assert_eq;
use sqlx::PgPool;
use tokio::sync::{Mutex, RwLock};
use tower_sessions::Session;
use tower_sessions_memory_store::MemoryStore;
use uuid::Uuid;

use regelrecht_auth::handlers::{
    SESSION_KEY_EMAIL, SESSION_KEY_EMAIL_VERIFIED, SESSION_KEY_NAME, SESSION_KEY_SUB,
};
use regelrecht_editor_api::accounts::AccountRecord;
use regelrecht_editor_api::config::AppConfig;
use regelrecht_editor_api::state::{AppState, CorpusState};
use regelrecht_editor_api::task_requests::{request_traject_harvest, TrajectHarvestRequest};
use regelrecht_editor_api::traject_corpus::TrajectCorpusCache;

use regelrecht_pipeline::test_utils::TestDb;

const BWB_ID: &str = "BWBR0002399";

fn empty_state(pool: PgPool) -> AppState {
    AppState {
        corpus: Arc::new(RwLock::new(CorpusState::empty())),
        oidc_client: None,
        end_session_url: None,
        config: Arc::new(AppConfig {
            oidc: None,
            base_url: None,
            github_oauth: None,
            task_enrich_provider: "claude".to_string(),
        }),
        http_client: reqwest::Client::new(),
        pool: Some(pool),
        pipeline_api_url: None,
        harvest_admin_url: None,
        reload_lock: Arc::new(Mutex::new(())),
        trajects: Arc::new(TrajectCorpusCache::new()),
    }
}

async fn seed_account(pool: &PgPool, email: &str) -> (Uuid, String) {
    let sub = format!("sub-{email}");
    let (id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO accounts (person_sub, email, name) VALUES ($1, $2, $3) RETURNING id",
    )
    .bind(&sub)
    .bind(email)
    .bind("Test User")
    .fetch_one(pool)
    .await
    .unwrap();
    (id, sub)
}

async fn local_traject(pool: &PgPool, owner_id: Uuid, corpus_dir: &std::path::Path) -> Uuid {
    let (traject_id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO trajects (name, description, scope, created_by)
         VALUES ('Test', '', '', $1) RETURNING id",
    )
    .bind(owner_id)
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO traject_members (traject_id, account_id, role)
         VALUES ($1, $2, 'owner')",
    )
    .bind(traject_id)
    .bind(owner_id)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO traject_corpus_sources
         (traject_id, source_id, name, source_type, local_path,
          priority, scopes, is_writable_own)
         VALUES ($1, 'local', 'Local', 'local'::corpus_source_type, $2,
                 0, '[]'::jsonb, TRUE)",
    )
    .bind(traject_id)
    .bind(corpus_dir.to_string_lossy().to_string())
    .execute(pool)
    .await
    .unwrap();
    traject_id
}

fn traject_ref(traject_id: Uuid) -> String {
    format!("test-{}", &traject_id.to_string()[..8])
}

async fn session_for(sub: &str) -> Session {
    let session = Session::new(None, Arc::new(MemoryStore::default()), None);
    session.insert(SESSION_KEY_SUB, sub).await.unwrap();
    session.insert(SESSION_KEY_NAME, "Test User").await.unwrap();
    session
        .insert(SESSION_KEY_EMAIL, "alice@test.local")
        .await
        .unwrap();
    session
        .insert(SESSION_KEY_EMAIL_VERIFIED, true)
        .await
        .unwrap();
    session
}

fn account(account_id: Uuid) -> AccountRecord {
    AccountRecord {
        id: account_id,
        person_sub: "test-sub".to_string(),
        email: "test@example.gov".to_string(),
        name: "Test User".to_string(),
    }
}

async fn request(
    state: AppState,
    sub: &str,
    account_id: Uuid,
    tref: &str,
    bwb_id: &str,
    law_name: Option<&str>,
) -> Result<StatusCode, (StatusCode, String)> {
    let (status, _body) = request_traject_harvest(
        State(state),
        session_for(sub).await,
        Extension(account(account_id)),
        Path(tref.to_string()),
        axum::Json(TrajectHarvestRequest {
            bwb_id: bwb_id.to_string(),
            law_name: law_name.map(String::from),
        }),
    )
    .await?;
    Ok(status)
}

#[tokio::test]
async fn harvest_request_enqueues_a_task_flow_job() {
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let (owner, sub) = seed_account(&db.pool, "alice@test.local").await;
    let corpus_dir = tempfile::tempdir().unwrap();
    let traject_id = local_traject(&db.pool, owner, corpus_dir.path()).await;
    let tref = traject_ref(traject_id);

    let status = request(
        state,
        &sub,
        owner,
        &tref,
        // lowercase input wordt genormaliseerd naar de canonieke vorm.
        &BWB_ID.to_lowercase(),
        Some("Voorbeeldwet"),
    )
    .await
    .expect("harvest request must be accepted");
    assert_eq!(status, StatusCode::ACCEPTED);

    let (job_type, law_id, job_tref, priority, payload): (
        String,
        String,
        Option<String>,
        i32,
        serde_json::Value,
    ) = sqlx::query_as("SELECT job_type::text, law_id, traject_ref, priority, payload FROM jobs")
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(job_type, "traject_harvest");
    assert_eq!(law_id, BWB_ID);
    assert_eq!(job_tref.as_deref(), Some(tref.as_str()));
    assert_eq!(
        priority, 80,
        "een mens wacht erop: boven de bulk-prioriteit"
    );
    assert_eq!(payload["bwb_id"], BWB_ID);
    assert_eq!(payload["deliver"], "task");
    assert_eq!(payload["law_name"], "Voorbeeldwet");
    assert_eq!(payload["requested_by"], owner.to_string());
    assert_eq!(payload["traject_id"], traject_id.to_string());
}

#[tokio::test]
async fn duplicate_harvest_request_for_same_law_and_traject_conflicts() {
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let (owner, sub) = seed_account(&db.pool, "alice@test.local").await;
    let corpus_dir = tempfile::tempdir().unwrap();
    let traject_id = local_traject(&db.pool, owner, corpus_dir.path()).await;
    let tref = traject_ref(traject_id);

    let first = request(state.clone(), &sub, owner, &tref, BWB_ID, None)
        .await
        .expect("first request must be accepted");
    assert_eq!(first, StatusCode::ACCEPTED);

    let err = request(state, &sub, owner, &tref, BWB_ID, None)
        .await
        .expect_err("second request must conflict");
    assert_eq!(err.0, StatusCode::CONFLICT);

    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM jobs")
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(count, 1, "dedup: er blijft één actieve job");
}

#[tokio::test]
async fn invalid_bwb_id_is_rejected_with_400() {
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let (owner, sub) = seed_account(&db.pool, "alice@test.local").await;
    let corpus_dir = tempfile::tempdir().unwrap();
    let traject_id = local_traject(&db.pool, owner, corpus_dir.path()).await;
    let tref = traject_ref(traject_id);

    let err = request(state, &sub, owner, &tref, "wet_op_de_zorgtoeslag", None)
        .await
        .expect_err("slug is geen BWB-id");
    assert_eq!(err.0, StatusCode::BAD_REQUEST);

    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM jobs")
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
}
