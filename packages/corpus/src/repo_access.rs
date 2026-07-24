//! Temporary compatibility shim.
//!
//! Repo-access validation moved into the shared `regelrecht-github` crate
//! ([`regelrecht_github::GithubClient::validate_repo_access`]). This module
//! re-exports the crate's types and keeps the old free-function signature so
//! the editor-api still compiles while it is migrated onto the client
//! directly. Removed once that migration lands.

#[cfg(feature = "github")]
pub use regelrecht_github::{RepoAccessError, RepoInfo};

/// Delegates to [`regelrecht_github::GithubClient::validate_repo_access`],
/// building a client pointed at `base_url`. Kept only for the transition.
#[cfg(feature = "github")]
pub async fn validate_repo_access(
    base_url: &str,
    owner: &str,
    repo: &str,
    base_branch: &str,
    token: &str,
) -> Result<RepoInfo, RepoAccessError> {
    let client = regelrecht_github::GithubClient::new()
        .map_err(|e| RepoAccessError::Transport(e.to_string()))?
        .with_base_url(base_url);
    client
        .validate_repo_access(owner, repo, base_branch, token)
        .await
}
