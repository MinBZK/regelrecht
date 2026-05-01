//! Shared DTOs and helpers for corpus list endpoints.
//!
//! The admin and editor-api crates each expose `GET /api/sources` and
//! related endpoints. The body of those handlers — pagination
//! parameters, source-summary serialization, and the registry-walk that
//! produces the summaries — is identical between the two callers. This
//! module is the single source of truth so they can stay in sync.

use serde::{Deserialize, Serialize};

use crate::{CorpusRegistry, SourceMap, SourceType};

/// Default page size for list endpoints.
pub const DEFAULT_LIMIT: usize = 100;
/// Maximum page size accepted on list endpoints.
pub const MAX_LIMIT: usize = 1000;

/// Pagination query parameters.
#[derive(Debug, Deserialize)]
pub struct PaginationParams {
    #[serde(default)]
    pub offset: usize,
    #[serde(default)]
    pub limit: Option<usize>,
}

impl PaginationParams {
    /// Resolve the effective page size from the request, clamped to `MAX_LIMIT`.
    pub fn effective_limit(&self) -> usize {
        self.limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT)
    }
}

/// Summary of a corpus source returned by `GET /api/sources`.
#[derive(Debug, Serialize)]
pub struct SourceSummary {
    pub id: String,
    pub name: String,
    pub source_type: String,
    pub priority: u32,
    pub law_count: usize,
}

/// Stable string tag for a [`SourceType`] used in JSON responses.
pub fn source_type_tag(source_type: &SourceType) -> &'static str {
    match source_type {
        SourceType::Local { .. } => "local",
        SourceType::GitHub { .. } => "github",
    }
}

/// Build the [`SourceSummary`] list returned by `GET /api/sources`.
pub fn build_source_summaries(
    registry: &CorpusRegistry,
    source_map: &SourceMap,
) -> Vec<SourceSummary> {
    registry
        .sources()
        .iter()
        .map(|source| {
            let law_count = source_map
                .laws()
                .filter(|law| law.source_id == source.id)
                .count();
            SourceSummary {
                id: source.id.clone(),
                name: source.name.clone(),
                source_type: source_type_tag(&source.source_type).to_string(),
                priority: source.priority,
                law_count,
            }
        })
        .collect()
}
