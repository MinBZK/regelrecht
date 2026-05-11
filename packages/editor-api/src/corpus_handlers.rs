use std::path::PathBuf;
use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tower_sessions::Session;

use regelrecht_auth::handlers::{SESSION_KEY_EMAIL, SESSION_KEY_NAME};
use regelrecht_corpus::backend::{EditorUser, PersistOutcome, RepoBackend, WriteContext};
use regelrecht_corpus::dto::{build_source_summaries, PaginationParams, SourceSummary};
use regelrecht_corpus::source_map::{
    collect_law_outputs, extract_law_id, resolve_display_name, validate_yaml_syntax, LoadedLaw,
};
use regelrecht_corpus::CorpusError;

use crate::state::{AppState, SessionResolveError};

/// Header used by the frontend to scope writes to one editor session.
///
/// The frontend mints a UUID in `sessionStorage` on first load and sends
/// it on every save. The backend uses it to key per-(session, source)
/// `SessionGitBackend`s so all edits in one browser session land on the
/// same upstream feature branch / PR.
const EDITOR_SESSION_HEADER: &str = "X-Editor-Session";

/// Response body for a successful save.
///
/// `pr` is `None` for local-source saves (no upstream PR to open) and
/// populated for federated GitHub-source saves so the frontend can render
/// a "Bekijk op GitHub" link.
#[derive(Debug, Serialize)]
pub struct SaveResponse {
    pub pr: Option<SavePrInfo>,
}

#[derive(Debug, Serialize)]
pub struct SavePrInfo {
    pub url: String,
    pub number: u64,
    pub branch: String,
}

/// A law entry with source provenance.
#[derive(Debug, Serialize)]
pub struct CorpusLawEntry {
    pub law_id: String,
    pub name: Option<String>,
    /// Resolved human-readable name. For laws with a literal `name:` field
    /// this equals `name`. For laws with `name: '#output_ref'` this is the
    /// resolved value from the matching action output. Falls back to `None`
    /// when the reference cannot be resolved.
    pub display_name: Option<String>,
    pub source_id: String,
    pub source_name: String,
}

/// A parameter required by the execution block that declares an output.
#[derive(Debug, Serialize)]
pub struct LawParamEntry {
    pub name: String,
    pub param_type: String,
}

/// An output entry from a law's machine_readable.execution.output.
#[derive(Debug, Serialize)]
pub struct LawOutputEntry {
    pub name: String,
    pub output_type: String,
    pub article_number: String,
    /// Parameters required by the article's execution block. The caller
    /// must supply these via `source.parameters` when referencing this output.
    pub parameters: Vec<LawParamEntry>,
}

/// GET /api/sources — list all registered corpus sources with law counts.
pub async fn list_sources(
    State(state): State<AppState>,
) -> Result<Json<Vec<SourceSummary>>, (StatusCode, String)> {
    let corpus = state.corpus.read().await;
    Ok(Json(build_source_summaries(
        &corpus.registry,
        &corpus.source_map,
    )))
}

/// GET /api/corpus/laws — list loaded laws with source metadata.
///
/// Supports pagination via `?offset=0&limit=100`. Default limit is 100,
/// maximum is 1000.
pub async fn list_corpus_laws(
    State(state): State<AppState>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<Vec<CorpusLawEntry>>, (StatusCode, String)> {
    let corpus = state.corpus.read().await;
    let limit = params.effective_limit();

    let mut entries: Vec<CorpusLawEntry> = corpus
        .source_map
        .laws()
        .map(|law| {
            let display_name = resolve_display_name(&law.yaml_content);
            CorpusLawEntry {
                law_id: law.law_id.clone(),
                name: law.name.clone(),
                display_name,
                source_id: law.source_id.clone(),
                source_name: law.source_name.clone(),
            }
        })
        .collect();

    entries.sort_by(|a, b| a.law_id.cmp(&b.law_id));

    let paginated: Vec<CorpusLawEntry> = entries
        .into_iter()
        .skip(params.offset)
        .take(limit)
        .collect();

    Ok(Json(paginated))
}

/// GET /api/corpus/laws/{law_id} — return raw YAML content for a specific law.
pub async fn get_corpus_law(
    State(state): State<AppState>,
    Path(law_id): Path<String>,
) -> Result<
    (
        StatusCode,
        [(axum::http::HeaderName, &'static str); 1],
        String,
    ),
    (StatusCode, String),
> {
    let corpus = state.corpus.read().await;

    let law = corpus
        .source_map
        .get_law(&law_id)
        .ok_or_else(|| (StatusCode::NOT_FOUND, format!("Law '{}' not found", law_id)))?;

    Ok((
        StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "text/yaml; charset=utf-8")],
        law.yaml_content.clone(),
    ))
}

/// GET /api/corpus/laws/{law_id}/outputs — list all outputs declared across articles.
pub async fn list_law_outputs(
    State(state): State<AppState>,
    Path(law_id): Path<String>,
) -> Result<Json<Vec<LawOutputEntry>>, (StatusCode, String)> {
    let corpus = state.corpus.read().await;

    let law = corpus
        .source_map
        .get_law(&law_id)
        .ok_or_else(|| (StatusCode::NOT_FOUND, format!("Law '{}' not found", law_id)))?;

    let outputs: Vec<LawOutputEntry> = collect_law_outputs(&law.yaml_content)
        .into_iter()
        .map(|out| LawOutputEntry {
            name: out.name,
            output_type: out.output_type,
            article_number: out.article_number,
            parameters: out
                .parameters
                .into_iter()
                .map(|(name, param_type)| LawParamEntry { name, param_type })
                .collect(),
        })
        .collect();

    Ok(Json(outputs))
}

/// A scenario file entry.
#[derive(Debug, Serialize)]
pub struct ScenarioEntry {
    pub filename: String,
}

/// GET /api/corpus/laws/{law_id}/scenarios — list available scenario files.
pub async fn list_scenarios(
    State(state): State<AppState>,
    Path(law_id): Path<String>,
) -> Result<Json<Vec<ScenarioEntry>>, (StatusCode, String)> {
    // Reads use the global backend resolution (writable fallback included).
    // For federated GitHub sources this is NOT the same backend writes use
    // (writes go through the per-session `SessionGitBackend`), so a save
    // followed immediately by a list/get returns the pre-edit content from
    // the global clone until the session branch is rebased back into it.
    // Frontend already keeps the just-saved content in local state, so
    // editor UX is unaffected; a follow-up can route reads through the
    // session backend if a strict read-your-writes guarantee is needed.
    let resolved = {
        let corpus = state.corpus.read().await;
        resolve_backend_for_law(&corpus, &law_id).await?
    };

    let scenarios_dir = match law_relative_dir(&resolved.law) {
        Ok(dir) => dir.join("scenarios"),
        Err(_) => return Ok(Json(Vec::new())),
    };

    let backend = resolved.backend.lock().await;
    // Surface real backend errors (permissions, broken git checkout, …) as
    // 500 instead of swallowing them as "no scenarios". `list_files` itself
    // already returns `Ok(vec![])` for a missing directory, so anything that
    // does reach the error arm is a genuine fault worth telling the client.
    let entries = backend
        .list_files(&scenarios_dir, Some("feature"))
        .await
        .map_err(|e| {
            tracing::warn!(law_id = %law_id, error = %e, "list_scenarios backend failure");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to list scenarios".to_string(),
            )
        })?;
    drop(backend);

    let mut out: Vec<ScenarioEntry> = entries
        .into_iter()
        .map(|e| ScenarioEntry { filename: e.name })
        .collect();
    out.sort_by(|a, b| a.filename.cmp(&b.filename));
    Ok(Json(out))
}

/// GET /api/corpus/laws/{law_id}/scenarios/{filename} — return raw .feature content.
pub async fn get_scenario(
    State(state): State<AppState>,
    Path((law_id, filename)): Path<(String, String)>,
) -> Result<
    (
        StatusCode,
        [(axum::http::HeaderName, &'static str); 1],
        String,
    ),
    (StatusCode, String),
> {
    validate_scenario_filename(&filename)?;

    let resolved = {
        let corpus = state.corpus.read().await;
        resolve_backend_for_law(&corpus, &law_id).await?
    };

    let scenarios_dir = law_relative_dir(&resolved.law)?.join("scenarios");
    let relative_path = scenarios_dir.join(&filename);

    let backend = resolved.backend.lock().await;
    let content = backend
        .read_file(&relative_path)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                format!("Scenario '{}' not found", filename),
            )
        })?;
    drop(backend);

    Ok((
        StatusCode::OK,
        [(
            axum::http::header::CONTENT_TYPE,
            "text/plain; charset=utf-8",
        )],
        content,
    ))
}

// ---------------------------------------------------------------------------
// Scenario write helpers
// ---------------------------------------------------------------------------

/// Validate a scenario filename (no path traversal, must end with `.feature`).
fn validate_scenario_filename(filename: &str) -> Result<(), (StatusCode, String)> {
    if filename.contains('/')
        || filename.contains('\\')
        || filename.contains("..")
        || filename.contains('\0')
    {
        return Err((StatusCode::BAD_REQUEST, "Invalid filename".to_string()));
    }
    if !filename.ends_with(".feature") {
        return Err((
            StatusCode::BAD_REQUEST,
            "Only .feature files are supported".to_string(),
        ));
    }
    Ok(())
}

/// Extract the law-relative directory from a law's file_path.
///
/// Returns the path of the law's directory, relative to the source root.
///
/// `LoadedLaw::relative_path` is computed at load time by stripping the
/// source root (for local sources) or the in-repo subpath (for GitHub
/// sources). Taking its parent gives the directory the backend writes to,
/// without making any assumption about the structural depth of the corpus
/// layout.
fn law_relative_dir(law: &LoadedLaw) -> Result<PathBuf, (StatusCode, String)> {
    let rel = std::path::Path::new(&law.relative_path);
    rel.parent().map(PathBuf::from).ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Cannot determine law directory".to_string(),
        )
    })
}

/// Resolved backend information for a law (read-path only).
///
/// The federated write path goes through [`SessionRegistry`] instead and
/// returns its own resolved target shape ([`EditorWriteTarget`]), so this
/// struct doesn't need a writability flag.
struct ResolvedBackend {
    law: LoadedLaw,
    backend: Arc<Mutex<Box<dyn RepoBackend>>>,
}

/// Resolve the backend that should be used for a law's scenario files.
///
/// Both read and write handlers go through this function so the editor
/// always uses the **same** backend for `get_scenario` / `list_scenarios` /
/// `save_scenario` / `delete_scenario` on a given law. Without this single
/// source of truth, a read can end up at one on-disk location while a write
/// for the same law lands at a different one — silent data loss.
///
/// Resolution order:
///
/// 1. **Law's own writable backend.** Happy path for normal local-only dev.
/// 2. **Verified writable fallback.** When the law's own source is read-only
///    (e.g. baked-in container filesystem) we look for another writable
///    backend whose root contains the **same** law file at the same
///    `law.relative_path`. A successful read of that path proves the two
///    sources share their structural layout, so subsequent reads/writes of
///    sibling scenario files land at consistent locations.
/// 3. **Law's own read-only backend.** No writable target available. Reads
///    still work; writes will be rejected with 403 by the caller.
///
/// The verification in step 2 is essential: without it the fallback could
/// silently produce files at a path the reader never looks at.
async fn resolve_backend_for_law(
    corpus: &crate::state::CorpusState,
    law_id: &str,
) -> Result<ResolvedBackend, (StatusCode, String)> {
    let law = corpus
        .source_map
        .get_law(law_id)
        .ok_or_else(|| (StatusCode::NOT_FOUND, format!("Law '{}' not found", law_id)))?
        .clone();

    // 1. Prefer the law's own backend if it can accept writes.
    if let Some(entry) = corpus.backends.get(&law.source_id) {
        if entry.writable {
            return Ok(ResolvedBackend {
                law,
                backend: entry.backend.clone(),
            });
        }
    }

    // 2. Look for another writable backend that contains the same law file
    //    at the same source-relative path. Alphabetical iteration keeps the
    //    choice deterministic across restarts.
    let law_rel = std::path::Path::new(&law.relative_path);
    let mut candidate_ids: Vec<&String> = corpus.backends.keys().collect();
    candidate_ids.sort();

    for source_id in candidate_ids {
        let Some(entry) = corpus.backends.get(source_id) else {
            continue;
        };
        if !entry.writable || source_id == &law.source_id {
            continue;
        }
        let backend = entry.backend.lock().await;
        let exists = backend.read_file(law_rel).await.ok().flatten().is_some();
        drop(backend);
        if exists {
            tracing::warn!(
                law_id = %law_id,
                law_source = %law.source_id,
                fallback_source = %source_id,
                "law's own source has no writable backend; routing reads through verified-matching source"
            );
            return Ok(ResolvedBackend {
                law,
                backend: entry.backend.clone(),
            });
        }
    }

    // 3. Fall through to the law's own read-only backend so reads still
    //    work.
    if let Some(entry) = corpus.backends.get(&law.source_id) {
        return Ok(ResolvedBackend {
            law,
            backend: entry.backend.clone(),
        });
    }

    Err((
        StatusCode::NOT_FOUND,
        format!(
            "No backend registered for source '{}' (the source that owns law '{}')",
            law.source_id, law_id
        ),
    ))
}

/// Map a [`CorpusError`] from a write / delete / persist operation to an
/// HTTP error tuple.
///
/// `ReadOnly` is an expected, recoverable precondition (e.g. the resolved
/// backend is a baked-in local source on a read-only container filesystem),
/// and the message is safe to surface to the user as `403 Forbidden`.
///
/// Every other variant (IO, git command failures, push failures, …) goes
/// out as `500 Internal Server Error` with a **generic** message. The full
/// error — which can include git stderr, repository URLs that may carry
/// push tokens for local-only backends, and absolute filesystem paths — is
/// logged at warn level for operators but never returned to the client.
///
/// `kind` is the short name of the resource being written ("scenario",
/// "law", …) so logs and the user-facing 500 body name the right thing
/// regardless of which handler is on the stack. The `FnOnce` wrapper is a
/// convenience for `.map_err(corpus_write_error("law"))` at call sites.
fn corpus_write_error(kind: &'static str) -> impl FnOnce(CorpusError) -> (StatusCode, String) {
    move |e| match e {
        CorpusError::ReadOnly(_) => (StatusCode::FORBIDDEN, e.to_string()),
        _ => {
            tracing::warn!(error = %e, kind = %kind, "corpus write/persist failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Internal error while writing {}", kind),
            )
        }
    }
}

// ---------------------------------------------------------------------------
// Save / Delete scenario endpoints
// ---------------------------------------------------------------------------

/// Maximum accepted length of the `X-Editor-Session` header value.
///
/// The header ends up as a path segment on disk
/// (`/tmp/regelrecht-editor-sessions/.../<session_id>`) and as a git ref
/// name (`editor/session-<id>`). 128 bytes comfortably fits a UUID, hex,
/// or other opaque identifier the frontend mints, while rejecting a
/// pathological client that sends a multi-kilobyte value.
const EDITOR_SESSION_MAX_LEN: usize = 128;

/// Read the editor session id from the `X-Editor-Session` header.
///
/// Required on every editor write. We deliberately accept-then-validate
/// (rather than minting one server-side) so the editor controls session
/// scope: closing the browser tab purges `sessionStorage`, the next save
/// arrives with a fresh UUID, and the user gets a fresh PR — matching
/// "one PR per editor session" without any server-side TTL bookkeeping.
fn require_editor_session(headers: &HeaderMap) -> Result<String, (StatusCode, String)> {
    let value = headers.get(EDITOR_SESSION_HEADER).ok_or((
        StatusCode::BAD_REQUEST,
        format!("Missing {} header", EDITOR_SESSION_HEADER),
    ))?;
    let s = value.to_str().map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            format!("{} header is not valid UTF-8", EDITOR_SESSION_HEADER),
        )
    })?;
    // Loose validation: non-empty + ASCII-safe for use in a git ref name.
    // Tighter shape (UUID) lives at the editor layer where it's minted.
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            format!("{} header is empty", EDITOR_SESSION_HEADER),
        ));
    }
    if trimmed.len() > EDITOR_SESSION_MAX_LEN {
        return Err((
            StatusCode::BAD_REQUEST,
            format!(
                "{} header exceeds {} characters",
                EDITOR_SESSION_HEADER, EDITOR_SESSION_MAX_LEN
            ),
        ));
    }
    if !trimmed
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err((
            StatusCode::BAD_REQUEST,
            format!(
                "{} header contains characters that are not allowed in a git ref",
                EDITOR_SESSION_HEADER
            ),
        ));
    }
    Ok(trimmed.to_string())
}

/// Pull the editor user identity out of the OIDC session, when both name
/// and email are populated.
///
/// Returns `None` when auth is disabled or the relevant session keys
/// haven't been set — in those cases the resulting commit just has no
/// `Co-Authored-By` trailer, which is harmless. We do NOT fail the save
/// over a missing identity.
async fn editor_user_from_session(session: &Session) -> Option<EditorUser> {
    let name: Option<String> = session.get(SESSION_KEY_NAME).await.ok().flatten();
    let email: Option<String> = session.get(SESSION_KEY_EMAIL).await.ok().flatten();
    match (name, email) {
        (Some(name), Some(email)) if !name.is_empty() && !email.is_empty() => {
            Some(EditorUser { name, email })
        }
        _ => None,
    }
}

/// Map a [`SessionResolveError`] into an HTTP error tuple suitable for an
/// `Result<_, (StatusCode, String)>` save handler.
fn session_resolve_error(law_id: &str, e: SessionResolveError) -> (StatusCode, String) {
    match e {
        SessionResolveError::SourceNotFound(_) => (
            StatusCode::NOT_FOUND,
            format!("Law '{}' references an unknown source", law_id),
        ),
        SessionResolveError::NoToken(source_id) => {
            tracing::warn!(
                law_id = %law_id,
                source_id = %source_id,
                "save: source has no PR token configured"
            );
            (
                StatusCode::FORBIDDEN,
                "Source is not configured for write-back".to_string(),
            )
        }
        SessionResolveError::Other(msg) => {
            tracing::error!(
                law_id = %law_id,
                error = %msg,
                "save: failed to resolve session backend"
            );
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to prepare write target".to_string(),
            )
        }
    }
}

/// Resolved write target for editor saves: a backend lock + the file
/// path. PR info comes back via `PersistOutcome.pr` from the actual
/// `persist` call, so we don't need to flag the backend here.
struct EditorWriteTarget {
    relative_path: PathBuf,
    backend: tokio::sync::OwnedMutexGuard<Box<dyn RepoBackend>>,
}

/// Resolve the write target for a law-content (`save_law`) edit by routing
/// through the [`SessionRegistry`] for GitHub sources and falling back to
/// the existing global backend resolution for local sources.
async fn resolve_law_write_target(
    state: &AppState,
    session_id: &str,
    law_id: &str,
) -> Result<EditorWriteTarget, (StatusCode, String)> {
    // Look up the law (and snapshot the source state) under a single
    // corpus read guard, then drop it before calling into the
    // SessionRegistry. The slow-path session-backend init runs `git
    // clone` and we must not hold the corpus read guard across that —
    // otherwise a concurrent `POST /api/corpus/reload` (which takes the
    // write guard) is starved for the duration of the clone.
    let (law, snapshot) = {
        let corpus = state.corpus.read().await;
        let law = corpus
            .source_map
            .get_law(law_id)
            .ok_or_else(|| (StatusCode::NOT_FOUND, format!("Law '{}' not found", law_id)))?
            .clone();
        let snapshot = corpus
            .snapshot_source_for_session(&law.source_id)
            .ok_or_else(|| {
                session_resolve_error(
                    law_id,
                    SessionResolveError::SourceNotFound(law.source_id.clone()),
                )
            })?;
        (law, snapshot)
    };

    let resolved = state
        .sessions
        .resolve_session_backend(&snapshot, session_id, &law.source_id)
        .await
        .map_err(|e| session_resolve_error(law_id, e))?;

    // For local sources the SessionRegistry hands back the existing global
    // backend, which may be read-only (baked-in container fs). Surface
    // that as 403, matching the previous behaviour.
    if !resolved.uses_session_pr {
        let writable = {
            let backend = resolved.backend.lock().await;
            backend.is_writable()
        };
        if !writable {
            return Err((
                StatusCode::FORBIDDEN,
                "Law is stored on a read-only source".to_string(),
            ));
        }
    }

    let backend = resolved.backend.lock_owned().await;
    Ok(EditorWriteTarget {
        relative_path: PathBuf::from(&law.relative_path),
        backend,
    })
}

/// Resolve the write target for a scenario file edit. Mirrors
/// `resolve_law_write_target` but appends the scenario subdir + filename.
async fn resolve_scenario_write_target(
    state: &AppState,
    session_id: &str,
    law_id: &str,
    filename: &str,
) -> Result<EditorWriteTarget, (StatusCode, String)> {
    // Snapshot under a single corpus read guard, then drop it — same
    // reasoning as `resolve_law_write_target` above (the slow-path
    // session-backend init runs `git clone` and must not hold the corpus
    // read guard across that or `POST /api/corpus/reload` is starved).
    let (law, snapshot) = {
        let corpus = state.corpus.read().await;
        let law = corpus
            .source_map
            .get_law(law_id)
            .ok_or_else(|| (StatusCode::NOT_FOUND, format!("Law '{}' not found", law_id)))?
            .clone();
        let snapshot = corpus
            .snapshot_source_for_session(&law.source_id)
            .ok_or_else(|| {
                session_resolve_error(
                    law_id,
                    SessionResolveError::SourceNotFound(law.source_id.clone()),
                )
            })?;
        (law, snapshot)
    };

    let resolved = state
        .sessions
        .resolve_session_backend(&snapshot, session_id, &law.source_id)
        .await
        .map_err(|e| session_resolve_error(law_id, e))?;

    if !resolved.uses_session_pr {
        let writable = {
            let backend = resolved.backend.lock().await;
            backend.is_writable()
        };
        if !writable {
            return Err((StatusCode::FORBIDDEN, "Source is read-only".to_string()));
        }
    }

    let rel_dir = law_relative_dir(&law)?;
    let backend = resolved.backend.lock_owned().await;
    Ok(EditorWriteTarget {
        relative_path: rel_dir.join("scenarios").join(filename),
        backend,
    })
}

/// Build a [`SaveResponse`] from a successful [`PersistOutcome`]. Pulls
/// the branch name out so the frontend can show "session-abc123" without
/// re-deriving it from the URL.
fn save_response_from(outcome: PersistOutcome, session_id: &str) -> SaveResponse {
    SaveResponse {
        pr: outcome.pr.map(|pr| SavePrInfo {
            url: pr.html_url,
            number: pr.number,
            branch: format!("editor/session-{session_id}"),
        }),
    }
}

/// PUT /api/corpus/laws/{law_id}/scenarios/{filename} — save a scenario file.
///
/// Requires the `X-Editor-Session` header on every call; the session id
/// scopes the per-(session, source) feature branch + PR for federated
/// write-back. For local sources the session header is still required
/// (uniform contract with the frontend) but is otherwise ignored.
pub async fn save_scenario(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Path((law_id, filename)): Path<(String, String)>,
    body: String,
) -> Result<Json<SaveResponse>, (StatusCode, String)> {
    validate_scenario_filename(&filename)?;
    let session_id = require_editor_session(&headers)?;
    let author = editor_user_from_session(&session).await;

    let target = resolve_scenario_write_target(&state, &session_id, &law_id, &filename).await?;
    let EditorWriteTarget {
        relative_path,
        backend,
    } = target;

    backend
        .write_file(&relative_path, &body)
        .await
        .map_err(corpus_write_error("scenario"))?;

    let outcome = backend
        .persist(&WriteContext {
            message: format!("Update scenario {} for {}", filename, law_id),
            author,
        })
        .await
        .map_err(corpus_write_error("scenario"))?;

    Ok(Json(save_response_from(outcome, &session_id)))
}

/// PUT /api/corpus/laws/{law_id} — save edited law YAML content.
///
/// Writes the new YAML to the backend (same RepoBackend used for scenario
/// saves, with the same writable-fallback resolution), then refreshes the
/// in-memory `yaml_content` on the law's `SourceMap` entry so subsequent
/// GETs see the edited text without waiting for a full corpus reload.
///
/// The `$id` in the body must match the path parameter: allowing them to
/// diverge would either create a phantom law (new `$id` lands on an
/// existing file) or orphan the original (old `$id` can never be fetched
/// again). We reject the mismatch up-front instead of silently corrupting
/// the source map.
pub async fn save_law(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Path(law_id): Path<String>,
    body: String,
) -> Result<Json<SaveResponse>, (StatusCode, String)> {
    let session_id = require_editor_session(&headers)?;
    let author = editor_user_from_session(&session).await;

    // Validation:
    //   1. Body must parse as well-formed YAML. extract_law_id below is a
    //      line-based scanner that happily accepts "$id: foo\n<garbage>",
    //      so without this check a syntactically broken body would land on
    //      disk and corrupt the corpus source file.
    //   2. Body must have a top-level `$id` field.
    //   3. That `$id` must match the path parameter. Any mismatch is either
    //      a phantom-law attempt (new id lands on an existing file) or an
    //      orphaning (old id becomes unfetchable); reject up-front.
    //
    // We do NOT run full JSON Schema validation here — the frontend blocks
    // incomplete operation stubs (findIncompleteOperation) and the YAML
    // pane has a live parse check. Full schema validation is a separate
    // follow-up (mirroring `just validate`).
    //
    // The mismatch error body intentionally does NOT echo the user-supplied
    // `body_id`: it flows through the frontend into ndd-inline-dialog's
    // supporting-text and we don't want self-XSS if the dialog ever renders
    // that attribute as markup. The path law_id is already known to the
    // caller, so the generic message is sufficient.
    validate_yaml_syntax(&body).map_err(|e| {
        tracing::debug!(law_id = %law_id, error = %e, "save_law received malformed YAML body");
        (
            StatusCode::BAD_REQUEST,
            "Body is not valid YAML".to_string(),
        )
    })?;

    let body_id = extract_law_id(&body).ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            "Body missing top-level `$id` field".to_string(),
        )
    })?;

    if body_id != law_id {
        return Err((
            StatusCode::BAD_REQUEST,
            "Body $id does not match path law_id".to_string(),
        ));
    }

    let target = resolve_law_write_target(&state, &session_id, &law_id).await?;
    let EditorWriteTarget {
        relative_path,
        backend,
    } = target;

    let outcome = {
        // The earlier write-target resolution checks writability for local
        // sources; for federated GitHub sources a missing token is a 403
        // before we even get here. The remaining failure mode here is a
        // TOCTOU on local backend writability (volume flips read-only
        // mid-request); `corpus_write_error` maps that as a generic 500.
        backend
            .write_file(&relative_path, &body)
            .await
            .map_err(corpus_write_error("law"))?;
        backend
            .persist(&WriteContext {
                message: format!("Update law {}", law_id),
                author,
            })
            .await
            .map_err(corpus_write_error("law"))?
    };

    // Refresh the in-memory cache so /api/corpus/laws/{law_id} (and
    // dependency walks) see the edit without a full corpus reload.
    {
        let mut corpus = state.corpus.write().await;
        let updated = corpus.source_map.update_yaml_content(&law_id, body);
        if !updated {
            tracing::warn!(
                law_id = %law_id,
                "save_law wrote to backend but law vanished from source_map between write and cache refresh"
            );
        }
    }

    Ok(Json(save_response_from(outcome, &session_id)))
}

/// DELETE /api/corpus/laws/{law_id}/scenarios/{filename} — delete a scenario file.
pub async fn delete_scenario(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Path((law_id, filename)): Path<(String, String)>,
) -> Result<Json<SaveResponse>, (StatusCode, String)> {
    validate_scenario_filename(&filename)?;
    let session_id = require_editor_session(&headers)?;
    let author = editor_user_from_session(&session).await;

    let target = resolve_scenario_write_target(&state, &session_id, &law_id, &filename).await?;
    let EditorWriteTarget {
        relative_path,
        backend,
    } = target;

    backend
        .delete_file(&relative_path)
        .await
        .map_err(corpus_write_error("scenario"))?;

    let outcome = backend
        .persist(&WriteContext {
            message: format!("Delete scenario {} for {}", filename, law_id),
            author,
        })
        .await
        .map_err(corpus_write_error("scenario"))?;

    Ok(Json(save_response_from(outcome, &session_id)))
}

/// POST /api/corpus/reload — refetch corpus from all sources.
///
/// Reloads the in-memory SourceMap from the registry (local + GitHub).
/// Accepts an optional JSON body with `law_ids` to include specific laws
/// that may not yet be in the corpus (e.g. freshly harvested laws).
pub async fn reload_corpus(
    State(state): State<AppState>,
    body: Option<Json<ReloadRequest>>,
) -> Result<Json<ReloadResponse>, (StatusCode, String)> {
    // Single-flight: each reload fans out to GitHub (one call per law
    // source). Without this gate, an authenticated client firing parallel
    // reloads can exhaust the 5000 req/hr token quota and break corpus
    // reads for everyone. Concurrent callers get 429 rather than being
    // serialised — a reload already in flight will pick up their changes.
    let _reload_guard = state.reload_lock.try_lock().map_err(|_| {
        (
            StatusCode::TOO_MANY_REQUESTS,
            "Corpus reload already in progress".to_string(),
        )
    })?;

    // Gather everything we need under a read lock so concurrent readers
    // (law fetches, scenario loads, dependency resolution) are not blocked
    // for the duration of the GitHub round-trip.
    let (registry, auth_file, mut law_ids) = {
        let corpus = state.corpus.read().await;
        let law_ids: std::collections::HashSet<String> =
            corpus.source_map.laws().map(|l| l.law_id.clone()).collect();
        (corpus.registry.clone(), corpus.auth_file.clone(), law_ids)
    };

    // Include any extras the caller explicitly requests (e.g. a freshly
    // harvested law not yet in the corpus).
    if let Some(Json(req)) = &body {
        for id in &req.law_ids {
            law_ids.insert(id.clone());
        }
    }

    let new_map = registry
        .load_favorites_async(&law_ids, auth_file.as_deref())
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "corpus reload failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to reload corpus".to_string(),
            )
        })?;

    let law_count = new_map.len();
    {
        let mut corpus = state.corpus.write().await;
        corpus.source_map = new_map;
    }
    tracing::info!(law_count, "corpus reloaded (local + GitHub)");
    Ok(Json(ReloadResponse { law_count }))
}

#[derive(Debug, Deserialize)]
pub struct ReloadRequest {
    #[serde(default)]
    pub law_ids: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct ReloadResponse {
    pub law_count: usize,
}

#[cfg(test)]
mod tests {
    //! Tests for the small, pure helpers in this module: header validation,
    //! error mapping, and response shaping. The full save/delete handlers
    //! require an axum harness with sessions + sqlx + a real source map
    //! and live behind separate integration tests.
    use super::*;
    use axum::http::HeaderValue;
    use regelrecht_corpus::backend::PrInfo;

    fn headers_with(value: &str) -> HeaderMap {
        let mut h = HeaderMap::new();
        h.insert(EDITOR_SESSION_HEADER, HeaderValue::from_str(value).unwrap());
        h
    }

    #[test]
    fn require_editor_session_missing_header_is_400() {
        let h = HeaderMap::new();
        let err = require_editor_session(&h).unwrap_err();
        assert_eq!(err.0, StatusCode::BAD_REQUEST);
        assert!(err.1.contains("Missing"));
    }

    #[test]
    fn require_editor_session_empty_value_is_400() {
        let h = headers_with("   ");
        let err = require_editor_session(&h).unwrap_err();
        assert_eq!(err.0, StatusCode::BAD_REQUEST);
        assert!(err.1.contains("empty"));
    }

    #[test]
    fn require_editor_session_invalid_chars_is_400() {
        // Slashes are not allowed in a git ref segment and must be rejected.
        let h = headers_with("foo/bar");
        let err = require_editor_session(&h).unwrap_err();
        assert_eq!(err.0, StatusCode::BAD_REQUEST);
    }

    #[test]
    fn require_editor_session_oversized_value_is_400() {
        // Defence against pathological clients sending a megabyte-long
        // header — that would end up as a branch name on disk.
        let h = headers_with(&"a".repeat(1024));
        let err = require_editor_session(&h).unwrap_err();
        assert_eq!(err.0, StatusCode::BAD_REQUEST);
    }

    #[test]
    fn require_editor_session_accepts_uuid_shape() {
        let h = headers_with("abc123-def4567890");
        let session_id = require_editor_session(&h).unwrap();
        assert_eq!(session_id, "abc123-def4567890");
    }

    #[test]
    fn session_resolve_error_source_not_found_is_404() {
        let err = session_resolve_error("law_x", SessionResolveError::SourceNotFound("src".into()));
        assert_eq!(err.0, StatusCode::NOT_FOUND);
    }

    #[test]
    fn session_resolve_error_no_token_is_403() {
        let err = session_resolve_error("law_x", SessionResolveError::NoToken("src".into()));
        assert_eq!(err.0, StatusCode::FORBIDDEN);
    }

    #[test]
    fn session_resolve_error_other_is_500() {
        let err = session_resolve_error("law_x", SessionResolveError::Other("boom".into()));
        assert_eq!(err.0, StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn save_response_from_local_source_has_no_pr() {
        // PersistOutcome.pr is None for local-source saves; the response
        // must mirror that shape so the frontend hides the PR badge.
        let out = PersistOutcome { pr: None };
        let body = save_response_from(out, "sess-abc");
        assert!(body.pr.is_none());
    }

    #[test]
    fn save_response_from_github_source_carries_pr_with_session_branch() {
        let out = PersistOutcome {
            pr: Some(PrInfo {
                number: 42,
                html_url: "https://github.com/x/y/pull/42".to_string(),
            }),
        };
        let body = save_response_from(out, "sess-abc");
        let pr = body.pr.expect("session-pr response must carry pr");
        assert_eq!(pr.number, 42);
        assert_eq!(pr.url, "https://github.com/x/y/pull/42");
        assert_eq!(pr.branch, "editor/session-sess-abc");
    }
}
