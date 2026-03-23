use axum::extract::{Path, Query, State};
use axum::http::{header, StatusCode};
use axum::response::IntoResponse;
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

/// A law entry with source provenance and metadata.
#[derive(Debug, Serialize)]
pub struct CorpusLawEntry {
    pub law_id: String,
    pub source_id: String,
    pub source_name: String,
    pub file_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub regulatory_layer: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub publication_date: Option<String>,
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
            let name = extract_yaml_field(&law.yaml_content, "name")
                .or_else(|| extract_yaml_field(&law.yaml_content, "officiele_titel"));
            let regulatory_layer = extract_yaml_field(&law.yaml_content, "regulatory_layer");
            let publication_date = extract_yaml_field(&law.yaml_content, "publication_date");

            CorpusLawEntry {
                law_id: law.law_id.clone(),
                source_id: law.source_id.clone(),
                source_name: law.source_name.clone(),
                file_path: law.file_path.clone(),
                name,
                regulatory_layer,
                publication_date,
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

/// GET /api/corpus/laws/{law_id} — return raw YAML content for a single law.
pub async fn get_corpus_law(
    State(state): State<AppState>,
    Path(law_id): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let corpus = state.corpus.read().await;

    let law = corpus
        .source_map
        .get_law(&law_id)
        .ok_or_else(|| (StatusCode::NOT_FOUND, format!("Law '{}' not found", law_id)))?;

    Ok((
        [(header::CONTENT_TYPE, "text/yaml; charset=utf-8")],
        law.yaml_content.clone(),
    ))
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
) -> Result<(StatusCode, Json<serde_json::Value>), (StatusCode, String)> {
    // Phase 1: read lock — validate source exists and is local, clone registry for I/O
    let registry = {
        let corpus = state.corpus.read().await;

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

        corpus.registry.clone()
    }; // read lock released

    // Phase 2: no lock held — do all disk I/O
    let mut new_map = registry.load_local_sources().map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to reload sources: {}", e),
        )
    })?;

    // Phase 2.5: read lock — preserve GitHub-sourced laws from the previous load
    {
        let old_corpus = state.corpus.read().await;
        for law in old_corpus.source_map.laws() {
            if let Some(source) = old_corpus.registry.get_source(&law.source_id) {
                if matches!(
                    source.source_type,
                    regelrecht_corpus::SourceType::GitHub { .. }
                ) {
                    if let Err(e) = new_map.load_fetched_file(
                        &law.yaml_content,
                        &law.file_path,
                        &law.source_id,
                        &law.source_name,
                        law.source_priority,
                    ) {
                        tracing::warn!(
                            law_id = %law.law_id,
                            source_id = %law.source_id,
                            error = %e,
                            "Failed to preserve GitHub law during sync"
                        );
                    }
                }
            }
        }
    } // read lock released

    // Phase 3: write lock — brief swap only
    let mut corpus = state.corpus.write().await;
    corpus.source_map = new_map;

    let source_law_count = corpus
        .source_map
        .laws()
        .filter(|l| l.source_id == source_id)
        .count();
    let total_law_count = corpus.source_map.len();

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

/// Extract a top-level YAML field value using simple line-based parsing.
///
/// Only matches `key:` at the start of a line (no leading whitespace).
fn extract_yaml_field(yaml: &str, key: &str) -> Option<String> {
    let prefix = format!("{}: ", key);
    for line in yaml.lines() {
        if let Some(rest) = line.strip_prefix(&prefix) {
            let value = rest.trim().trim_matches('"').trim_matches('\'');
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}
