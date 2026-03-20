use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};

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
            source_id: law.source_id.clone(),
            source_name: law.source_name.clone(),
            file_path: law.file_path.clone(),
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
pub async fn sync_source(
    State(state): State<AppState>,
    Path(source_id): Path<String>,
) -> Result<(StatusCode, Json<serde_json::Value>), (StatusCode, String)> {
    let mut corpus = state.corpus.write().await;

    // Verify source exists
    let source = corpus.registry.get_source(&source_id).ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            format!("Source '{}' not found", source_id),
        )
    })?;

    if matches!(
        source.source_type,
        regelrecht_corpus::SourceType::GitHub { .. }
    ) {
        return Err((
            StatusCode::NOT_IMPLEMENTED,
            format!(
                "Source '{}' is a GitHub source — sync is not yet supported for remote sources",
                source_id
            ),
        ));
    }

    // Full rebuild: reload all local sources from scratch.
    // This is the only correct approach because conflict resolution
    // depends on all sources being present (a removed law in source A
    // must promote source B's previously-shadowed version).
    let previous_map = std::mem::take(&mut corpus.source_map);
    match corpus.registry.load_local_sources() {
        Ok(new_map) => {
            corpus.source_map = new_map;
        }
        Err(e) => {
            // Restore previous state on failure
            corpus.source_map = previous_map;
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to reload sources: {}", e),
            ));
        }
    }

    let law_count = corpus
        .source_map
        .laws()
        .filter(|l| l.source_id == source_id)
        .count();

    Ok((
        StatusCode::OK,
        Json(serde_json::json!({
            "source_id": source_id,
            "law_count": law_count,
            "status": "synced"
        })),
    ))
}
