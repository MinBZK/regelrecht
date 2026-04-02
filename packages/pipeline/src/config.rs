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
    /// Maximum time a single job may run before being aborted by the worker.
    /// Default: 20 minutes. Configurable via `WORKER_JOB_TIMEOUT_SECS`.
    pub job_timeout: Duration,
    /// Jobs stuck in 'processing' longer than this are reaped (reset or failed).
    /// Default: 30 minutes. Configurable via `WORKER_ORPHAN_TIMEOUT_SECS`.
    pub orphan_timeout: Duration,
    /// Number of consecutive failures before a law is marked as exhausted.
    /// Default: 10. Configurable via `EXHAUSTED_THRESHOLD`.
    pub exhausted_threshold: i32,
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
            .field("job_timeout", &self.job_timeout)
            .field("orphan_timeout", &self.orphan_timeout)
            .field("exhausted_threshold", &self.exhausted_threshold)
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

        let job_timeout_secs: u64 = std::env::var("WORKER_JOB_TIMEOUT_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(20 * 60); // 20 minutes

        let orphan_timeout_secs: u64 = std::env::var("WORKER_ORPHAN_TIMEOUT_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(30 * 60); // 30 minutes

        let exhausted_threshold: i32 = std::env::var("EXHAUSTED_THRESHOLD")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(10)
            .max(1);

        Ok(Self {
            database_url,
            max_connections,
            output_dir,
            regulation_output_base,
            poll_interval: Duration::from_secs(poll_interval_secs),
            max_poll_interval: Duration::from_secs(max_poll_interval_secs),
            corpus_config,
            job_timeout: Duration::from_secs(job_timeout_secs),
            orphan_timeout: Duration::from_secs(orphan_timeout_secs),
            exhausted_threshold,
        })
    }

    pub fn pipeline_config(&self) -> PipelineConfig {
        PipelineConfig {
            database_url: self.database_url.clone(),
            max_connections: self.max_connections,
        }
    }
}
