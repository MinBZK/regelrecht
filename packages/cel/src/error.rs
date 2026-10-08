//! What can go wrong in a cell.

/// An error of the cell.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The configuration or the law does not say what the cell needs.
    #[error("setup: {0}")]
    Setup(String),
    /// A request the cell refuses: a field the law does not ask, a missing
    /// reference, an unknown event.
    #[error("refused: {0}")]
    Refused(String),
    /// An execution that no longer arises in the case: it has a gram of the
    /// stage that ends it (`until`). A refusal too, but one a caller may read
    /// as "nothing more to come", so it has its own variant.
    #[error("ended: {0}")]
    Ended(String),
    /// A message about a gram that already has its answer (one answer per
    /// reference). A refusal too, but a sender that delivers again may read
    /// it as "delivered", so it has its own variant.
    #[error("answered: {0}")]
    Answered(String),
    /// The engine failed.
    #[error("engine: {0}")]
    Engine(#[from] regelrecht_engine::EngineError),
    /// Reading or writing the chronicle failed.
    #[error("chronicle: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

pub(crate) fn setup(message: impl Into<String>) -> Error {
    Error::Setup(message.into())
}

impl Error {
    /// A short, stable name of the kind of error, for a caller outside Rust
    /// (the browser) to tell them apart.
    pub fn code(&self) -> &'static str {
        match self {
            Error::Setup(_) => "setup",
            Error::Refused(_) => "refused",
            Error::Ended(_) => "ended",
            Error::Answered(_) => "answered",
            Error::Engine(_) => "engine",
            Error::Io(_) => "io",
        }
    }
}

pub(crate) fn refused(message: impl Into<String>) -> Error {
    Error::Refused(message.into())
}
