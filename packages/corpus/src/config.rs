use std::path::PathBuf;

use crate::error::{CorpusError, Result};

#[derive(Debug, Clone)]
pub struct CorpusConfig {
    pub repo_url: String,
    pub repo_path: PathBuf,
    pub branch: String,
    pub git_author_name: String,
    pub git_author_email: String,
    git_token: Option<String>,
}

impl CorpusConfig {
    /// Create a new `CorpusConfig` without authentication.
    pub fn new(repo_url: impl Into<String>, repo_path: impl Into<PathBuf>) -> Self {
        Self {
            repo_url: repo_url.into(),
            repo_path: repo_path.into(),
            branch: "main".into(),
            git_author_name: "regelrecht-harvester".into(),
            git_author_email: "noreply@minbzk.nl".into(),
            git_token: None,
        }
    }

    /// Load configuration from environment variables.
    ///
    /// Required: `CORPUS_REPO_URL`
    /// Optional: `CORPUS_REPO_PATH` (default: `/data/corpus-repo`),
    ///           `CORPUS_BRANCH` (default: `main`),
    ///           `CORPUS_GIT_AUTHOR_NAME` (default: `regelrecht-harvester`),
    ///           `CORPUS_GIT_AUTHOR_EMAIL` (default: `noreply@minbzk.nl`),
    ///           `CORPUS_GIT_TOKEN` (for authentication)
    pub fn from_env() -> Result<Self> {
        let repo_url = std::env::var("CORPUS_REPO_URL")
            .map_err(|_| CorpusError::Config("CORPUS_REPO_URL not set".into()))?;

        let repo_path = std::env::var("CORPUS_REPO_PATH")
            .unwrap_or_else(|_| "/data/corpus-repo".into())
            .into();

        let branch = std::env::var("CORPUS_BRANCH").unwrap_or_else(|_| "main".into());

        let git_author_name = std::env::var("CORPUS_GIT_AUTHOR_NAME")
            .unwrap_or_else(|_| "regelrecht-harvester".into());

        let git_author_email =
            std::env::var("CORPUS_GIT_AUTHOR_EMAIL").unwrap_or_else(|_| "noreply@minbzk.nl".into());

        let git_token = std::env::var("CORPUS_GIT_TOKEN").ok();

        Ok(Self {
            repo_url,
            repo_path,
            branch,
            git_author_name,
            git_author_email,
            git_token,
        })
    }

    /// Try to load configuration from environment variables.
    /// Returns `None` if `CORPUS_REPO_URL` is not set (corpus disabled).
    pub fn from_env_optional() -> Option<Self> {
        if std::env::var("CORPUS_REPO_URL").is_err() {
            return None;
        }
        Self::from_env().ok()
    }

    /// Build the authenticated clone URL by injecting the token.
    pub(crate) fn authenticated_url(&self) -> String {
        match &self.git_token {
            Some(token) if self.repo_url.starts_with("https://") => {
                self.repo_url
                    .replacen("https://", &format!("https://x-access-token:{token}@"), 1)
            }
            _ => self.repo_url.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_authenticated_url_with_token() {
        let config = CorpusConfig {
            repo_url: "https://github.com/MinBZK/regelrecht-corpus.git".into(),
            repo_path: "/tmp/test".into(),
            branch: "main".into(),
            git_author_name: "test".into(),
            git_author_email: "test@test.nl".into(),
            git_token: Some("ghp_abc123".into()),
        };
        assert_eq!(
            config.authenticated_url(),
            "https://x-access-token:ghp_abc123@github.com/MinBZK/regelrecht-corpus.git"
        );
    }

    #[test]
    fn test_authenticated_url_without_token() {
        let config = CorpusConfig {
            repo_url: "https://github.com/MinBZK/regelrecht-corpus.git".into(),
            repo_path: "/tmp/test".into(),
            branch: "main".into(),
            git_author_name: "test".into(),
            git_author_email: "test@test.nl".into(),
            git_token: None,
        };
        assert_eq!(
            config.authenticated_url(),
            "https://github.com/MinBZK/regelrecht-corpus.git"
        );
    }

    #[test]
    fn test_authenticated_url_ssh() {
        let config = CorpusConfig {
            repo_url: "git@github.com:MinBZK/regelrecht-corpus.git".into(),
            repo_path: "/tmp/test".into(),
            branch: "main".into(),
            git_author_name: "test".into(),
            git_author_email: "test@test.nl".into(),
            git_token: Some("ghp_abc123".into()),
        };
        // SSH URLs should not be modified
        assert_eq!(
            config.authenticated_url(),
            "git@github.com:MinBZK/regelrecht-corpus.git"
        );
    }
}
