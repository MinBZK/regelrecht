use std::sync::Arc;

use regelrecht_corpus::SourceMap;
use tokio::sync::RwLock;

use crate::vlam_client::VlamClient;

#[derive(Clone)]
pub struct AppState {
    /// Loaded corpus sources with provenance metadata.
    pub corpus: Arc<RwLock<CorpusState>>,
    /// Optional VLAM LLM client for AI-generated operation titles.
    pub vlam: Option<VlamClient>,
}

/// State for the corpus subsystem.
pub struct CorpusState {
    pub registry: regelrecht_corpus::CorpusRegistry,
    pub source_map: SourceMap,
}
