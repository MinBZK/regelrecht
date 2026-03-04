use std::path::PathBuf;

use crate::error::{CorpusError, Result};

#[derive(Clone)]
pub struct CorpusConfig {
    pub repo_url: String,
    pub repo_path: PathBuf,
    pub branch: String,
    pub git_author_name: String,
    pub git_author_email: String,
    git_token: Option<String>,
}

impl std::fmt::Debug for CorpusConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CorpusConfig")
            .field("repo_url", &self.repo_url)
            .field("repo_path", &self.repo_path)
            .field("branch", &self.branch)
            .field("git_author_name", &self.git_author_name)
            .field("git_author_email", &self.git_author_email)
            .field("git_token", &self.git_token.as_ref().map(|_| "***"))
            .finish()
    }
}

impl CorpusConfig {
    /// Create a new `CorpusConfig` without authentication.
    pub fn new(repo_url: impl Into<String>, repo_path: impl Into<PathBuf>) -> Self {
        Self {
            repo_url: repo_url.into(),
            repo_path: repo_path.into(),
            branch: "development".into(),
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

        let branch = std::env::var("CORPUS_BRANCH").unwrap_or_else(|_| "development".into());

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

    /// Set the git token for authentication.
    pub fn with_token(mut self, token: impl Into<String>) -> Self {
        self.git_token = Some(token.into());
        self
    }

    /// Returns the git token, if configured.
    pub(crate) fn git_token(&self) -> Option<&str> {
        self.git_token.as_deref()
    }

    /// Build the clone URL with the username embedded (but NOT the token).
    ///
    /// The token is provided separately via `GIT_ASKPASS` to avoid exposing
    /// credentials in `/proc/[pid]/cmdline`.
    pub(crate) fn clone_url(&self) -> String {
        match &self.git_token {
            Some(_) if self.repo_url.starts_with("https://") => {
                self.repo_url.replacen("https://", "https://token@", 1)
            }
            _ => self.repo_url.clone(),
        }
    }

    /// Path where the GIT_ASKPASS helper script is written.
    pub(crate) fn askpass_script_path(&self) -> PathBuf {
        self.repo_path
            .parent()
            .unwrap_or(std::path::Path::new("/tmp"))
            .join(".git-askpass.sh")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clone_url_with_token_embeds_username_only() {
        let config = CorpusConfig {
            repo_url: "https://github.com/MinBZK/regelrecht-corpus.git".into(),
            repo_path: "/tmp/test".into(),
            branch: "main".into(),
            git_author_name: "test".into(),
            git_author_email: "test@test.nl".into(),
            git_token: Some("ghp_abc123".into()),
        };
        // Token should NOT appear in the URL — only the username
        let url = config.clone_url();
        assert_eq!(url, "https://token@github.com/MinBZK/regelrecht-corpus.git");
        assert!(!url.contains("ghp_abc123"));
    }

    #[test]
    fn test_clone_url_without_token() {
        let config = CorpusConfig {
            repo_url: "https://github.com/MinBZK/regelrecht-corpus.git".into(),
            repo_path: "/tmp/test".into(),
            branch: "main".into(),
            git_author_name: "test".into(),
            git_author_email: "test@test.nl".into(),
            git_token: None,
        };
        assert_eq!(
            config.clone_url(),
            "https://github.com/MinBZK/regelrecht-corpus.git"
        );
    }

    #[test]
    fn test_clone_url_ssh() {
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
            config.clone_url(),
            "git@github.com:MinBZK/regelrecht-corpus.git"
        );
    }

    #[test]
    fn test_debug_hides_token() {
        let config = CorpusConfig {
            repo_url: "https://github.com/MinBZK/regelrecht-corpus.git".into(),
            repo_path: "/tmp/test".into(),
            branch: "main".into(),
            git_author_name: "test".into(),
            git_author_email: "test@test.nl".into(),
            git_token: Some("ghp_abc123".into()),
        };
        let debug_output = format!("{:?}", config);
        assert!(!debug_output.contains("ghp_abc123"));
        assert!(debug_output.contains("***"));
    }
}
