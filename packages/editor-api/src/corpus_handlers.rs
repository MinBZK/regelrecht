use std::path::PathBuf;
use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use regelrecht_corpus::backend::{RepoBackend, WriteContext};
use regelrecht_corpus::source_map::{
    collect_law_outputs, extract_law_id, resolve_display_name, validate_yaml_syntax, LoadedLaw,
};
use regelrecht_corpus::CorpusError;

use crate::state::AppState;

/// Default and maximum page size for list endpoints.
const DEFAULT_LIMIT: usize = 100;
const MAX_LIMIT: usize = 1000;

/// Pagination query parameters.
#[derive(Debug, Deserialize)]
pub struct PaginationParams {
    #[serde(default)]
    pub offset: usize,
    #[serde(default)]
    pub limit: Option<usize>,
}

/// Summary of a corpus source.
#[derive(Debug, Serialize)]
pub struct SourceSummary {
    pub id: String,
    pub name: String,
    pub source_type: String,
    pub priority: u32,
    pub law_count: usize,
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

    let summaries: Vec<SourceSummary> = corpus
        .registry
        .sources()
        .iter()
        .map(|source| {
            let law_count = corpus
                .source_map
                .laws()
                .filter(|law| law.source_id == source.id)
                .count();

            let source_type = match &source.source_type {
                regelrecht_corpus::SourceType::Local { .. } => "local",
                regelrecht_corpus::SourceType::GitHub { .. } => "github",
            };

            SourceSummary {
                id: source.id.clone(),
                name: source.name.clone(),
                source_type: source_type.to_string(),
                priority: source.priority,
                law_count,
            }
        })
        .collect();

    Ok(Json(summaries))
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
    let limit = params.limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT);

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
    // Route reads through the same backend resolution as writes so a save
    // followed by a list/get always sees its own writes.
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

/// Resolved backend information for a law.
struct ResolvedBackend {
    law: LoadedLaw,
    backend: Arc<Mutex<Box<dyn RepoBackend>>>,
    /// Whether the resolved backend supports writes. Read handlers ignore
    /// this; write handlers must reject the request with 403 if `false`.
    writable: bool,
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
                writable: true,
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
                "law's own source has no writable backend; routing reads and writes through verified-matching source"
            );
            return Ok(ResolvedBackend {
                law,
                backend: entry.backend.clone(),
                writable: true,
            });
        }
    }

    // 3. Fall through to the law's own read-only backend so reads still
    //    work. Write handlers turn this into a 403.
    if let Some(entry) = corpus.backends.get(&law.source_id) {
        return Ok(ResolvedBackend {
            law,
            backend: entry.backend.clone(),
            writable: entry.writable,
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

/// Resolve write target: pick the backend, compute the scenario path, and
/// lock the backend. Shared by save and delete handlers. Returns 403 if the
/// resolved backend is read-only — the caller cannot recover from this.
async fn resolve_write_target(
    state: &AppState,
    law_id: &str,
    filename: &str,
) -> Result<(PathBuf, tokio::sync::OwnedMutexGuard<Box<dyn RepoBackend>>), (StatusCode, String)> {
    let resolved = {
        let corpus = state.corpus.read().await;
        resolve_backend_for_law(&corpus, law_id).await?
    };

    if !resolved.writable {
        return Err((
            StatusCode::FORBIDDEN,
            format!(
                "No writable backend available for law '{}' (source '{}' is read-only \
                 and no other registered source contains a matching copy of the law)",
                law_id, resolved.law.source_id
            ),
        ));
    }

    let rel_dir = law_relative_dir(&resolved.law)?;
    let relative_path = rel_dir.join("scenarios").join(filename);

    let backend = resolved.backend.lock_owned().await;

    Ok((relative_path, backend))
}

/// PUT /api/corpus/laws/{law_id}/scenarios/{filename} — save a scenario file.
pub async fn save_scenario(
    State(state): State<AppState>,
    Path((law_id, filename)): Path<(String, String)>,
    body: String,
) -> Result<StatusCode, (StatusCode, String)> {
    validate_scenario_filename(&filename)?;

    let (relative_path, backend) = resolve_write_target(&state, &law_id, &filename).await?;

    backend
        .write_file(&relative_path, &body)
        .await
        .map_err(corpus_write_error("scenario"))?;

    backend
        .persist(&WriteContext {
            message: format!("Update scenario {} for {}", filename, law_id),
        })
        .await
        .map_err(corpus_write_error("scenario"))?;

    Ok(StatusCode::OK)
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
    Path(law_id): Path<String>,
    body: String,
) -> Result<StatusCode, (StatusCode, String)> {
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

    // Resolve backend with writable fallback (same path scenarios take).
    let resolved = {
        let corpus = state.corpus.read().await;
        resolve_backend_for_law(&corpus, &law_id).await?
    };

    if !resolved.writable {
        // Log the internal source id (and the law id, even though the
        // caller already knows it) for operators but keep both out of the
        // HTTP body. `source_id` is a registry key ("central",
        // "local-scratch", …) so leaking it exposes infrastructure naming;
        // `law_id` echoed in the body would also flow through
        // useLaw.saveError into ndd-inline-dialog's supporting-text, with
        // the same self-XSS concern as the $id-mismatch branch above. The
        // hard-coded message keeps both branches consistent.
        tracing::warn!(
            law_id = %law_id,
            source_id = %resolved.law.source_id,
            "save_law: no writable backend for law"
        );
        return Err((
            StatusCode::FORBIDDEN,
            "Law is stored on a read-only source".to_string(),
        ));
    }

    let relative_path = PathBuf::from(&resolved.law.relative_path);

    {
        // The `if !resolved.writable` early return above ensures the
        // RepoBackend will not refuse the write under normal operation, so
        // the only realistic path to a `CorpusError::ReadOnly` here is a
        // TOCTOU between the writability check and the write itself
        // (e.g. the underlying volume flips read-only mid-request). In
        // that race the `corpus_write_error` helper falls through to its
        // generic 500-style mapping, mirroring the existing scenario
        // write paths; the `ReadOnly` arm of `corpus_write_error` —
        // which echoes `e.to_string()` — is unreachable here in
        // practice and is not in scope to harden in this PR.
        let backend = resolved.backend.lock_owned().await;
        backend
            .write_file(&relative_path, &body)
            .await
            .map_err(corpus_write_error("law"))?;
        backend
            .persist(&WriteContext {
                message: format!("Update law {}", law_id),
            })
            .await
            .map_err(corpus_write_error("law"))?;
    }

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

    Ok(StatusCode::OK)
}

/// POST /api/corpus/reload — re-scan local corpus sources.
///
/// Picks up newly harvested YAML files without restarting the server. Only
/// reloads local sources (fast, ~ms) — GitHub-backed sources are unchanged.
pub async fn reload_corpus(
    State(state): State<AppState>,
) -> Result<Json<ReloadResponse>, (StatusCode, String)> {
    let mut corpus = state.corpus.write().await;
    let new_map = corpus.registry.load_local_sources().map_err(|e| {
        tracing::error!(error = %e, "corpus reload failed");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to reload corpus".to_string(),
        )
    })?;
    let law_count = new_map.len();
    corpus.source_map = new_map;
    tracing::info!(law_count, "corpus reloaded from local sources");
    Ok(Json(ReloadResponse { law_count }))
}

#[derive(Debug, Serialize)]
pub struct ReloadResponse {
    pub law_count: usize,
}

/// DELETE /api/corpus/laws/{law_id}/scenarios/{filename} — delete a scenario file.
pub async fn delete_scenario(
    State(state): State<AppState>,
    Path((law_id, filename)): Path<(String, String)>,
) -> Result<StatusCode, (StatusCode, String)> {
    validate_scenario_filename(&filename)?;

    let (relative_path, backend) = resolve_write_target(&state, &law_id, &filename).await?;

    backend
        .delete_file(&relative_path)
        .await
        .map_err(corpus_write_error("scenario"))?;

    backend
        .persist(&WriteContext {
            message: format!("Delete scenario {} for {}", filename, law_id),
        })
        .await
        .map_err(corpus_write_error("scenario"))?;

    Ok(StatusCode::OK)
}
