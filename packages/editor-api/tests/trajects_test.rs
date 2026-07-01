//! Integration tests for the traject HTTP handlers.
//!
//! Each test spins up an isolated Postgres container via
//! `regelrecht_pipeline::test_utils::TestDb`, so the migrations under
//! `packages/pipeline/migrations/` are exercised end-to-end — including
//! `0014_trajects.sql`. Handlers are invoked directly (with `axum`
//! extractors constructed inline) rather than through a `Router`, which
//! keeps the tests focused on the handler logic and skips the cookie /
//! middleware plumbing.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::{Extension, Json};
use pretty_assertions::assert_eq;
use sqlx::PgPool;
use tokio::sync::{Mutex, RwLock};
use uuid::Uuid;

use regelrecht_editor_api::accounts::AccountRecord;
use regelrecht_editor_api::config::AppConfig;
use regelrecht_editor_api::state::{AppState, CorpusState};
use regelrecht_editor_api::traject_corpus::TrajectCorpusCache;
use regelrecht_editor_api::trajects::{
    self, AddMemberRequest, CreateTrajectRequest, UpdateMemberRequest, UpdateTrajectRequest,
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
            github_oauth: None,
        }),
        http_client: reqwest::Client::new(),
        pool: Some(pool),
        pipeline_api_url: None,
        reload_lock: Arc::new(Mutex::new(())),
        trajects: Arc::new(TrajectCorpusCache::new()),
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
        repo_owner: None,
        repo_name: None,
        base_branch: None,
        repo_path: None,
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

/// Test helper that calls `trajects::add_member` and returns either the
/// resulting `(StatusCode::OK, "active" | "pending")` on success or the
/// error status on failure. Tests that care about the response body call
/// `trajects::add_member` directly.
async fn add_member(
    state: &AppState,
    owner: &AccountRecord,
    traject_id: Uuid,
    invitee_email: &str,
    role: &str,
) -> Result<&'static str, StatusCode> {
    trajects::add_member(
        State(state.clone()),
        Extension(owner.clone()),
        Path(traject_id),
        Json(AddMemberRequest {
            email: invitee_email.to_string(),
            role: role.to_string(),
        }),
    )
    .await
    .map(|Json(body)| body.status)
}

// ---------------------------------------------------------------------------
// `create`
// ---------------------------------------------------------------------------

#[tokio::test]
async fn create_inserts_traject_with_creator_as_owner_and_writable_own_source() {
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
    assert_eq!(summary.role, "owner");

    let (role,): (String,) = sqlx::query_as(
        "SELECT role::text FROM traject_members WHERE traject_id = $1 AND account_id = $2",
    )
    .bind(summary.id)
    .bind(alice.id)
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(role, "owner");

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
    let (status, _msg) = trajects::create(State(state), Extension(alice), Json(req))
        .await
        .unwrap_err();
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn normal_traject_is_writable_with_github_own() {
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let owner = seed_account(&db.pool, "owner2@example.com", "Owner2").await;

    let id = create_traject(&state, &owner, "Gewoon traject").await;

    let src_type: String = sqlx::query_scalar(
        "SELECT source_type::text FROM traject_corpus_sources
         WHERE traject_id = $1 AND is_writable_own = TRUE",
    )
    .bind(id)
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(src_type, "github");
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
    assert_eq!(detail.summary.role, "owner");
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
async fn update_rejects_contributor_and_accepts_owner() {
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let alice = seed_account(&db.pool, "alice@test.local", "Alice").await;
    let bob = seed_account(&db.pool, "bob@test.local", "Bob").await;
    let traject_id = create_traject(&state, &alice, "Tarief").await;
    assert_eq!(
        add_member(&state, &alice, traject_id, &bob.email, "contributor").await,
        Ok("active")
    );

    let body = UpdateTrajectRequest {
        name: Some("Renamed".to_string()),
        description: None,
        scope: None,
        status: None,
    };

    // contributor → 403
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

    // owner → 204
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
async fn add_member_with_unknown_email_creates_pending_invite() {
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let alice = seed_account(&db.pool, "alice@test.local", "Alice").await;
    let traject_id = create_traject(&state, &alice, "Tarief").await;

    let status = add_member(
        &state,
        &alice,
        traject_id,
        "ghost@test.local",
        "contributor",
    )
    .await;
    assert_eq!(status, Ok("pending"));

    let (email, role): (String, String) =
        sqlx::query_as("SELECT email, role::text FROM traject_invites WHERE traject_id = $1")
            .bind(traject_id)
            .fetch_one(&db.pool)
            .await
            .unwrap();
    assert_eq!(email, "ghost@test.local");
    assert_eq!(role, "contributor");

    // No traject_members row was created — only the inviter.
    let (member_count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM traject_members WHERE traject_id = $1")
            .bind(traject_id)
            .fetch_one(&db.pool)
            .await
            .unwrap();
    assert_eq!(member_count, 1);
}

#[tokio::test]
async fn add_member_normalizes_email_case_for_pending_invites() {
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let alice = seed_account(&db.pool, "alice@test.local", "Alice").await;
    let traject_id = create_traject(&state, &alice, "Tarief").await;

    assert_eq!(
        add_member(
            &state,
            &alice,
            traject_id,
            "Mixed@Case.LOCAL",
            "contributor"
        )
        .await,
        Ok("pending")
    );
    // A re-invite using a different casing of the same email must
    // collide on the (traject_id, email) primary key.
    assert_eq!(
        add_member(&state, &alice, traject_id, "mixed@case.local", "owner").await,
        Ok("pending")
    );

    let rows: Vec<(String, String)> =
        sqlx::query_as("SELECT email, role::text FROM traject_invites WHERE traject_id = $1")
            .bind(traject_id)
            .fetch_all(&db.pool)
            .await
            .unwrap();
    assert_eq!(rows.len(), 1, "case variants must collide");
    assert_eq!(rows[0].0, "mixed@case.local");
    assert_eq!(rows[0].1, "owner", "re-invite updates the role");
}

#[tokio::test]
async fn add_member_upserts_role_on_conflict() {
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let alice = seed_account(&db.pool, "alice@test.local", "Alice").await;
    let bob = seed_account(&db.pool, "bob@test.local", "Bob").await;
    let traject_id = create_traject(&state, &alice, "Tarief").await;

    assert_eq!(
        add_member(&state, &alice, traject_id, &bob.email, "contributor").await,
        Ok("active")
    );
    assert_eq!(
        add_member(&state, &alice, traject_id, &bob.email, "owner").await,
        Ok("active")
    );

    let (role,): (String,) = sqlx::query_as(
        "SELECT role::text FROM traject_members WHERE traject_id = $1 AND account_id = $2",
    )
    .bind(traject_id)
    .bind(bob.id)
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(role, "owner");
}

#[tokio::test]
async fn add_member_blocks_demoting_last_owner_via_upsert() {
    // Without this guard, add_member would be a back-door around the
    // last-owner check that update_member already enforces.
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let alice = seed_account(&db.pool, "alice@test.local", "Alice").await;
    let traject_id = create_traject(&state, &alice, "Tarief").await;

    let status = add_member(&state, &alice, traject_id, &alice.email, "contributor").await;
    assert_eq!(status, Err(StatusCode::CONFLICT));
}

#[tokio::test]
async fn update_member_blocks_demoting_last_owner() {
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let alice = seed_account(&db.pool, "alice@test.local", "Alice").await;
    let traject_id = create_traject(&state, &alice, "Tarief").await;

    let err = trajects::update_member(
        State(state),
        Extension(alice.clone()),
        Path((traject_id, alice.id)),
        Json(UpdateMemberRequest {
            role: "contributor".to_string(),
        }),
    )
    .await
    .unwrap_err();
    assert_eq!(err, StatusCode::CONFLICT);
}

#[tokio::test]
async fn remove_member_blocks_removing_last_owner() {
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
async fn delete_is_owner_only_and_cascades() {
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let alice = seed_account(&db.pool, "alice@test.local", "Alice").await;
    let bob = seed_account(&db.pool, "bob@test.local", "Bob").await;
    let traject_id = create_traject(&state, &alice, "Tarief").await;
    assert_eq!(
        add_member(&state, &alice, traject_id, &bob.email, "contributor").await,
        Ok("active")
    );

    // contributor cannot delete
    let err = trajects::delete(State(state.clone()), Extension(bob), Path(traject_id))
        .await
        .unwrap_err();
    assert_eq!(err, StatusCode::FORBIDDEN);

    // owner can
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
async fn leave_blocks_last_owner() {
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let alice = seed_account(&db.pool, "alice@test.local", "Alice").await;
    let traject_id = create_traject(&state, &alice, "Tarief").await;

    let err = trajects::leave(State(state), Extension(alice), Path(traject_id))
        .await
        .unwrap_err();
    assert_eq!(err, StatusCode::CONFLICT);
}

#[tokio::test]
async fn leave_allows_contributor() {
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let alice = seed_account(&db.pool, "alice@test.local", "Alice").await;
    let bob = seed_account(&db.pool, "bob@test.local", "Bob").await;
    let traject_id = create_traject(&state, &alice, "Tarief").await;
    assert_eq!(
        add_member(&state, &alice, traject_id, &bob.email, "contributor").await,
        Ok("active")
    );

    let ok = trajects::leave(
        State(state.clone()),
        Extension(bob.clone()),
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
}

// ---------------------------------------------------------------------------
// Pending invites + promotion
// ---------------------------------------------------------------------------

#[tokio::test]
async fn pending_invite_is_promoted_to_membership_on_login() {
    // End-to-end: owner invites an unknown email, the invitee later logs
    // in (their account is upserted), and the next time
    // `account_middleware` runs the invite turns into a real membership.
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let alice = seed_account(&db.pool, "alice@test.local", "Alice").await;
    let traject_id = create_traject(&state, &alice, "Tarief").await;

    assert_eq!(
        add_member(
            &state,
            &alice,
            traject_id,
            "Ghost@Test.Local",
            "contributor"
        )
        .await,
        Ok("pending")
    );

    // Simulate the invitee logging in: a new accounts row is created.
    let ghost = seed_account(&db.pool, "ghost@test.local", "Ghost").await;

    // Simulate `account_middleware` post-upsert step.
    regelrecht_editor_api::accounts::promote_pending_invites(&db.pool, ghost.id, &ghost.email)
        .await;

    let (invite_count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM traject_invites WHERE traject_id = $1")
            .bind(traject_id)
            .fetch_one(&db.pool)
            .await
            .unwrap();
    assert_eq!(invite_count, 0);

    let (role,): (String,) = sqlx::query_as(
        "SELECT role::text FROM traject_members WHERE traject_id = $1 AND account_id = $2",
    )
    .bind(traject_id)
    .bind(ghost.id)
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(role, "contributor");

    let Json(list) = trajects::list(State(state.clone()), Extension(ghost.clone()))
        .await
        .unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].id, traject_id);
    assert_eq!(list[0].role, "contributor");
}

#[tokio::test]
async fn promotion_is_idempotent_when_user_is_already_a_member() {
    // Defence-in-depth: even if an invite somehow co-exists with a
    // membership for the same email + traject, promotion must not error
    // and the invite must be cleaned up. Bob's existing role wins.
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let alice = seed_account(&db.pool, "alice@test.local", "Alice").await;
    let bob = seed_account(&db.pool, "bob@test.local", "Bob").await;
    let traject_id = create_traject(&state, &alice, "Tarief").await;

    add_member(&state, &alice, traject_id, &bob.email, "contributor")
        .await
        .unwrap();

    sqlx::query(
        "INSERT INTO traject_invites (traject_id, email, role, invited_by)
         VALUES ($1, $2, 'owner', $3)",
    )
    .bind(traject_id)
    .bind(&bob.email)
    .bind(alice.id)
    .execute(&db.pool)
    .await
    .unwrap();

    regelrecht_editor_api::accounts::promote_pending_invites(&db.pool, bob.id, &bob.email).await;

    let (invite_count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM traject_invites WHERE email = $1")
            .bind(&bob.email)
            .fetch_one(&db.pool)
            .await
            .unwrap();
    assert_eq!(invite_count, 0);

    let (role,): (String,) = sqlx::query_as(
        "SELECT role::text FROM traject_members WHERE traject_id = $1 AND account_id = $2",
    )
    .bind(traject_id)
    .bind(bob.id)
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(role, "contributor");
}

#[tokio::test]
async fn get_returns_pending_invites_alongside_members() {
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let alice = seed_account(&db.pool, "alice@test.local", "Alice").await;
    let traject_id = create_traject(&state, &alice, "Tarief").await;

    assert_eq!(
        add_member(
            &state,
            &alice,
            traject_id,
            "ghost@test.local",
            "contributor"
        )
        .await,
        Ok("pending")
    );

    let Json(detail) = trajects::get(State(state), Extension(alice), Path(traject_id))
        .await
        .unwrap();
    assert_eq!(detail.members.len(), 1, "alice is still the only member");
    assert_eq!(detail.pending_invites.len(), 1);
    assert_eq!(detail.pending_invites[0].email, "ghost@test.local");
    assert_eq!(detail.pending_invites[0].role, "contributor");
}

#[tokio::test]
async fn remove_invite_deletes_and_is_idempotent_404() {
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let alice = seed_account(&db.pool, "alice@test.local", "Alice").await;
    let traject_id = create_traject(&state, &alice, "Tarief").await;

    assert_eq!(
        add_member(
            &state,
            &alice,
            traject_id,
            "ghost@test.local",
            "contributor"
        )
        .await,
        Ok("pending")
    );

    let ok = trajects::remove_invite(
        State(state.clone()),
        Extension(alice.clone()),
        Path((traject_id, "ghost@test.local".to_string())),
    )
    .await
    .unwrap();
    assert_eq!(ok, StatusCode::NO_CONTENT);

    let err = trajects::remove_invite(
        State(state),
        Extension(alice),
        Path((traject_id, "ghost@test.local".to_string())),
    )
    .await
    .unwrap_err();
    assert_eq!(err, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn remove_invite_requires_owner() {
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let alice = seed_account(&db.pool, "alice@test.local", "Alice").await;
    let bob = seed_account(&db.pool, "bob@test.local", "Bob").await;
    let traject_id = create_traject(&state, &alice, "Tarief").await;
    add_member(&state, &alice, traject_id, &bob.email, "contributor")
        .await
        .unwrap();
    add_member(
        &state,
        &alice,
        traject_id,
        "ghost@test.local",
        "contributor",
    )
    .await
    .unwrap();

    let err = trajects::remove_invite(
        State(state),
        Extension(bob),
        Path((traject_id, "ghost@test.local".to_string())),
    )
    .await
    .unwrap_err();
    assert_eq!(err, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn add_member_active_path_cleans_up_stale_invite() {
    // Race window: invite an unknown email → ghost's account gets
    // created later (e.g. they registered with the IdP but haven't
    // hit any API yet, so promote_pending_invites hasn't run) → owner
    // re-invites. The active path must clean up the stale invite so
    // GET doesn't return ghost in both members and pending_invites.
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let alice = seed_account(&db.pool, "alice@test.local", "Alice").await;
    let traject_id = create_traject(&state, &alice, "Tarief").await;

    assert_eq!(
        add_member(
            &state,
            &alice,
            traject_id,
            "ghost@test.local",
            "contributor"
        )
        .await,
        Ok("pending")
    );

    // Ghost's account materialises (e.g. via OIDC login) but
    // promote_pending_invites hasn't run yet.
    let _ghost = seed_account(&db.pool, "ghost@test.local", "Ghost").await;

    // Owner re-invites — now the active path is taken.
    assert_eq!(
        add_member(
            &state,
            &alice,
            traject_id,
            "ghost@test.local",
            "contributor"
        )
        .await,
        Ok("active")
    );

    let (invite_count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM traject_invites WHERE traject_id = $1")
            .bind(traject_id)
            .fetch_one(&db.pool)
            .await
            .unwrap();
    assert_eq!(invite_count, 0, "stale invite must be cleaned up");
}

#[tokio::test]
async fn get_hides_pending_invites_from_contributors() {
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let alice = seed_account(&db.pool, "alice@test.local", "Alice").await;
    let bob = seed_account(&db.pool, "bob@test.local", "Bob").await;
    let traject_id = create_traject(&state, &alice, "Tarief").await;
    add_member(&state, &alice, traject_id, &bob.email, "contributor")
        .await
        .unwrap();
    add_member(
        &state,
        &alice,
        traject_id,
        "ghost@test.local",
        "contributor",
    )
    .await
    .unwrap();

    // Alice (owner) sees the pending invite.
    let Json(owner_view) = trajects::get(State(state.clone()), Extension(alice), Path(traject_id))
        .await
        .unwrap();
    assert_eq!(owner_view.pending_invites.len(), 1);

    // Bob (contributor) does not.
    let Json(contributor_view) = trajects::get(State(state), Extension(bob), Path(traject_id))
        .await
        .unwrap();
    assert_eq!(contributor_view.pending_invites.len(), 0);
    assert_eq!(
        contributor_view.members.len(),
        2,
        "members list itself still visible to contributors"
    );
}

#[tokio::test]
async fn add_member_rejects_malformed_emails() {
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let alice = seed_account(&db.pool, "alice@test.local", "Alice").await;
    let traject_id = create_traject(&state, &alice, "Tarief").await;

    for bad in [
        "notanemail",
        "@nolocal.com",
        "nodomain@",
        "no-dot-in-domain@localhost",
        "two@@signs.com",
        "   ",
    ] {
        assert_eq!(
            add_member(&state, &alice, traject_id, bad, "contributor").await,
            Err(StatusCode::BAD_REQUEST),
            "expected 400 for {bad:?}",
        );
    }
}
