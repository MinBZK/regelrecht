//! Integration tests for the persoonlijke-notities HTTP handlers.
//!
//! Same setup as `trajects_test.rs`: an isolated Postgres container via
//! `regelrecht_pipeline::test_utils::TestDb` (exercising
//! `0024_user_notes.sql` end-to-end) and direct handler invocation with
//! inline axum extractors, skipping router/middleware plumbing.

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
use regelrecht_editor_api::user_notes::{self, NoteRequest, UserNote};

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
        harvest_admin_url: None,
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

fn note_req(value: &str) -> NoteRequest {
    NoteRequest {
        value: value.to_string(),
        format: None,
        motivation: None,
        selector: None,
    }
}

async fn create_note(
    state: &AppState,
    account: &AccountRecord,
    law_id: &str,
    value: &str,
) -> UserNote {
    let (status, Json(note)) = user_notes::create(
        State(state.clone()),
        Extension(account.clone()),
        Path(law_id.to_string()),
        Json(note_req(value)),
    )
    .await
    .unwrap();
    assert_eq!(status, StatusCode::CREATED);
    note
}

async fn list_notes(state: &AppState, account: &AccountRecord, law_id: &str) -> Vec<UserNote> {
    let Json(notes) = user_notes::list(
        State(state.clone()),
        Extension(account.clone()),
        Path(law_id.to_string()),
    )
    .await
    .unwrap();
    notes
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn create_returns_w3c_annotation_shape_with_markdown_default() {
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let alice = seed_account(&db.pool, "alice@example.org", "Alice").await;

    let note = create_note(&state, &alice, "test_wet", "**belangrijk**: zie artikel 2").await;

    assert_eq!(note.note_type, "Annotation");
    assert_eq!(note.motivation, "commenting");
    assert_eq!(note.target.source, "regelrecht://test_wet");
    assert_eq!(note.body.body_type, "TextualBody");
    assert_eq!(note.body.value, "**belangrijk**: zie artikel 2");
    assert_eq!(note.body.format, "text/markdown");
    assert_eq!(note.body.purpose, "commenting");
}

#[tokio::test]
async fn list_returns_own_notes_oldest_first() {
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let alice = seed_account(&db.pool, "alice@example.org", "Alice").await;

    create_note(&state, &alice, "test_wet", "eerste").await;
    create_note(&state, &alice, "test_wet", "tweede").await;
    create_note(&state, &alice, "andere_wet", "elders").await;

    let notes = list_notes(&state, &alice, "test_wet").await;
    let values: Vec<&str> = notes.iter().map(|n| n.body.value.as_str()).collect();
    assert_eq!(values, vec!["eerste", "tweede"]);
}

#[tokio::test]
async fn notes_are_private_per_account() {
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let alice = seed_account(&db.pool, "alice@example.org", "Alice").await;
    let bob = seed_account(&db.pool, "bob@example.org", "Bob").await;

    let note = create_note(&state, &alice, "test_wet", "alleen van alice").await;

    // Bob sees nothing.
    assert!(list_notes(&state, &bob, "test_wet").await.is_empty());

    // Bob cannot update Alice's note — indistinguishable from absent.
    let err = user_notes::update(
        State(state.clone()),
        Extension(bob.clone()),
        Path(("test_wet".to_string(), note.id)),
        Json(note_req("gekaapt")),
    )
    .await
    .unwrap_err();
    assert_eq!(err, StatusCode::NOT_FOUND);

    // Bob cannot delete Alice's note either.
    let err = user_notes::remove(
        State(state.clone()),
        Extension(bob),
        Path(("test_wet".to_string(), note.id)),
    )
    .await
    .unwrap_err();
    assert_eq!(err, StatusCode::NOT_FOUND);

    // Alice's note is untouched.
    let notes = list_notes(&state, &alice, "test_wet").await;
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].body.value, "alleen van alice");
}

#[tokio::test]
async fn update_changes_body_and_bumps_modified() {
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let alice = seed_account(&db.pool, "alice@example.org", "Alice").await;

    let note = create_note(&state, &alice, "test_wet", "oud").await;

    let Json(updated) = user_notes::update(
        State(state.clone()),
        Extension(alice.clone()),
        Path(("test_wet".to_string(), note.id)),
        Json(NoteRequest {
            value: "nieuw".to_string(),
            format: Some("text/plain".to_string()),
            motivation: Some("questioning".to_string()),
            selector: None,
        }),
    )
    .await
    .unwrap();

    assert_eq!(updated.id, note.id);
    assert_eq!(updated.body.value, "nieuw");
    assert_eq!(updated.body.format, "text/plain");
    assert_eq!(updated.motivation, "questioning");
    assert_eq!(updated.body.purpose, "questioning");
    assert_eq!(updated.created, note.created);
    assert!(updated.modified >= note.modified);

    // Absent format/motivation keep the stored values (a `{"value": ...}`
    // client cannot silently reset metadata to the defaults).
    let Json(again) = user_notes::update(
        State(state.clone()),
        Extension(alice),
        Path(("test_wet".to_string(), note.id)),
        Json(note_req("nog nieuwer")),
    )
    .await
    .unwrap();
    assert_eq!(again.body.value, "nog nieuwer");
    assert_eq!(again.body.format, "text/plain");
    assert_eq!(again.motivation, "questioning");
}

#[tokio::test]
async fn selector_roundtrips_and_is_kept_on_value_only_update() {
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let alice = seed_account(&db.pool, "alice@example.org", "Alice").await;

    let selector = serde_json::json!({
        "type": "TextQuoteSelector",
        "exact": "zorgtoeslag",
        "prefix": "aanspraak op een ",
        "suffix": " ter grootte van"
    });
    let (status, Json(note)) = user_notes::create(
        State(state.clone()),
        Extension(alice.clone()),
        Path("test_wet".to_string()),
        Json(NoteRequest {
            value: "verankerde notitie".to_string(),
            format: None,
            motivation: None,
            selector: Some(selector.clone()),
        }),
    )
    .await
    .unwrap();
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(note.target.selector, Some(selector.clone()));

    // A value-only PUT keeps the anchoring.
    let Json(updated) = user_notes::update(
        State(state.clone()),
        Extension(alice.clone()),
        Path(("test_wet".to_string(), note.id)),
        Json(note_req("aangepaste tekst")),
    )
    .await
    .unwrap();
    assert_eq!(updated.target.selector, Some(selector.clone()));

    // And the selector survives a list read.
    let notes = list_notes(&state, &alice, "test_wet").await;
    assert_eq!(notes[0].target.selector, Some(selector));

    // Invalid selectors are rejected: not an object / missing type.
    for bad in [
        serde_json::json!("tekst"),
        serde_json::json!({"exact": "x"}),
    ] {
        let err = user_notes::create(
            State(state.clone()),
            Extension(alice.clone()),
            Path("test_wet".to_string()),
            Json(NoteRequest {
                value: "x".to_string(),
                format: None,
                motivation: None,
                selector: Some(bad),
            }),
        )
        .await
        .unwrap_err();
        assert_eq!(err, StatusCode::BAD_REQUEST);
    }

    // Oversized selector (> 8 KiB serialized) is rejected.
    let err = user_notes::create(
        State(state.clone()),
        Extension(alice),
        Path("test_wet".to_string()),
        Json(NoteRequest {
            value: "x".to_string(),
            format: None,
            motivation: None,
            selector: Some(serde_json::json!({
                "type": "TextQuoteSelector",
                "exact": "a".repeat(8 * 1024 + 1)
            })),
        }),
    )
    .await
    .unwrap_err();
    assert_eq!(err, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn create_rejects_beyond_per_law_cap() {
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let alice = seed_account(&db.pool, "alice@example.org", "Alice").await;

    // Fill up to the cap (200) directly, then the next create must 409.
    sqlx::query(
        "INSERT INTO user_notes (account_id, law_id, body_value) \
         SELECT $1, 'test_wet', 'notitie ' || n FROM generate_series(1, 200) n",
    )
    .bind(alice.id)
    .execute(&db.pool)
    .await
    .unwrap();

    let err = user_notes::create(
        State(state.clone()),
        Extension(alice.clone()),
        Path("test_wet".to_string()),
        Json(note_req("te veel")),
    )
    .await
    .unwrap_err();
    assert_eq!(err, StatusCode::CONFLICT);

    // Another law is unaffected by the cap.
    create_note(&state, &alice, "andere_wet", "past nog").await;
}

#[tokio::test]
async fn delete_removes_note() {
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let alice = seed_account(&db.pool, "alice@example.org", "Alice").await;

    let note = create_note(&state, &alice, "test_wet", "weg ermee").await;

    let status = user_notes::remove(
        State(state.clone()),
        Extension(alice.clone()),
        Path(("test_wet".to_string(), note.id)),
    )
    .await
    .unwrap();
    assert_eq!(status, StatusCode::NO_CONTENT);

    assert!(list_notes(&state, &alice, "test_wet").await.is_empty());

    // Deleting again is a 404.
    let err = user_notes::remove(
        State(state.clone()),
        Extension(alice),
        Path(("test_wet".to_string(), note.id)),
    )
    .await
    .unwrap_err();
    assert_eq!(err, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn rejects_invalid_input() {
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let alice = seed_account(&db.pool, "alice@example.org", "Alice").await;

    // Empty / whitespace-only body.
    for value in ["", "   \n\t"] {
        let err = user_notes::create(
            State(state.clone()),
            Extension(alice.clone()),
            Path("test_wet".to_string()),
            Json(note_req(value)),
        )
        .await
        .unwrap_err();
        assert_eq!(err, StatusCode::BAD_REQUEST);
    }

    // Unknown format / motivation.
    let err = user_notes::create(
        State(state.clone()),
        Extension(alice.clone()),
        Path("test_wet".to_string()),
        Json(NoteRequest {
            value: "x".to_string(),
            format: Some("text/html".to_string()),
            motivation: None,
            selector: None,
        }),
    )
    .await
    .unwrap_err();
    assert_eq!(err, StatusCode::BAD_REQUEST);

    let err = user_notes::create(
        State(state.clone()),
        Extension(alice.clone()),
        Path("test_wet".to_string()),
        Json(NoteRequest {
            value: "x".to_string(),
            format: None,
            motivation: Some("linking".to_string()),
            selector: None,
        }),
    )
    .await
    .unwrap_err();
    assert_eq!(err, StatusCode::BAD_REQUEST);

    // Oversized body value (> 64 KiB).
    let err = user_notes::create(
        State(state.clone()),
        Extension(alice.clone()),
        Path("test_wet".to_string()),
        Json(note_req(&"a".repeat(64 * 1024 + 1))),
    )
    .await
    .unwrap_err();
    assert_eq!(err, StatusCode::BAD_REQUEST);

    // Empty and oversized law id.
    for law_id in [String::new(), "x".repeat(257)] {
        let err = user_notes::list(State(state.clone()), Extension(alice.clone()), Path(law_id))
            .await
            .unwrap_err();
        assert_eq!(err, StatusCode::BAD_REQUEST);
    }
}
