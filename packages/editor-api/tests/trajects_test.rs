//! Integration tests for the traject HTTP handlers.
//!
//! Each test spins up an isolated Postgres container via
//! `regelrecht_pipeline::test_utils::TestDb`, so the migrations under
//! `packages/pipeline/migrations/` are exercised end-to-end — including
//! `0014_trajects.sql`. Handlers are invoked directly (with `axum`
//! extractors constructed inline) rather than through a `Router`, which
//! keeps the tests focused on the handler logic and skips the cookie /
//! middleware plumbing.
//!
//! `tower_sessions::Session` can be constructed directly against a
//! `MemoryStore` (see `make_session` below), so even the two
//! session-touching handlers (`set_active`, `leave`) are reachable
//! without a router.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::HashSet;
use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::{Extension, Json};
use pretty_assertions::assert_eq;
use sqlx::PgPool;
use tokio::sync::{Mutex, RwLock};
use tower_sessions::Session;
use tower_sessions_memory_store::MemoryStore;
use uuid::Uuid;

use regelrecht_editor_api::accounts::AccountRecord;
use regelrecht_editor_api::config::AppConfig;
use regelrecht_editor_api::state::{AppState, CorpusState};
use regelrecht_editor_api::traject_corpus::TrajectCorpusCache;
use regelrecht_editor_api::trajects::{
    self, AddMemberRequest, CreateTrajectRequest, SetActiveTrajectRequest, UpdateMemberRequest,
    UpdateTrajectRequest, SESSION_KEY_ACTIVE_TRAJECT,
};

use regelrecht_pipeline::test_utils::TestDb;

// ---------------------------------------------------------------------------
// Helpers
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
        favorites: Arc::new(HashSet::new()),
    }
}

async fn seed_account(pool: &PgPool, email: &str, name: &str) -> AccountRecord {
    let sub = format!("sub-{email}");
    let row: (Uuid,) = sqlx::query_as(
        "INSERT INTO accounts (person_sub, email, name) VALUES ($1, $2, $3) RETURNING id",
    )
    .bind(&sub)
    .bind(email)
    .bind(name)
    .fetch_one(pool)
    .await
    .unwrap();
    AccountRecord {
        id: row.0,
        person_sub: sub,
        email: email.to_string(),
        name: name.to_string(),
    }
}

fn create_req(name: &str) -> CreateTrajectRequest {
    CreateTrajectRequest {
        name: name.to_string(),
        description: String::new(),
        scope: String::new(),
    }
}

/// Create a traject owned by `owner` and return its id.
async fn create_traject(state: &AppState, owner: &AccountRecord, name: &str) -> Uuid {
    let (status, Json(summary)) = trajects::create(
        State(state.clone()),
        Extension(owner.clone()),
        Json(create_req(name)),
    )
    .await
    .unwrap();
    assert_eq!(status, StatusCode::CREATED);
    summary.id
}

async fn add_member(
    state: &AppState,
    beheerder: &AccountRecord,
    traject_id: Uuid,
    invitee_email: &str,
    role: &str,
) -> StatusCode {
    trajects::add_member(
        State(state.clone()),
        Extension(beheerder.clone()),
        Path(traject_id),
        Json(AddMemberRequest {
            email: invitee_email.to_string(),
            role: role.to_string(),
        }),
    )
    .await
    .unwrap_or_else(|s| s)
}

fn make_session() -> Session {
    Session::new(None, Arc::new(MemoryStore::default()), None)
}

// ---------------------------------------------------------------------------
// `create`
// ---------------------------------------------------------------------------

#[tokio::test]
async fn create_inserts_traject_with_owner_as_beheerder_and_writable_own_source() {
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let alice = seed_account(&db.pool, "alice@test.local", "Alice").await;

    let (status, Json(summary)) = trajects::create(
        State(state.clone()),
        Extension(alice.clone()),
        Json(create_req("Tarief")),
    )
    .await
    .unwrap();

    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(summary.name, "Tarief");
    assert_eq!(summary.status, "bezig");
    assert_eq!(summary.role, "beheerder");

    let (role,): (String,) = sqlx::query_as(
        "SELECT role::text FROM traject_members WHERE traject_id = $1 AND account_id = $2",
    )
    .bind(summary.id)
    .bind(alice.id)
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(role, "beheerder");

    let (priority, is_writable_own, gh_branch): (i32, bool, String) = sqlx::query_as(
        "SELECT priority, is_writable_own, gh_branch
         FROM traject_corpus_sources
         WHERE traject_id = $1 AND is_writable_own = TRUE",
    )
    .bind(summary.id)
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(priority, 0, "writable_own should beat the seeded sources");
    assert!(is_writable_own);
    assert!(
        gh_branch.starts_with("traject/tarief-"),
        "branch should be slugified, got {gh_branch}"
    );
}

#[tokio::test]
async fn create_rejects_empty_name() {
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let alice = seed_account(&db.pool, "alice@test.local", "Alice").await;

    let mut req = create_req("");
    req.name = "   ".to_string();
    let err = trajects::create(State(state), Extension(alice), Json(req))
        .await
        .unwrap_err();
    assert_eq!(err, StatusCode::BAD_REQUEST);
}

// ---------------------------------------------------------------------------
// `list`
// ---------------------------------------------------------------------------

#[tokio::test]
async fn list_returns_only_trajects_caller_is_a_member_of() {
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let alice = seed_account(&db.pool, "alice@test.local", "Alice").await;
    let bob = seed_account(&db.pool, "bob@test.local", "Bob").await;

    let t_alice = create_traject(&state, &alice, "Alice traject").await;
    let _t_bob = create_traject(&state, &bob, "Bob traject").await;

    let Json(list) = trajects::list(State(state.clone()), Extension(alice.clone()))
        .await
        .unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].id, t_alice);
}

// ---------------------------------------------------------------------------
// `get`
// ---------------------------------------------------------------------------

#[tokio::test]
async fn get_returns_403_for_non_member() {
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let alice = seed_account(&db.pool, "alice@test.local", "Alice").await;
    let bob = seed_account(&db.pool, "bob@test.local", "Bob").await;

    let traject_id = create_traject(&state, &alice, "Alice traject").await;

    let err = trajects::get(State(state), Extension(bob), Path(traject_id))
        .await
        .unwrap_err();
    assert_eq!(err, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn get_returns_detail_for_member() {
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let alice = seed_account(&db.pool, "alice@test.local", "Alice").await;

    let traject_id = create_traject(&state, &alice, "Tarief").await;

    let Json(detail) = trajects::get(State(state), Extension(alice.clone()), Path(traject_id))
        .await
        .unwrap();
    assert_eq!(detail.summary.id, traject_id);
    assert_eq!(detail.summary.role, "beheerder");
    assert_eq!(detail.members.len(), 1);
    assert_eq!(detail.members[0].account_id, alice.id);
    // Only the writable-own source is present because the empty CorpusState
    // had nothing to seed from.
    assert_eq!(detail.sources.len(), 1);
    assert!(detail.sources[0].is_writable_own);
}

// ---------------------------------------------------------------------------
// `update`
// ---------------------------------------------------------------------------

#[tokio::test]
async fn update_rejects_lid_and_accepts_beheerder() {
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let alice = seed_account(&db.pool, "alice@test.local", "Alice").await;
    let bob = seed_account(&db.pool, "bob@test.local", "Bob").await;
    let traject_id = create_traject(&state, &alice, "Tarief").await;
    assert_eq!(
        add_member(&state, &alice, traject_id, &bob.email, "lid").await,
        StatusCode::NO_CONTENT
    );

    let body = UpdateTrajectRequest {
        name: Some("Renamed".to_string()),
        description: None,
        scope: None,
        status: None,
    };

    // lid → 403
    let err = trajects::update(
        State(state.clone()),
        Extension(bob),
        Path(traject_id),
        Json(UpdateTrajectRequest {
            name: Some("Hacked".to_string()),
            description: None,
            scope: None,
            status: None,
        }),
    )
    .await
    .unwrap_err();
    assert_eq!(err, StatusCode::FORBIDDEN);

    // beheerder → 204
    let ok = trajects::update(
        State(state.clone()),
        Extension(alice.clone()),
        Path(traject_id),
        Json(body),
    )
    .await
    .unwrap();
    assert_eq!(ok, StatusCode::NO_CONTENT);

    let (name,): (String,) = sqlx::query_as("SELECT name FROM trajects WHERE id = $1")
        .bind(traject_id)
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(name, "Renamed");
}

#[tokio::test]
async fn update_validates_status_enum() {
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let alice = seed_account(&db.pool, "alice@test.local", "Alice").await;
    let traject_id = create_traject(&state, &alice, "Tarief").await;

    let err = trajects::update(
        State(state),
        Extension(alice),
        Path(traject_id),
        Json(UpdateTrajectRequest {
            name: None,
            description: None,
            scope: None,
            status: Some("klaar".to_string()),
        }),
    )
    .await
    .unwrap_err();
    assert_eq!(err, StatusCode::BAD_REQUEST);
}

// ---------------------------------------------------------------------------
// `add_member` / `update_member` / `remove_member`
// ---------------------------------------------------------------------------

#[tokio::test]
async fn add_member_returns_404_when_email_has_no_account() {
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let alice = seed_account(&db.pool, "alice@test.local", "Alice").await;
    let traject_id = create_traject(&state, &alice, "Tarief").await;

    let status = add_member(&state, &alice, traject_id, "ghost@test.local", "lid").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn add_member_upserts_role_on_conflict() {
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let alice = seed_account(&db.pool, "alice@test.local", "Alice").await;
    let bob = seed_account(&db.pool, "bob@test.local", "Bob").await;
    let traject_id = create_traject(&state, &alice, "Tarief").await;

    assert_eq!(
        add_member(&state, &alice, traject_id, &bob.email, "lid").await,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        add_member(&state, &alice, traject_id, &bob.email, "beheerder").await,
        StatusCode::NO_CONTENT
    );

    let (role,): (String,) = sqlx::query_as(
        "SELECT role::text FROM traject_members WHERE traject_id = $1 AND account_id = $2",
    )
    .bind(traject_id)
    .bind(bob.id)
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(role, "beheerder");
}

#[tokio::test]
async fn add_member_blocks_demoting_last_beheerder_via_upsert() {
    // Without this guard, add_member would be a back-door around the
    // last-beheerder check that update_member already enforces.
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let alice = seed_account(&db.pool, "alice@test.local", "Alice").await;
    let traject_id = create_traject(&state, &alice, "Tarief").await;

    let status = add_member(&state, &alice, traject_id, &alice.email, "lid").await;
    assert_eq!(status, StatusCode::CONFLICT);
}

#[tokio::test]
async fn update_member_blocks_demoting_last_beheerder() {
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let alice = seed_account(&db.pool, "alice@test.local", "Alice").await;
    let traject_id = create_traject(&state, &alice, "Tarief").await;

    let err = trajects::update_member(
        State(state),
        Extension(alice.clone()),
        Path((traject_id, alice.id)),
        Json(UpdateMemberRequest {
            role: "lid".to_string(),
        }),
    )
    .await
    .unwrap_err();
    assert_eq!(err, StatusCode::CONFLICT);
}

#[tokio::test]
async fn remove_member_blocks_removing_last_beheerder() {
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let alice = seed_account(&db.pool, "alice@test.local", "Alice").await;
    let traject_id = create_traject(&state, &alice, "Tarief").await;

    let err = trajects::remove_member(
        State(state),
        Extension(alice.clone()),
        Path((traject_id, alice.id)),
    )
    .await
    .unwrap_err();
    assert_eq!(err, StatusCode::CONFLICT);
}

// ---------------------------------------------------------------------------
// `delete`
// ---------------------------------------------------------------------------

#[tokio::test]
async fn delete_is_beheerder_only_and_cascades() {
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let alice = seed_account(&db.pool, "alice@test.local", "Alice").await;
    let bob = seed_account(&db.pool, "bob@test.local", "Bob").await;
    let traject_id = create_traject(&state, &alice, "Tarief").await;
    assert_eq!(
        add_member(&state, &alice, traject_id, &bob.email, "lid").await,
        StatusCode::NO_CONTENT
    );

    // lid cannot delete
    let err = trajects::delete(State(state.clone()), Extension(bob), Path(traject_id))
        .await
        .unwrap_err();
    assert_eq!(err, StatusCode::FORBIDDEN);

    // beheerder can
    let ok = trajects::delete(State(state.clone()), Extension(alice), Path(traject_id))
        .await
        .unwrap();
    assert_eq!(ok, StatusCode::NO_CONTENT);

    let (members, sources, trajects_count): (i64, i64, i64) = sqlx::query_as(
        "SELECT
           (SELECT COUNT(*) FROM traject_members WHERE traject_id = $1),
           (SELECT COUNT(*) FROM traject_corpus_sources WHERE traject_id = $1),
           (SELECT COUNT(*) FROM trajects WHERE id = $1)",
    )
    .bind(traject_id)
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(members, 0, "traject_members should cascade-delete");
    assert_eq!(sources, 0, "traject_corpus_sources should cascade-delete");
    assert_eq!(trajects_count, 0);
}

// ---------------------------------------------------------------------------
// `leave`
// ---------------------------------------------------------------------------

#[tokio::test]
async fn leave_blocks_last_beheerder() {
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let alice = seed_account(&db.pool, "alice@test.local", "Alice").await;
    let traject_id = create_traject(&state, &alice, "Tarief").await;

    let session = make_session();
    let err = trajects::leave(State(state), Extension(alice), session, Path(traject_id))
        .await
        .unwrap_err();
    assert_eq!(err, StatusCode::CONFLICT);
}

#[tokio::test]
async fn leave_allows_lid_and_clears_active_session_when_leaving_active_traject() {
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let alice = seed_account(&db.pool, "alice@test.local", "Alice").await;
    let bob = seed_account(&db.pool, "bob@test.local", "Bob").await;
    let traject_id = create_traject(&state, &alice, "Tarief").await;
    assert_eq!(
        add_member(&state, &alice, traject_id, &bob.email, "lid").await,
        StatusCode::NO_CONTENT
    );

    let session = make_session();
    session
        .insert(SESSION_KEY_ACTIVE_TRAJECT, traject_id)
        .await
        .unwrap();

    let ok = trajects::leave(
        State(state.clone()),
        Extension(bob.clone()),
        session.clone(),
        Path(traject_id),
    )
    .await
    .unwrap();
    assert_eq!(ok, StatusCode::NO_CONTENT);

    let (count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM traject_members WHERE traject_id = $1 AND account_id = $2",
    )
    .bind(traject_id)
    .bind(bob.id)
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(count, 0);

    let active: Option<Uuid> = session.get(SESSION_KEY_ACTIVE_TRAJECT).await.unwrap();
    assert_eq!(active, None);
}

// ---------------------------------------------------------------------------
// `set_active`
// ---------------------------------------------------------------------------

#[tokio::test]
async fn set_active_rejects_non_member() {
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let alice = seed_account(&db.pool, "alice@test.local", "Alice").await;
    let bob = seed_account(&db.pool, "bob@test.local", "Bob").await;
    let traject_id = create_traject(&state, &alice, "Tarief").await;

    let session = make_session();
    let err = trajects::set_active(
        State(state),
        Extension(bob),
        session,
        Json(SetActiveTrajectRequest {
            traject_id: Some(traject_id),
        }),
    )
    .await
    .unwrap_err();
    assert_eq!(err, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn set_active_clears_session_when_traject_id_is_null() {
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let alice = seed_account(&db.pool, "alice@test.local", "Alice").await;
    let traject_id = create_traject(&state, &alice, "Tarief").await;

    let session = make_session();
    session
        .insert(SESSION_KEY_ACTIVE_TRAJECT, traject_id)
        .await
        .unwrap();

    let Json(resp) = trajects::set_active(
        State(state),
        Extension(alice),
        session.clone(),
        Json(SetActiveTrajectRequest { traject_id: None }),
    )
    .await
    .unwrap();
    assert_eq!(resp.traject_id, None);

    let active: Option<Uuid> = session.get(SESSION_KEY_ACTIVE_TRAJECT).await.unwrap();
    assert_eq!(active, None);
}
