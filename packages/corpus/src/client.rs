use std::path::{Path, PathBuf};

use tokio::process::Command;

use crate::config::CorpusConfig;
use crate::error::{CorpusError, Result};

pub struct CorpusClient {
    config: CorpusConfig,
    askpass_path: Option<PathBuf>,
}

impl CorpusClient {
    pub fn new(config: CorpusConfig) -> Self {
        Self {
            config,
            askpass_path: None,
        }
    }

    /// Write a GIT_ASKPASS helper script so the git token is passed via
    /// environment variables instead of being embedded in the clone URL
    /// (which would be visible via `/proc/[pid]/cmdline`).
    fn ensure_askpass_script(&mut self) -> Result<()> {
        if self.config.git_token().is_none() {
            return Ok(());
        }

        let script_path = self.config.askpass_script_path();
        if let Some(parent) = script_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| CorpusError::Git(format!("failed to create askpass dir: {e}")))?;
        }
        std::fs::write(
            &script_path,
            "#!/bin/sh\nprintf '%s\\n' \"$REGELRECHT_GIT_TOKEN\"\n",
        )
        .map_err(|e| CorpusError::Git(format!("failed to write askpass script: {e}")))?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&script_path, std::fs::Permissions::from_mode(0o700))
                .map_err(|e| CorpusError::Git(format!("failed to set askpass permissions: {e}")))?;
        }

        self.askpass_path = Some(script_path);
        Ok(())
    }

    /// Ensure the corpus repo is available locally.
    ///
    /// If the repo directory doesn't exist, clones it (shallow, single branch).
    /// If it exists, fetches and resets to the remote branch.
    pub async fn ensure_repo(&mut self) -> Result<()> {
        self.ensure_askpass_script()?;

        let repo_path = &self.config.repo_path;

        if repo_path.join(".git").exists() {
            tracing::info!(path = %repo_path.display(), "corpus repo exists, updating");
            self.git_fetch_reset().await?;
        } else {
            tracing::info!(path = %repo_path.display(), "cloning corpus repo");
            self.git_clone().await?;
        }

        Ok(())
    }

    /// Returns the local path to the corpus repo working directory.
    pub fn repo_path(&self) -> &Path {
        &self.config.repo_path
    }

    /// Stage the given paths, commit, and push to the remote branch.
    ///
    /// If there are no changes to commit (working tree is clean), this is a no-op.
    pub async fn commit_and_push(&self, paths: &[PathBuf], message: &str) -> Result<()> {
        // Stage the specific files
        let mut add_args = vec!["add", "--"];
        let path_strings: Vec<String> = paths
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect();
        for p in &path_strings {
            add_args.push(p);
        }
        self.run_git(&add_args).await?;

        // Check if there's anything to commit
        let status_output = self.run_git_output(&["status", "--porcelain"]).await?;

        if status_output.trim().is_empty() {
            tracing::debug!("no changes to commit, skipping");
            return Ok(());
        }

        // Commit
        self.run_git(&["commit", "-m", message]).await?;

        // Pull --rebase to incorporate any concurrent remote changes.
        // If rebase fails, abort it to prevent leaving repo in broken state.
        if let Err(e) = self
            .run_git(&["pull", "--rebase", "origin", &self.config.branch])
            .await
        {
            tracing::warn!(error = %e, "pull --rebase failed, aborting rebase");
            let _ = self.run_git(&["rebase", "--abort"]).await;
            return Err(e);
        }

        // Push
        self.run_git(&["push", "origin", &self.config.branch])
            .await?;

        tracing::info!(message = %message, "committed and pushed to corpus repo");
        Ok(())
    }

    async fn git_clone(&self) -> Result<()> {
        let url = self.config.clone_url();
        let path_str = self.config.repo_path.to_string_lossy().to_string();

        let output = Command::new("git")
            .args([
                "clone",
                "--branch",
                &self.config.branch,
                "--single-branch",
                &url,
                &path_str,
            ])
            .envs(self.git_env())
            .output()
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);

            // Branch doesn't exist on remote — clone development and create the branch
            if stderr.contains("not found in upstream") || stderr.contains("Remote branch") {
                tracing::info!(
                    branch = %self.config.branch,
                    "branch not found on remote, creating from development"
                );
                return self.git_clone_and_create_branch().await;
            }

            let sanitized = self.sanitize_output(&stderr);
            return Err(CorpusError::Git(format!("git clone failed: {sanitized}")));
        }

        self.configure_git_user().await?;
        Ok(())
    }

    /// Clone the `development` base branch, then create and push the target branch.
    async fn git_clone_and_create_branch(&self) -> Result<()> {
        let url = self.config.clone_url();
        let path_str = self.config.repo_path.to_string_lossy().to_string();

        let output = Command::new("git")
            .args([
                "clone",
                "--branch",
                "development",
                "--single-branch",
                &url,
                &path_str,
            ])
            .envs(self.git_env())
            .output()
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let sanitized = self.sanitize_output(&stderr);
            return Err(CorpusError::Git(format!(
                "git clone (development) failed: {sanitized}"
            )));
        }

        self.configure_git_user().await?;

        // Create the target branch and push it
        self.run_git(&["checkout", "-b", &self.config.branch])
            .await?;
        self.run_git(&["push", "-u", "origin", &self.config.branch])
            .await?;

        tracing::info!(branch = %self.config.branch, "created and pushed new branch");
        Ok(())
    }

    async fn configure_git_user(&self) -> Result<()> {
        self.run_git(&["config", "user.name", &self.config.git_author_name])
            .await?;
        self.run_git(&["config", "user.email", &self.config.git_author_email])
            .await?;
        Ok(())
    }

    async fn git_fetch_reset(&self) -> Result<()> {
        self.run_git(&["fetch", "origin", &self.config.branch])
            .await?;

        let remote_ref = format!("origin/{}", self.config.branch);
        self.run_git(&["reset", "--hard", &remote_ref]).await?;

        Ok(())
    }

    /// Run a git command in the repo directory and check for success.
    async fn run_git(&self, args: &[&str]) -> Result<()> {
        let output = Command::new("git")
            .args(args)
            .current_dir(&self.config.repo_path)
            .envs(self.git_env())
            .output()
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let sanitized = self.sanitize_output(&stderr);
            return Err(CorpusError::Git(format!(
                "git {} failed: {}",
                args.first().unwrap_or(&""),
                sanitized
            )));
        }

        Ok(())
    }

    /// Run a git command and return stdout.
    async fn run_git_output(&self, args: &[&str]) -> Result<String> {
        let output = Command::new("git")
            .args(args)
            .current_dir(&self.config.repo_path)
            .envs(self.git_env())
            .output()
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let sanitized = self.sanitize_output(&stderr);
            return Err(CorpusError::Git(format!(
                "git {} failed: {}",
                args.first().unwrap_or(&""),
                sanitized
            )));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// Strip the git token from output to prevent credential leaks in logs.
    fn sanitize_output(&self, output: &str) -> String {
        match self.config.git_token() {
            Some(token) if !token.is_empty() => output.replace(token, "***"),
            _ => output.to_string(),
        }
    }

    /// Environment variables for git commands (author/committer identity
    /// and optional GIT_ASKPASS for token-based authentication).
    fn git_env(&self) -> Vec<(String, String)> {
        let mut env = vec![
            (
                "GIT_AUTHOR_NAME".into(),
                self.config.git_author_name.clone(),
            ),
            (
                "GIT_AUTHOR_EMAIL".into(),
                self.config.git_author_email.clone(),
            ),
            (
                "GIT_COMMITTER_NAME".into(),
                self.config.git_author_name.clone(),
            ),
            (
                "GIT_COMMITTER_EMAIL".into(),
                self.config.git_author_email.clone(),
            ),
            ("GIT_TERMINAL_PROMPT".into(), "0".into()),
        ];

        if let (Some(askpass), Some(token)) = (&self.askpass_path, self.config.git_token()) {
            env.push(("GIT_ASKPASS".into(), askpass.to_string_lossy().into()));
            env.push(("REGELRECHT_GIT_TOKEN".into(), token.into()));
        }

        env
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Create a bare git repo with one empty initial commit on `development`.
    async fn setup_bare_repo(dir: &Path) -> PathBuf {
        let bare_path = dir.join("bare.git");
        Command::new("git")
            .args(["init", "--bare", "--initial-branch=development"])
            .arg(&bare_path)
            .output()
            .await
            .unwrap();

        // Push an initial commit via a temp clone (use file:// for --depth support)
        let tmp_clone = dir.join("tmp-clone");
        let bare_url = format!("file://{}", bare_path.display());
        Command::new("git")
            .args(["clone", &bare_url])
            .arg(&tmp_clone)
            .output()
            .await
            .unwrap();
        for args in [
            vec!["config", "user.name", "test"],
            vec!["config", "user.email", "test@test.nl"],
            vec!["commit", "--allow-empty", "-m", "init"],
            vec!["push", "origin", "development"],
        ] {
            Command::new("git")
                .args(&args)
                .current_dir(&tmp_clone)
                .output()
                .await
                .unwrap();
        }

        bare_path
    }

    /// Clone a bare repo and configure git user.
    async fn clone_with_config(bare_path: &Path, repo_path: &Path) {
        let bare_url = format!("file://{}", bare_path.display());
        Command::new("git")
            .args(["clone", &bare_url])
            .arg(repo_path)
            .output()
            .await
            .unwrap();
        for args in [
            vec!["config", "user.name", "test"],
            vec!["config", "user.email", "test@test.nl"],
        ] {
            Command::new("git")
                .args(&args)
                .current_dir(repo_path)
                .output()
                .await
                .unwrap();
        }
    }

    #[test]
    fn test_sanitize_output_strips_token() {
        let config = CorpusConfig::new("https://github.com/example/repo.git", "/tmp/test")
            .with_token("ghp_secret123");
        let client = CorpusClient::new(config);

        let output = "fatal: could not read from remote https://token:ghp_secret123@github.com/example/repo.git";
        let sanitized = client.sanitize_output(output);
        assert!(!sanitized.contains("ghp_secret123"));
        assert!(sanitized.contains("***"));
    }

    #[test]
    fn test_sanitize_output_no_token() {
        let config = CorpusConfig::new("https://github.com/example/repo.git", "/tmp/test");
        let client = CorpusClient::new(config);

        let output = "fatal: repository not found";
        let sanitized = client.sanitize_output(output);
        assert_eq!(sanitized, output);
    }

    #[tokio::test]
    async fn test_ensure_repo_clones_when_missing() {
        let dir = tempfile::tempdir().unwrap();
        let repo_path = dir.path().join("corpus");
        let bare_path = setup_bare_repo(dir.path()).await;
        let bare_url = format!("file://{}", bare_path.display());

        let config = CorpusConfig::new(&bare_url, &repo_path);
        let mut client = CorpusClient::new(config);
        client.ensure_repo().await.unwrap();

        assert!(repo_path.join(".git").exists());
    }

    #[tokio::test]
    async fn test_commit_and_push_no_changes() {
        let dir = tempfile::tempdir().unwrap();
        let bare_path = setup_bare_repo(dir.path()).await;
        let bare_url = format!("file://{}", bare_path.display());
        let repo_path = dir.path().join("corpus");
        clone_with_config(&bare_path, &repo_path).await;

        let config = CorpusConfig::new(&bare_url, &repo_path);
        let client = CorpusClient::new(config);

        // No changes — should be a no-op
        let result = client.commit_and_push(&[], "no changes").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_commit_and_push_with_changes() {
        let dir = tempfile::tempdir().unwrap();
        let bare_path = setup_bare_repo(dir.path()).await;
        let bare_url = format!("file://{}", bare_path.display());
        let repo_path = dir.path().join("corpus");
        clone_with_config(&bare_path, &repo_path).await;

        // Write a test file
        let test_file = repo_path.join("test.txt");
        tokio::fs::write(&test_file, "hello").await.unwrap();

        let config = CorpusConfig::new(&bare_url, &repo_path);
        let client = CorpusClient::new(config);
        client
            .commit_and_push(&[test_file], "add test file")
            .await
            .unwrap();

        // Verify commit was pushed by checking the bare repo
        let log = Command::new("git")
            .args(["log", "--oneline", "-1"])
            .current_dir(&bare_path)
            .output()
            .await
            .unwrap();
        let log_str = String::from_utf8_lossy(&log.stdout);
        assert!(log_str.contains("add test file"));
    }

    #[tokio::test]
    async fn test_ensure_repo_creates_branch_if_missing() {
        let dir = tempfile::tempdir().unwrap();
        let repo_path = dir.path().join("corpus");
        let bare_path = setup_bare_repo(dir.path()).await;
        let bare_url = format!("file://{}", bare_path.display());

        // Request a branch that doesn't exist — should clone development and create it
        let mut config = CorpusConfig::new(&bare_url, &repo_path);
        config.branch = "pr999".into();
        let mut client = CorpusClient::new(config);
        client.ensure_repo().await.unwrap();

        assert!(repo_path.join(".git").exists());

        // Verify local branch is pr999
        let output = Command::new("git")
            .args(["branch", "--show-current"])
            .current_dir(&repo_path)
            .output()
            .await
            .unwrap();
        let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
        assert_eq!(branch, "pr999");

        // Verify the branch was pushed to the bare remote
        let output = Command::new("git")
            .args(["branch"])
            .current_dir(&bare_path)
            .output()
            .await
            .unwrap();
        let branches = String::from_utf8_lossy(&output.stdout);
        assert!(branches.contains("pr999"));
    }
}
