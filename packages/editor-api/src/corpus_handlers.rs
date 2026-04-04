use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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
        while let Ok(Some(entry)) = read_dir.next_entry().await {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "feature") {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    entries.push(ScenarioEntry {
                        filename: name.to_string(),
                    });
                }
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
