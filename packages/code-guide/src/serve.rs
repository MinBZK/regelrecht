//! `serve` — the app and its data, on one port.
//!
//! The model is built from the cached index on the first request and rebuilt
//! when the index file or any source changes (newest modification time), so a
//! fresh `just code-guide-index`, or an edit, shows on the next request. A
//! source edited after indexing is reported as stale rather than misread: the
//! model leaves the references in such files out (see `model.rs`).

use std::hash::{Hash, Hasher};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::SystemTime;

use axum::extract::{Query, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use serde::{Deserialize, Serialize};
use tower_http::services::{ServeDir, ServeFile};
use walkdir::WalkDir;

use crate::index::{self, IndexState};
use crate::model::Model;
use crate::views::Views;
use crate::workspace::Workspace;

/// Default port: next to the architecture explorer's 7180, inside the dev
/// container's forwarded 7100-7300 range.
const DEFAULT_PORT: u16 = 7190;

struct Loaded {
    stamp: (Option<SystemTime>, Option<SystemTime>, u64),
    model: Arc<Model>,
}

struct AppState {
    ws: Workspace,
    loaded: Mutex<Option<Loaded>>,
}

pub fn run(ws: Workspace, port: Option<u16>, ui_dir: Option<PathBuf>) -> Result<(), String> {
    let port = port
        .or_else(|| {
            std::env::var("CODE_GUIDE_PORT")
                .ok()
                .and_then(|v| v.parse().ok())
        })
        .unwrap_or(DEFAULT_PORT);
    let ui_dir = ui_dir.unwrap_or_else(|| ws.paths.workspace.join("code-guide/ui/dist"));
    if !ui_dir.join("index.html").exists() {
        eprintln!(
            "code-guide: no built UI at {}; run `just code-guide`, which builds it first",
            ui_dir.display()
        );
    }
    let state = Arc::new(AppState {
        ws,
        loaded: Mutex::new(None),
    });
    let runtime =
        tokio::runtime::Runtime::new().map_err(|e| format!("starting the runtime: {e}"))?;
    runtime.block_on(async move {
        let index = ui_dir.join("index.html");
        let app = Router::new()
            .route("/api/status", get(status))
            .route("/api/workspace", get(workspace))
            .route("/api/crate", get(krate))
            .route("/api/type", get(type_view))
            .route("/api/function", get(function_view))
            .route("/api/source", get(source))
            .with_state(state)
            .fallback_service(ServeDir::new(&ui_dir).not_found_service(ServeFile::new(index)));
        let addr = SocketAddr::from(([0, 0, 0, 0], port));
        let listener = tokio::net::TcpListener::bind(addr)
            .await
            .map_err(|e| format!("binding {addr}: {e}"))?;
        eprintln!("code-guide: listening on http://0.0.0.0:{port}  (open http://localhost:{port})");
        axum::serve(listener, app)
            .await
            .map_err(|e| format!("server: {e}"))
    })
}

/// A fingerprint of the Rust sources of the workspace: every path with its
/// modification time. Unlike the newest time alone, it also changes when a file
/// is deleted or renamed (a rename keeps the time).
fn sources_stamp(workspace: &Path) -> u64 {
    let mut files: Vec<(PathBuf, Option<SystemTime>)> = WalkDir::new(workspace)
        .into_iter()
        .filter_entry(|e| {
            !(e.file_type().is_dir()
                && matches!(
                    e.file_name().to_str(),
                    Some("target" | "node_modules" | ".git" | "dist" | ".cargo")
                ))
        })
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("rs"))
        .map(|e| {
            let time = e.metadata().ok().and_then(|m| m.modified().ok());
            (e.into_path(), time)
        })
        .collect();
    files.sort();
    let mut hasher = std::hash::DefaultHasher::new();
    files.hash(&mut hasher);
    hasher.finish()
}

/// The current model, rebuilt when the index or a source changed.
async fn model(state: &Arc<AppState>) -> Result<Arc<Model>, Response> {
    let s = state.clone();
    let stamp = tokio::task::spawn_blocking(move || {
        let time = |p: &Path| std::fs::metadata(p).and_then(|m| m.modified()).ok();
        // The manifest is written last, so its time marks a finished build.
        (
            time(&s.ws.paths.index_file),
            time(&s.ws.paths.key_file),
            sources_stamp(&s.ws.paths.workspace),
        )
    })
    .await
    .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, format!("{e}")))?;
    if stamp.0.is_none() {
        return Err(error(
            StatusCode::SERVICE_UNAVAILABLE,
            format!(
                "No index yet. Run `{}` to build it.",
                index::REBUILD_COMMAND
            ),
        ));
    }
    {
        let guard = state.loaded.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(l) = guard.as_ref().filter(|l| l.stamp == stamp) {
            return Ok(l.model.clone());
        }
    }
    let s = state.clone();
    let built = tokio::task::spawn_blocking(move || crate::build_model(&s.ws))
        .await
        .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, format!("{e}")))?
        .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let model = Arc::new(built);
    *state.loaded.lock().unwrap_or_else(PoisonError::into_inner) = Some(Loaded {
        stamp,
        model: model.clone(),
    });
    Ok(model)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Status {
    index: IndexState,
    command: &'static str,
    counts: Option<crate::model::Counts>,
    message: Option<String>,
}

async fn status(State(state): State<Arc<AppState>>) -> Response {
    let s = state.clone();
    let index_state = tokio::task::spawn_blocking(move || {
        let version = index::ra_version()?;
        index::state(&s.ws.paths, &version).map_err(|e| e.to_string())
    })
    .await;
    let (index_state, message) = match index_state {
        Ok(Ok(st)) => (st, None),
        // Without rust-analyzer the index cannot be checked, but one may exist.
        Ok(Err(e)) => (
            if state.ws.paths.index_file.is_file() {
                IndexState::Stale
            } else {
                IndexState::Missing
            },
            Some(e),
        ),
        Err(e) => return error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };
    let counts = match index_state {
        IndexState::Missing => None,
        _ => model(&state).await.ok().map(|m| m.counts.clone()),
    };
    json(&Status {
        index: index_state,
        command: index::REBUILD_COMMAND,
        counts,
        message,
    })
}

async fn workspace(State(state): State<Arc<AppState>>) -> Response {
    match model(&state).await {
        Ok(m) => json(&Views::new(&m).workspace()),
        Err(r) => r,
    }
}

#[derive(Deserialize)]
struct CrateQuery {
    name: String,
}

async fn krate(State(state): State<Arc<AppState>>, Query(q): Query<CrateQuery>) -> Response {
    match model(&state).await {
        Ok(m) => match Views::new(&m).krate(&q.name) {
            Some(v) => json(&v),
            None => error(
                StatusCode::NOT_FOUND,
                format!("no crate named `{}`", q.name),
            ),
        },
        Err(r) => r,
    }
}

#[derive(Deserialize)]
struct ItemQuery {
    #[serde(rename = "crate")]
    krate: String,
    #[serde(default)]
    module: String,
    name: String,
}

async fn type_view(State(state): State<Arc<AppState>>, Query(q): Query<ItemQuery>) -> Response {
    match model(&state).await {
        Ok(m) => match Views::new(&m).type_view(&q.krate, &q.module, &q.name) {
            Some(v) => json(&v),
            None => error(
                StatusCode::NOT_FOUND,
                format!("no type `{}` in `{}::{}`", q.name, q.krate, q.module),
            ),
        },
        Err(r) => r,
    }
}

async fn function_view(State(state): State<Arc<AppState>>, Query(q): Query<ItemQuery>) -> Response {
    match model(&state).await {
        Ok(m) => match Views::new(&m).function_view(&q.krate, &q.module, &q.name) {
            Some(v) => json(&v),
            None => error(
                StatusCode::NOT_FOUND,
                format!("no function `{}` in `{}::{}`", q.name, q.krate, q.module),
            ),
        },
        Err(r) => r,
    }
}

#[derive(Deserialize)]
struct SourceQuery {
    path: String,
    from: Option<usize>,
    to: Option<usize>,
}

/// Most lines one `/api/source` call returns.
const MAX_SOURCE_LINES: usize = 600;

async fn source(State(state): State<Arc<AppState>>, Query(q): Query<SourceQuery>) -> Response {
    let file = match resolve_source(&state.ws.paths.workspace, &q.path) {
        Ok(f) => f,
        Err((status, msg)) => return error(status, msg.to_string()),
    };
    let text = match std::fs::read_to_string(&file) {
        Ok(t) => t,
        Err(e) => return error(StatusCode::NOT_FOUND, format!("reading {}: {e}", q.path)),
    };
    let total = text.lines().count();
    if let Some(from) = q.from {
        if from > total.max(1) {
            return error(
                StatusCode::BAD_REQUEST,
                format!("from is {from}, but the file has {total} lines"),
            );
        }
    }
    json(&slice_lines(&q.path, &text, q.from, q.to))
}

/// A workspace-relative path from a URL, as a real file: only a `.rs` file
/// inside the workspace. Checked lexically (no `..`, no absolute or
/// drive-prefixed path) and again after symlinks are resolved.
fn resolve_source(workspace: &Path, rel: &str) -> Result<PathBuf, (StatusCode, &'static str)> {
    const BAD: StatusCode = StatusCode::BAD_REQUEST;
    if rel.is_empty()
        || rel.starts_with('/')
        || rel.contains('\\')
        || rel.contains(':')
        || rel.split('/').any(|seg| seg == ".." || seg.is_empty())
    {
        return Err((BAD, "path must be a plain workspace-relative path"));
    }
    if !rel.ends_with(".rs") {
        return Err((BAD, "only .rs files can be read"));
    }
    let root = workspace
        .canonicalize()
        .map_err(|_| (StatusCode::NOT_FOUND, "workspace not found"))?;
    let file = workspace
        .join(rel)
        .canonicalize()
        .map_err(|_| (StatusCode::NOT_FOUND, "no such file"))?;
    if file.starts_with(&root) && !file.components().any(|c| c.as_os_str() == "target") {
        Ok(file)
    } else {
        Err((BAD, "path resolves outside the workspace sources"))
    }
}

#[derive(Serialize)]
struct SourceSlice<'a> {
    path: &'a str,
    from: usize,
    to: usize,
    total: usize,
    text: String,
}

fn slice_lines<'a>(
    path: &'a str,
    text: &str,
    from: Option<usize>,
    to: Option<usize>,
) -> SourceSlice<'a> {
    let lines: Vec<&str> = text.lines().collect();
    let total = lines.len();
    let from = from.unwrap_or(1).clamp(1, total.max(1));
    let to = to
        .unwrap_or(from + MAX_SOURCE_LINES - 1)
        .clamp(from, total.max(from))
        .min(from + MAX_SOURCE_LINES - 1)
        .min(total.max(1));
    let body = lines
        .get(from - 1..to.min(total))
        .map(|l| l.join("\n"))
        .unwrap_or_default();
    SourceSlice {
        path,
        from,
        to,
        total,
        text: body,
    }
}

fn json<T: Serialize>(value: &T) -> Response {
    match serde_json::to_string(value) {
        Ok(body) => ([(header::CONTENT_TYPE, "application/json")], body).into_response(),
        Err(e) => error(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("serializing: {e}"),
        ),
    }
}

fn error(status: StatusCode, message: String) -> Response {
    if status.is_server_error() {
        eprintln!("code-guide: {message}");
    }
    (status, message).into_response()
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::testutil::Dir;

    fn ws() -> Dir {
        let d = Dir::new();
        d.write("w/x/src/lib.rs", "a\nb\nc\nd\ne\n");
        d.write("w/x/Cargo.toml", "[package]\n");
        d.write("w/target/debug/build/gen.rs", "generated\n");
        d.write("secret.rs", "token\n");
        d
    }

    #[test]
    fn a_rust_file_in_the_workspace_resolves() {
        let d = ws();
        assert!(resolve_source(&d.path().join("w"), "x/src/lib.rs").is_ok());
    }

    #[test]
    fn traversal_absolute_odd_and_non_rust_paths_are_refused() {
        let d = ws();
        let w = d.path().join("w");
        for bad in [
            "",
            "../secret.rs",
            "x/../../secret.rs",
            "/etc/passwd",
            "x//src/lib.rs",
            "x\\src\\lib.rs",
            "C:/x.rs",
            "x/Cargo.toml",
            "x/src/missing.rs",
        ] {
            assert!(resolve_source(&w, bad).is_err(), "accepted `{bad}`");
        }
    }

    #[test]
    fn build_output_is_not_readable() {
        let d = ws();
        assert!(resolve_source(&d.path().join("w"), "target/debug/build/gen.rs").is_err());
    }

    #[cfg(unix)]
    #[test]
    fn a_symlink_out_of_the_workspace_is_refused() {
        let d = ws();
        std::os::unix::fs::symlink(d.path().join("secret.rs"), d.path().join("w/x/src/link.rs"))
            .unwrap();
        assert!(resolve_source(&d.path().join("w"), "x/src/link.rs").is_err());
    }

    #[test]
    fn slices_are_one_based_inclusive_clamped_and_capped() {
        let text = "a\nb\nc\nd\ne\n";
        let s = slice_lines("p", text, Some(2), Some(3));
        assert_eq!((s.from, s.to, s.total, s.text.as_str()), (2, 3, 5, "b\nc"));
        let s = slice_lines("p", text, Some(4), Some(99));
        assert_eq!((s.from, s.to, s.text.as_str()), (4, 5, "d\ne"));
        let long = (1..=2000)
            .map(|i| i.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        let s = slice_lines("p", &long, Some(1), Some(2000));
        assert_eq!(s.to - s.from + 1, MAX_SOURCE_LINES);
        assert_eq!(slice_lines("p", "", None, None).text, "");
    }
}
