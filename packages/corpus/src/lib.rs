pub mod auth;
pub mod client;
pub mod config;
pub mod error;
#[cfg(feature = "github")]
pub mod github;
pub mod models;
pub mod registry;
pub mod source_map;

pub use client::CorpusClient;
pub use config::CorpusConfig;
pub use error::CorpusError;
#[cfg(feature = "github")]
pub use github::GitHubFetcher;
pub use models::{RegistryManifest, Source, SourceType};
pub use registry::CorpusRegistry;
pub use source_map::SourceMap;
