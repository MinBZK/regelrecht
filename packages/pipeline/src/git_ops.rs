use std::path::{Path, PathBuf};

use tokio::process::Command;

use crate::error::{PipelineError, Result};

/// Wrapper around git CLI operations for managing a regulation repository.
pub struct GitRepo {
    path: PathBuf,
}

impl GitRepo {
    /// Clone a repository or open an existing one at the given path.
    ///
    /// If `.git` already exists at `path`, reuses the existing checkout.
    /// Otherwise, clones `url` into `path`.
    pub async fn clone_or_open(url: &str, path: &Path) -> Result<Self> {
        let git_dir = path.join(".git");
        if git_dir.exists() {
            tracing::info!(path = %path.display(), "opening existing git repo");
        } else {
            tracing::info!(url, path = %path.display(), "cloning git repo");
            run_git(Path::new("."), &["clone", url, &path.to_string_lossy()]).await?;
        }
        Ok(Self {
            path: path.to_path_buf(),
        })
    }

    /// Pull latest changes with fast-forward only.
    pub async fn pull(&self) -> Result<()> {
        run_git(&self.path, &["pull", "--ff-only"]).await?;
        Ok(())
    }

    /// Stage files for commit.
    pub async fn add(&self, paths: &[&Path]) -> Result<()> {
        let mut args = vec!["add"];
        let path_strs: Vec<String> = paths.iter().map(|p| p.to_string_lossy().into_owned()).collect();
        for p in &path_strs {
            args.push(p);
        }
        run_git(&self.path, &args).await?;
        Ok(())
    }

    /// Commit staged changes. Returns `false` if there was nothing to commit.
    pub async fn commit(&self, message: &str) -> Result<bool> {
        let has_staged = has_staged_changes(&self.path).await?;
        if !has_staged {
            tracing::debug!("nothing staged to commit");
            return Ok(false);
        }
        run_git(&self.path, &["commit", "-m", message]).await?;
        Ok(true)
    }

    /// Push to remote. No-op if `enabled` is false.
    pub async fn push(&self, enabled: bool) -> Result<()> {
        if !enabled {
            tracing::info!("git push disabled by configuration");
            return Ok(());
        }
        run_git(&self.path, &["push"]).await?;
        Ok(())
    }

    /// Return the path to the repository.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// Check if there are staged changes in the index.
/// Returns `true` if there are changes staged for commit.
async fn has_staged_changes(repo_path: &Path) -> Result<bool> {
    let output = Command::new("git")
        .args(["diff", "--cached", "--quiet"])
        .current_dir(repo_path)
        .output()
        .await
        .map_err(|e| PipelineError::Git {
            message: format!("failed to execute git: {e}"),
            stderr: String::new(),
        })?;

    Ok(!output.status.success())
}

/// Execute a git command in the given directory and return stdout.
async fn run_git(repo_path: &Path, args: &[&str]) -> Result<String> {
    tracing::debug!(cwd = %repo_path.display(), args = ?args, "running git command");

    let output = Command::new("git")
        .args(args)
        .current_dir(repo_path)
        .output()
        .await
        .map_err(|e| PipelineError::Git {
            message: format!("failed to execute git: {e}"),
            stderr: String::new(),
        })?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if !output.status.success() {
        return Err(PipelineError::Git {
            message: format!("git {} failed with exit code {:?}", args.join(" "), output.status.code()),
            stderr,
        });
    }

    if !stderr.is_empty() {
        tracing::debug!(stderr = %stderr, "git stderr (non-fatal)");
    }

    Ok(stdout)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    async fn init_bare_remote(path: &Path) {
        run_git(Path::new("."), &["init", "--bare", &path.to_string_lossy()])
            .await
            .unwrap();
    }

    async fn clone_remote(remote: &Path, local: &Path) -> GitRepo {
        GitRepo::clone_or_open(&remote.to_string_lossy(), local)
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn test_clone_and_reopen() {
        let tmp = tempdir().unwrap();
        let remote_path = tmp.path().join("remote.git");
        let clone_path = tmp.path().join("clone");

        init_bare_remote(&remote_path).await;

        let repo = clone_remote(&remote_path, &clone_path).await;
        assert!(clone_path.join(".git").exists());
        assert_eq!(repo.path(), clone_path);

        let repo2 = GitRepo::clone_or_open(&remote_path.to_string_lossy(), &clone_path)
            .await
            .unwrap();
        assert_eq!(repo2.path(), clone_path);
    }

    #[tokio::test]
    async fn test_add_commit_push() {
        let tmp = tempdir().unwrap();
        let remote_path = tmp.path().join("remote.git");
        let clone_path = tmp.path().join("clone");

        init_bare_remote(&remote_path).await;
        let repo = clone_remote(&remote_path, &clone_path).await;

        run_git(&clone_path, &["config", "user.email", "test@test.com"])
            .await
            .unwrap();
        run_git(&clone_path, &["config", "user.name", "Test"])
            .await
            .unwrap();

        let test_file = clone_path.join("test.txt");
        tokio::fs::write(&test_file, "hello").await.unwrap();

        repo.add(&[Path::new("test.txt")]).await.unwrap();
        let committed = repo.commit("test commit").await.unwrap();
        assert!(committed);

        repo.push(true).await.unwrap();
    }

    #[tokio::test]
    async fn test_commit_nothing_returns_false() {
        let tmp = tempdir().unwrap();
        let remote_path = tmp.path().join("remote.git");
        let clone_path = tmp.path().join("clone");

        init_bare_remote(&remote_path).await;
        let repo = clone_remote(&remote_path, &clone_path).await;

        run_git(&clone_path, &["config", "user.email", "test@test.com"])
            .await
            .unwrap();
        run_git(&clone_path, &["config", "user.name", "Test"])
            .await
            .unwrap();

        let committed = repo.commit("empty commit").await.unwrap();
        assert!(!committed);
    }

    #[tokio::test]
    async fn test_push_disabled_is_noop() {
        let tmp = tempdir().unwrap();
        let remote_path = tmp.path().join("remote.git");
        let clone_path = tmp.path().join("clone");

        init_bare_remote(&remote_path).await;
        let repo = clone_remote(&remote_path, &clone_path).await;

        repo.push(false).await.unwrap();
    }

    #[tokio::test]
    async fn test_pull() {
        let tmp = tempdir().unwrap();
        let remote_path = tmp.path().join("remote.git");
        let clone1_path = tmp.path().join("clone1");
        let clone2_path = tmp.path().join("clone2");

        init_bare_remote(&remote_path).await;
        let repo1 = clone_remote(&remote_path, &clone1_path).await;
        let repo2 = clone_remote(&remote_path, &clone2_path).await;

        run_git(&clone1_path, &["config", "user.email", "test@test.com"])
            .await
            .unwrap();
        run_git(&clone1_path, &["config", "user.name", "Test"])
            .await
            .unwrap();

        tokio::fs::write(clone1_path.join("file.txt"), "content")
            .await
            .unwrap();
        repo1.add(&[Path::new("file.txt")]).await.unwrap();
        repo1.commit("add file").await.unwrap();
        repo1.push(true).await.unwrap();

        repo2.pull().await.unwrap();
        assert!(clone2_path.join("file.txt").exists());
    }
}
