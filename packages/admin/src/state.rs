use std::sync::Arc;

use regelrecht_corpus::SourceMap;
use sqlx::PgPool;
use tokio::sync::RwLock;

use crate::config::AppConfig;
use crate::metrics::MetricsCache;
use crate::oidc::ConfiguredClient;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub oidc_client: Option<Arc<ConfiguredClient>>,
    pub end_session_url: Option<String>,
    pub config: Arc<AppConfig>,
    pub metrics_cache: Arc<MetricsCache>,
    /// Shared HTTP client for outgoing requests (connection pool reuse).
    pub http_client: reqwest::Client,
    /// Loaded corpus sources with provenance metadata.
    pub corpus: Arc<RwLock<CorpusState>>,
}

/// State for the corpus subsystem.
pub struct CorpusState {
    pub registry: regelrecht_corpus::CorpusRegistry,
    pub source_map: SourceMap,
}

impl CorpusState {
    /// Create an empty corpus state (used by tests).
    #[allow(dead_code)]
    pub fn empty() -> Self {
        Self {
            registry: regelrecht_corpus::CorpusRegistry::empty(),
            source_map: SourceMap::new(),
        }
    }
}
