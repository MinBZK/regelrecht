use thiserror::Error;

#[derive(Debug, Error)]
pub enum CorpusError {
    #[error("git command failed: {0}")]
    Git(String),

    #[error("configuration error: {0}")]
    Config(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("YAML parse error: {0}")]
    Yaml(#[from] serde_yaml_ng::Error),

    #[error("write not supported: {0}")]
    ReadOnly(String),

    /// Optimistic-concurrency conflict on a remote write (e.g. GitHub
    /// Contents API returned 409 because the file's `sha` moved between
    /// the read and the PUT). Surfaced separately from `Git` so a backend
    /// can recognise it and retry with a fresh SHA without parsing error
    /// strings.
    #[error("conflict: {0}")]
    Conflict(String),

    /// Client-provided precondition (e.g. an `If-Match` ETag) did not
    /// match the current state. Distinct from `Conflict` so handlers can
    /// surface a 412 instead of a 409 — semantically "your view is stale,
    /// reload" rather than "the upstream raced us, retry".
    #[error("precondition failed: {0}")]
    PreconditionFailed(String),
}

pub type Result<T> = std::result::Result<T, CorpusError>;
