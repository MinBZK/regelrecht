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

    /// Whether this client has a push token configured.
    pub fn has_push_token(&self) -> bool {
        self.config.git_token().is_some()
    }

    /// Create a local branch (no push).
    pub async fn create_local_branch(&self, branch: &str) -> Result<()> {
        self.run_git(&["checkout", "-b", branch]).await?;
        tracing::info!(branch = %branch, "created local branch");
        Ok(())
    }

    /// Stage the given paths and commit locally (no push).
    ///
    /// If there are no changes to commit (working tree is clean), this is a no-op.
    pub async fn commit_local(&self, paths: &[PathBuf], message: &str) -> Result<()> {
        let mut add_args = vec!["add", "--"];
        let path_strings: Vec<String> = paths
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect();
        for p in &path_strings {
            add_args.push(p);
        }
        self.run_git(&add_args).await?;

        let status_output = self.run_git_output(&["status", "--porcelain"]).await?;
        if status_output.trim().is_empty() {
            tracing::debug!("no changes to commit, skipping");
            return Ok(());
        }

        self.run_git(&["commit", "-m", message]).await?;
        tracing::info!(message = %message, "committed locally (no push)");
        Ok(())
    }

    /// Maximum number of rebase+push attempts before giving up.
    const MAX_PUSH_ATTEMPTS: u32 = 5;

    /// Stage the given paths, commit, and push to the remote branch.
    ///
    /// If there are no changes to commit (working tree is clean), this is a no-op.
    /// Uses a retry loop around rebase+push to handle concurrent push race
    /// conditions where multiple workers push to the same branch.
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

        // Retry loop: rebase on remote, then push. If push fails due to a
        // concurrent update, fetch+rebase again and retry with backoff.
        let mut last_error = None;
        for attempt in 1..=Self::MAX_PUSH_ATTEMPTS {
            // Pull --rebase to incorporate any concurrent remote changes.
            // On shallow clones (--depth 1), rebase may fail if the remote
            // advanced by many commits. The error-recovery path below
            // restores the working tree (abort rebase + hard-reset to remote)
            // and propagates the error for job-level retry. The enriched
            // files remain on disk so the next attempt can re-stage them.
            if let Err(e) = self
                .run_git(&["pull", "--rebase", "origin", &self.config.branch])
                .await
            {
                tracing::warn!(attempt, error = %e, "pull --rebase failed, aborting rebase");
                let _ = self.run_git(&["rebase", "--abort"]).await;
                // Hard-reset to remote to recover from force-pushes or diverged
                // history. The harvested files are still on disk so the next
                // job-level retry can re-stage and commit them cleanly.
                let remote_ref = format!("origin/{}", self.config.branch);
                let _ = self
                    .run_git(&["fetch", "--depth", "1", "origin", &self.config.branch])
                    .await;
                let _ = self.run_git(&["reset", "--hard", &remote_ref]).await;
                return Err(e);
            }

            // Push — may fail if another worker pushed between our rebase and push.
            match self.run_git(&["push", "origin", &self.config.branch]).await {
                Ok(()) => {
                    if attempt > 1 {
                        tracing::info!(attempt, "push succeeded after retry");
                    }
                    tracing::info!(message = %message, "committed and pushed to corpus repo");
                    return Ok(());
                }
                Err(e) => {
                    tracing::warn!(
                        attempt,
                        max = Self::MAX_PUSH_ATTEMPTS,
                        error = %e,
                        "push failed{}",
                        if attempt < Self::MAX_PUSH_ATTEMPTS {
                            ", will retry with rebase"
                        } else {
                            ", all attempts exhausted"
                        }
                    );
                    last_error = Some(e);
                }
            }

            if attempt < Self::MAX_PUSH_ATTEMPTS {
                // Exponential backoff: 500ms, 1s, 2s, 4s
                let delay = std::time::Duration::from_millis(500 * 2u64.pow(attempt - 1));
                tokio::time::sleep(delay).await;
            }
        }

        Err(last_error.unwrap_or_else(|| CorpusError::Git("push failed after retries".into())))
    }

    async fn git_clone(&self) -> Result<()> {
        let url = self.config.clone_url();
        let path_str = self.config.repo_path.to_string_lossy().to_string();
        let sparse = self.config.sparse_paths.is_some();

        let mut args = vec![
            "clone",
            "--depth",
            "1",
            "--quiet",
            "--branch",
            &self.config.branch,
            "--single-branch",
        ];
        if sparse {
            args.push("--no-checkout");
        }
        args.push(&url);
        args.push(&path_str);

        let output = Command::new("git")
            .args(&args)
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
        self.setup_sparse_checkout().await?;
        Ok(())
    }

    /// Clone the `development` base branch, then create and push the target branch.
    async fn git_clone_and_create_branch(&self) -> Result<()> {
        let url = self.config.clone_url();
        let path_str = self.config.repo_path.to_string_lossy().to_string();
        let sparse = self.config.sparse_paths.is_some();

        let mut args = vec![
            "clone",
            "--depth",
            "1",
            "--quiet",
            "--branch",
            "development",
            "--single-branch",
        ];
        if sparse {
            args.push("--no-checkout");
        }
        args.push(&url);
        args.push(&path_str);

        let output = Command::new("git")
            .args(&args)
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
        self.setup_sparse_checkout().await?;

        // Create the target branch and push it
        self.run_git(&["checkout", "-b", &self.config.branch])
            .await?;
        self.run_git(&["push", "-u", "origin", &self.config.branch])
            .await?;

        tracing::info!(branch = %self.config.branch, "created and pushed new branch");
        Ok(())
    }

    /// Configure sparse-checkout if `sparse_paths` is set on the config.
    ///
    /// Uses cone mode so only the listed directory trees are materialized.
    /// No-op when `sparse_paths` is `None` (full checkout).
    async fn setup_sparse_checkout(&self) -> Result<()> {
        let paths = match self.config.sparse_paths {
            Some(ref p) if !p.is_empty() => p,
            _ => return Ok(()),
        };

        self.run_git(&["sparse-checkout", "init", "--cone"]).await?;

        let mut args = vec!["sparse-checkout", "set"];
        let refs: Vec<&str> = paths.iter().map(|s| s.as_str()).collect();
        args.extend(refs);
        self.run_git(&args).await?;

        // Materialize the working tree (only the sparse paths)
        self.run_git(&["checkout"]).await?;

        tracing::info!(paths = ?paths, "sparse checkout configured");
        Ok(())
    }

    /// Pull `paths` from `base_branch` into the working tree without a merge commit, leaving them unstaged so the next `commit_and_push` absorbs them.
    pub async fn checkout_from_branch(&self, base_branch: &str, paths: &[&str]) -> Result<()> {
        // Skip already-tracked paths so prior machine_readable additions
        // aren't overwritten by the raw base-branch version.
        let mut missing: Vec<&str> = Vec::new();
        for path in paths {
            let listed = self.run_git_output(&["ls-files", "--", path]).await?;
            if listed.trim().is_empty() {
                missing.push(path);
            }
        }
        if missing.is_empty() {
            tracing::debug!(paths = ?paths, "all paths already tracked, skipping checkout from base");
            return Ok(());
        }

        self.run_git(&["fetch", "--depth", "1", "origin", base_branch])
            .await?;

        // Use FETCH_HEAD rather than `origin/<base_branch>`: the enrichment
        // clone is `--single-branch` for `enrich/<provider>`, so its fetch
        // refspec never creates `refs/remotes/origin/<base_branch>` even
        // after `git fetch origin <base_branch>`. FETCH_HEAD is always set.
        let mut args = vec!["checkout", "FETCH_HEAD", "--"];
        args.extend(&missing);
        self.run_git(&args).await?;

        // Unstage the checked-out files so they don't pollute an empty
        // `git status --porcelain` check before the enrichment commits.
        let mut reset_args = vec!["reset", "HEAD", "--"];
        reset_args.extend(&missing);
        self.run_git(&reset_args).await?;

        tracing::info!(
            base = %base_branch,
            paths = ?missing,
            "checked out paths from base branch"
        );
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
        self.run_git(&["fetch", "--depth", "1", "origin", &self.config.branch])
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

    /// Environment variables for git commands (author/committer identity,
    /// optional GIT_ASKPASS for token-based authentication, and resource
    /// limits for container environments).
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

        // Disable threaded index preloading (core.preloadIndex) and limit
        // index operations to a single thread (index.threads) to prevent
        // "unable to create threaded lstat: Resource temporarily unavailable"
        // errors in resource-constrained containers with low PID/thread limits.
        let git_configs: &[(&str, &str)] =
            &[("core.preloadIndex", "false"), ("index.threads", "1")];
        env.push(("GIT_CONFIG_COUNT".into(), git_configs.len().to_string()));
        for (i, (key, value)) in git_configs.iter().enumerate() {
            env.push((format!("GIT_CONFIG_KEY_{i}"), (*key).into()));
            env.push((format!("GIT_CONFIG_VALUE_{i}"), (*value).into()));
        }

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

    /// Verify that a worker whose local repo is behind the remote can
    /// still push successfully via the pull-rebase-push loop.  This
    /// exercises the same rebase path that resolves real concurrent
    /// push race conditions ("remote rejected: cannot lock ref").
    #[tokio::test]
    async fn test_commit_and_push_rebases_over_concurrent_changes() {
        let dir = tempfile::tempdir().unwrap();
        let bare_path = setup_bare_repo(dir.path()).await;
        let bare_url = format!("file://{}", bare_path.display());

        // Clone two working copies simulating two concurrent workers
        let repo_a = dir.path().join("worker-a");
        let repo_b = dir.path().join("worker-b");
        clone_with_config(&bare_path, &repo_a).await;
        clone_with_config(&bare_path, &repo_b).await;

        // Worker B pushes a commit first (simulating a concurrent push)
        let file_b = repo_b.join("from-b.txt");
        tokio::fs::write(&file_b, "from worker B").await.unwrap();
        let config_b = CorpusConfig::new(&bare_url, &repo_b);
        let client_b = CorpusClient::new(config_b);
        client_b
            .commit_and_push(&[file_b], "worker B commit")
            .await
            .unwrap();

        // Worker A now commits — its local repo is behind by one commit.
        // The pull --rebase inside commit_and_push must incorporate B's
        // changes before pushing.
        let file_a = repo_a.join("from-a.txt");
        tokio::fs::write(&file_a, "from worker A").await.unwrap();
        let config_a = CorpusConfig::new(&bare_url, &repo_a);
        let client_a = CorpusClient::new(config_a);
        client_a
            .commit_and_push(&[file_a], "worker A commit")
            .await
            .unwrap();

        // Verify both commits are on the remote
        let log = Command::new("git")
            .args(["log", "--oneline"])
            .current_dir(&bare_path)
            .output()
            .await
            .unwrap();
        let log_str = String::from_utf8_lossy(&log.stdout);
        assert!(
            log_str.contains("worker A commit"),
            "worker A commit not found in log: {log_str}"
        );
        assert!(
            log_str.contains("worker B commit"),
            "worker B commit not found in log: {log_str}"
        );
    }

    #[tokio::test]
    async fn test_commit_local_without_push() {
        let dir = tempfile::tempdir().unwrap();
        let bare_path = setup_bare_repo(dir.path()).await;
        let bare_url = format!("file://{}", bare_path.display());
        let repo_path = dir.path().join("corpus");
        clone_with_config(&bare_path, &repo_path).await;

        // Create a local branch
        let config = CorpusConfig::new(&bare_url, &repo_path);
        let client = CorpusClient::new(config);
        client
            .create_local_branch("editor/test-session")
            .await
            .unwrap();

        // Write and commit locally
        let test_file = repo_path.join("local-edit.txt");
        tokio::fs::write(&test_file, "local change").await.unwrap();
        client
            .commit_local(&[test_file], "local edit")
            .await
            .unwrap();

        // Verify commit exists locally
        let log = Command::new("git")
            .args(["log", "--oneline", "-1"])
            .current_dir(&repo_path)
            .output()
            .await
            .unwrap();
        let log_str = String::from_utf8_lossy(&log.stdout);
        assert!(log_str.contains("local edit"));

        // Verify it was NOT pushed to the bare repo
        let remote_log = Command::new("git")
            .args(["log", "--oneline", "--all"])
            .current_dir(&bare_path)
            .output()
            .await
            .unwrap();
        let remote_str = String::from_utf8_lossy(&remote_log.stdout);
        assert!(
            !remote_str.contains("local edit"),
            "commit should not be on remote: {remote_str}"
        );

        // Verify we're on the session branch
        let branch = Command::new("git")
            .args(["branch", "--show-current"])
            .current_dir(&repo_path)
            .output()
            .await
            .unwrap();
        let branch_str = String::from_utf8_lossy(&branch.stdout);
        assert_eq!(branch_str.trim(), "editor/test-session");
    }

    /// Create a bare repo with files in multiple directories for sparse checkout testing.
    async fn setup_bare_repo_with_files(dir: &Path) -> PathBuf {
        let bare_path = dir.join("bare.git");
        Command::new("git")
            .args(["init", "--bare", "--initial-branch=development"])
            .arg(&bare_path)
            .output()
            .await
            .unwrap();

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
        ] {
            Command::new("git")
                .args(&args)
                .current_dir(&tmp_clone)
                .output()
                .await
                .unwrap();
        }

        // Create files in multiple directories
        let law_a = tmp_clone.join("regulation/nl/wet/law_a");
        let law_b = tmp_clone.join("regulation/nl/wet/law_b");
        let features = tmp_clone.join("features");
        tokio::fs::create_dir_all(&law_a).await.unwrap();
        tokio::fs::create_dir_all(&law_b).await.unwrap();
        tokio::fs::create_dir_all(&features).await.unwrap();

        tokio::fs::write(law_a.join("2025-01-01.yaml"), "law_a content")
            .await
            .unwrap();
        tokio::fs::write(law_b.join("2025-01-01.yaml"), "law_b content")
            .await
            .unwrap();
        tokio::fs::write(features.join("law_a.feature"), "feature content")
            .await
            .unwrap();

        for args in [
            vec!["add", "."],
            vec!["commit", "-m", "add test files"],
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

    #[tokio::test]
    async fn test_sparse_checkout_only_materializes_requested_paths() {
        let dir = tempfile::tempdir().unwrap();
        let bare_path = setup_bare_repo_with_files(dir.path()).await;
        let bare_url = format!("file://{}", bare_path.display());
        let repo_path = dir.path().join("sparse-corpus");

        let mut config = CorpusConfig::new(&bare_url, &repo_path);
        config.sparse_paths = Some(vec![
            "regulation/nl/wet/law_a".to_string(),
            "features".to_string(),
        ]);

        let mut client = CorpusClient::new(config);
        client.ensure_repo().await.unwrap();

        // law_a should be present
        assert!(repo_path
            .join("regulation/nl/wet/law_a/2025-01-01.yaml")
            .exists());
        // features should be present
        assert!(repo_path.join("features/law_a.feature").exists());
        // law_b should NOT be present (excluded by sparse checkout)
        assert!(!repo_path
            .join("regulation/nl/wet/law_b/2025-01-01.yaml")
            .exists());
    }

    #[tokio::test]
    async fn test_checkout_from_branch_incorporates_new_files() {
        let dir = tempfile::tempdir().unwrap();
        let bare_path = setup_bare_repo(dir.path()).await;
        let bare_url = format!("file://{}", bare_path.display());

        // ensure_repo with a non-existent branch creates it from development
        // (mirrors production: enrichment branches are born from development)
        let repo_path = dir.path().join("enrich-clone");
        let mut config = CorpusConfig::new(&bare_url, &repo_path);
        config.branch = "enrich/test".into();
        let mut client = CorpusClient::new(config);
        client.ensure_repo().await.unwrap();

        // Push a new file to development (simulating a harvested law)
        let tmp = dir.path().join("setup");
        clone_with_config(&bare_path, &tmp).await;
        let new_law = tmp.join("regulation/nl/wet/new_law");
        tokio::fs::create_dir_all(&new_law).await.unwrap();
        tokio::fs::write(new_law.join("2025-01-01.yaml"), "new law content")
            .await
            .unwrap();
        for args in [
            vec!["add", "."],
            vec!["commit", "-m", "harvest new law"],
            vec!["push", "origin", "development"],
        ] {
            Command::new("git")
                .args(&args)
                .current_dir(&tmp)
                .output()
                .await
                .unwrap();
        }

        // The new law should NOT be present on the enrichment branch yet
        assert!(!repo_path
            .join("regulation/nl/wet/new_law/2025-01-01.yaml")
            .exists());

        // Checkout the specific file from development (matches production code path)
        client
            .checkout_from_branch(
                "development",
                &["regulation/nl/wet/new_law/2025-01-01.yaml"],
            )
            .await
            .unwrap();
        assert!(repo_path
            .join("regulation/nl/wet/new_law/2025-01-01.yaml")
            .exists());

        // Unstaged so the file appears as `??` (untracked) rather than `A`
        // (staged-new). commit_and_push uses explicit `git add -- <paths>`
        // which picks up untracked files, so the machine_readable additions
        // and new file land in the same commit.
        let status = Command::new("git")
            .args(["status", "--porcelain", "--untracked-files=all"])
            .current_dir(&repo_path)
            .output()
            .await
            .unwrap();
        let status_str = String::from_utf8_lossy(&status.stdout);
        assert!(
            !status_str.contains("A "),
            "file should not be staged: {status_str}"
        );
        assert!(
            status_str.contains("?? regulation/nl/wet/new_law/2025-01-01.yaml"),
            "file should be untracked: {status_str}"
        );
    }

    #[tokio::test]
    async fn test_checkout_from_branch_skips_already_tracked_files() {
        let dir = tempfile::tempdir().unwrap();
        let bare_path = setup_bare_repo(dir.path()).await;
        let bare_url = format!("file://{}", bare_path.display());

        // Push a law file to development
        let tmp = dir.path().join("setup");
        clone_with_config(&bare_path, &tmp).await;
        let law_dir = tmp.join("regulation/nl/wet/enriched_law");
        tokio::fs::create_dir_all(&law_dir).await.unwrap();
        tokio::fs::write(law_dir.join("2025-01-01.yaml"), "raw content")
            .await
            .unwrap();
        for args in [
            vec!["add", "."],
            vec!["commit", "-m", "harvest law"],
            vec!["push", "origin", "development"],
        ] {
            Command::new("git")
                .args(&args)
                .current_dir(&tmp)
                .output()
                .await
                .unwrap();
        }

        // Create enrichment branch (inherits the law from development)
        let repo_path = dir.path().join("enrich-clone");
        let mut config = CorpusConfig::new(&bare_url, &repo_path);
        config.branch = "enrich/test".into();
        let mut client = CorpusClient::new(config);
        client.ensure_repo().await.unwrap();

        // Simulate enrichment: modify the file with machine_readable content
        let yaml_path = repo_path.join("regulation/nl/wet/enriched_law/2025-01-01.yaml");
        tokio::fs::write(&yaml_path, "enriched content with machine_readable")
            .await
            .unwrap();
        client
            .commit_and_push(&[yaml_path.clone()], "enrich: add machine_readable")
            .await
            .unwrap();

        // Now call checkout_from_branch — it should SKIP the file because
        // it's already tracked on the enrichment branch
        client
            .checkout_from_branch(
                "development",
                &["regulation/nl/wet/enriched_law/2025-01-01.yaml"],
            )
            .await
            .unwrap();

        // File content should still be the enriched version, NOT overwritten
        let content = tokio::fs::read_to_string(&yaml_path).await.unwrap();
        assert_eq!(
            content, "enriched content with machine_readable",
            "already-tracked file should not be overwritten by development version"
        );
    }

    #[tokio::test]
    async fn test_checkout_from_branch_fetches_new_version_when_old_exists() {
        let dir = tempfile::tempdir().unwrap();
        let bare_path = setup_bare_repo(dir.path()).await;
        let bare_url = format!("file://{}", bare_path.display());

        // Push an old version of a law to development
        let tmp = dir.path().join("setup");
        clone_with_config(&bare_path, &tmp).await;
        let law_dir = tmp.join("regulation/nl/wet/some_law");
        tokio::fs::create_dir_all(&law_dir).await.unwrap();
        tokio::fs::write(law_dir.join("2024-01-01.yaml"), "old version")
            .await
            .unwrap();
        for args in [
            vec!["add", "."],
            vec!["commit", "-m", "harvest old version"],
            vec!["push", "origin", "development"],
        ] {
            Command::new("git")
                .args(&args)
                .current_dir(&tmp)
                .output()
                .await
                .unwrap();
        }

        // Create enrichment branch (inherits old version from development)
        let repo_path = dir.path().join("enrich-clone");
        let mut config = CorpusConfig::new(&bare_url, &repo_path);
        config.branch = "enrich/test".into();
        let mut client = CorpusClient::new(config);
        client.ensure_repo().await.unwrap();
        assert!(repo_path
            .join("regulation/nl/wet/some_law/2024-01-01.yaml")
            .exists());

        // Push a NEW version to development
        tokio::fs::write(law_dir.join("2025-01-01.yaml"), "new version")
            .await
            .unwrap();
        for args in [
            vec!["add", "."],
            vec!["commit", "-m", "harvest new version"],
            vec!["push", "origin", "development"],
        ] {
            Command::new("git")
                .args(&args)
                .current_dir(&tmp)
                .output()
                .await
                .unwrap();
        }

        // New version should NOT be on the enrichment branch yet
        assert!(!repo_path
            .join("regulation/nl/wet/some_law/2025-01-01.yaml")
            .exists());

        // Checkout the specific new file (not the directory!)
        client
            .checkout_from_branch(
                "development",
                &["regulation/nl/wet/some_law/2025-01-01.yaml"],
            )
            .await
            .unwrap();

        // New version present, old version still intact
        assert!(repo_path
            .join("regulation/nl/wet/some_law/2025-01-01.yaml")
            .exists());
        assert!(repo_path
            .join("regulation/nl/wet/some_law/2024-01-01.yaml")
            .exists());
    }

    #[tokio::test]
    async fn test_checkout_from_branch_works_with_preexisting_enrichment_branch() {
        // Regression: when the enrichment branch already exists on the remote,
        // ensure_repo clones it with `--single-branch --branch enrich/<provider>`.
        // The configured fetch refspec then only matches enrich/<provider>, so
        // `git fetch origin development` does NOT update refs/remotes/origin/development.
        // checkout_from_branch must still be able to reach the fetched commit —
        // which it does via FETCH_HEAD.
        let dir = tempfile::tempdir().unwrap();
        let bare_path = setup_bare_repo(dir.path()).await;
        let bare_url = format!("file://{}", bare_path.display());

        // Pre-create enrich/test on the bare, branching from development.
        let tmp = dir.path().join("setup");
        clone_with_config(&bare_path, &tmp).await;
        for args in [
            vec!["checkout", "-b", "enrich/test"],
            vec!["push", "-u", "origin", "enrich/test"],
            vec!["checkout", "development"],
        ] {
            Command::new("git")
                .args(&args)
                .current_dir(&tmp)
                .output()
                .await
                .unwrap();
        }

        // Now push a new law to development (not yet on enrich/test).
        let law_dir = tmp.join("regulation/nl/wet/new_law");
        tokio::fs::create_dir_all(&law_dir).await.unwrap();
        tokio::fs::write(law_dir.join("2025-01-01.yaml"), "new law content")
            .await
            .unwrap();
        for args in [
            vec!["add", "."],
            vec!["commit", "-m", "harvest new law"],
            vec!["push", "origin", "development"],
        ] {
            Command::new("git")
                .args(&args)
                .current_dir(&tmp)
                .output()
                .await
                .unwrap();
        }

        // Clone enrich/test — this goes through git_clone (not the fallback),
        // so the fetch refspec is +refs/heads/enrich/test:refs/remotes/origin/enrich/test only.
        let repo_path = dir.path().join("enrich-clone");
        let mut config = CorpusConfig::new(&bare_url, &repo_path);
        config.branch = "enrich/test".into();
        let mut client = CorpusClient::new(config);
        client.ensure_repo().await.unwrap();

        // Sanity-check: origin/development must NOT exist as a remote-tracking ref.
        let refs = Command::new("git")
            .args([
                "for-each-ref",
                "--format=%(refname)",
                "refs/remotes/origin/",
            ])
            .current_dir(&repo_path)
            .output()
            .await
            .unwrap();
        let refs_str = String::from_utf8_lossy(&refs.stdout);
        assert!(
            !refs_str.contains("refs/remotes/origin/development"),
            "expected no origin/development tracking ref before checkout_from_branch, got: {refs_str}"
        );

        // This would fail with `invalid reference: origin/development` before the FETCH_HEAD fix.
        client
            .checkout_from_branch(
                "development",
                &["regulation/nl/wet/new_law/2025-01-01.yaml"],
            )
            .await
            .unwrap();

        assert!(repo_path
            .join("regulation/nl/wet/new_law/2025-01-01.yaml")
            .exists());
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
