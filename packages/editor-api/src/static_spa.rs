//! Static-file serving for the built editor frontend, with an SPA index
//! fallback that answers `200` instead of `404`.
//!
//! The editor image has no nginx in front of it — this binary is the web
//! server for `frontend/dist` as well as the API — so both compression
//! and the deep-link behaviour have to be decided here or not at all.
//!
//! # Compression
//!
//! `precompressed_br` / `precompressed_gzip` make `ServeDir` look for
//! `foo.js.br` and `foo.js.gz` beside `foo.js` and serve one of those when
//! the client's `Accept-Encoding` allows it. The variants are written at
//! build time by `frontend/scripts/precompress.mjs`, so the container spends
//! no CPU compressing per request, and a client that accepts neither still
//! gets the plain file. `ServeDir` sets `Vary: accept-encoding` itself once a
//! precompressed variant is configured, so shared caches stay correct.
//!
//! Which encoding wins is the client's call, not ours: `ServeDir` picks the
//! highest q-value from `Accept-Encoding` and breaks ties in favour of
//! brotli. Builder order here is cosmetic. Note that a future
//! `precompressed_zstd()` would outrank brotli in that tie-break — worth
//! knowing before adding one.
//!
//! Compression stops at the static files on purpose: the JSON API keeps
//! serving uncompressed bodies. The index served on the deep-link path
//! gets the same flags, because it is the same `index.html` and it is
//! precompressed too.
//!
//! # Caching
//!
//! Which URLs may be cached for a year and which must revalidate is
//! decided in [`crate::static_cache`]; [`static_service`] is where that
//! decision is stamped onto the response.
//!
//! # Deep links
//!
//! The editor is a single-page app: every deep route (`/trajecten/…`,
//! `/library/…`) exists only in the client-side router, so the server has
//! no file to hand back. `ServeDir::not_found_service` does return
//! `index.html`, but it wraps the inner service in `SetStatus` and forces
//! the status to `404` — the page renders, the status code lies. Anything
//! that reads the status instead of the body (uptime monitors, crawlers,
//! `curl -f`, caches, smoketests) then treats a working page as missing.
//!
//! Answering `200` for *everything* unmatched is the opposite mistake: a
//! bundle asset that genuinely disappeared would be served as HTML, the
//! browser would fail on `Unexpected token '<'`, and the real error would
//! surface far from its cause. So the fallback is deliberately narrow —
//! see [`wants_index_fallback`] for the boundary.

use std::path::Path;
use std::path::PathBuf;

use axum::body::Body;
use axum::extract::Request;
use axum::http::{header, HeaderMap, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::routing::{any, get, MethodRouter};
use tower::ServiceExt;
use tower_http::services::{ServeDir, ServeFile};

/// Path prefixes that belong to the backend, never to the client router.
/// An unmatched path under one of these is a wrong URL or a removed
/// endpoint; handing it `index.html` would turn a broken API call into a
/// confusing HTML parse error at the caller.
const BACKEND_PREFIXES: &[&str] = &["/api", "/auth", "/health"];

/// Directories the build writes files into: `/assets` is Vite's own
/// output, `/data` is the corpus tree that `frontend/scripts/copy-laws.js`
/// copies from `public/data`. Everything under either is a file or nothing
/// at all — see [`wants_index_fallback`] gate 2.
const STATIC_DIRS: &[&str] = &["/assets", "/data"];

/// Extensions of things that are *files or nothing* outside the bundle
/// directory: sourcemaps, fonts, images, data files, and anything else
/// copied from `public/` to the root of `dist`. A request for one of
/// these that `ServeDir` could not satisfy is a genuinely missing asset
/// and must stay `404`.
///
/// Deliberately an allowlist rather than "any path with a dot": editor
/// routes do carry dots in their parameters — the legacy `/editor.html`
/// route, and werkdocument deeplinks such as
/// `/werkdocumenten/<traject>/notities/plan.md` — and those are real
/// client-side routes that deserve the index.
const ASSET_EXTENSIONS: &[&str] = &[
    "js",
    "mjs",
    "cjs",
    "css",
    "map",
    "json",
    "wasm",
    "ico",
    "png",
    "jpg",
    "jpeg",
    "gif",
    "svg",
    "webp",
    "avif",
    "woff",
    "woff2",
    "ttf",
    "otf",
    "eot",
    "txt",
    "xml",
    "webmanifest",
    "mp4",
    "webm",
];

/// Decide whether an unmatched request should be answered with the SPA
/// index (`200`) or left as a real `404`.
///
/// Four gates, all of which must pass:
///
/// 1. the path is not under a backend prefix ([`BACKEND_PREFIXES`]);
/// 2. the path is not one of the [`STATIC_DIRS`], nor under one, whatever
///    it looks like;
/// 3. the path does not end in an asset extension ([`ASSET_EXTENSIONS`]);
/// 4. the client is willing to accept HTML — an `Accept` header that asks
///    only for, say, `application/json` gets its `404` rather than a page
///    it cannot parse. An absent or wildcard `Accept` (curl, monitors)
///    counts as willing.
///
/// Gates 2 and 3 overlap on the common case and are both needed at the
/// edges. The extension allowlist alone leaves a hole exactly where the
/// build puts real files under names it does not recognise: the
/// precompressed variants `…-Ab12Cd34.js.gz` and `…-Ab12Cd34.js.br`
/// written by `frontend/scripts/precompress.mjs`, any extensionless
/// chunk, and under `/data/` the corpus itself — `*.yaml` law files,
/// `*.feature` scenarios and the `annotations.yaml` note sidecars, which
/// exist for one law in twenty-five. Those would fall through to the
/// index at `200`, and that is worse than a `404` in two ways. A bundle
/// URL answering `200 text/html` defeats caching policy keyed on the
/// immutability of hashed bundle names: the HTML gets pinned under an
/// asset URL and no deploy can reach a browser holding it. A corpus URL
/// answering `200 text/html` defeats the callers, which read the absence
/// of a file from its `404` — `useDraftNotes.js` runs `yaml.load()` over
/// whatever comes back. Neither directory ever holds a client-side
/// route, so blanket-excluding both costs nothing.
pub fn wants_index_fallback(uri: &Uri, headers: &HeaderMap) -> bool {
    let path = uri.path();

    if is_under_any(path, BACKEND_PREFIXES) {
        return false;
    }

    if is_under_any(path, STATIC_DIRS) {
        return false;
    }

    if let Some(segment) = path.rsplit('/').next() {
        if let Some((_, ext)) = segment.rsplit_once('.') {
            let ext = ext.to_ascii_lowercase();
            if ASSET_EXTENSIONS.contains(&ext.as_str()) {
                return false;
            }
        }
    }

    accepts_html(headers)
}

/// `true` when `path` is one of `prefixes` or lies under one. Matching is
/// on whole segments, so `/apiary` is not under `/api` and
/// `/assetsoverzicht` is not under `/assets`; both are ordinary client
/// routes. The bare directory itself counts as a match: `/assets` has no
/// file behind it either.
fn is_under_any(path: &str, prefixes: &[&str]) -> bool {
    prefixes.iter().any(|prefix| {
        path == *prefix
            || (path.starts_with(prefix) && path.as_bytes().get(prefix.len()) == Some(&b'/'))
    })
}

/// `true` when the `Accept` header does not rule out HTML. Missing,
/// empty or unparseable headers are treated as permissive: a monitor that
/// sends no `Accept` should still see the page it would get in a browser.
fn accepts_html(headers: &HeaderMap) -> bool {
    let Some(accept) = headers.get(header::ACCEPT) else {
        return true;
    };
    let Ok(accept) = accept.to_str() else {
        return true;
    };
    if accept.trim().is_empty() {
        return true;
    }
    accept.split(',').any(|entry| {
        let media = entry.split(';').next().unwrap_or("").trim();
        media.eq_ignore_ascii_case("text/html")
            || media.eq_ignore_ascii_case("text/*")
            || media == "*/*"
            || media.eq_ignore_ascii_case("application/xhtml+xml")
    })
}

/// The static-file service mounted as the router's fallback: `ServeDir`
/// for real files, and for everything else the narrow index fallback
/// above — served with its own status (`200`), not forced to `404`.
///
/// The outer `get` does two things. It is the method gate for the whole
/// static surface (`GET`/`HEAD`; anything else is `405`), and it gives
/// every static response a single exit point where [`static_cache`]
/// stamps its `Cache-Control`. That header depends on the URL rather
/// than on the bytes, so it has to be set per request; a plain
/// `SetResponseHeaderLayer` could not tell a hashed bundle file from the
/// index. `ServeDir` sets no `Cache-Control` of its own, and the ETag
/// and `Vary` it does set are left untouched: they are what makes the
/// `no-cache` half cheap.
pub fn static_service(static_dir: &str, index_file: impl AsRef<Path>) -> MethodRouter {
    let files = ServeDir::new(static_dir)
        .precompressed_br()
        .precompressed_gzip()
        .fallback(index_fallback_route(index_file.as_ref().to_path_buf()));

    get(move |request: Request| {
        let files = files.clone();
        async move {
            let uri = request.uri().clone();
            // `ServeDir`'s error type is `Infallible` once a fallback is
            // set, so this cannot fail.
            let mut response = files
                .oneshot(request)
                .await
                .unwrap_or_else(|e| match e {})
                .map(Body::new);
            crate::static_cache::apply(&uri, &mut response);
            response
        }
    })
}

/// `any`, not `get`: the method gate lives on the outer service in
/// [`static_service`], and `ServeDir` never reaches its fallback for
/// anything but `GET`/`HEAD` anyway.
fn index_fallback_route(index_file: PathBuf) -> MethodRouter {
    any(move |req: Request| {
        let index_file = index_file.clone();
        async move { serve_index_or_404(index_file, req).await }
    })
}

async fn serve_index_or_404(index_file: PathBuf, req: Request) -> Response {
    if !wants_index_fallback(req.uri(), req.headers()) {
        return (StatusCode::NOT_FOUND, "Not Found").into_response();
    }

    // The index on this path is the same `index.html` `ServeDir` serves
    // at `/`, and it is precompressed at build time like the rest of the
    // bundle — hence the same flags here.
    let index = ServeFile::new(&index_file)
        .precompressed_br()
        .precompressed_gzip();

    match index.oneshot(req).await {
        Ok(response) => response.map(Body::new),
        Err(e) => {
            tracing::error!(error = %e, path = %index_file.display(), "failed to serve SPA index");
            (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error").into_response()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers(accept: Option<&str>) -> HeaderMap {
        let mut headers = HeaderMap::new();
        if let Some(accept) = accept {
            headers.insert(header::ACCEPT, accept.parse().unwrap());
        }
        headers
    }

    fn wants(path: &str, accept: Option<&str>) -> bool {
        wants_index_fallback(&path.parse::<Uri>().unwrap(), &headers(accept))
    }

    #[test]
    fn deep_routes_get_the_index() {
        assert!(wants("/trajecten/zorgtoeslag-0a1b2c3d", None));
        assert!(wants("/library/wet_op_de_zorgtoeslag/3", Some("text/html")));
        assert!(wants("/editor.html", Some("text/html,*/*;q=0.8")));
        assert!(wants(
            "/werkdocumenten/zorgtoeslag-0a1b2c3d/notities/plan.md",
            None
        ));
        assert!(wants("/trajecten/zorgtoeslag-0a1b2c3d?tab=leden", None));
    }

    #[test]
    fn missing_bundle_assets_stay_404() {
        assert!(!wants("/assets/index-deadbeef.js", Some("*/*")));
        assert!(!wants("/assets/index-deadbeef.css", Some("*/*")));
        assert!(!wants("/assets/index-deadbeef.js.map", None));
        assert!(!wants("/favorites.json", None));
        assert!(!wants("/regelrecht-icon.SVG", None));
    }

    /// The hole gate 3 alone leaves: real build output under `/assets/`
    /// whose name ends in something the extension allowlist does not
    /// know. These must not become HTML at `200`.
    #[test]
    fn everything_under_assets_stays_404_whatever_the_extension() {
        assert!(!wants("/assets/index-Ab12Cd34.js.gz", Some("text/html")));
        assert!(!wants("/assets/index-Ab12Cd34.js.br", Some("text/html")));
        assert!(!wants("/assets/index-Ab12Cd34.css.br", None));
        assert!(!wants("/assets/chunk-Ab12Cd34.yaml", None));
        assert!(!wants("/assets/index-Ab12Cd34", None));
        assert!(!wants("/assets/nested/chunk-Ab12Cd34.js.gz", None));
        assert!(!wants("/assets/", Some("text/html")));
        assert!(!wants("/assets", Some("text/html")));
        // A route that merely starts with the same letters is not the
        // bundle directory.
        assert!(wants("/assetsoverzicht", None));
    }

    /// The corpus tree `frontend/scripts/copy-laws.js` writes to
    /// `public/data`. The sidecar is the sharp case: it exists for one
    /// law in twenty-five, the browser asks for it with `Accept: */*`,
    /// and `useDraftNotes.js` reads "no committed notes yet" from the
    /// `404`.
    #[test]
    fn missing_corpus_files_stay_404() {
        assert!(!wants(
            "/data/annotations/wet_op_de_zorgtoeslag/annotations.yaml",
            Some("*/*")
        ));
        assert!(!wants("/data/annotations/_vocabulary/ambiguity.yaml", None));
        assert!(!wants(
            "/data/nl/wet/wet_op_de_zorgtoeslag/2025-01-01.yaml",
            Some("text/html")
        ));
        assert!(!wants(
            "/data/nl/wet/wet_op_de_zorgtoeslag/scenarios/eligibility.feature",
            Some("*/*")
        ));
        assert!(!wants("/data/index.json", None));
        assert!(!wants("/data/", Some("text/html")));
        assert!(!wants("/data", Some("text/html")));
        // A route that merely starts with the same letters is not the
        // corpus directory.
        assert!(wants("/dataverkenner", None));
    }

    #[test]
    fn backend_paths_stay_404() {
        assert!(!wants("/api/does-not-exist", Some("text/html")));
        assert!(!wants("/auth/nope", Some("text/html")));
        assert!(!wants("/health/extra", None));
        // A path that merely starts with the same letters is a normal route.
        assert!(wants("/apiary", None));
    }

    #[test]
    fn json_clients_keep_their_404() {
        assert!(!wants(
            "/trajecten/zorgtoeslag-0a1b2c3d",
            Some("application/json")
        ));
    }
}
