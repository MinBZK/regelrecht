#[cfg(feature = "annotation-validation")]
pub mod annotation_schema;
pub mod auth;
pub mod backend;
pub mod client;
pub mod config;
pub mod dto;
pub mod error;
#[cfg(feature = "github")]
pub mod github;
#[cfg(feature = "github")]
pub mod github_api_backend;
pub mod models;
#[cfg(feature = "github")]
pub mod pr_client;
pub mod registry;
#[cfg(feature = "github")]
pub mod repo_access;
pub mod source_map;
pub mod timing;
pub mod validation;

pub use backend::PrInfo;
#[cfg(feature = "github")]
pub use backend::SessionGitBackend;
pub use client::CorpusClient;
pub use config::{deployment_from_hostname, CorpusConfig};
pub use error::CorpusError;
#[cfg(feature = "github")]
pub use github::{FetchResult, GitHubFetcher};
#[cfg(feature = "github")]
pub use github_api_backend::GitHubApiBackend;
pub use models::{RegistryManifest, Source, SourceType};
#[cfg(feature = "github")]
pub use pr_client::PullRequestClient;
pub use registry::{CorpusRegistry, SourceIndexFailure};
#[cfg(feature = "github")]
pub use repo_access::{validate_repo_access, RepoAccessError, RepoInfo};
pub use source_map::SourceMap;
