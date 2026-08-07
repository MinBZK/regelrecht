//! End-to-end test for the SPA deeplink fallback.
//!
//! The regression this pins: the editor answered every deep route with
//! `index.html` under a `404` status, so the page rendered but every
//! status-reading client (monitor, crawler, `curl -f`, cache) saw a
//! missing page. These tests drive the real fallback service against a
//! real static directory on disk, so the premise is actually built: the
//! deeplink resolves to no file, the asset resolves to no file, and the
//! two must end up with different statuses.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use pretty_assertions::assert_eq;
use tower::ServiceExt;

use regelrecht_editor_api::static_spa::static_service;

const INDEX_HTML: &str = "<!doctype html><title>regelrecht editor</title><div id=\"app\"></div>";

const INDEX_BROTLI: &str = "pretend brotli index";

const SIDECAR_YAML: &str = "annotations: []\n";

/// A static dir shaped like a Vite build: an index with its precompressed
/// variant, one content-hashed asset with both variants beside it, one
/// public file, and the corpus tree `frontend/scripts/copy-laws.js` writes
/// to `public/data` — with a note sidecar for one law and none for the
/// other, which is the ratio in the real corpus. Everything else
/// genuinely does not exist.
fn static_dir() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("index.html"), INDEX_HTML).unwrap();
    std::fs::write(dir.path().join("index.html.br"), INDEX_BROTLI).unwrap();
    std::fs::create_dir(dir.path().join("assets")).unwrap();
    std::fs::write(
        dir.path().join("assets/index-Ab12Cd34.js"),
        "console.log('bundle');",
    )
    .unwrap();
    std::fs::write(dir.path().join("assets/index-Ab12Cd34.js.gz"), "gz").unwrap();
    std::fs::write(dir.path().join("assets/index-Ab12Cd34.js.br"), "br").unwrap();
    std::fs::write(dir.path().join("favorites.json"), "[]").unwrap();
    let sidecar = dir
        .path()
        .join("data/annotations/wet_op_de_zorgtoeslag/annotations.yaml");
    std::fs::create_dir_all(sidecar.parent().unwrap()).unwrap();
    std::fs::write(&sidecar, SIDECAR_YAML).unwrap();
    dir
}

fn app(dir: &tempfile::TempDir) -> Router {
    let static_path = dir.path().to_string_lossy().into_owned();
    let index = dir.path().join("index.html");
    Router::new()
        .route("/health", axum::routing::get(|| async { "OK" }))
        .fallback_service(static_service(&static_path, &index))
}

async fn get(dir: &tempfile::TempDir, path: &str, accept: Option<&str>) -> (StatusCode, String) {
    let response = raw_get(dir, path, accept, None).await;
    let status = response.status();
    (status, body_of(response).await)
}

async fn raw_get(
    dir: &tempfile::TempDir,
    path: &str,
    accept: Option<&str>,
    accept_encoding: Option<&str>,
) -> axum::http::Response<Body> {
    let mut request = Request::builder().uri(path).method("GET");
    if let Some(accept) = accept {
        request = request.header("accept", accept);
    }
    if let Some(encoding) = accept_encoding {
        request = request.header("accept-encoding", encoding);
    }
    app(dir)
        .oneshot(request.body(Body::empty()).unwrap())
        .await
        .unwrap()
}

async fn body_of(response: axum::http::Response<Body>) -> String {
    let body = axum::body::to_bytes(response.into_body(), 1024 * 1024)
        .await
        .unwrap();
    String::from_utf8_lossy(&body).into_owned()
}

#[tokio::test]
async fn deeplink_answers_200_with_the_index() {
    let dir = static_dir();

    // Sanity: the deeplink is not a file on disk, so this really exercises
    // the fallback rather than a lucky hit in ServeDir.
    assert!(!dir.path().join("trajecten").exists());

    let (status, body) = get(
        &dir,
        "/trajecten/zorgtoeslag-0a1b2c3d/corpus/wet_op_de_zorgtoeslag/3",
        Some("text/html,application/xhtml+xml"),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, INDEX_HTML);
}

#[tokio::test]
async fn deeplink_without_accept_header_also_answers_200() {
    // The uptime-monitor / `curl -f` case: no Accept header at all.
    let dir = static_dir();
    let (status, _) = get(&dir, "/trajecten/zorgtoeslag-0a1b2c3d", None).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn missing_bundle_asset_stays_404() {
    let dir = static_dir();
    assert!(!dir.path().join("assets/index-gone.js").exists());

    let (status, body) = get(&dir, "/assets/index-gone.js", Some("*/*")).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_ne!(
        body, INDEX_HTML,
        "a missing script must not be answered with HTML"
    );
}

#[tokio::test]
async fn existing_asset_is_still_served() {
    let dir = static_dir();
    let (status, body) = get(&dir, "/assets/index-Ab12Cd34.js", Some("*/*")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, "console.log('bundle');");
}

#[tokio::test]
async fn unknown_api_path_stays_404() {
    let dir = static_dir();
    let (status, body) = get(&dir, "/api/verdwenen-endpoint", Some("application/json")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_ne!(body, INDEX_HTML);
}

#[tokio::test]
async fn json_client_on_a_deep_route_stays_404() {
    let dir = static_dir();
    let (status, _) = get(
        &dir,
        "/trajecten/zorgtoeslag-0a1b2c3d",
        Some("application/json"),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn real_routes_are_untouched() {
    let dir = static_dir();
    let (status, body) = get(&dir, "/health", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, "OK");
}

#[tokio::test]
async fn root_serves_the_index_directly() {
    let dir = static_dir();
    let (status, body) = get(&dir, "/", Some("text/html")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, INDEX_HTML);
}

#[tokio::test]
async fn non_get_on_an_unmatched_path_is_405() {
    let dir = static_dir();
    let response = app(&dir)
        .oneshot(
            Request::builder()
                .uri("/trajecten/zorgtoeslag-0a1b2c3d")
                .method("POST")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
}

/// The hole the extension allowlist alone leaves. `precompress.mjs`
/// writes `.js.gz` and `.js.br` beside every bundle file, and neither
/// tail is in the allowlist, so without the `/assets/` gate this URL
/// would answer `200 text/html`. Under a cache policy that treats hashed
/// bundle names as immutable, a visitor would then hold that HTML at an
/// asset URL for a year and no deploy could reach them.
#[tokio::test]
async fn missing_precompressed_variant_under_assets_stays_404() {
    let dir = static_dir();
    // The premise: this exact file is not on disk, while a sibling of the
    // very same shape is — so the 404 comes from the gate, not from an
    // empty directory that would 404 anything.
    assert!(!dir.path().join("assets/index-Zz98Yx76.js.gz").exists());
    assert!(dir.path().join("assets/index-Ab12Cd34.js.gz").exists());

    let (status, body) = get(&dir, "/assets/index-Zz98Yx76.js.gz", Some("text/html")).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_ne!(
        body, INDEX_HTML,
        "a missing bundle variant must not be answered with the index"
    );
}

/// Same gate, for the shapes that carry no allowlisted extension at all.
#[tokio::test]
async fn extensionless_and_unknown_assets_stay_404() {
    let dir = static_dir();
    for path in [
        "/assets/index-Zz98Yx76",
        "/assets/chunk-Zz98Yx76.yaml",
        "/assets/nested/chunk-Zz98Yx76.js.br",
    ] {
        let (status, body) = get(&dir, path, Some("text/html")).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
        assert_ne!(body, INDEX_HTML, "{path}");
    }
}

/// The note sidecar is the case the editor reads a `404` as data:
/// `useDraftNotes.js` treats it as "no committed notes yet" and exports
/// the drafts alone. Answer `200 text/html` instead and it runs
/// `yaml.load()` over an HTML document. The browser asks with
/// `Accept: */*`, so gate 4 does not save this one.
#[tokio::test]
async fn missing_note_sidecar_stays_404() {
    let dir = static_dir();
    let present = "/data/annotations/wet_op_de_zorgtoeslag/annotations.yaml";
    let absent = "/data/annotations/algemene_wet_bestuursrecht/annotations.yaml";

    let (status, body) = get(&dir, present, Some("*/*")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, SIDECAR_YAML);

    let (status, body) = get(&dir, absent, Some("*/*")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_ne!(
        body, INDEX_HTML,
        "a law without notes must not be answered with the index"
    );
}

/// The rest of the corpus tree, which the extension allowlist knows just
/// as little about: law YAML and the scenario `.feature` files copied
/// alongside them.
#[tokio::test]
async fn missing_corpus_files_stay_404() {
    let dir = static_dir();
    for path in [
        "/data/nl/wet/wet_op_de_zorgtoeslag/2025-01-01.yaml",
        "/data/nl/wet/wet_op_de_zorgtoeslag/scenarios/eligibility.feature",
    ] {
        let (status, body) = get(&dir, path, Some("*/*")).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
        assert_ne!(body, INDEX_HTML, "{path}");
    }
}

/// The precompressed variants must survive on the fallback path too, or
/// deep links quietly stop being compressed while the rest of the bundle
/// still is.
#[tokio::test]
async fn the_deeplink_index_is_still_precompressed() {
    let dir = static_dir();
    let response = raw_get(
        &dir,
        "/trajecten/zorgtoeslag-0a1b2c3d",
        Some("text/html"),
        Some("br"),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["content-encoding"], "br");
    assert_eq!(body_of(response).await, INDEX_BROTLI);
}
