//! Integration tests for `corpus_handlers::list_traject_scenarios` and
//! `corpus_handlers::list_scenarios` (global) — verify that
//! `ScenarioEntry::target_law_ids` is populated from the feature file
//! content on both read paths.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::{Path, State};
use sqlx::PgPool;
use tokio::sync::{Mutex, RwLock};
use tower_sessions::Session;
use tower_sessions_memory_store::MemoryStore;
use uuid::Uuid;

use regelrecht_auth::handlers::SESSION_KEY_SUB;
use regelrecht_editor_api::config::AppConfig;
use regelrecht_editor_api::corpus_handlers::{list_scenarios, list_traject_scenarios};
use regelrecht_editor_api::state::{AppState, BackendEntry, CorpusState};
use regelrecht_editor_api::traject_corpus::TrajectCorpusCache;

use regelrecht_pipeline::test_utils::TestDb;

const LAW_ID: &str = "wet_op_de_zorgtoeslag";

// ---------------------------------------------------------------------------
// Helpers — deliberately duplicated per test file (suite convention)
// ---------------------------------------------------------------------------

fn empty_state(pool: PgPool) -> AppState {
    AppState {
        corpus: Arc::new(RwLock::new(CorpusState::empty())),
        oidc_client: None,
        end_session_url: None,
        config: Arc::new(AppConfig {
            oidc: None,
            base_url: None,
        }),
        http_client: reqwest::Client::new(),
        pool: Some(pool),
        pipeline_api_url: None,
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

/// Write a minimal but schema-valid law file and a `scenarios/` directory
/// under `corpus_dir/wet/{LAW_ID}/`.
fn write_corpus(corpus_dir: &std::path::Path) {
    let law_dir = corpus_dir.join("wet").join(LAW_ID);
    std::fs::create_dir_all(&law_dir).unwrap();
    std::fs::write(
        law_dir.join("2025-01-01.yaml"),
        format!("$id: {LAW_ID}\nname: Zorgtoeslagwet\n"),
    )
    .unwrap();

    let scenarios_dir = law_dir.join("scenarios");
    std::fs::create_dir_all(&scenarios_dir).unwrap();

    // A scenario that evaluates the law under test.
    std::fs::write(
        scenarios_dir.join("matching.feature"),
        format!("Feature: m\n  Scenario: a\n    When I evaluate \"recht\" of \"{LAW_ID}\"\n"),
    )
    .unwrap();

    // A scenario that evaluates a different law.
    std::fs::write(
        scenarios_dir.join("other.feature"),
        "Feature: o\n  Scenario: b\n    When I evaluate \"x\" of \"andere_wet\"\n",
    )
    .unwrap();

    // A work-in-progress scenario with no execution step.
    std::fs::write(scenarios_dir.join("wip.feature"), "Feature: wip\n").unwrap();
}

/// Create a traject with a single local writable-own source at `corpus_dir`.
/// Returns the traject id; `owner_id` is added as an owner so the membership
/// re-check inside the handler passes.
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

/// Build a session that carries `sub` as the authenticated user identity.
/// The active traject is passed in the URL path (traject_ref), not in the
/// session — so no `SESSION_KEY_ACTIVE_TRAJECT` insertion needed.
async fn session_for(sub: &str) -> Session {
    let session = Session::new(None, Arc::new(MemoryStore::default()), None);
    session.insert(SESSION_KEY_SUB, sub).await.unwrap();
    session
}

/// Build the URL-form traject ref used in path parameters.
/// The resolver matches on `left(id::text, 8)` (first 8 hex chars of the
/// hyphenated UUID), so `t-{8hex}` is a valid ref (slug="t", suffix=8hex).
fn traject_ref(id: Uuid) -> String {
    format!("t-{}", &id.to_string()[..8])
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn list_traject_scenarios_returns_target_law_ids() {
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let (owner, sub) = seed_account(&db.pool, "alice@test.local").await;

    let corpus = tempfile::tempdir().unwrap();
    write_corpus(corpus.path());
    let traject = local_traject(&db.pool, owner, corpus.path()).await;

    let axum::Json(entries) = list_traject_scenarios(
        State(state),
        session_for(&sub).await,
        Path((traject_ref(traject), LAW_ID.to_string())),
    )
    .await
    .expect("list_traject_scenarios must succeed");

    let summary: Vec<(String, Vec<String>)> = entries
        .into_iter()
        .map(|e| (e.filename, e.target_law_ids))
        .collect();

    assert_eq!(
        summary,
        vec![
            ("matching.feature".to_string(), vec![LAW_ID.to_string()]),
            ("other.feature".to_string(), vec!["andere_wet".to_string()]),
            ("wip.feature".to_string(), Vec::new()),
        ]
    );
}

#[tokio::test]
async fn list_scenarios_global_returns_target_law_ids() {
    let corpus = tempfile::tempdir().unwrap();
    write_corpus(corpus.path());

    // Registry with a single local source rooted at the temp corpus, the
    // same shape `init_corpus` builds from corpus-registry.yaml.
    let manifest_dir = tempfile::tempdir().unwrap();
    let manifest = manifest_dir.path().join("corpus-registry.yaml");
    std::fs::write(
        &manifest,
        format!(
            "---\nschema_version: '1.0'\nsources:\n  - id: local\n    name: Local\n    \
             type: local\n    local:\n      path: {}\n    scopes: []\n    priority: 1\n",
            corpus.path().display()
        ),
    )
    .unwrap();
    let registry = regelrecht_corpus::CorpusRegistry::load(&manifest, None).unwrap();
    let source_map = registry.load_local_sources().unwrap();

    let mut backends = HashMap::new();
    for source in registry.sources() {
        let mut backend = regelrecht_corpus::backend::create_backend(source, None).unwrap();
        backend.ensure_ready().await.unwrap();
        let writable = backend.is_writable();
        backends.insert(
            source.id.clone(),
            BackendEntry {
                backend: Arc::new(Mutex::new(backend)),
                writable,
            },
        );
    }

    let state = AppState {
        corpus: Arc::new(RwLock::new(CorpusState {
            registry,
            source_map,
            backends,
            auth_file: None,
        })),
        oidc_client: None,
        end_session_url: None,
        config: Arc::new(AppConfig {
            oidc: None,
            base_url: None,
        }),
        http_client: reqwest::Client::new(),
        pool: None,
        pipeline_api_url: None,
        reload_lock: Arc::new(Mutex::new(())),
        trajects: Arc::new(TrajectCorpusCache::new()),
    };

    let axum::Json(entries) = list_scenarios(State(state), Path(LAW_ID.to_string()))
        .await
        .expect("list_scenarios must succeed");

    let summary: Vec<(String, Vec<String>)> = entries
        .into_iter()
        .map(|e| (e.filename, e.target_law_ids))
        .collect();

    assert_eq!(
        summary,
        vec![
            ("matching.feature".to_string(), vec![LAW_ID.to_string()]),
            ("other.feature".to_string(), vec!["andere_wet".to_string()]),
            ("wip.feature".to_string(), Vec::new()),
        ]
    );
}
