use thiserror::Error;

#[derive(Debug, Error)]
pub enum CorpusError {
    #[error("git command failed: {0}")]
    Git(String),

    #[error("configuration error: {0}")]
    Config(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, CorpusError>;
