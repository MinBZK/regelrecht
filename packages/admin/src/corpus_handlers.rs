use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::error::ApiError;
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
    pub source_id: String,
    pub source_name: String,
    pub file_path: String,
}

/// GET /api/sources — list all registered corpus sources with law counts.
pub async fn list_sources(
    State(state): State<AppState>,
) -> Result<Json<Vec<SourceSummary>>, ApiError> {
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
) -> Result<Json<Vec<CorpusLawEntry>>, ApiError> {
    let corpus = state.corpus.read().await;
    let limit = params.limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT);

    let mut entries: Vec<CorpusLawEntry> = corpus
        .source_map
        .laws()
        .map(|law| CorpusLawEntry {
            law_id: law.law_id.clone(),
            source_id: law.source_id.clone(),
            source_name: law.source_name.clone(),
            file_path: std::path::Path::new(&law.file_path)
                .file_name()
                .map(|f| f.to_string_lossy().into_owned())
                .unwrap_or_else(|| {
                    tracing::warn!(path = %law.file_path, "unexpected file_path with no filename");
                    String::new()
                }),
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

/// POST /api/sources/{id}/sync — rebuild all local corpus sources.
///
/// Validates that the given source exists and is local, then does a
/// full rebuild of the source map from all local sources. This ensures
/// conflict resolution is correct: if a winning source drops a law,
/// the previously-shadowed version from another source gets promoted.
///
/// Disk I/O runs outside the write lock to avoid blocking reads.
pub async fn sync_source(
    State(state): State<AppState>,
    Path(source_id): Path<String>,
) -> Result<(StatusCode, Json<serde_json::Value>), ApiError> {
    // Phase 1: read lock — validate source exists and is local, clone registry for I/O
    let registry = {
        let corpus = state.corpus.read().await;

        let source = corpus
            .registry
            .get_source(&source_id)
            .ok_or_else(|| ApiError::NotFound(format!("Source '{}' not found", source_id)))?;

        if matches!(
            source.source_type,
            regelrecht_corpus::SourceType::GitHub { .. }
        ) {
            return Err(ApiError::NotImplemented(format!(
                "Source '{}' is a GitHub source — sync is not yet supported for remote sources",
                source_id
            )));
        }

        corpus.registry.clone()
    }; // read lock released

    // Phase 2: no lock held — do all disk I/O on a blocking thread
    // to avoid blocking the Tokio worker pool during directory traversal.
    let new_map = tokio::task::spawn_blocking(move || registry.load_local_sources())
        .await
        .map_err(|e| ApiError::Internal(format!("spawn_blocking failed: {}", e)))?
        .map_err(|e| ApiError::Internal(format!("Failed to reload sources: {}", e)))?;

    // Compute counts before acquiring write lock to minimize lock duration.
    let source_law_count = new_map.laws().filter(|l| l.source_id == source_id).count();
    let total_law_count = new_map.len();

    // Phase 3: write lock — brief swap only
    let mut corpus = state.corpus.write().await;
    corpus.source_map = new_map;
    drop(corpus);

    Ok((
        StatusCode::OK,
        Json(serde_json::json!({
            "source_id": source_id,
            "source_law_count": source_law_count,
            "total_law_count": total_law_count,
            "status": "synced"
        })),
    ))
}
