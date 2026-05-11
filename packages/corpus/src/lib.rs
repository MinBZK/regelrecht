pub mod auth;
pub mod backend;
pub mod client;
pub mod config;
pub mod dto;
pub mod error;
#[cfg(feature = "github")]
pub mod github;
pub mod models;
#[cfg(feature = "github")]
pub mod pr_client;
pub mod registry;
pub mod source_map;
pub mod validation;

pub use backend::PrInfo;
#[cfg(feature = "github")]
pub use backend::SessionGitBackend;
pub use client::CorpusClient;
pub use config::{deployment_from_hostname, CorpusConfig};
pub use error::CorpusError;
#[cfg(feature = "github")]
pub use github::{FetchResult, GitHubFetcher};
pub use models::{RegistryManifest, Source, SourceType};
#[cfg(feature = "github")]
pub use pr_client::PullRequestClient;
pub use registry::CorpusRegistry;
pub use source_map::SourceMap;
