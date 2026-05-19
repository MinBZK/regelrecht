use std::path::PathBuf;
use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tower_sessions::Session;
use uuid::Uuid;

use regelrecht_auth::handlers::{SESSION_KEY_EMAIL, SESSION_KEY_NAME, SESSION_KEY_SUB};
use regelrecht_corpus::annotation_schema::{
    append_notes_to_sidecar, first_note_not_targeting_law, parse_and_validate_annotation_yaml,
    validate_annotation_doc, AppendOutcome,
};
use regelrecht_corpus::backend::{EditorUser, PersistOutcome, RepoBackend, WriteContext};
use regelrecht_corpus::dto::{build_source_summaries, PaginationParams, SourceSummary};
use regelrecht_corpus::source_map::{
    collect_law_outputs, extract_law_id, resolve_display_name, validate_yaml_syntax, LoadedLaw,
};
use regelrecht_corpus::CorpusError;

use crate::state::AppState;
use crate::traject_corpus::{TrajectCorpus, TrajectCorpusError};
use crate::trajects::read_active_from_session;

/// Response body for a successful save.
///
/// `pr` is populated when the traject's writable backend opened (or
/// updated) a pull request for this save — currently a future enhancement;
/// the default traject backend commits straight to the configured branch
/// and returns `None`.
#[derive(Debug, Serialize)]
pub struct SaveResponse {
    pub pr: Option<SavePrInfo>,
    /// `true` when a notes save was a no-op: every submitted note was
    /// already present on the branch, so nothing was written/committed.
    /// Lets the frontend show "al opgeslagen" and keep any existing PR
    /// badge instead of treating a PR-less 200 as a lost save (review
    /// finding NEW-2). Always `false` for law/scenario saves; those
    /// clients ignore it.
    pub no_change: bool,
}

#[derive(Debug, Serialize)]
pub struct SavePrInfo {
    pub url: String,
    pub number: u64,
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
/// The write path goes through the per-traject corpus
/// ([`require_traject_corpus`]) and returns its own resolved target shape
/// ([`EditorWriteTarget`]), so this struct doesn't need a writability flag.
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

/// Resolved write target for editor saves: a backend lock + the file
/// path. PR info comes back via `PersistOutcome.pr` from the actual
/// `persist` call, so we don't need to flag the backend here.
struct EditorWriteTarget {
    relative_path: PathBuf,
    backend: tokio::sync::OwnedMutexGuard<Box<dyn RepoBackend>>,
}

/// Resolve the per-traject corpus from session state, returning 403 when
/// no traject is active. Bumps the cache on a miss; calls
/// `ensure_ready` (i.e. `git clone`) for every source in the traject's
/// federated config on first use.
///
/// Re-verifies the caller's membership against `traject_members` on every
/// call — `set_active` only checks once at the time the active id is
/// stored, so without a re-check a member who has been removed (or whose
/// traject has been deleted) since picking it active could keep writing
/// to that traject's branch through their stale session.
async fn require_traject_corpus(
    state: &AppState,
    session: &Session,
) -> Result<Arc<TrajectCorpus>, (StatusCode, String)> {
    let traject_id = read_active_from_session(session)
        .await
        .map_err(|status| (status, "session read failed".to_string()))?
        .ok_or_else(|| {
            (
                StatusCode::FORBIDDEN,
                "Selecteer eerst een traject om te bewerken".to_string(),
            )
        })?;
    let pool = state.pool.as_ref().ok_or((
        StatusCode::SERVICE_UNAVAILABLE,
        "database not configured".to_string(),
    ))?;

    // Membership re-check: a single EXISTS join keeps this on the hot
    // path while catching session/state drift (membership revoked,
    // traject deleted, account never linked to a sub).
    let sub: String = session
        .get(SESSION_KEY_SUB)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "session read sub in require_traject_corpus");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "session read failed".to_string(),
            )
        })?
        .ok_or((
            StatusCode::FORBIDDEN,
            "session has no subject claim".to_string(),
        ))?;
    let (is_member,): (bool,) = sqlx::query_as(
        "SELECT EXISTS(
             SELECT 1 FROM accounts a
             JOIN traject_members m ON m.account_id = a.id
             WHERE a.person_sub = $1 AND m.traject_id = $2
         )",
    )
    .bind(&sub)
    .bind(traject_id)
    .fetch_one(pool)
    .await
    .map_err(|e| {
        tracing::error!(error = %e, "membership re-check query failed");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "membership check failed".to_string(),
        )
    })?;
    if !is_member {
        // Clear the stale pointer so the next request from this session
        // hits the "no active traject" path and the user can rebind via
        // the menu instead of seeing 403 on every save.
        let _: Option<Uuid> = session
            .remove(crate::trajects::SESSION_KEY_ACTIVE_TRAJECT)
            .await
            .unwrap_or(None);
        return Err((
            StatusCode::FORBIDDEN,
            "Je hebt geen toegang meer tot dit traject".to_string(),
        ));
    }

    let auth_file = {
        let corpus = state.corpus.read().await;
        corpus.auth_file.clone()
    };
    match state
        .trajects
        .get_or_build(pool, traject_id, auth_file, &state.favorites)
        .await
    {
        Ok(corpus) => Ok(corpus),
        Err(e) => {
            // Also clear the session on NotFound so the user isn't stuck
            // 404-on-every-save after the traject was deleted. The clear
            // must run inline (NOT via tokio::spawn) so SessionManagerLayer
            // sees the mutation when it persists the session on the way
            // out — a detached task wouldn't have run yet at save time.
            if matches!(e, TrajectCorpusError::NotFound) {
                let _: Option<Uuid> = session
                    .remove(crate::trajects::SESSION_KEY_ACTIVE_TRAJECT)
                    .await
                    .unwrap_or(None);
            }
            Err(traject_corpus_error(e))
        }
    }
}

fn traject_corpus_error(e: TrajectCorpusError) -> (StatusCode, String) {
    match e {
        TrajectCorpusError::NotFound => (
            StatusCode::NOT_FOUND,
            "Actief traject niet gevonden".to_string(),
        ),
        TrajectCorpusError::NoWritableOwn => (
            StatusCode::CONFLICT,
            "Traject heeft geen eigen schrijfbare source".to_string(),
        ),
        other => {
            tracing::error!(error = %other, "traject corpus build failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Traject corpus init failed".to_string(),
            )
        }
    }
}

/// Resolve the writable-own backend within an active traject's corpus.
/// Returns the looked-up law (for its `relative_path`) and an owned guard
/// over the traject's writable backend.
async fn resolve_traject_law_write(
    state: &AppState,
    session: &Session,
    law_id: &str,
) -> Result<
    (
        LoadedLaw,
        tokio::sync::OwnedMutexGuard<Box<dyn RepoBackend>>,
    ),
    (StatusCode, String),
> {
    let traject = require_traject_corpus(state, session).await?;
    let law = traject
        .corpus
        .source_map
        .get_law(law_id)
        .ok_or_else(|| (StatusCode::NOT_FOUND, format!("Law '{}' not found", law_id)))?
        .clone();

    // "Save back where the law came from": laws from a source that has
    // an entry in `write_target_for_source` get routed through that
    // mapped backend (typically the writable_own's traject-branched
    // backend on the same upstream repo). Laws from a source without an
    // entry — e.g. the local source, which is natively writable on its
    // scratch dir — write directly through their own source's backend.
    let write_target_source_id = traject
        .write_target_for_source
        .get(&law.source_id)
        .cloned()
        .unwrap_or_else(|| law.source_id.clone());
    let entry = traject
        .corpus
        .backends
        .get(&write_target_source_id)
        .ok_or((
            StatusCode::SERVICE_UNAVAILABLE,
            "Writable backend not initialised".to_string(),
        ))?;
    if !entry.writable {
        return Err((StatusCode::FORBIDDEN, "Source is read-only".to_string()));
    }
    let backend = entry.backend.clone().lock_owned().await;
    Ok((law, backend))
}

async fn resolve_traject_law_target(
    state: &AppState,
    session: &Session,
    law_id: &str,
) -> Result<EditorWriteTarget, (StatusCode, String)> {
    let (law, backend) = resolve_traject_law_write(state, session, law_id).await?;
    Ok(EditorWriteTarget {
        relative_path: PathBuf::from(&law.relative_path),
        backend,
    })
}

async fn resolve_traject_scenario_target(
    state: &AppState,
    session: &Session,
    law_id: &str,
    filename: &str,
) -> Result<EditorWriteTarget, (StatusCode, String)> {
    let (law, backend) = resolve_traject_law_write(state, session, law_id).await?;
    let rel_dir = law_relative_dir(&law)?;
    Ok(EditorWriteTarget {
        relative_path: rel_dir.join("scenarios").join(filename),
        backend,
    })
}

/// Resolve the write target for a law's stand-off notes sidecar within the
/// active traject.
///
/// The path is `annotations/{law_id}/annotations.yaml` at the source root,
/// NOT under the law's own `regulation/...` directory: RFC-018 §1 keys the
/// sidecar by law id, independent of where the law file lives. Routing,
/// writability and membership checks all come from `resolve_traject_law_write`
/// (same backend the law/scenario writes use), so notes land in the same
/// traject branch/PR as the rest of the edits in the session.
async fn resolve_traject_annotation_target(
    state: &AppState,
    session: &Session,
    law_id: &str,
) -> Result<EditorWriteTarget, (StatusCode, String)> {
    let (_law, backend) = resolve_traject_law_write(state, session, law_id).await?;
    Ok(EditorWriteTarget {
        relative_path: PathBuf::from("annotations")
            .join(law_id)
            .join("annotations.yaml"),
        backend,
    })
}

/// Build a [`SaveResponse`] for a traject write. Traject backends commit
/// straight to the configured branch without opening a PR for now, so the
/// outcome typically carries `pr: None` and the response is just `{ pr:
/// null }` — the frontend treats that as a successful save without an
/// upstream link to display.
fn save_response_from_traject(outcome: PersistOutcome) -> SaveResponse {
    SaveResponse {
        pr: outcome.pr.map(|pr| SavePrInfo {
            url: pr.html_url,
            number: pr.number,
        }),
        no_change: false,
    }
}

/// PUT /api/corpus/laws/{law_id}/scenarios/{filename} — save a scenario file.
///
/// Requires an active traject in the session; the save is routed through
/// that traject's writable-own source (its branch on the writable repo).
/// Without an active traject the handler returns 403.
pub async fn save_scenario(
    State(state): State<AppState>,
    session: Session,
    Path((law_id, filename)): Path<(String, String)>,
    body: String,
) -> Result<Json<SaveResponse>, (StatusCode, String)> {
    validate_scenario_filename(&filename)?;
    let author = editor_user_from_session(&session).await;

    let target = resolve_traject_scenario_target(&state, &session, &law_id, &filename).await?;
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

    Ok(Json(save_response_from_traject(outcome)))
}

/// Schema the produced notes document is validated against before it is
/// written. Must match the version embedded in `regelrecht-corpus`'
/// annotation validator (kept in lockstep with the engine's resolver).
const ANNOTATION_SCHEMA_URL: &str = "https://raw.githubusercontent.com/MinBZK/regelrecht/refs/heads/main/schema/v0.5.2/annotation-schema.json";

/// Upper bound on notes accepted in a single save. The body limit on the
/// route already caps raw size; this caps the *count* so a single request
/// cannot append an unreasonable number of notes in one commit.
const MAX_NOTES_PER_SAVE: usize = 500;

/// PUT /api/corpus/laws/{law_id}/annotations — append stand-off notes.
///
/// Requires an active traject, exactly like `save_law`/`save_scenario`:
/// the notes land in that traject's writable backend (its branch), so a
/// note and a law edit made in the same session ride the same PR. Without
/// an active traject the underlying `resolve_traject_law_write` returns 403.
///
/// The body is a JSON array of *new* notes (drafts). The handler reads the
/// sidecar as it stands on the traject branch and appends only the new,
/// deduped notes, keeping the existing bytes verbatim (RFC-018 Dec. 1 /
/// RFC-005: per-note `git blame` and the curated motivering comments must
/// survive). Error bodies are deliberately generic — schema instance paths
/// can echo attacker-controlled map keys and would flow into an nldd
/// dialog (the self-XSS vector `save_law` also avoids).
pub async fn save_annotations(
    State(state): State<AppState>,
    session: Session,
    Path(law_id): Path<String>,
    body: String,
) -> Result<Json<SaveResponse>, (StatusCode, String)> {
    let author = editor_user_from_session(&session).await;

    // Body is a JSON array of new notes. Parse + bound it before touching
    // the backend.
    let new_notes: Vec<serde_json::Value> = serde_json::from_str(&body).map_err(|e| {
        tracing::debug!(law_id = %law_id, error = %e, "save_annotations: body is not a JSON note array");
        (
            StatusCode::BAD_REQUEST,
            "Request body must be a JSON array of notes".to_string(),
        )
    })?;
    if new_notes.len() > MAX_NOTES_PER_SAVE {
        return Err((
            StatusCode::PAYLOAD_TOO_LARGE,
            format!("Too many notes in one save (max {})", MAX_NOTES_PER_SAVE),
        ));
    }

    let target = resolve_traject_annotation_target(&state, &session, &law_id).await?;
    let EditorWriteTarget {
        relative_path,
        backend,
    } = target;

    // Read the current sidecar from the traject backend (the branch this
    // traject's PR is built on — read-your-writes within the traject).
    // Absent file = first notes for this law.
    let base_text: Option<String> = backend
        .read_file(&relative_path)
        .await
        .map_err(corpus_write_error("annotations"))?;

    // Validate the EXISTING file first, before merging in the new notes.
    // The post-merge validation below cannot tell "your note is invalid"
    // from "the file on the branch was already invalid" — same generic
    // message for both, so a user with a perfectly good note would edit it
    // forever while the real fault is a pre-existing note they did not
    // write. Schema drift on a committed sidecar is rare, but when it
    // happens the user must get a distinct, actionable error. Accepted
    // limitation documented in RFC-018 §10.
    if let Some(text) = base_text.as_deref() {
        if !text.trim().is_empty() {
            if let Err(errors) = parse_and_validate_annotation_yaml(text) {
                tracing::warn!(
                    law_id = %law_id,
                    errors = ?errors,
                    "save_annotations: existing sidecar fails the current schema; blocking append"
                );
                return Err((
                    StatusCode::CONFLICT,
                    "The notes file on the branch is itself invalid against the \
                     current schema. This is not a problem with your note; the \
                     existing file must be repaired before new notes can be added."
                        .to_string(),
                ));
            }
        }
    }

    // Append-only: keep the base file's bytes verbatim (preserves the
    // curated motivering comments and per-note git blame, RFC-018 Dec. 1 /
    // RFC-005) and append only the new, deduped notes. NoChange short-
    // circuits the whole write/commit so a no-op save is silent.
    let new_text =
        match append_notes_to_sidecar(base_text.as_deref(), &new_notes, ANNOTATION_SCHEMA_URL)
            .map_err(|e| {
                tracing::error!(law_id = %law_id, error = %e, "save_annotations: append failed");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Failed to prepare notes for writing".to_string(),
                )
            })? {
            AppendOutcome::NoChange => {
                // Nothing new survived dedup (re-save of already-committed
                // notes). Skip write/commit entirely — no empty commit, no
                // branch noise (review finding NEW-2). `no_change: true` tells
                // the frontend to show "al opgeslagen" and keep the existing
                // PR badge instead of treating a PR-less 200 as a lost save.
                tracing::debug!(law_id = %law_id, "save_annotations: no new notes, skipping write");
                return Ok(Json(SaveResponse {
                    pr: None,
                    no_change: true,
                }));
            }
            AppendOutcome::Write(text) => text,
        };

    // Validate the *resulting* document. The base was already validated
    // separately above (a pre-existing-invalid file returns a distinct
    // 409), so a failure here means the newly submitted notes are bad, or
    // serialisation produced something off. Detailed errors are logged,
    // never returned (self-XSS stance, as `save_law`).
    let doc: serde_json::Value = serde_yaml_ng::from_str(&new_text).map_err(|e| {
        tracing::error!(law_id = %law_id, error = %e, "save_annotations: produced YAML does not parse");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Produced an invalid notes file".to_string(),
        )
    })?;
    if let Err(errors) = validate_annotation_doc(&doc) {
        tracing::debug!(
            law_id = %law_id,
            errors = ?errors,
            "save_annotations rejected: resulting document failed schema validation"
        );
        return Err((
            StatusCode::BAD_REQUEST,
            "Notes are not valid against the annotation schema".to_string(),
        ));
    }

    // Every note must be about the law this path writes to (RFC-018 §1
    // keys the sidecar by law id). Allowlist, not blocklist: a note whose
    // `target.source` is absent or not a parseable `regelrecht://{law_id}`
    // is rejected, not silently ignored — the note-side analogue of
    // `save_law`'s `$id`/path guard. Runs on the merged result so a hostile
    // appended note cannot reference another law.
    if let Some((idx, err)) = first_note_not_targeting_law(&doc, &law_id) {
        tracing::debug!(
            law_id = %law_id,
            note_index = idx,
            reason = ?err,
            "save_annotations rejected: a note's target.source does not resolve to this law"
        );
        return Err((
            StatusCode::BAD_REQUEST,
            "A note's target does not match the law it is being saved to".to_string(),
        ));
    }

    backend
        .write_file(&relative_path, &new_text)
        .await
        .map_err(corpus_write_error("annotations"))?;

    let outcome = backend
        .persist(&WriteContext {
            message: format!("Notities bijgewerkt voor {}", law_id),
            author,
        })
        .await
        .map_err(corpus_write_error("annotations"))?;

    Ok(Json(save_response_from_traject(outcome)))
}

/// PUT /api/corpus/laws/{law_id} — save edited law YAML content.
///
/// Writes the new YAML to the active traject's writable-own backend (its
/// branch on the writable repo). The save does NOT mirror the new content
/// into `state.corpus.source_map`: that cache feeds GETs for users outside
/// the traject, so pushing in-progress traject edits there would leak
/// across users. Routing GETs through the per-traject corpus is a separate
/// follow-up.
///
/// The `$id` in the body must match the path parameter: allowing them to
/// diverge would either create a phantom law (new `$id` lands on an
/// existing file) or orphan the original (old `$id` can never be fetched
/// again). We reject the mismatch up-front instead of silently corrupting
/// the source map.
pub async fn save_law(
    State(state): State<AppState>,
    session: Session,
    Path(law_id): Path<String>,
    body: String,
) -> Result<Json<SaveResponse>, (StatusCode, String)> {
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

    let target = resolve_traject_law_target(&state, &session, &law_id).await?;
    let EditorWriteTarget {
        relative_path,
        backend,
    } = target;

    let outcome = {
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

    // Deliberately NOT mirroring the new YAML into `state.corpus.source_map`:
    // that cache is the read source for `/api/corpus/laws/...` for users
    // *outside* the active traject (and anyone browsing without a traject),
    // so pushing a traject's in-progress edits into it would leak unmerged
    // changes across users. GET handlers under `state.corpus.*` will keep
    // returning the last globally-loaded content until `POST
    // /api/corpus/reload` runs — routing GETs through the per-traject
    // corpus when a traject is active is a separate follow-up.

    Ok(Json(save_response_from_traject(outcome)))
}

/// DELETE /api/corpus/laws/{law_id}/scenarios/{filename} — delete a scenario file.
///
/// Requires an active traject in the session, same as `save_scenario` /
/// `save_law`: the deletion is routed through the traject's writable-own
/// backend. Without an active traject the handler returns 403.
pub async fn delete_scenario(
    State(state): State<AppState>,
    session: Session,
    Path((law_id, filename)): Path<(String, String)>,
) -> Result<Json<SaveResponse>, (StatusCode, String)> {
    validate_scenario_filename(&filename)?;
    let author = editor_user_from_session(&session).await;

    let target = resolve_traject_scenario_target(&state, &session, &law_id, &filename).await?;
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

    Ok(Json(save_response_from_traject(outcome)))
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
    //! Tests for the small, pure helpers in this module. The full
    //! save/delete handlers require an axum harness with sessions +
    //! sqlx + a real source map and live behind separate integration
    //! tests.
    use super::*;

    #[test]
    fn save_response_from_traject_passes_through_pr_when_set() {
        use regelrecht_corpus::backend::PrInfo;
        let out = PersistOutcome {
            pr: Some(PrInfo {
                number: 7,
                html_url: "https://github.com/x/y/pull/7".to_string(),
            }),
        };
        let body = save_response_from_traject(out);
        let pr = body.pr.expect("response must carry pr");
        assert_eq!(pr.number, 7);
        assert_eq!(pr.url, "https://github.com/x/y/pull/7");
    }

    #[test]
    fn save_response_from_traject_returns_none_for_plain_commit() {
        let body = save_response_from_traject(PersistOutcome { pr: None });
        assert!(body.pr.is_none());
        // Law/scenario saves are never a notes no-op.
        assert!(!body.no_change);
    }
}
