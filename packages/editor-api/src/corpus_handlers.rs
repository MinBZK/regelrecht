use std::path::PathBuf;
use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use regelrecht_corpus::backend::{RepoBackend, WriteContext};
use regelrecht_corpus::source_map::LoadedLaw;
use regelrecht_corpus::SourceType;

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
    pub source_id: String,
    pub source_name: String,
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
        .map(|law| CorpusLawEntry {
            law_id: law.law_id.clone(),
            name: law.name.clone(),
            source_id: law.source_id.clone(),
            source_name: law.source_name.clone(),
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

/// A scenario file entry.
#[derive(Debug, Serialize)]
pub struct ScenarioEntry {
    pub filename: String,
}

/// Derive the scenarios directory from a law's file_path.
///
/// Given a law file at `.../wet_op_de_zorgtoeslag/2025-01-01.yaml`,
/// the scenarios directory is `.../wet_op_de_zorgtoeslag/scenarios/`.
fn scenarios_dir_for_law(file_path: &str) -> Option<PathBuf> {
    let path = PathBuf::from(file_path);
    let parent = path.parent()?;
    Some(parent.join("scenarios"))
}

/// GET /api/corpus/laws/{law_id}/scenarios — list available scenario files.
pub async fn list_scenarios(
    State(state): State<AppState>,
    Path(law_id): Path<String>,
) -> Result<Json<Vec<ScenarioEntry>>, (StatusCode, String)> {
    // Resolve the scenarios directory while holding the lock, then drop it before I/O.
    let scenarios_dir = {
        let corpus = state.corpus.read().await;
        let law = corpus
            .source_map
            .get_law(&law_id)
            .ok_or_else(|| (StatusCode::NOT_FOUND, format!("Law '{}' not found", law_id)))?;
        match scenarios_dir_for_law(&law.file_path) {
            Some(dir) => dir,
            _ => return Ok(Json(Vec::new())),
        }
    };

    if !scenarios_dir.is_dir() {
        return Ok(Json(Vec::new()));
    }

    let mut entries = Vec::new();
    if let Ok(mut read_dir) = tokio::fs::read_dir(&scenarios_dir).await {
        loop {
            match read_dir.next_entry().await {
                Ok(Some(entry)) => {
                    let path = entry.path();
                    if path.extension().is_some_and(|ext| ext == "feature") {
                        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                            entries.push(ScenarioEntry {
                                filename: name.to_string(),
                            });
                        }
                    }
                }
                Ok(None) => break,
                Err(_) => continue,
            }
        }
    }

    entries.sort_by(|a, b| a.filename.cmp(&b.filename));
    Ok(Json(entries))
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
    // Reject path traversal attempts
    if filename.contains('/') || filename.contains('\\') || filename.contains("..") {
        return Err((StatusCode::BAD_REQUEST, "Invalid filename".to_string()));
    }

    if !filename.ends_with(".feature") {
        return Err((
            StatusCode::BAD_REQUEST,
            "Only .feature files are served".to_string(),
        ));
    }

    // Resolve path while holding the lock, then drop it before I/O.
    let file_path = {
        let corpus = state.corpus.read().await;
        let law = corpus
            .source_map
            .get_law(&law_id)
            .ok_or_else(|| (StatusCode::NOT_FOUND, format!("Law '{}' not found", law_id)))?;
        let scenarios_dir = scenarios_dir_for_law(&law.file_path)
            .ok_or_else(|| (StatusCode::NOT_FOUND, "No scenarios directory".to_string()))?;
        scenarios_dir.join(&filename)
    };

    let content = tokio::fs::read_to_string(&file_path).await.map_err(|_| {
        (
            StatusCode::NOT_FOUND,
            format!("Scenario '{}' not found", filename),
        )
    })?;

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
    if filename.contains('/') || filename.contains('\\') || filename.contains("..") {
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

/// Compute the backend-relative path for a scenario file.
///
/// Given a law's file_path and its source configuration, returns the path
/// relative to the backend's root that points to `scenarios/{filename}`.
fn scenario_relative_path(
    law: &LoadedLaw,
    source: &regelrecht_corpus::Source,
    filename: &str,
) -> Result<PathBuf, (StatusCode, String)> {
    let file_path = std::path::Path::new(&law.file_path);
    let law_dir = file_path.parent().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Cannot determine law directory".to_string(),
        )
    })?;

    // Determine the source root to strip from the law's file_path.
    let source_root: PathBuf = match &source.source_type {
        SourceType::Local { local } => local.path.clone(),
        SourceType::GitHub { github } => {
            // For GitHub-fetched laws, file_path is repo-relative (e.g.
            // "regulation/nl/wet/law_id/2025.yaml"). The backend's subpath
            // already accounts for github.path, so strip it here.
            github.path.as_ref().map(PathBuf::from).unwrap_or_default()
        }
    };

    let relative_law_dir = law_dir.strip_prefix(&source_root).unwrap_or(law_dir);
    Ok(relative_law_dir.join("scenarios").join(filename))
}

/// Resolved backend information for a law.
struct ResolvedBackend {
    law: LoadedLaw,
    source: regelrecht_corpus::Source,
    backend: Arc<Mutex<Box<dyn RepoBackend>>>,
}

/// Look up the backend and source for a law.
fn resolve_backend(
    corpus: &crate::state::CorpusState,
    law_id: &str,
) -> Result<ResolvedBackend, (StatusCode, String)> {
    let law = corpus
        .source_map
        .get_law(law_id)
        .ok_or_else(|| (StatusCode::NOT_FOUND, format!("Law '{}' not found", law_id)))?
        .clone();

    let source = corpus
        .registry
        .sources()
        .iter()
        .find(|s| s.id == law.source_id)
        .ok_or_else(|| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Source '{}' not found in registry", law.source_id),
            )
        })?
        .clone();

    let backend = corpus
        .backends
        .get(&law.source_id)
        .ok_or_else(|| {
            (
                StatusCode::FORBIDDEN,
                format!("No write backend available for source '{}'", law.source_id),
            )
        })?
        .clone();

    Ok(ResolvedBackend {
        law,
        source,
        backend,
    })
}

// ---------------------------------------------------------------------------
// Save / Delete scenario endpoints
// ---------------------------------------------------------------------------

/// PUT /api/corpus/laws/{law_id}/scenarios/{filename} — save a scenario file.
pub async fn save_scenario(
    State(state): State<AppState>,
    Path((law_id, filename)): Path<(String, String)>,
    body: String,
) -> Result<StatusCode, (StatusCode, String)> {
    validate_scenario_filename(&filename)?;

    let resolved = {
        let corpus = state.corpus.read().await;
        resolve_backend(&corpus, &law_id)?
    };

    let relative_path = scenario_relative_path(&resolved.law, &resolved.source, &filename)?;

    let backend = resolved.backend.lock().await;
    if !backend.is_writable() {
        return Err((StatusCode::FORBIDDEN, "Source is read-only".to_string()));
    }

    backend
        .write_file(&relative_path, &body)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    backend
        .persist(&WriteContext {
            message: format!("Update scenario {} for {}", filename, law_id),
        })
        .await
        .map_err(|e| (StatusCode::CONFLICT, format!("Failed to persist: {e}")))?;

    Ok(StatusCode::OK)
}

/// DELETE /api/corpus/laws/{law_id}/scenarios/{filename} — delete a scenario file.
pub async fn delete_scenario(
    State(state): State<AppState>,
    Path((law_id, filename)): Path<(String, String)>,
) -> Result<StatusCode, (StatusCode, String)> {
    validate_scenario_filename(&filename)?;

    let resolved = {
        let corpus = state.corpus.read().await;
        resolve_backend(&corpus, &law_id)?
    };

    let relative_path = scenario_relative_path(&resolved.law, &resolved.source, &filename)?;

    let backend = resolved.backend.lock().await;
    if !backend.is_writable() {
        return Err((StatusCode::FORBIDDEN, "Source is read-only".to_string()));
    }

    backend
        .delete_file(&relative_path)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    backend
        .persist(&WriteContext {
            message: format!("Delete scenario {} for {}", filename, law_id),
        })
        .await
        .map_err(|e| (StatusCode::CONFLICT, format!("Failed to persist: {e}")))?;

    Ok(StatusCode::OK)
}
