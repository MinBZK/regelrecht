use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Serialize;

use crate::state::AppState;

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

/// GET /api/corpus/laws — list all loaded laws with source metadata.
pub async fn list_corpus_laws(
    State(state): State<AppState>,
) -> Result<Json<Vec<CorpusLawEntry>>, (StatusCode, String)> {
    let corpus = state.corpus.read().await;

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

    Ok(Json(entries))
}

/// POST /api/sources/{id}/sync — reload all local corpus sources.
///
/// Validates that the given source exists, then rebuilds the entire
/// source map from all local sources. GitHub sources are not included
/// in the sync — they require async fetching.
pub async fn sync_source(
    State(state): State<AppState>,
    Path(source_id): Path<String>,
) -> Result<(StatusCode, Json<serde_json::Value>), (StatusCode, String)> {
    let mut corpus = state.corpus.write().await;

    // Verify that the source exists
    let source = corpus
        .registry
        .get_source(&source_id)
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                format!("Source '{}' not found", source_id),
            )
        })?;

    let is_github = matches!(source.source_type, regelrecht_corpus::SourceType::GitHub { .. });
    if is_github {
        return Err((
            StatusCode::BAD_REQUEST,
            format!("Source '{}' is a GitHub source — sync is not yet supported for remote sources", source_id),
        ));
    }

    // Rebuild source map from all local sources
    let new_map = corpus.registry.load_local_sources().map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to reload sources: {}", e),
        )
    })?;

    let law_count = new_map.laws().filter(|l| l.source_id == source_id).count();
    corpus.source_map = new_map;

    Ok((
        StatusCode::OK,
        Json(serde_json::json!({
            "source_id": source_id,
            "law_count": law_count,
            "status": "synced"
        })),
    ))
}
