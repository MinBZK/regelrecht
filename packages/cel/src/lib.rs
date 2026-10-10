//! A cell that records facts from the law in a chronicle (chronolex,
//! RFC-022): an application, and the decision taken on it.
//!
//! What a fact contains is not configured here but follows from executing the
//! law. The article that establishes an application (`produces.submission`,
//! RFC-046) is executed with an empty application; the engine fires the hooks
//! on it (Awb 4:2 and 4:13 on every application on which a beschikking is
//! taken) and yields with the model of the application. The parameters in
//! that model, as the articles mark them in `produces.extensions.chronolex`,
//! are the fields of the gram. A decision (a decretogram) carries the outputs
//! of the article that takes it.
//!
//! The cell itself only says which facts it records (its streams) and how it
//! reads them back from its chronicle (its lexostatuses): "the reduction
//! belongs to the source".
//!
//! This is the compact core of `poc/chronolex`: no channels, forms,
//! synthesis, registers, implementing policy or HTTP.

pub mod cell;
pub mod chronicle;
pub mod config;
pub mod error;
pub mod extension;
pub mod lexostatus;
pub mod shape;
#[cfg(feature = "wasm")]
pub mod wasm;

pub use cell::{Cell, Input};
pub use chronicle::Gram;
pub use error::{Error, Result};
