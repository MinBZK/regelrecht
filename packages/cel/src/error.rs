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

pub(crate) fn refused(message: impl Into<String>) -> Error {
    Error::Refused(message.into())
}
