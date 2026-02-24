use thiserror::Error;

#[derive(Debug, Error)]
pub enum PipelineError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("migration error: {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),

    #[error("job not found: {0}")]
    JobNotFound(uuid::Uuid),

    #[error("job {0} is not in processing state")]
    JobNotProcessing(uuid::Uuid),

    #[error("law not found: {0}")]
    LawNotFound(String),

    #[error("invalid state transition: {0}")]
    InvalidStateTransition(String),

    #[error("configuration error: {0}")]
    Config(String),

    #[error("invalid input: {0}")]
    InvalidInput(String),

    #[error("harvester error: {0}")]
    Harvester(#[from] regelrecht_harvester::HarvesterError),

    #[error("git error: {message}")]
    Git { message: String, stderr: String },

    #[error("worker error: {0}")]
    Worker(String),

    #[error("task join error: {0}")]
    Join(#[from] tokio::task::JoinError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("YAML error: {0}")]
    Yaml(#[from] serde_yaml_ng::Error),
}

pub type Result<T> = std::result::Result<T, PipelineError>;
