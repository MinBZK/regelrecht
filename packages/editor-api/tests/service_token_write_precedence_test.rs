//! Voorrangsregel voor traject-writes: een geconfigureerd server-side
//! service-token (`CORPUS_AUTH_*`) gaat vóór het persoonlijke GitHub-token
//! van de gebruiker — het schrijfpad-spiegelbeeld van de leespad-regel uit
//! PR #955. Plus: een GitHub-403 op de write landt als een duidelijke
//! Nederlandse 403 bij de gebruiker in plaats van een generieke 500.
//!
//! Deployed-achtige federatie-config zoals `promote_user_token_write_test`:
//! een GitHub-backed writable-own plus een lokale seed als centraal corpus,
//! met de user-token-schrijfmodus AAN. Twee trajecten:
//!
//! * **mét service-token** (auth_ref met geconfigureerd
//!   `CORPUS_AUTH_*_TOKEN`-env): saves lopen over het service-token — ook
//!   voor een gebruiker zónder GitHub-koppeling (geen 428) én voor een
//!   gebruiker mét koppeling (het user-token overschrijft niet meer). De
//!   commit draagt de sessie-identiteit als committer-stempel.
//! * **zónder service-token** (auth_ref zonder env): de bestaande
//!   user-token-flow blijft werken — de save committet met het gekoppelde
//!   token (regressiebescherming).
//!
//! GitHub wordt gespeeld door wiremock via de proces-brede
//! `GITHUB_API_BASE`-seam — daarom staan alle scenario's in ÉÉN test
//! (zelfde afweging als `promote_user_token_write_test.rs`). Het
//! service-token komt uit een env var en is dus eveneens proces-breed;
//! test-binaries zijn aparte processen, dus dit lekt niet naar andere
//! testbestanden.

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
use wiremock::matchers::{body_partial_json, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use regelrecht_auth::handlers::{
    SESSION_KEY_EMAIL, SESSION_KEY_EMAIL_VERIFIED, SESSION_KEY_NAME, SESSION_KEY_SUB,
};
use regelrecht_editor_api::accounts::AccountRecord;
use regelrecht_editor_api::config::AppConfig;
use regelrecht_editor_api::corpus_handlers::save_law;
use regelrecht_editor_api::github_oauth::{self, GithubOAuth};
use regelrecht_editor_api::state::{AppState, CorpusState};
use regelrecht_editor_api::traject_corpus::TrajectCorpusCache;

use regelrecht_pipeline::test_utils::TestDb;

const LAW_ID: &str = "wet_voorbeeld_service";
const DENIED_LAW_ID: &str = "wet_voorbeeld_geweigerd";
const SERVICE_REPO: &str = "example-org/example-service-repo";
const TOKENLESS_REPO: &str = "example-org/example-traject-repo";
const OWN_BRANCH: &str = "traject-voorbeeld";
const OWN_SUBPATH: &str = "corpus/regulation";
const SERVICE_AUTH_REF: &str = "example-service-token-ref";
const SERVICE_TOKEN_ENV: &str = "CORPUS_AUTH_EXAMPLE_SERVICE_TOKEN_REF_TOKEN";
const SERVICE_TOKEN: &str = "service-token";
const USER_TOKEN: &str = "user-token";

/// De exacte gebruikerstekst waarop de "Opslaan mislukt"-modal de
/// GitHub-weigering toont (acceptatiecriterium — niet herformuleren).
const DENIED_MESSAGE: &str = "GitHub staat deze wijziging niet toe met jouw gekoppelde account: \
     geen schrijftoegang tot deze repository of organisatie.";

fn state_with_user_token_mode(pool: PgPool, oauth: GithubOAuth) -> AppState {
    AppState {
        corpus: Arc::new(RwLock::new(CorpusState::empty())),
        oidc_client: None,
        end_session_url: None,
        config: Arc::new(AppConfig {
            oidc: None,
            base_url: None,
            github_oauth: Some(oauth),
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

/// Traject met een GitHub writable-own (repo + auth_ref parametrisch) en
/// een lokale read-seed als centraal corpus.
async fn seeded_traject(
    pool: &PgPool,
    owner_id: Uuid,
    seed_dir: &std::path::Path,
    repo: &str,
    auth_ref: &str,
) -> Uuid {
    let (gh_owner, gh_repo) = repo.split_once('/').unwrap();
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
         (traject_id, source_id, name, source_type,
          gh_owner, gh_repo, gh_branch, gh_base_branch, gh_path,
          priority, auth_ref, is_writable_own)
         VALUES ($1, 'traject-own-test', 'Eigen repo', 'github'::corpus_source_type,
                 $2, $3, $4, 'main', $5,
                 0, $6, TRUE)",
    )
    .bind(traject_id)
    .bind(gh_owner)
    .bind(gh_repo)
    .bind(OWN_BRANCH)
    .bind(OWN_SUBPATH)
    .bind(auth_ref)
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

/// Seed-wetten in het "centrale corpus": de save-target en de wet waarop
/// GitHub de write gaat weigeren.
fn write_central_laws(central_dir: &std::path::Path) {
    for law_id in [LAW_ID, DENIED_LAW_ID] {
        let law_dir = central_dir.join("wet").join(law_id);
        std::fs::create_dir_all(&law_dir).unwrap();
        std::fs::write(
            law_dir.join("2025-01-01.yaml"),
            format!("$id: {law_id}\nname: Seed-versie\n"),
        )
        .unwrap();
    }
}

/// Branch-check op een repo: de traject-branch bestaat al, dus geen
/// create-branch-bootstrap nodig.
async fn mount_branch_exists(server: &MockServer, repo: &str) {
    Mock::given(method("GET"))
        .and(path(format!("/repos/{repo}/git/ref/heads/{OWN_BRANCH}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "ref": format!("refs/heads/{OWN_BRANCH}"),
            "object": { "sha": "branch-sha" },
        })))
        .mount(server)
        .await;
}

/// Lege Trees-listing zodat de indexscan van de GitHub-source niets
/// oplevert (de wetten komen uit de lokale seed) maar ook niet faalt.
async fn mount_empty_tree(server: &MockServer, repo: &str) {
    Mock::given(method("GET"))
        .and(path(format!("/repos/{repo}/git/trees/{OWN_BRANCH}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "sha": "tree-sha",
            "truncated": false,
            "tree": [],
        })))
        .mount(server)
        .await;
}

/// Alle Authorization-headers van PUT-requests die wiremock ontving.
async fn received_put_auths(server: &MockServer) -> Vec<String> {
    server
        .received_requests()
        .await
        .unwrap()
        .iter()
        .filter(|r| r.method.as_str() == "PUT")
        .map(|r| {
            r.headers
                .get("authorization")
                .map(|v| v.to_str().unwrap_or_default().to_string())
                .unwrap_or_default()
        })
        .collect()
}

/// Eén test voor alle scenario's: de `GITHUB_API_BASE`-override en het
/// `CORPUS_AUTH_*`-service-token zijn proces-brede env vars, dus parallelle
/// tests zouden elkaars configuratie verhangen.
#[tokio::test]
async fn service_token_takes_precedence_on_writes_and_github_403_maps_to_dutch_403() {
    let server = MockServer::start().await;
    std::env::set_var("GITHUB_API_BASE", server.uri());
    std::env::set_var(SERVICE_TOKEN_ENV, SERVICE_TOKEN);

    let db = TestDb::new().await;
    let central = tempfile::tempdir().unwrap();
    write_central_laws(central.path());

    let oauth = GithubOAuth::for_tests(true); // user-token-schrijfmodus AAN
    let state = state_with_user_token_mode(db.pool.clone(), oauth.clone());

    let (owner_id, sub) = seed_account(&db.pool, "alice@test.local").await;
    let account = AccountRecord {
        id: owner_id,
        person_sub: sub.clone(),
        email: "alice@test.local".to_string(),
        name: "Test User".to_string(),
    };

    // Traject A: writable-own MET geconfigureerd service-token.
    let service_traject = seeded_traject(
        &db.pool,
        owner_id,
        central.path(),
        SERVICE_REPO,
        SERVICE_AUTH_REF,
    )
    .await;
    let service_tref = traject_ref(service_traject);
    // Traject B: writable-own ZONDER service-token (auth_ref zonder env).
    let tokenless_traject = seeded_traject(
        &db.pool,
        owner_id,
        central.path(),
        TOKENLESS_REPO,
        "example-unset-token-ref",
    )
    .await;
    let tokenless_tref = traject_ref(tokenless_traject);

    mount_branch_exists(&server, SERVICE_REPO).await;
    mount_branch_exists(&server, TOKENLESS_REPO).await;
    mount_empty_tree(&server, SERVICE_REPO).await;
    mount_empty_tree(&server, TOKENLESS_REPO).await;

    // Contents-PUT op de service-repo: alleen het service-token matcht, en
    // de commit moet de sessie-identiteit als committer-stempel dragen —
    // het pre-user-token-gedrag.
    Mock::given(method("PUT"))
        .and(path(format!(
            "/repos/{SERVICE_REPO}/contents/{OWN_SUBPATH}/wet/{LAW_ID}/2025-01-01.yaml"
        )))
        .and(header("authorization", format!("Bearer {SERVICE_TOKEN}")))
        .and(body_partial_json(serde_json::json!({
            "committer": { "name": "Test User", "email": "alice@test.local" },
        })))
        .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
            "content": { "sha": "sha-new" },
        })))
        .mount(&server)
        .await;

    // GitHub weigert de write op de tweede wet: 403 (bijv. OAuth App
    // access restrictions of ontbrekende push-rechten).
    Mock::given(method("PUT"))
        .and(path(format!(
            "/repos/{SERVICE_REPO}/contents/{OWN_SUBPATH}/wet/{DENIED_LAW_ID}/2025-01-01.yaml"
        )))
        .respond_with(
            ResponseTemplate::new(403)
                .set_body_string("{\"message\":\"Resource not accessible by integration\"}"),
        )
        .mount(&server)
        .await;

    // Contents-PUT op de token-loze repo: alleen het user-token matcht.
    Mock::given(method("PUT"))
        .and(path(format!(
            "/repos/{TOKENLESS_REPO}/contents/{OWN_SUBPATH}/wet/{LAW_ID}/2025-01-01.yaml"
        )))
        .and(header("authorization", format!("Bearer {USER_TOKEN}")))
        .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
            "content": { "sha": "sha-user" },
        })))
        .expect(1)
        .mount(&server)
        .await;

    // ------------------------------------------------------------------
    // Scenario 1: service-token-repo + NIET-gekoppelde gebruiker → de
    // save slaagt over het service-token, zonder 428/koppel-flow, en de
    // commit is gestempeld met de sessie-identiteit (body-matcher op de
    // PUT-mock hierboven).
    // ------------------------------------------------------------------
    let body = format!("$id: {LAW_ID}\nname: Bewerkt zonder koppeling\n");
    save_law(
        State(state.clone()),
        Extension(account.clone()),
        session_for(&sub).await,
        Path((service_tref.clone(), LAW_ID.to_string())),
        HeaderMap::new(),
        body,
    )
    .await
    .expect("save op een service-token-repo mag geen GitHub-koppeling eisen");
    assert_eq!(
        received_put_auths(&server).await,
        vec![format!("Bearer {SERVICE_TOKEN}")],
        "de write moet over het geconfigureerde service-token lopen"
    );

    // ------------------------------------------------------------------
    // Scenario 2: service-token-repo + WEL-gekoppelde gebruiker → nog
    // steeds het service-token; het persoonlijke token overschrijft de
    // write niet meer (de oude override-regel).
    // ------------------------------------------------------------------
    let mut linked_headers = HeaderMap::new();
    linked_headers.insert(
        axum::http::header::COOKIE,
        axum::http::HeaderValue::from_str(&github_oauth::seal_token_cookie_for_tests(
            &oauth, account.id, USER_TOKEN,
        ))
        .unwrap(),
    );

    let body = format!("$id: {LAW_ID}\nname: Bewerkt met koppeling\n");
    save_law(
        State(state.clone()),
        Extension(account.clone()),
        session_for(&sub).await,
        Path((service_tref.clone(), LAW_ID.to_string())),
        linked_headers.clone(),
        body,
    )
    .await
    .expect("save met koppeling moet slagen — over het service-token");
    let auths = received_put_auths(&server).await;
    assert_eq!(
        auths,
        vec![
            format!("Bearer {SERVICE_TOKEN}"),
            format!("Bearer {SERVICE_TOKEN}"),
        ],
        "een gekoppeld user-token mag het service-token niet meer overschrijven"
    );

    // ------------------------------------------------------------------
    // Scenario 3: GitHub weigert de write (403) → de gebruiker krijgt een
    // duidelijke Nederlandse 403, niet de generieke 500.
    // ------------------------------------------------------------------
    let body = format!("$id: {DENIED_LAW_ID}\nname: Wordt geweigerd\n");
    let err = save_law(
        State(state.clone()),
        Extension(account.clone()),
        session_for(&sub).await,
        Path((service_tref.clone(), DENIED_LAW_ID.to_string())),
        HeaderMap::new(),
        body,
    )
    .await
    .expect_err("een GitHub-403 moet de save laten falen");
    assert_eq!(
        err.0,
        StatusCode::FORBIDDEN,
        "geen generieke 500: {}",
        err.1
    );
    assert_eq!(err.1, DENIED_MESSAGE);

    // ------------------------------------------------------------------
    // Scenario 4: token-loze repo + gekoppelde gebruiker → de bestaande
    // user-token-override blijft werken (de PUT-mock matcht alleen
    // `Bearer user-token`, `.expect(1)` verifieert bij drop).
    // ------------------------------------------------------------------
    let body = format!("$id: {LAW_ID}\nname: Bewerkt op token-loze repo\n");
    save_law(
        State(state.clone()),
        Extension(account.clone()),
        session_for(&sub).await,
        Path((tokenless_tref.clone(), LAW_ID.to_string())),
        linked_headers,
        body,
    )
    .await
    .expect("save op een token-loze repo moet met het gekoppelde token slagen");

    std::env::remove_var(SERVICE_TOKEN_ENV);
    std::env::remove_var("GITHUB_API_BASE");
}
