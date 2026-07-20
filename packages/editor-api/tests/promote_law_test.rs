//! Integration tests for `corpus_handlers::promote_corpus_law`: een wet uit
//! de centrale-corpus-seed van het traject kopiëren naar de traject-repo —
//! alle versie-YAML's plus de `scenarios/*.feature`-bestanden, en een 409
//! wanneer de wet al in het traject staat (geen dubbele bestanden).
//!
//! Zelfde hermetische opzet als `traject_reads_test.rs`: lokale sources, geen
//! GitHub/netwerk, handlers direct aangeroepen met inline axum-extractors.
//! Het traject heeft twee sources, zoals in productie: een read-seed die het
//! centrale corpus speelt en een (lege) writable-own repo.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use axum::extract::{Extension, Path, State};
use axum::http::{HeaderMap, StatusCode};
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
use regelrecht_editor_api::corpus_handlers::promote_corpus_law;
use regelrecht_editor_api::state::{AppState, CorpusState};
use regelrecht_editor_api::traject_corpus::TrajectCorpusCache;

use regelrecht_pipeline::test_utils::TestDb;

const LAW_ID: &str = "wet_voorbeeld_promotie";

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

/// Traject met twee lokale sources, zoals in productie: een read-seed op
/// `seed_dir` (het "centrale corpus", priority 2) en een writable-own op
/// `own_dir` (priority 0). Saves op seed-wetten routeren naar de
/// writable-own via `write_target_for_source` — hetzelfde pad dat promote
/// gebruikt.
async fn seeded_traject(
    pool: &PgPool,
    owner_id: Uuid,
    own_dir: &std::path::Path,
    seed_dir: &std::path::Path,
) -> Uuid {
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
    .bind(own_dir.to_string_lossy().to_string())
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO traject_corpus_sources
         (traject_id, source_id, name, source_type, local_path,
          priority, scopes, is_writable_own)
         VALUES ($1, 'central-seed', 'Centrale Corpus', 'local'::corpus_source_type, $2,
                 2, '[]'::jsonb, FALSE)",
    )
    .bind(traject_id)
    .bind(seed_dir.to_string_lossy().to_string())
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

fn test_account() -> AccountRecord {
    AccountRecord {
        id: Uuid::new_v4(),
        person_sub: "test-sub".to_string(),
        email: "test@example.gov".to_string(),
        name: "Test User".to_string(),
    }
}

/// Schrijf de volledige wet-map in het centrale corpus: twee versies (met
/// `machine_readable`-marker in de nieuwste) en één scenario-file.
fn write_central_law(central_dir: &std::path::Path) {
    let law_dir = central_dir.join("wet").join(LAW_ID);
    std::fs::create_dir_all(law_dir.join("scenarios")).unwrap();
    std::fs::write(
        law_dir.join("2025-01-01.yaml"),
        format!("$id: {LAW_ID}\nname: Versie 2025\nmachine_readable: MARKER_2025\n"),
    )
    .unwrap();
    std::fs::write(
        law_dir.join("2024-01-01.yaml"),
        format!("$id: {LAW_ID}\nname: Versie 2024\n"),
    )
    .unwrap();
    std::fs::write(
        law_dir.join("scenarios").join("basis.feature"),
        "Feature: basis\n",
    )
    .unwrap();
}

async fn promote(
    state: AppState,
    session: Session,
    tref: &str,
    law_id: &str,
) -> Result<axum::Json<regelrecht_editor_api::corpus_handlers::PromoteResponse>, (StatusCode, String)>
{
    promote_corpus_law(
        State(state),
        Extension(test_account()),
        session,
        Path((tref.to_string(), law_id.to_string())),
        HeaderMap::new(),
    )
    .await
}

#[tokio::test]
async fn promote_copies_all_versions_and_scenarios_to_the_traject_repo() {
    let db = TestDb::new().await;
    let central = tempfile::tempdir().unwrap();
    write_central_law(central.path());
    let state = empty_state(db.pool.clone());

    let (owner, sub) = seed_account(&db.pool, "alice@test.local").await;
    let traject_dir = tempfile::tempdir().unwrap();
    let traject_id = seeded_traject(&db.pool, owner, traject_dir.path(), central.path()).await;
    let tref = traject_ref(traject_id);

    let response = promote(state.clone(), session_for(&sub).await, &tref, LAW_ID)
        .await
        .expect("promote must succeed");
    assert_eq!(response.0.law_id, LAW_ID);
    assert_eq!(response.0.copied_files, 3, "2 versies + 1 scenario");

    // De volledige wet-map staat nu in de traject-repo, byte-voor-byte,
    // inclusief machine_readable en het scenario.
    let base = traject_dir.path().join("wet").join(LAW_ID);
    let newest = std::fs::read_to_string(base.join("2025-01-01.yaml")).unwrap();
    assert!(newest.contains("MARKER_2025"), "got: {newest}");
    assert_eq!(
        newest,
        std::fs::read_to_string(
            central
                .path()
                .join("wet")
                .join(LAW_ID)
                .join("2025-01-01.yaml")
        )
        .unwrap()
    );
    assert!(base.join("2024-01-01.yaml").exists());
    assert_eq!(
        std::fs::read_to_string(base.join("scenarios").join("basis.feature")).unwrap(),
        "Feature: basis\n"
    );
}

#[tokio::test]
async fn promote_refuses_a_law_already_in_the_traject_repo() {
    let db = TestDb::new().await;
    let central = tempfile::tempdir().unwrap();
    write_central_law(central.path());
    let state = empty_state(db.pool.clone());

    let (owner, sub) = seed_account(&db.pool, "alice@test.local").await;
    let traject_dir = tempfile::tempdir().unwrap();
    let traject_id = seeded_traject(&db.pool, owner, traject_dir.path(), central.path()).await;
    let tref = traject_ref(traject_id);

    let _ = promote(state.clone(), session_for(&sub).await, &tref, LAW_ID)
        .await
        .expect("first promote must succeed");

    // Tweede promote: de wet staat al in de traject-repo → 409, en er
    // veranderen geen bestanden (geen dubbele/overschreven bestanden).
    let before = std::fs::read_to_string(
        traject_dir
            .path()
            .join("wet")
            .join(LAW_ID)
            .join("2025-01-01.yaml"),
    )
    .unwrap();
    let err = promote(state.clone(), session_for(&sub).await, &tref, LAW_ID)
        .await
        .expect_err("second promote must be refused");
    assert_eq!(err.0, StatusCode::CONFLICT);
    let after = std::fs::read_to_string(
        traject_dir
            .path()
            .join("wet")
            .join(LAW_ID)
            .join("2025-01-01.yaml"),
    )
    .unwrap();
    assert_eq!(before, after);
}

#[tokio::test]
async fn promote_404s_for_a_law_missing_from_the_central_corpus() {
    let db = TestDb::new().await;
    let central = tempfile::tempdir().unwrap();
    let state = empty_state(db.pool.clone());

    let (owner, sub) = seed_account(&db.pool, "alice@test.local").await;
    let traject_dir = tempfile::tempdir().unwrap();
    let traject_id = seeded_traject(&db.pool, owner, traject_dir.path(), central.path()).await;
    let tref = traject_ref(traject_id);

    let err = promote(state, session_for(&sub).await, &tref, "wet_bestaat_niet")
        .await
        .expect_err("unknown law must 404");
    assert_eq!(err.0, StatusCode::NOT_FOUND);
}
