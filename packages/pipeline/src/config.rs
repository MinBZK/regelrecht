use std::path::PathBuf;
use std::time::Duration;

use crate::error::{PipelineError, Result};

#[derive(Debug, Clone)]
pub struct PipelineConfig {
    pub database_url: String,
    pub max_connections: u32,
}

impl PipelineConfig {
    pub fn from_env() -> Result<Self> {
        let database_url = std::env::var("DATABASE_URL")
            .or_else(|_| std::env::var("DATABASE_SERVER_FULL"))
            .map_err(|_| PipelineError::Config(
                "DATABASE_URL or DATABASE_SERVER_FULL not set".into(),
            ))?;

        let max_connections = std::env::var("DATABASE_MAX_CONNECTIONS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(5);

        Ok(Self {
            database_url,
            max_connections,
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

#[derive(Debug, Clone)]
pub struct WorkerConfig {
    pub database_url: String,
    pub max_connections: u32,
    pub regulation_repo_url: String,
    pub regulation_repo_path: PathBuf,
    pub regulation_output_base: String,
    pub poll_interval: Duration,
    pub max_poll_interval: Duration,
    pub push_to_git: bool,
}

impl WorkerConfig {
    pub fn from_env() -> Result<Self> {
        let database_url = std::env::var("DATABASE_URL")
            .or_else(|_| std::env::var("DATABASE_SERVER_FULL"))
            .map_err(|_| PipelineError::Config(
                "DATABASE_URL or DATABASE_SERVER_FULL not set".into(),
            ))?;

        let max_connections = std::env::var("DATABASE_MAX_CONNECTIONS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(5);

        let regulation_repo_url = std::env::var("REGULATION_REPO_URL")
            .map_err(|_| PipelineError::Config("REGULATION_REPO_URL not set".into()))?;

        let regulation_repo_path = std::env::var("REGULATION_REPO_PATH")
            .unwrap_or_else(|_| "./regulation-repo".into())
            .into();

        let regulation_output_base = std::env::var("REGULATION_OUTPUT_BASE")
            .unwrap_or_else(|_| "regulation/nl".into());

        let poll_interval_secs: u64 = std::env::var("WORKER_POLL_INTERVAL_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(5);

        let max_poll_interval_secs: u64 = std::env::var("WORKER_MAX_POLL_INTERVAL_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(60);

        let push_to_git = std::env::var("FEATURE_PUSH_TO_GIT")
            .ok()
            .map(|v| v == "true" || v == "1")
            .unwrap_or(false);

        Ok(Self {
            database_url,
            max_connections,
            regulation_repo_url,
            regulation_repo_path,
            regulation_output_base,
            poll_interval: Duration::from_secs(poll_interval_secs),
            max_poll_interval: Duration::from_secs(max_poll_interval_secs),
            push_to_git,
        })
    }

    pub fn pipeline_config(&self) -> PipelineConfig {
        PipelineConfig {
            database_url: self.database_url.clone(),
            max_connections: self.max_connections,
        }
    }
}
