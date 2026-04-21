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
    /// Optional sparse-checkout paths (cone mode). When set, only these
    /// directory trees are materialized in the working copy after clone.
    pub sparse_paths: Option<Vec<String>>,
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
            .field("sparse_paths", &self.sparse_paths)
            .finish()
    }
}

/// Extract the ZAD deployment name from a Kubernetes pod hostname.
///
/// Pod hostnames follow `{deployment}-{component}-{rs-hash}-{pod-hash}`.
/// Only the literal `regelrecht` (production) and `pr<digits>` (PR previews)
/// are recognised; anything else returns `None` so a stray multi-segment
/// deployment name can't be misread as a bare first segment.
fn deployment_from_hostname(hostname: &str) -> Option<String> {
    let first = hostname.split('-').next()?;
    let is_pr_preview = first
        .strip_prefix("pr")
        .is_some_and(|rest| !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit()));
    (first == "regelrecht" || is_pr_preview).then(|| first.to_string())
}

/// Resolve the corpus branch from explicit config and platform variables.
///
/// Priority: `CORPUS_BRANCH` > `DEPLOYMENT_NAME` > `HOSTNAME` prefix > `"development"`.
/// Both `DEPLOYMENT_NAME` and the hostname-derived prefix are ignored when they
/// equal `"regelrecht"` (production), so production workers always fall through
/// to the default `"development"` branch.
fn resolve_branch(
    corpus_branch: Option<String>,
    deployment_name: Option<String>,
    hostname: Option<String>,
) -> String {
    if let Some(branch) = corpus_branch.filter(|b| !b.is_empty()) {
        return branch;
    }
    let derived = deployment_name
        .filter(|n| !n.is_empty())
        .or_else(|| hostname.as_deref().and_then(deployment_from_hostname));
    if let Some(name) = derived.filter(|n| n != "regelrecht") {
        return name;
    }
    "development".into()
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
            sparse_paths: None,
        }
    }

    /// Load configuration from environment variables.
    ///
    /// Required: `CORPUS_REPO_URL`
    /// Optional: `CORPUS_REPO_PATH` (default: `/tmp/corpus-repo`),
    ///           `CORPUS_BRANCH` (default: `DEPLOYMENT_NAME`, else `HOSTNAME` prefix
    ///            for PR previews, else `development`),
    ///           `CORPUS_GIT_AUTHOR_NAME` (default: `regelrecht-harvester`),
    ///           `CORPUS_GIT_AUTHOR_EMAIL` (default: `noreply@minbzk.nl`),
    ///           `CORPUS_GIT_TOKEN` (for authentication)
    pub fn from_env() -> Result<Self> {
        let repo_url = std::env::var("CORPUS_REPO_URL")
            .map_err(|_| CorpusError::Config("CORPUS_REPO_URL not set".into()))?;

        let repo_path = std::env::var("CORPUS_REPO_PATH")
            .unwrap_or_else(|_| "/tmp/corpus-repo".into())
            .into();

        let branch = resolve_branch(
            std::env::var("CORPUS_BRANCH").ok(),
            std::env::var("DEPLOYMENT_NAME").ok(),
            std::env::var("HOSTNAME").ok(),
        );

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
            sparse_paths: None,
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
            sparse_paths: None,
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
            sparse_paths: None,
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
            sparse_paths: None,
        };
        // SSH URLs should not be modified
        assert_eq!(
            config.clone_url(),
            "git@github.com:MinBZK/regelrecht-corpus.git"
        );
    }

    #[test]
    fn resolve_branch_defaults_to_development() {
        assert_eq!(resolve_branch(None, None, None), "development");
    }

    #[test]
    fn resolve_branch_uses_corpus_branch() {
        assert_eq!(
            resolve_branch(
                Some("custom".into()),
                Some("pr42".into()),
                Some("pr99-harvester-worker-abc-xyz".into())
            ),
            "custom"
        );
    }

    #[test]
    fn resolve_branch_uses_corpus_branch_without_deployment() {
        assert_eq!(resolve_branch(Some("custom".into()), None, None), "custom");
    }

    #[test]
    fn resolve_branch_uses_deployment_name_for_preview() {
        assert_eq!(resolve_branch(None, Some("pr42".into()), None), "pr42");
    }

    #[test]
    fn resolve_branch_ignores_production_deployment() {
        assert_eq!(
            resolve_branch(None, Some("regelrecht".into()), None),
            "development"
        );
    }

    #[test]
    fn resolve_branch_ignores_empty_values() {
        assert_eq!(
            resolve_branch(Some("".into()), Some("".into()), Some("".into())),
            "development"
        );
    }

    #[test]
    fn resolve_branch_uses_pr_hostname_when_deployment_name_missing() {
        assert_eq!(
            resolve_branch(None, None, Some("pr429-harvester-worker-abc-xyz".into())),
            "pr429"
        );
    }

    #[test]
    fn resolve_branch_ignores_production_hostname() {
        assert_eq!(
            resolve_branch(
                None,
                None,
                Some("regelrecht-harvester-worker-abc-xyz".into())
            ),
            "development"
        );
    }

    #[test]
    fn resolve_branch_deployment_name_beats_hostname() {
        assert_eq!(
            resolve_branch(
                None,
                Some("pr99".into()),
                Some("regelrecht-harvester-worker-abc-xyz".into())
            ),
            "pr99"
        );
    }

    #[test]
    fn resolve_branch_production_deployment_name_beats_pr_hostname() {
        assert_eq!(
            resolve_branch(
                None,
                Some("regelrecht".into()),
                Some("pr429-harvester-worker-abc-xyz".into())
            ),
            "development"
        );
    }

    #[test]
    fn deployment_from_hostname_recognises_pr_and_prod() {
        assert_eq!(
            deployment_from_hostname("pr568-enrichworker-abc-xyz"),
            Some("pr568".into())
        );
        assert_eq!(
            deployment_from_hostname("regelrecht-harvester-admin-abc-xyz"),
            Some("regelrecht".into())
        );
    }

    #[test]
    fn deployment_from_hostname_rejects_unknown_prefixes() {
        assert_eq!(deployment_from_hostname("feature-x-foo-a-b"), None);
        assert_eq!(deployment_from_hostname("prabc-foo-a-b"), None);
        assert_eq!(deployment_from_hostname("pr-foo-a-b"), None);
        assert_eq!(deployment_from_hostname(""), None);
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
            sparse_paths: None,
        };
        let debug_output = format!("{:?}", config);
        assert!(!debug_output.contains("ghp_abc123"));
        assert!(debug_output.contains("***"));
    }
}
