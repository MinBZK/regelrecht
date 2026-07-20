//! Integration tests for the traject-routed notes write path
//! (`corpus_handlers::save_annotations`).
//!
//! This is the first end-to-end coverage of the traject save path at all:
//! #632 shipped the traject model with handler-CRUD tests only, and PR
//! #652's earlier tests were against the deleted per-session backend. The
//! path is hard to exercise by hand locally (it needs an authenticated
//! user + an active traject, and OIDC is off in the local stack), so it
//! is pinned here instead.
//!
//! Each test spins up an isolated Postgres container via
//! `regelrecht_pipeline::test_utils::TestDb` (so `0014_trajects.sql` runs
//! for real) and a hermetic, **local** writable-own traject source backed
//! by a `tempfile` corpus — no GitHub clone, no network. The handler is
//! called directly with inline `axum` extractors, exactly like
//! `trajects_test.rs`.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use pretty_assertions::assert_eq;
use sqlx::PgPool;
use tokio::sync::{Mutex, RwLock};
use tower_sessions::Session;
use tower_sessions_memory_store::MemoryStore;
use uuid::Uuid;

use regelrecht_auth::handlers::SESSION_KEY_SUB;
use regelrecht_editor_api::config::AppConfig;
use regelrecht_editor_api::corpus_handlers::{get_annotations, save_annotations, SaveResponse};
use regelrecht_editor_api::state::{AppState, CorpusState};
use regelrecht_editor_api::traject_corpus::TrajectCorpusCache;
use regelrecht_editor_api::trajects::SESSION_KEY_ACTIVE_TRAJECT;

use regelrecht_pipeline::test_utils::TestDb;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const LAW_ID: &str = "wet_op_de_zorgtoeslag";

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

/// Seed an account and return its `person_sub` (what the session carries).
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

/// A minimal but schema-valid law file so the source map's `$id` scan
/// resolves `LAW_ID` to a file with a `relative_path`.
fn write_law(corpus_dir: &std::path::Path) {
    let law_dir = corpus_dir.join("wet").join(LAW_ID);
    std::fs::create_dir_all(&law_dir).unwrap();
    std::fs::write(
        law_dir.join("2025-01-01.yaml"),
        format!("$id: {LAW_ID}\nname: Zorgtoeslagwet\n"),
    )
    .unwrap();
}

/// One well-formed W3C annotation targeting `target_law`, as the browser
/// would send it (no `__draft` marker — that is stripped client-side).
fn note(target_law: &str, exact: &str) -> serde_json::Value {
    serde_json::json!({
        "type": "Annotation",
        "motivation": "commenting",
        "creator": "tester",
        "target": {
            "source": format!("regelrecht://{target_law}"),
            "selector": { "type": "TextQuoteSelector", "exact": exact }
        },
        "body": { "type": "TextualBody", "value": "een toelichting", "purpose": "commenting" }
    })
}

/// Create a traject with a single **local** writable-own source whose
/// path is `corpus_dir`. No GitHub seed, so `build_traject_corpus` never
/// touches the network. Returns the traject id; `owner_id` is made a
/// member so the handler's membership re-check passes.
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

    // The local source is writable_own with no auth_ref: it writes back
    // through its own backend (no write_target_for_source entry needed —
    // the law is read from this very source).
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

fn sidecar_path(corpus_dir: &std::path::Path) -> std::path::PathBuf {
    corpus_dir
        .join("annotations")
        .join(LAW_ID)
        .join("annotations.yaml")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn writes_a_note_to_the_traject_local_sidecar() {
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let (owner, sub) = seed_account(&db.pool, "alice@test.local").await;

    let corpus = tempfile::tempdir().unwrap();
    write_law(corpus.path());
    let traject_id = local_traject(&db.pool, owner, corpus.path()).await;
    let session = session_for(&sub, Some(traject_id)).await;

    let body = serde_json::to_string(&[note(LAW_ID, "zorgtoeslag")]).unwrap();
    let Json(SaveResponse { pr, no_change, .. }) =
        save_annotations(State(state), session, Path(LAW_ID.to_string()), body)
            .await
            .expect("save should succeed");

    // A local backend commits in place, no PR; the note is new, so it is
    // not a no-op.
    assert!(pr.is_none());
    assert!(!no_change);

    // The sidecar landed at annotations/{law_id}/annotations.yaml under
    // the source root and contains the note.
    let written = std::fs::read_to_string(sidecar_path(corpus.path()))
        .expect("sidecar must exist on the traject source");
    let doc: serde_json::Value = serde_yaml_ng::from_str(&written).unwrap();
    let notes = doc["annotations"].as_array().unwrap();
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0]["target"]["selector"]["exact"], "zorgtoeslag");
}

#[tokio::test]
async fn no_active_traject_is_403() {
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let (_owner, sub) = seed_account(&db.pool, "alice@test.local").await;
    // Session has a subject but NO active traject.
    let session = session_for(&sub, None).await;

    let body = serde_json::to_string(&[note(LAW_ID, "x")]).unwrap();
    let err = save_annotations(State(state), session, Path(LAW_ID.to_string()), body)
        .await
        .expect_err("must refuse without an active traject");

    assert_eq!(err.0, StatusCode::FORBIDDEN, "{}", err.1);
}

#[tokio::test]
async fn a_note_targeting_another_law_is_rejected() {
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let (owner, sub) = seed_account(&db.pool, "alice@test.local").await;

    let corpus = tempfile::tempdir().unwrap();
    write_law(corpus.path());
    let traject_id = local_traject(&db.pool, owner, corpus.path()).await;
    let session = session_for(&sub, Some(traject_id)).await;

    // The note's target.source points at a DIFFERENT law than the path.
    let body = serde_json::to_string(&[note("andere_wet", "x")]).unwrap();
    let err = save_annotations(State(state), session, Path(LAW_ID.to_string()), body)
        .await
        .expect_err("cross-law note must be rejected");

    assert_eq!(err.0, StatusCode::BAD_REQUEST, "{}", err.1);
    // And nothing was written.
    assert!(!sidecar_path(corpus.path()).exists());
}

#[tokio::test]
async fn re_saving_an_already_committed_note_is_a_no_op() {
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let (owner, sub) = seed_account(&db.pool, "alice@test.local").await;

    let corpus = tempfile::tempdir().unwrap();
    write_law(corpus.path());
    let traject_id = local_traject(&db.pool, owner, corpus.path()).await;

    let body = serde_json::to_string(&[note(LAW_ID, "zorgtoeslag")]).unwrap();

    // First save writes the note.
    let session1 = session_for(&sub, Some(traject_id)).await;
    let _ = save_annotations(
        State(state.clone()),
        session1,
        Path(LAW_ID.to_string()),
        body.clone(),
    )
    .await
    .expect("first save ok");
    let after_first = std::fs::read_to_string(sidecar_path(corpus.path())).unwrap();

    // Second save of the SAME note: dedup leaves nothing, so no_change is
    // reported and the file is byte-identical (no empty commit / churn).
    let session2 = session_for(&sub, Some(traject_id)).await;
    let Json(SaveResponse { pr, no_change, .. }) =
        save_annotations(State(state), session2, Path(LAW_ID.to_string()), body)
            .await
            .expect("second save ok");

    assert!(pr.is_none());
    assert!(
        no_change,
        "a re-save of an already-present note must be a no-op"
    );
    let after_second = std::fs::read_to_string(sidecar_path(corpus.path())).unwrap();
    assert_eq!(
        after_first, after_second,
        "the sidecar must not change on a no-op re-save"
    );
}

// ---------------------------------------------------------------------------
// Read path (`get_annotations`)
// ---------------------------------------------------------------------------
//
// These pin the gap #662 documented as out-of-scope: annotation reads used
// to come from the static `/data` mirror baked into the frontend container
// at image build time, so an API-saved note was invisible after refresh.
// The new GET routes through the same backend the write went to.

/// Helper: call `get_annotations` and return the response body. Panics on
/// any status other than 200.
async fn read_annotations(state: AppState, session: Session) -> String {
    let (status, _headers, body) = get_annotations(State(state), session, Path(LAW_ID.to_string()))
        .await
        .expect("get_annotations must succeed");
    assert_eq!(status, StatusCode::OK);
    body
}

#[tokio::test]
async fn saved_note_is_readable_on_the_same_traject() {
    // Closes the loop the bug report opened: save in traject A, then GET
    // returns the just-saved sidecar instead of falling back to the
    // static mirror (which would 404 or serve the pre-baked main copy).
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let (owner, sub) = seed_account(&db.pool, "alice@test.local").await;

    let corpus = tempfile::tempdir().unwrap();
    write_law(corpus.path());
    let traject_id = local_traject(&db.pool, owner, corpus.path()).await;

    // Save a note.
    let body = serde_json::to_string(&[note(LAW_ID, "zorgtoeslag")]).unwrap();
    let _ = save_annotations(
        State(state.clone()),
        session_for(&sub, Some(traject_id)).await,
        Path(LAW_ID.to_string()),
        body,
    )
    .await
    .expect("save should succeed");

    // GET it back through the new read endpoint.
    let yaml_text = read_annotations(state, session_for(&sub, Some(traject_id)).await).await;
    let doc: serde_json::Value = serde_yaml_ng::from_str(&yaml_text).unwrap();
    let notes = doc["annotations"].as_array().unwrap();
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0]["target"]["selector"]["exact"], "zorgtoeslag");
}

#[tokio::test]
async fn missing_sidecar_returns_404() {
    // A law without any notes is the normal case; the frontend's
    // `useNotes.js` treats 404 as "no notes" rather than an error.
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let (owner, sub) = seed_account(&db.pool, "alice@test.local").await;

    let corpus = tempfile::tempdir().unwrap();
    write_law(corpus.path());
    let traject_id = local_traject(&db.pool, owner, corpus.path()).await;

    let err = get_annotations(
        State(state),
        session_for(&sub, Some(traject_id)).await,
        Path(LAW_ID.to_string()),
    )
    .await
    .expect_err("no sidecar yet, expect 404");
    assert_eq!(err.0, StatusCode::NOT_FOUND, "{}", err.1);
}

#[tokio::test]
async fn read_with_no_active_traject_uses_global_corpus() {
    // Pin the `ReadScope::Global` branch of `resolve_annotation_read_backend`:
    // anonymous-browsing reads (no active traject in the session) must
    // route through the global corpus's law-own backend, NOT the per-
    // traject writable_own. With `CorpusState::empty()` there is no
    // backend at all for any law, so a GET resolves to NOT_FOUND from
    // the inner `get_law` lookup — the contract is "no 403, no
    // 500-on-missing-traject; a clean 404 just like the old static
    // mirror".
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let (_owner, sub) = seed_account(&db.pool, "alice@test.local").await;
    // Session has a subject but no active traject — same shape as the
    // anonymous-after-logout case that the `useNotes.js` 404→[] handler
    // already supports.
    let session = session_for(&sub, None).await;

    let err = get_annotations(State(state), session, Path(LAW_ID.to_string()))
        .await
        .expect_err("global corpus is empty so the law isn't found");
    assert_eq!(
        err.0,
        StatusCode::NOT_FOUND,
        "no-traject read must degrade to 404, not 403 or 500; got: {}",
        err.1
    );
}

/// Provision a traject with TWO local sources, mirroring the production
/// federated layout:
///   - a read-only `seed` source (carrying the law file, no auth token)
///   - a writable-own source at priority 0 (the traject's own "branch",
///     starts empty)
///
/// This is the shape that production uses (central GitHub seed + a
/// writable_own pointing at the same repo's traject branch). It exposes
/// any read-path that assumes the writable_own ALSO contains the law
/// file at a verifiable path — a check that holds for scenarios (which
/// live under the law's own directory) but breaks for annotations
/// (separate `annotations/{law_id}/...` tree).
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
    // Seed source — carries the law file.
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
    // Writable-own at priority 0. Starts empty; first save lands here.
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
async fn saved_note_is_readable_with_seeded_traject() {
    // Regression: in production the writable_own backend is a separate
    // source from the seed that owns the law file. `save_annotations`
    // routes through `write_target_for_source` and lands on the
    // writable_own, but the read path went through the scenarios'
    // `resolve_backend_for_law` helper, which verifies the candidate
    // writable backend by trying to read the LAW FILE from it. The
    // writable_own starts with only the saved annotation and no law
    // file (annotations live under a separate `annotations/{law_id}/`
    // tree), so the verification fell through and the read landed on
    // the read-only seed instead — which had no annotation either,
    // hence the 404 the user saw after refresh.
    //
    // The single-writable-source tests above mask this because the
    // law's own source IS the writable_own, so the verification step
    // is skipped entirely.
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let (owner, sub) = seed_account(&db.pool, "alice@test.local").await;

    let seed = tempfile::tempdir().unwrap();
    let writable_own = tempfile::tempdir().unwrap();
    // Seed has the law; writable_own starts empty (mirrors fresh
    // traject branch before any law edits).
    write_law(seed.path());
    let traject_id = seeded_traject(&db.pool, owner, seed.path(), writable_own.path()).await;

    // Save a note via the actual handler.
    let body = serde_json::to_string(&[note(LAW_ID, "zorgtoeslag")]).unwrap();
    let _ = save_annotations(
        State(state.clone()),
        session_for(&sub, Some(traject_id)).await,
        Path(LAW_ID.to_string()),
        body,
    )
    .await
    .expect("save should succeed");

    // Sanity check: the save landed on the writable_own backend, not
    // the seed. (Mirror of `save_on_seed_loaded_law_lands_on_writable_own_backend`
    // in `traject_reads_test.rs`.)
    let writable_own_sidecar = writable_own
        .path()
        .join("annotations")
        .join(LAW_ID)
        .join("annotations.yaml");
    assert!(
        writable_own_sidecar.exists(),
        "save must land on writable_own, not seed"
    );

    // The read must return the just-saved note — same backend as the
    // write, not the seed.
    let yaml_text = read_annotations(state, session_for(&sub, Some(traject_id)).await).await;
    let doc: serde_json::Value = serde_yaml_ng::from_str(&yaml_text).unwrap();
    let notes = doc["annotations"].as_array().unwrap();
    assert_eq!(
        notes.len(),
        1,
        "expected the freshly-saved note; got {doc:?}"
    );
    assert_eq!(notes[0]["target"]["selector"]["exact"], "zorgtoeslag");
}

#[tokio::test]
async fn cross_traject_isolation_on_reads() {
    // A note saved in traject A must NOT be visible from traject B —
    // each traject reads from its own writable backend. Without this
    // isolation the static-mirror replacement would leak in-flight
    // edits from one traject into another's view.
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let (owner, sub) = seed_account(&db.pool, "alice@test.local").await;

    let corpus_a = tempfile::tempdir().unwrap();
    write_law(corpus_a.path());
    let traject_a = local_traject(&db.pool, owner, corpus_a.path()).await;

    let corpus_b = tempfile::tempdir().unwrap();
    write_law(corpus_b.path());
    let traject_b = local_traject(&db.pool, owner, corpus_b.path()).await;

    // Save in A only.
    let body = serde_json::to_string(&[note(LAW_ID, "A-only-note")]).unwrap();
    let _ = save_annotations(
        State(state.clone()),
        session_for(&sub, Some(traject_a)).await,
        Path(LAW_ID.to_string()),
        body,
    )
    .await
    .expect("save in A should succeed");

    // A sees its own note.
    let from_a = read_annotations(state.clone(), session_for(&sub, Some(traject_a)).await).await;
    assert!(from_a.contains("A-only-note"));

    // B sees nothing — its sidecar does not exist.
    let err = get_annotations(
        State(state),
        session_for(&sub, Some(traject_b)).await,
        Path(LAW_ID.to_string()),
    )
    .await
    .expect_err("traject B must not see traject A's notes");
    assert_eq!(err.0, StatusCode::NOT_FOUND, "{}", err.1);
}
