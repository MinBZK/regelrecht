//! Integration tests for the traject-aware read path
//! (`corpus_handlers::get_corpus_law` and friends).
//!
//! These tests exist to pin the cross-traject read isolation introduced
//! alongside `GitHubApiBackend`. Each test uses local writable-own
//! sources so the runs are hermetic — no GitHub, no network — and the
//! handler is invoked directly with inline `axum` extractors, mirroring
//! the pattern in `save_annotations_test.rs` and `trajects_test.rs`.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use pretty_assertions::assert_eq;
use sqlx::PgPool;
use tokio::sync::{Mutex, RwLock};
use tower_sessions::Session;
use tower_sessions_memory_store::MemoryStore;
use uuid::Uuid;

use regelrecht_auth::handlers::SESSION_KEY_SUB;
use regelrecht_editor_api::config::AppConfig;
use regelrecht_editor_api::corpus_handlers::{get_corpus_law, save_law};
use regelrecht_editor_api::state::{AppState, CorpusState};
use regelrecht_editor_api::traject_corpus::TrajectCorpusCache;
use regelrecht_editor_api::trajects::SESSION_KEY_ACTIVE_TRAJECT;

use regelrecht_pipeline::test_utils::TestDb;

const LAW_ID: &str = "zorgtoeslagwet";

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

/// Write a minimal but schema-valid law file under `corpus_dir/wet/{law_id}/2025-01-01.yaml`
/// with a single distinguishing `name:` so we can verify which copy a
/// read served back.
fn write_law(corpus_dir: &std::path::Path, name_marker: &str) {
    let law_dir = corpus_dir.join("wet").join(LAW_ID);
    std::fs::create_dir_all(&law_dir).unwrap();
    std::fs::write(
        law_dir.join("2025-01-01.yaml"),
        format!("$id: {LAW_ID}\nname: {name_marker}\n"),
    )
    .unwrap();
}

/// Create a traject with one local writable-own source pointing at
/// `corpus_dir`. Returns the traject id. `owner_id` is added as an
/// `owner` so the membership re-check on reads/writes passes.
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

async fn session_for(sub: &str, traject_id: Option<Uuid>) -> Session {
    let session = Session::new(None, Arc::new(MemoryStore::default()), None);
    session.insert(SESSION_KEY_SUB, sub).await.unwrap();
    if let Some(id) = traject_id {
        session
            .insert(SESSION_KEY_ACTIVE_TRAJECT, id)
            .await
            .unwrap();
    }
    session
}

/// Helper: call `get_corpus_law` and return the response body text.
async fn read_law(state: AppState, session: Session) -> String {
    let (status, _headers, body) = get_corpus_law(State(state), session, Path(LAW_ID.to_string()))
        .await
        .expect("get_corpus_law must succeed");
    assert_eq!(status, StatusCode::OK);
    body
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn get_corpus_law_serves_active_trajects_content() {
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let (owner, sub) = seed_account(&db.pool, "alice@test.local").await;

    // Two trajects, each owning a different corpus copy with the same
    // law id but distinguishable `name:` field.
    let corpus_a = tempfile::tempdir().unwrap();
    write_law(corpus_a.path(), "TRAJECT_A");
    let traject_a = local_traject(&db.pool, owner, corpus_a.path()).await;

    let corpus_b = tempfile::tempdir().unwrap();
    write_law(corpus_b.path(), "TRAJECT_B");
    let traject_b = local_traject(&db.pool, owner, corpus_b.path()).await;

    // Reading with traject A active returns A's copy.
    let body_a = read_law(state.clone(), session_for(&sub, Some(traject_a)).await).await;
    assert!(
        body_a.contains("TRAJECT_A"),
        "expected A's content, got: {body_a}"
    );
    assert!(!body_a.contains("TRAJECT_B"));

    // Reading with traject B active returns B's copy.
    let body_b = read_law(state.clone(), session_for(&sub, Some(traject_b)).await).await;
    assert!(
        body_b.contains("TRAJECT_B"),
        "expected B's content, got: {body_b}"
    );
    assert!(!body_b.contains("TRAJECT_A"));
}

#[tokio::test]
async fn save_then_read_returns_the_just_saved_content() {
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let (owner, sub) = seed_account(&db.pool, "alice@test.local").await;

    let corpus = tempfile::tempdir().unwrap();
    write_law(corpus.path(), "ORIGINAL");
    let traject_id = local_traject(&db.pool, owner, corpus.path()).await;

    // Verify the pre-save read returns the on-disk content.
    let before = read_law(state.clone(), session_for(&sub, Some(traject_id)).await).await;
    assert!(before.contains("ORIGINAL"));

    // Save a new body via the actual handler.
    let new_body = format!("$id: {LAW_ID}\nname: UPDATED\n");
    let _ = save_law(
        State(state.clone()),
        session_for(&sub, Some(traject_id)).await,
        Path(LAW_ID.to_string()),
        new_body,
    )
    .await
    .expect("save should succeed");

    // The next read must reflect the save — overlay populated by
    // `save_law` short-circuits the source_map snapshot.
    let after = read_law(state, session_for(&sub, Some(traject_id)).await).await;
    assert!(
        after.contains("UPDATED"),
        "expected the saved content; got: {after}"
    );
    assert!(!after.contains("ORIGINAL"));
}

#[tokio::test]
async fn no_active_traject_falls_back_to_global_corpus() {
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let (_owner, sub) = seed_account(&db.pool, "alice@test.local").await;

    // No active traject in the session. The global `state.corpus` is
    // CorpusState::empty(), so `get_corpus_law` should 404 — but
    // crucially without erroring out on the missing-traject path.
    let err = get_corpus_law(
        State(state),
        session_for(&sub, None).await,
        Path(LAW_ID.to_string()),
    )
    .await
    .expect_err("law is not in the global corpus; expect 404");

    assert_eq!(err.0, StatusCode::NOT_FOUND, "{}", err.1);
}

#[tokio::test]
async fn revoked_membership_falls_back_to_global_instead_of_403() {
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let (owner, sub) = seed_account(&db.pool, "alice@test.local").await;

    let corpus = tempfile::tempdir().unwrap();
    write_law(corpus.path(), "TRAJECT_CONTENT");
    let traject_id = local_traject(&db.pool, owner, corpus.path()).await;

    // Yank the membership AFTER the session was issued — the stale
    // session still carries the active traject id.
    sqlx::query("DELETE FROM traject_members WHERE traject_id = $1")
        .bind(traject_id)
        .execute(&db.pool)
        .await
        .unwrap();

    // Read should *not* 403; it should silently degrade to the global
    // corpus (empty here, hence 404 — but the contract is "no 403").
    let err = get_corpus_law(
        State(state),
        session_for(&sub, Some(traject_id)).await,
        Path(LAW_ID.to_string()),
    )
    .await
    .expect_err("global corpus is empty so the law isn't found");
    assert_eq!(
        err.0,
        StatusCode::NOT_FOUND,
        "revoked membership must degrade to global, not 403; got: {}",
        err.1
    );
}

/// Provision a traject with TWO local sources — a `seed` source at low
/// priority that carries the existing law file, and a `writable_own`
/// at priority 0 that starts empty. The shape mirrors a real federated
/// deploy where the seed is read-only (baked into the container or
/// pulled from a central repo) and the writable_own is the traject's
/// own branch on the writable upstream. Returns the traject id.
async fn seeded_traject(
    pool: &PgPool,
    owner_id: Uuid,
    seed_dir: &std::path::Path,
    writable_own_dir: &std::path::Path,
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

    // Seed source — read-only logically (we treat it as such; the
    // LocalBackend's write_path is what makes the test crisp).
    sqlx::query(
        "INSERT INTO traject_corpus_sources
         (traject_id, source_id, name, source_type, local_path,
          priority, scopes, is_writable_own)
         VALUES ($1, 'seed', 'Seed', 'local'::corpus_source_type, $2,
                 5, '[]'::jsonb, FALSE)",
    )
    .bind(traject_id)
    .bind(seed_dir.to_string_lossy().to_string())
    .execute(pool)
    .await
    .unwrap();

    // Writable-own source at priority 0 (highest priority). Starts
    // empty; first save lands here.
    sqlx::query(
        "INSERT INTO traject_corpus_sources
         (traject_id, source_id, name, source_type, local_path,
          priority, scopes, is_writable_own)
         VALUES ($1, 'writable_own', 'Writable Own',
                 'local'::corpus_source_type, $2,
                 0, '[]'::jsonb, TRUE)",
    )
    .bind(traject_id)
    .bind(writable_own_dir.to_string_lossy().to_string())
    .execute(pool)
    .await
    .unwrap();

    traject_id
}

#[tokio::test]
async fn save_on_seed_loaded_law_lands_on_writable_own_backend() {
    // Regression test for the routing bug where a law loaded from a
    // seed source (e.g. the baked-in `local` corpus on the editor
    // container) had its save fall back to the seed's own backend
    // instead of being routed to the writable_own's branch. Symptom:
    // save returned 200, read-your-writes worked via the overlay, but
    // no commit landed on the traject branch. The fix routes every
    // non-writable_own source to the writable_own at build time, not
    // just the source matching `auth_ref`.
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let (owner, sub) = seed_account(&db.pool, "alice@test.local").await;

    let seed = tempfile::tempdir().unwrap();
    let writable_own = tempfile::tempdir().unwrap();
    // Seed has the law; writable_own starts empty.
    write_law(seed.path(), "SEED_VERSION");
    let traject_id = seeded_traject(&db.pool, owner, seed.path(), writable_own.path()).await;

    // Sanity check: pre-save, the writable_own dir does NOT have the
    // law file. The read falls through priority order and lands on
    // the seed copy.
    let writable_own_law_path = writable_own
        .path()
        .join("wet")
        .join(LAW_ID)
        .join("2025-01-01.yaml");
    assert!(!writable_own_law_path.exists());

    let new_body = format!("$id: {LAW_ID}\nname: SAVED_TO_WRITABLE_OWN\n");
    let _ = save_law(
        State(state.clone()),
        session_for(&sub, Some(traject_id)).await,
        Path(LAW_ID.to_string()),
        new_body.clone(),
    )
    .await
    .expect("save should succeed");

    // The file must now exist on the writable_own's backend root.
    // Before the fix the save silently landed on the seed source
    // (because write_target_for_source only fired for sources whose
    // id matched the writable_own's auth_ref) and this assertion
    // failed.
    let written = std::fs::read_to_string(&writable_own_law_path)
        .expect("save must land on the writable_own backend, not the seed");
    assert!(
        written.contains("SAVED_TO_WRITABLE_OWN"),
        "writable_own file must contain the saved body; got: {written}"
    );

    // The seed file is left untouched — saves never reach the seed's
    // backend.
    let seed_law_path = seed.path().join("wet").join(LAW_ID).join("2025-01-01.yaml");
    let seed_text = std::fs::read_to_string(&seed_law_path).unwrap();
    assert!(seed_text.contains("SEED_VERSION"));
    assert!(!seed_text.contains("SAVED_TO_WRITABLE_OWN"));
}
