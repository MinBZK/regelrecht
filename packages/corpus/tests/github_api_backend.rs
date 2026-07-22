//! Unit tests for `GitHubApiBackend`. Uses a wiremock server in place of
//! `api.github.com` so the tests are hermetic and the backend's HTTP
//! interactions can be asserted precisely (which queries fire, which
//! payloads land at which endpoints).

#![cfg(feature = "github")]
#![allow(clippy::unwrap_used)]

use std::path::Path;

use base64::Engine;
use regelrecht_corpus::backend::{RepoBackend, WriteContext};
use regelrecht_corpus::github_api_backend::GitHubApiBackend;
use regelrecht_corpus::models::GitHubSource;
use serde_json::json;
use wiremock::matchers::{body_partial_json, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn b64(s: &str) -> String {
    base64::engine::general_purpose::STANDARD.encode(s.as_bytes())
}

fn github_source(owner: &str, repo: &str, branch: &str, sub: Option<&str>) -> GitHubSource {
    GitHubSource {
        owner: owner.to_string(),
        repo: repo.to_string(),
        branch: branch.to_string(),
        path: sub.map(|s| s.to_string()),
        git_ref: None,
    }
}

fn backend(server: &MockServer) -> GitHubApiBackend {
    let src = github_source("acme", "corpus", "traject/abc", None);
    GitHubApiBackend::new(
        &src,
        Some("main".to_string()),
        Some("test-token".to_string()),
    )
    .unwrap()
    .with_api_base(server.uri())
}

fn ctx() -> WriteContext {
    WriteContext::new("test commit".to_string(), None)
}

/// `persist` bootstraps the traject branch lazily when `ensure_ready` never
/// ran (these tests construct the backend directly, so `branch_ready` is
/// false): mount the ref-GET reporting the branch as already existing so a
/// persisting test needs no create-branch mocks.
async fn mount_branch_exists(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/repos/acme/corpus/git/ref/heads/traject/abc"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "ref": "refs/heads/traject/abc",
            "object": { "sha": "branch-sha" },
        })))
        .mount(server)
        .await;
}

#[tokio::test]
async fn read_file_returns_content_and_caches_sha_for_later_write() {
    let server = MockServer::start().await;
    mount_branch_exists(&server).await;

    // Read returns content + sha.
    Mock::given(method("GET"))
        .and(path("/repos/acme/corpus/contents/laws/x.yaml"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "name": "x.yaml",
            "path": "laws/x.yaml",
            "sha": "sha-v1",
            "type": "file",
            "content": b64("hello\n"),
            "encoding": "base64",
        })))
        .expect(1)
        .mount(&server)
        .await;

    // PUT must reuse the cached sha.
    Mock::given(method("PUT"))
        .and(path("/repos/acme/corpus/contents/laws/x.yaml"))
        .and(body_partial_json(
            json!({ "sha": "sha-v1", "branch": "traject/abc" }),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "content": { "sha": "sha-v2" }
        })))
        .expect(1)
        .mount(&server)
        .await;

    let b = backend(&server);
    let got = b.read_file(Path::new("laws/x.yaml")).await.unwrap();
    assert_eq!(got.as_deref(), Some("hello\n"));

    b.write_file(Path::new("laws/x.yaml"), "goodbye\n")
        .await
        .unwrap();
    b.persist(&ctx()).await.unwrap();
}

#[tokio::test]
async fn persist_with_token_override_authenticates_as_the_user() {
    let server = MockServer::start().await;
    mount_branch_exists(&server).await;

    // The PUT must carry the *override* token — the acting user's own
    // credential — not the backend's baked-in "test-token". Matching on the
    // Authorization header pins the precedence in `persist`
    // (`ctx.token_override` before `self.token`); if that regressed, this
    // mock would not match and persist would fail on the resulting 404.
    Mock::given(method("PUT"))
        .and(path("/repos/acme/corpus/contents/laws/x.yaml"))
        .and(header("authorization", "Bearer user-token"))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "content": { "sha": "sha-new" }
        })))
        .expect(1)
        .mount(&server)
        .await;

    let b = backend(&server);
    b.write_file(Path::new("laws/x.yaml"), "als de gebruiker\n")
        .await
        .unwrap();
    b.persist(&WriteContext {
        message: "user-authored commit".to_string(),
        author: None,
        token_override: Some("user-token".to_string()),
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn persist_without_override_keeps_the_backend_token() {
    let server = MockServer::start().await;
    mount_branch_exists(&server).await;

    // The counterpart: no override → the configured backend token, exactly
    // the pre-spike behaviour every non-editor call site relies on.
    Mock::given(method("PUT"))
        .and(path("/repos/acme/corpus/contents/laws/x.yaml"))
        .and(header("authorization", "Bearer test-token"))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "content": { "sha": "sha-new" }
        })))
        .expect(1)
        .mount(&server)
        .await;

    let b = backend(&server);
    b.write_file(Path::new("laws/x.yaml"), "als de service\n")
        .await
        .unwrap();
    b.persist(&ctx()).await.unwrap();
}

#[tokio::test]
async fn write_without_prior_read_resolves_sha_lazily_at_persist() {
    let server = MockServer::start().await;
    mount_branch_exists(&server).await;

    // First PUT (no sha) returns 422 — file already exists; backend must
    // then GET sha and re-PUT.
    Mock::given(method("PUT"))
        .and(path("/repos/acme/corpus/contents/laws/x.yaml"))
        .and(body_partial_json(json!({ "branch": "traject/abc" })))
        .respond_with(
            ResponseTemplate::new(422).set_body_string("{\"message\":\"sha was not supplied\"}"),
        )
        .up_to_n_times(1)
        .mount(&server)
        .await;

    // SHA-resolve GET.
    Mock::given(method("GET"))
        .and(path("/repos/acme/corpus/contents/laws/x.yaml"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "name": "x.yaml",
            "path": "laws/x.yaml",
            "sha": "sha-existing",
            "type": "file",
            "content": b64("old"),
            "encoding": "base64",
        })))
        .expect(1)
        .mount(&server)
        .await;

    // Retry PUT with the resolved sha succeeds.
    Mock::given(method("PUT"))
        .and(path("/repos/acme/corpus/contents/laws/x.yaml"))
        .and(body_partial_json(json!({ "sha": "sha-existing" })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "content": { "sha": "sha-new" }
        })))
        .expect(1)
        .mount(&server)
        .await;

    let b = backend(&server);
    b.write_file(Path::new("laws/x.yaml"), "new").await.unwrap();
    b.persist(&ctx()).await.unwrap();
}

#[tokio::test]
async fn put_409_refreshes_sha_and_retries_once() {
    let server = MockServer::start().await;
    mount_branch_exists(&server).await;

    // The caller read the file once, caching sha-old.
    Mock::given(method("GET"))
        .and(path("/repos/acme/corpus/contents/laws/x.yaml"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "name": "x.yaml", "path": "laws/x.yaml",
            "sha": "sha-old", "type": "file",
            "content": b64("v1"), "encoding": "base64",
        })))
        .up_to_n_times(1)
        .mount(&server)
        .await;

    // First PUT with sha-old → 409.
    Mock::given(method("PUT"))
        .and(path("/repos/acme/corpus/contents/laws/x.yaml"))
        .and(body_partial_json(json!({ "sha": "sha-old" })))
        .respond_with(ResponseTemplate::new(409).set_body_string("conflict"))
        .up_to_n_times(1)
        .mount(&server)
        .await;

    // Backend re-fetches sha → sha-fresh.
    Mock::given(method("GET"))
        .and(path("/repos/acme/corpus/contents/laws/x.yaml"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "name": "x.yaml", "path": "laws/x.yaml",
            "sha": "sha-fresh", "type": "file",
            "content": b64("v1.5"), "encoding": "base64",
        })))
        .up_to_n_times(1)
        .mount(&server)
        .await;

    // Retry PUT with sha-fresh → 200.
    Mock::given(method("PUT"))
        .and(path("/repos/acme/corpus/contents/laws/x.yaml"))
        .and(body_partial_json(json!({ "sha": "sha-fresh" })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "content": { "sha": "sha-new" }
        })))
        .expect(1)
        .mount(&server)
        .await;

    let b = backend(&server);
    b.read_file(Path::new("laws/x.yaml")).await.unwrap();
    b.write_file(Path::new("laws/x.yaml"), "v2").await.unwrap();
    b.persist(&ctx()).await.unwrap();
}

#[tokio::test]
async fn put_403_maps_to_write_denied() {
    let server = MockServer::start().await;
    mount_branch_exists(&server).await;

    // GitHub refuses the write outright — e.g. the authenticating identity
    // has no push access, or an org's OAuth App access restrictions block
    // the token. Must surface as `WriteDenied` (with the response text for
    // logging), not as a generic `Git` error, and without any sha-refresh
    // retry (`expect(1)` pins that).
    Mock::given(method("PUT"))
        .and(path("/repos/acme/corpus/contents/laws/x.yaml"))
        .respond_with(
            ResponseTemplate::new(403)
                .set_body_string("{\"message\":\"Resource not accessible by integration\"}"),
        )
        .expect(1)
        .mount(&server)
        .await;

    let b = backend(&server);
    b.write_file(Path::new("laws/x.yaml"), "geweigerd\n")
        .await
        .unwrap();
    let err = b
        .persist(&ctx())
        .await
        .expect_err("a GitHub 403 on PUT must fail the persist");
    match err {
        regelrecht_corpus::error::CorpusError::WriteDenied(msg) => {
            assert!(
                msg.contains("Resource not accessible"),
                "the GitHub response text must ride along for logging, got: {msg}"
            );
        }
        other => panic!("expected WriteDenied, got: {other:?}"),
    }
}

#[tokio::test]
async fn delete_403_maps_to_write_denied() {
    let server = MockServer::start().await;
    mount_branch_exists(&server).await;

    // Sha-resolve GET for the blind delete.
    Mock::given(method("GET"))
        .and(path("/repos/acme/corpus/contents/laws/y.yaml"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "name": "y.yaml", "path": "laws/y.yaml",
            "sha": "sha-d1", "type": "file",
            "content": b64("doomed"), "encoding": "base64",
        })))
        .mount(&server)
        .await;

    Mock::given(method("DELETE"))
        .and(path("/repos/acme/corpus/contents/laws/y.yaml"))
        .respond_with(ResponseTemplate::new(403).set_body_string("forbidden"))
        .expect(1)
        .mount(&server)
        .await;

    let b = backend(&server);
    b.delete_file(Path::new("laws/y.yaml")).await.unwrap();
    let err = b
        .persist(&ctx())
        .await
        .expect_err("a GitHub 403 on DELETE must fail the persist");
    assert!(
        matches!(err, regelrecht_corpus::error::CorpusError::WriteDenied(_)),
        "expected WriteDenied, got: {err:?}"
    );
}

#[tokio::test]
async fn delete_path_resolves_sha_then_deletes() {
    let server = MockServer::start().await;
    mount_branch_exists(&server).await;

    // No prior read → backend must GET sha first.
    Mock::given(method("GET"))
        .and(path("/repos/acme/corpus/contents/laws/y.yaml"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "name": "y.yaml", "path": "laws/y.yaml",
            "sha": "sha-d1", "type": "file",
            "content": b64("doomed"), "encoding": "base64",
        })))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("DELETE"))
        .and(path("/repos/acme/corpus/contents/laws/y.yaml"))
        .and(body_partial_json(json!({ "sha": "sha-d1" })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
        .expect(1)
        .mount(&server)
        .await;

    let b = backend(&server);
    b.delete_file(Path::new("laws/y.yaml")).await.unwrap();
    b.persist(&ctx()).await.unwrap();
}

#[tokio::test]
async fn delete_already_gone_is_no_op() {
    let server = MockServer::start().await;
    mount_branch_exists(&server).await;

    // GET 404 → file already gone → persist swallows the delete.
    Mock::given(method("GET"))
        .and(path("/repos/acme/corpus/contents/laws/gone.yaml"))
        .respond_with(ResponseTemplate::new(404).set_body_string("not found"))
        .expect(1)
        .mount(&server)
        .await;

    let b = backend(&server);
    b.delete_file(Path::new("laws/gone.yaml")).await.unwrap();
    // No DELETE mock — if backend tried to call it, wiremock would 404
    // again and we'd see a failure. The point is that it doesn't.
    b.persist(&ctx()).await.unwrap();
}

#[tokio::test]
async fn ensure_ready_creates_branch_when_missing() {
    let server = MockServer::start().await;

    // branch_exists check → 404.
    Mock::given(method("GET"))
        .and(path("/repos/acme/corpus/git/ref/heads/traject/abc"))
        .respond_with(ResponseTemplate::new(404).set_body_string("missing"))
        .expect(1)
        .mount(&server)
        .await;

    // Resolve base ref.
    Mock::given(method("GET"))
        .and(path("/repos/acme/corpus/git/ref/heads/main"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "object": { "sha": "abc123" }
        })))
        .expect(1)
        .mount(&server)
        .await;

    // POST new ref.
    Mock::given(method("POST"))
        .and(path("/repos/acme/corpus/git/refs"))
        .and(body_partial_json(json!({
            "ref": "refs/heads/traject/abc",
            "sha": "abc123",
        })))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({})))
        .expect(1)
        .mount(&server)
        .await;

    let mut b = backend(&server);
    b.ensure_ready().await.unwrap();
}

#[tokio::test]
async fn ensure_ready_no_op_when_branch_exists() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/repos/acme/corpus/git/ref/heads/traject/abc"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "object": { "sha": "abc123" }
        })))
        .expect(1)
        .mount(&server)
        .await;

    // No POST mock — if create_branch is called, the test fails.
    let mut b = backend(&server);
    b.ensure_ready().await.unwrap();
}

#[tokio::test]
async fn list_files_filters_to_extension() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/repos/acme/corpus/contents/scenarios"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            { "name": "a.feature", "path": "scenarios/a.feature", "sha": "s1", "type": "file" },
            { "name": "b.feature", "path": "scenarios/b.feature", "sha": "s2", "type": "file" },
            { "name": "notes.md",  "path": "scenarios/notes.md",  "sha": "s3", "type": "file" },
            { "name": "subdir",    "path": "scenarios/subdir",    "sha": "s4", "type": "dir"  },
        ])))
        .mount(&server)
        .await;

    let b = backend(&server);
    let entries = b
        .list_files(Path::new("scenarios"), Some("feature"))
        .await
        .unwrap();
    let names: Vec<_> = entries.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(names, vec!["a.feature", "b.feature"]);
}

#[tokio::test]
async fn list_files_missing_directory_returns_empty() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/repos/acme/corpus/contents/scenarios"))
        .respond_with(ResponseTemplate::new(404).set_body_string("not found"))
        .mount(&server)
        .await;

    let b = backend(&server);
    let entries = b
        .list_files(Path::new("scenarios"), Some("feature"))
        .await
        .unwrap();
    assert!(entries.is_empty());
}

#[tokio::test]
async fn sub_path_prefixes_api_calls() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/repos/acme/corpus/contents/regulation/nl/wet/x.yaml"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "name": "x.yaml", "path": "regulation/nl/wet/x.yaml",
            "sha": "sub-sha", "type": "file",
            "content": b64("c"), "encoding": "base64",
        })))
        .expect(1)
        .mount(&server)
        .await;

    let src = github_source("acme", "corpus", "traject/abc", Some("regulation/nl"));
    let b = GitHubApiBackend::new(&src, Some("main".to_string()), Some("t".to_string()))
        .unwrap()
        .with_api_base(server.uri());
    let got = b.read_file(Path::new("wet/x.yaml")).await.unwrap();
    assert_eq!(got.as_deref(), Some("c"));
}

#[tokio::test]
async fn changed_files_maps_compare_to_source_relative_paths() {
    let server = MockServer::start().await;

    // Compare base...head returns two changed law files plus one file
    // outside the source's sub_path (repo-root config) that must be dropped.
    Mock::given(method("GET"))
        .and(path("/repos/acme/corpus/compare/main...traject/abc"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "files": [
                { "filename": "regulation/nl/wet/a/2025-01-01.yaml" },
                { "filename": "regulation/nl/wet/b/2025-01-01.yaml" },
                { "filename": "README.md" },
            ]
        })))
        .expect(1)
        .mount(&server)
        .await;

    let src = github_source("acme", "corpus", "traject/abc", Some("regulation/nl"));
    let b = GitHubApiBackend::new(&src, Some("main".to_string()), Some("t".to_string()))
        .unwrap()
        .with_api_base(server.uri());

    let mut changed = b.changed_files().await.unwrap();
    changed.sort();
    assert_eq!(
        changed,
        vec![
            "wet/a/2025-01-01.yaml".to_string(),
            "wet/b/2025-01-01.yaml".to_string(),
        ],
        "sub_path prefix must be stripped and out-of-subtree files dropped"
    );
}

#[tokio::test]
async fn changed_files_missing_branch_returns_empty() {
    let server = MockServer::start().await;

    // No traject branch yet (nothing saved) → Compare API 404 → empty list.
    Mock::given(method("GET"))
        .and(path("/repos/acme/corpus/compare/main...traject/abc"))
        .respond_with(ResponseTemplate::new(404).set_body_json(json!({
            "message": "Not Found"
        })))
        .expect(1)
        .mount(&server)
        .await;

    let b = backend(&server);
    assert!(b.changed_files().await.unwrap().is_empty());
}

#[tokio::test]
async fn changed_files_without_token_is_empty_and_makes_no_request() {
    let server = MockServer::start().await;
    // No mounted mock: any HTTP call would 404 from wiremock. A read-only
    // backend (no token) must short-circuit before hitting the network.
    let src = github_source("acme", "corpus", "traject/abc", None);
    let b = GitHubApiBackend::new(&src, Some("main".to_string()), None)
        .unwrap()
        .with_api_base(server.uri());
    assert!(b.changed_files().await.unwrap().is_empty());
}
