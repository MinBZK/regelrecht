use std::path::PathBuf;
use std::time::Duration;

use regelrecht_corpus::CorpusConfig;

use crate::error::{PipelineError, Result};

fn resolve_database_url() -> Result<String> {
    std::env::var("DATABASE_URL")
        .or_else(|_| std::env::var("DATABASE_SERVER_FULL"))
        .map_err(|_| PipelineError::Config("DATABASE_URL or DATABASE_SERVER_FULL not set".into()))
}

fn resolve_max_connections() -> u32 {
    std::env::var("DATABASE_MAX_CONNECTIONS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(5)
}

#[derive(Clone)]
pub struct PipelineConfig {
    pub database_url: String,
    pub max_connections: u32,
}

impl std::fmt::Debug for PipelineConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PipelineConfig")
            .field("database_url", &"<redacted>")
            .field("max_connections", &self.max_connections)
            .finish()
    }
}

impl PipelineConfig {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            database_url: resolve_database_url()?,
            max_connections: resolve_max_connections(),
        })
    }

    pub fn new(database_url: impl Into<String>) -> Self {
        Self {
            database_url: database_url.into(),
            max_connections: 5,
        }
    }

    pub fn with_max_connections(mut self, max_connections: u32) -> Self {
        self.max_connections = max_connections;
        self
    }
}

#[derive(Clone)]
pub struct WorkerConfig {
    pub database_url: String,
    pub max_connections: u32,
    pub output_dir: PathBuf,
    pub regulation_output_base: String,
    pub poll_interval: Duration,
    pub max_poll_interval: Duration,
    pub corpus_config: Option<CorpusConfig>,
}

impl std::fmt::Debug for WorkerConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorkerConfig")
            .field("database_url", &"<redacted>")
            .field("max_connections", &self.max_connections)
            .field("output_dir", &self.output_dir)
            .field("regulation_output_base", &self.regulation_output_base)
            .field("poll_interval", &self.poll_interval)
            .field("max_poll_interval", &self.max_poll_interval)
            .field("corpus_config", &self.corpus_config)
            .finish()
    }
}

impl WorkerConfig {
    pub fn from_env() -> Result<Self> {
        let database_url = resolve_database_url()?;
        let max_connections = resolve_max_connections();

        let output_dir = std::env::var("REGULATION_REPO_PATH")
            .unwrap_or_else(|_| "./regulation-repo".into())
            .into();

        let regulation_output_base =
            std::env::var("REGULATION_OUTPUT_BASE").unwrap_or_else(|_| "regulation/nl".into());

        let poll_interval_secs: u64 = std::env::var("WORKER_POLL_INTERVAL_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(5);

        let max_poll_interval_secs: u64 = std::env::var("WORKER_MAX_POLL_INTERVAL_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(60);

        let corpus_config = CorpusConfig::from_env_optional();

        Ok(Self {
            database_url,
            max_connections,
            output_dir,
            regulation_output_base,
            poll_interval: Duration::from_secs(poll_interval_secs),
            max_poll_interval: Duration::from_secs(max_poll_interval_secs),
            corpus_config,
        })
    }

    pub fn pipeline_config(&self) -> PipelineConfig {
        PipelineConfig {
            database_url: self.database_url.clone(),
            max_connections: self.max_connections,
        }
    }
}
