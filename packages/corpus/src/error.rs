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

    /// GitHub refused a write with a 403: the authenticating identity has
    /// no push access to the repository or organisation (e.g. missing
    /// repo permissions, or an org's OAuth App access restrictions
    /// blocking a user token). Surfaced separately from `Git` so callers
    /// can translate it into a clear "not allowed" message instead of a
    /// generic internal error. Carries the GitHub response text for
    /// operator logging — never show it to end users verbatim.
    #[error("write denied by GitHub (403): {0}")]
    WriteDenied(String),
}

pub type Result<T> = std::result::Result<T, CorpusError>;
