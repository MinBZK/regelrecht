//! Shared DTOs and helpers for corpus list endpoints.
//!
//! The admin and editor-api crates each expose `GET /api/sources` and
//! related endpoints. The body of those handlers — pagination
//! parameters, source-summary serialization, and the registry-walk that
//! produces the summaries — is identical between the two callers. This
//! module is the single source of truth so they can stay in sync.

use serde::{Deserialize, Serialize};

use crate::{CorpusRegistry, SourceMap, SourceType};

/// Default page size for list endpoints. Internal — callers go through
/// [`PaginationParams::effective_limit`].
const DEFAULT_LIMIT: usize = 100;
/// Maximum page size accepted on list endpoints. Internal — callers go
/// through [`PaginationParams::effective_limit`].
const MAX_LIMIT: usize = 1000;

/// Pagination query parameters.
#[derive(Debug, Deserialize)]
pub struct PaginationParams {
    #[serde(default)]
    pub offset: usize,
    #[serde(default)]
    pub limit: Option<usize>,
    /// Optional case-insensitive search term. When present, the law list is
    /// filtered server-side to laws whose `law_id` or name contains it —
    /// letting the editor search the full corpus index without shipping every
    /// law to the browser.
    #[serde(default)]
    pub q: Option<String>,
    /// Optional comma-separated set of exact `law_id`s. When present, only
    /// those laws are returned — so the library sidebar can resolve metadata
    /// for just the user's favorites and traject edits instead of fetching the
    /// whole corpus and filtering client-side.
    ///
    /// Takes precedence over [`q`](Self::q): if both are sent, `ids` wins and
    /// `q` is ignored (they're not combined). Callers pick one or the other.
    #[serde(default)]
    pub ids: Option<String>,
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
