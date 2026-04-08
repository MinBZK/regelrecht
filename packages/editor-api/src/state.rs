use std::sync::Arc;

use regelrecht_corpus::SourceMap;
use tokio::sync::RwLock;

use crate::config::AppConfig;
use crate::oidc::ConfiguredClient;

#[derive(Clone)]
pub struct AppState {
    /// Loaded corpus sources with provenance metadata.
    pub corpus: Arc<RwLock<CorpusState>>,
    pub oidc_client: Option<Arc<ConfiguredClient>>,
    pub end_session_url: Option<String>,
    pub config: Arc<AppConfig>,
    pub http_client: reqwest::Client,
}

/// State for the corpus subsystem.
pub struct CorpusState {
    pub registry: regelrecht_corpus::CorpusRegistry,
    pub source_map: SourceMap,
}

impl CorpusState {
    #[allow(dead_code)]
    pub fn empty() -> Self {
        Self {
            registry: regelrecht_corpus::CorpusRegistry::empty(),
            source_map: SourceMap::new(),
        }
    }
}
