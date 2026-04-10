use std::path::{Path, PathBuf};

use async_trait::async_trait;

use crate::client::CorpusClient;
use crate::config::CorpusConfig;
use crate::error::{CorpusError, Result};
use crate::models::{Source, SourceType};

/// Metadata for a write operation (used as commit message for git backends).
pub struct WriteContext {
    pub message: String,
}

/// A file entry returned by list operations.
pub struct FileEntry {
    /// Filename only (not a full path), e.g. "eligibility.feature".
    pub name: String,
}

/// Abstraction over different corpus storage backends.
///
/// All paths are relative to the source root directory. The backend resolves
/// them to absolute paths internally.
#[async_trait]
pub trait RepoBackend: Send + Sync {
    /// Read a file's contents. Returns `None` if the file does not exist.
    async fn read_file(&self, relative_path: &Path) -> Result<Option<String>>;

    /// Write a file's contents, creating parent directories as needed.
    ///
    /// For git backends this writes to the local checkout without committing.
    /// Call [`persist`] afterwards to commit and push.
    async fn write_file(&self, relative_path: &Path, content: &str) -> Result<()>;

    /// Delete a file. Returns `Ok(())` even if the file did not exist.
    async fn delete_file(&self, relative_path: &Path) -> Result<()>;

    /// List files in a directory, optionally filtered by extension (without dot).
    async fn list_files(&self, dir: &Path, extension: Option<&str>) -> Result<Vec<FileEntry>>;

    /// Persist pending changes.
    ///
    /// No-op for local backends. For git backends this commits dirty files and
    /// pushes to the remote.
    async fn persist(&self, ctx: &WriteContext) -> Result<()>;

    /// Prepare the backend for use (validate directories, clone repos, etc.).
    async fn ensure_ready(&mut self) -> Result<()>;

    /// Whether this backend supports write operations.
    fn is_writable(&self) -> bool;
}

// ---------------------------------------------------------------------------
// LocalBackend
// ---------------------------------------------------------------------------

/// Backend that reads/writes directly to the local filesystem.
pub struct LocalBackend {
    root: PathBuf,
    writable: bool,
}

impl LocalBackend {
    pub fn new(root: PathBuf, writable: bool) -> Self {
        Self { root, writable }
    }

    fn resolve(&self, relative: &Path) -> Result<PathBuf> {
        validate_relative_path(relative)?;
        Ok(self.root.join(relative))
    }
}

/// Reject paths that are absolute or contain `..` components.
fn validate_relative_path(path: &Path) -> Result<()> {
    if path.is_absolute() {
        return Err(CorpusError::Config(format!(
            "path must be relative: {}",
            path.display()
        )));
    }
    for component in path.components() {
        if matches!(component, std::path::Component::ParentDir) {
            return Err(CorpusError::Config(format!(
                "path must not contain '..': {}",
                path.display()
            )));
        }
    }
    Ok(())
}

#[async_trait]
impl RepoBackend for LocalBackend {
    async fn read_file(&self, relative_path: &Path) -> Result<Option<String>> {
        let abs = self.resolve(relative_path)?;
        match tokio::fs::read_to_string(&abs).await {
            Ok(content) => Ok(Some(content)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    async fn write_file(&self, relative_path: &Path, content: &str) -> Result<()> {
        if !self.writable {
            return Err(CorpusError::ReadOnly(
                "local source is read-only".to_string(),
            ));
        }
        let abs = self.resolve(relative_path)?;
        if let Some(parent) = abs.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::write(&abs, content).await?;
        Ok(())
    }

    async fn delete_file(&self, relative_path: &Path) -> Result<()> {
        if !self.writable {
            return Err(CorpusError::ReadOnly(
                "local source is read-only".to_string(),
            ));
        }
        let abs = self.resolve(relative_path)?;
        match tokio::fs::remove_file(&abs).await {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }

    async fn list_files(&self, dir: &Path, extension: Option<&str>) -> Result<Vec<FileEntry>> {
        let abs = self.resolve(dir)?;
        let mut entries = Vec::new();

        let mut read_dir = match tokio::fs::read_dir(&abs).await {
            Ok(rd) => rd,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(entries),
            Err(e) => return Err(e.into()),
        };

        while let Some(entry) = read_dir.next_entry().await? {
            let ft = entry.file_type().await?;
            if !ft.is_file() {
                continue;
            }
            let path = entry.path();
            if let Some(ext) = extension {
                if path.extension().is_none_or(|e| e != ext) {
                    continue;
                }
            }
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                entries.push(FileEntry {
                    name: name.to_string(),
                });
            }
        }

        entries.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(entries)
    }

    async fn persist(&self, _ctx: &WriteContext) -> Result<()> {
        // Local writes are immediate — nothing to persist.
        Ok(())
    }

    async fn ensure_ready(&mut self) -> Result<()> {
        if !self.root.exists() {
            return Err(CorpusError::Config(format!(
                "local source root does not exist: {}",
                self.root.display()
            )));
        }

        // Probe write access: try creating and removing a temporary file.
        // If the filesystem is read-only (e.g. inside a container), downgrade
        // to read-only mode rather than failing at save time.
        if self.writable {
            let probe = self.root.join(".write-probe");
            match tokio::fs::write(&probe, b"").await {
                Ok(()) => {
                    if let Err(e) = tokio::fs::remove_file(&probe).await {
                        // The write succeeded but cleanup failed — leave a
                        // visible warning rather than silently leaving a
                        // .write-probe file behind in the source root.
                        tracing::warn!(
                            path = %probe.display(),
                            error = %e,
                            "failed to remove write probe file after success"
                        );
                    }
                }
                Err(_) => {
                    tracing::info!(
                        path = %self.root.display(),
                        "local source is not writable, disabling writes"
                    );
                    self.writable = false;
                }
            }
        }

        Ok(())
    }

    fn is_writable(&self) -> bool {
        self.writable
    }
}

// ---------------------------------------------------------------------------
// GitBackend
// ---------------------------------------------------------------------------

/// Backend that reads/writes to a local git checkout and commits/pushes on persist.
///
/// When no push token is configured (`local_only` mode), edits are committed
/// to a local session branch without pushing. This is useful for playground
/// environments where users want to experiment without affecting the remote.
pub struct GitBackend {
    client: CorpusClient,
    /// Sub-path within the repo that corresponds to the source root
    /// (e.g. "regulation/nl").
    repo_subpath: Option<String>,
    /// Files written since the last persist, as absolute paths.
    dirty_files: tokio::sync::Mutex<Vec<PathBuf>>,
    /// When true, commits stay local (no push). Set when no push token is available.
    local_only: bool,
    /// Name of the local session branch (created on first persist).
    session_branch: Option<String>,
    /// Whether the session branch has been created yet.
    branched: tokio::sync::Mutex<bool>,
}

impl GitBackend {
    pub fn new(client: CorpusClient, repo_subpath: Option<String>) -> Self {
        let local_only = !client.has_push_token();
        let session_branch = if local_only {
            let ts = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            Some(format!("editor/session-{ts}"))
        } else {
            None
        };
        Self {
            client,
            repo_subpath,
            dirty_files: tokio::sync::Mutex::new(Vec::new()),
            local_only,
            session_branch,
            branched: tokio::sync::Mutex::new(false),
        }
    }

    /// Resolve a source-relative path to an absolute path in the checkout.
    fn resolve(&self, relative: &Path) -> Result<PathBuf> {
        validate_relative_path(relative)?;
        let base = match &self.repo_subpath {
            Some(sub) => self.client.repo_path().join(sub),
            None => self.client.repo_path().to_path_buf(),
        };
        Ok(base.join(relative))
    }
}

#[async_trait]
impl RepoBackend for GitBackend {
    async fn read_file(&self, relative_path: &Path) -> Result<Option<String>> {
        let abs = self.resolve(relative_path)?;
        match tokio::fs::read_to_string(&abs).await {
            Ok(content) => Ok(Some(content)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    async fn write_file(&self, relative_path: &Path, content: &str) -> Result<()> {
        let abs = self.resolve(relative_path)?;
        if let Some(parent) = abs.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::write(&abs, content).await?;

        self.dirty_files.lock().await.push(abs);
        Ok(())
    }

    async fn delete_file(&self, relative_path: &Path) -> Result<()> {
        let abs = self.resolve(relative_path)?;
        match tokio::fs::remove_file(&abs).await {
            Ok(()) => {
                self.dirty_files.lock().await.push(abs);
                Ok(())
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }

    async fn list_files(&self, dir: &Path, extension: Option<&str>) -> Result<Vec<FileEntry>> {
        let abs = self.resolve(dir)?;
        let mut entries = Vec::new();

        let mut read_dir = match tokio::fs::read_dir(&abs).await {
            Ok(rd) => rd,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(entries),
            Err(e) => return Err(e.into()),
        };

        while let Some(entry) = read_dir.next_entry().await? {
            let ft = entry.file_type().await?;
            if !ft.is_file() {
                continue;
            }
            let path = entry.path();
            if let Some(ext) = extension {
                if path.extension().is_none_or(|e| e != ext) {
                    continue;
                }
            }
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                entries.push(FileEntry {
                    name: name.to_string(),
                });
            }
        }

        entries.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(entries)
    }

    async fn persist(&self, ctx: &WriteContext) -> Result<()> {
        let paths: Vec<PathBuf> = {
            let mut dirty = self.dirty_files.lock().await;
            std::mem::take(&mut *dirty)
        };

        if paths.is_empty() {
            return Ok(());
        }

        let result = if self.local_only {
            // Create session branch on first persist
            let branch_ok = {
                let mut branched = self.branched.lock().await;
                if !*branched {
                    let res = if let Some(branch) = &self.session_branch {
                        self.client.create_local_branch(branch).await
                    } else {
                        Ok(())
                    };
                    if res.is_ok() {
                        *branched = true;
                    }
                    res
                } else {
                    Ok(())
                }
            };
            match branch_ok {
                Ok(()) => self.client.commit_local(&paths, &ctx.message).await,
                Err(e) => Err(e),
            }
        } else {
            self.client.commit_and_push(&paths, &ctx.message).await
        };

        if let Err(e) = result {
            // Restore dirty files so the next persist attempt can retry.
            self.dirty_files.lock().await.extend(paths);
            return Err(e);
        }

        Ok(())
    }

    async fn ensure_ready(&mut self) -> Result<()> {
        self.client.ensure_repo().await
    }

    /// A `GitBackend` is always considered writable: even in `local_only`
    /// mode (no push token) edits are committed to a local session branch.
    /// "Writable" here means "the backend will accept `write_file` calls",
    /// not "the backend will push to a remote".
    fn is_writable(&self) -> bool {
        true
    }
}

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

/// Create a [`RepoBackend`] for a given corpus source.
///
/// For GitHub sources, an optional authentication token can be provided.
///
/// The on-disk checkout path is namespaced by the host identifier
/// (`HOSTNAME` env var, falling back to `"local"`) so that multiple replicas
/// of the editor running on the same node — or a pod restart racing with a
/// previous instance — do not share a working directory and corrupt each
/// other's git state during concurrent `clone`/`pull --rebase`/`push`.
pub fn create_backend(source: &Source, auth_token: Option<&str>) -> Result<Box<dyn RepoBackend>> {
    match &source.source_type {
        SourceType::Local { local } => Ok(Box::new(LocalBackend::new(local.path.clone(), true))),
        SourceType::GitHub { github } => {
            let repo_url = format!("https://github.com/{}/{}.git", github.owner, github.repo);
            let host_id = std::env::var("HOSTNAME").unwrap_or_else(|_| "local".to_string());
            let repo_path = std::env::temp_dir()
                .join("corpus-editor")
                .join(&host_id)
                .join(&source.id);
            let mut config = CorpusConfig::new(&repo_url, &repo_path);
            config.branch = github.effective_ref().to_string();

            if let Some(token) = auth_token {
                config = config.with_token(token);
            }

            let client = CorpusClient::new(config);
            Ok(Box::new(GitBackend::new(client, github.path.clone())))
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn local_read_write_roundtrip() {
        let dir = TempDir::new().unwrap();
        let mut backend = LocalBackend::new(dir.path().to_path_buf(), true);
        backend.ensure_ready().await.unwrap();

        let path = Path::new("scenarios/test.feature");

        // File doesn't exist yet
        assert!(backend.read_file(path).await.unwrap().is_none());

        // Write
        backend.write_file(path, "Feature: Test\n").await.unwrap();

        // Read back
        let content = backend.read_file(path).await.unwrap().unwrap();
        assert_eq!(content, "Feature: Test\n");

        // Persist is a no-op
        backend
            .persist(&WriteContext {
                message: "test".to_string(),
            })
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn local_delete_file() {
        let dir = TempDir::new().unwrap();
        let mut backend = LocalBackend::new(dir.path().to_path_buf(), true);
        backend.ensure_ready().await.unwrap();

        let path = Path::new("test.feature");
        backend.write_file(path, "content").await.unwrap();
        assert!(backend.read_file(path).await.unwrap().is_some());

        backend.delete_file(path).await.unwrap();
        assert!(backend.read_file(path).await.unwrap().is_none());

        // Deleting non-existent file is fine
        backend.delete_file(path).await.unwrap();
    }

    #[tokio::test]
    async fn local_list_files_with_extension() {
        let dir = TempDir::new().unwrap();
        let mut backend = LocalBackend::new(dir.path().to_path_buf(), true);
        backend.ensure_ready().await.unwrap();

        backend
            .write_file(Path::new("scenarios/a.feature"), "a")
            .await
            .unwrap();
        backend
            .write_file(Path::new("scenarios/b.feature"), "b")
            .await
            .unwrap();
        backend
            .write_file(Path::new("scenarios/readme.md"), "r")
            .await
            .unwrap();

        let entries = backend
            .list_files(Path::new("scenarios"), Some("feature"))
            .await
            .unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].name, "a.feature");
        assert_eq!(entries[1].name, "b.feature");
    }

    #[tokio::test]
    async fn local_list_files_missing_dir() {
        let dir = TempDir::new().unwrap();
        let mut backend = LocalBackend::new(dir.path().to_path_buf(), true);
        backend.ensure_ready().await.unwrap();

        let entries = backend
            .list_files(Path::new("nonexistent"), None)
            .await
            .unwrap();
        assert!(entries.is_empty());
    }

    #[tokio::test]
    async fn local_read_only_rejects_writes() {
        let dir = TempDir::new().unwrap();
        let backend = LocalBackend::new(dir.path().to_path_buf(), false);

        let result = backend.write_file(Path::new("test.txt"), "content").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("read-only"));
    }

    #[tokio::test]
    async fn local_rejects_path_traversal() {
        let dir = TempDir::new().unwrap();
        let mut backend = LocalBackend::new(dir.path().to_path_buf(), true);
        backend.ensure_ready().await.unwrap();

        let result = backend.read_file(Path::new("../etc/passwd")).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains(".."));

        let result = backend.read_file(Path::new("/etc/passwd")).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("relative"));
    }

    #[tokio::test]
    async fn local_ensure_ready_fails_for_missing_dir() {
        let mut backend = LocalBackend::new(PathBuf::from("/nonexistent/path"), true);
        let result = backend.ensure_ready().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn git_local_only_commits_without_push() {
        use tokio::process::Command;

        // Set up a bare repo
        let dir = TempDir::new().unwrap();
        let bare_path = dir.path().join("bare.git");
        Command::new("git")
            .args(["init", "--bare", "--initial-branch=development"])
            .arg(&bare_path)
            .output()
            .await
            .unwrap();
        let bare_url = format!("file://{}", bare_path.display());

        // Push an initial commit
        let tmp_clone = dir.path().join("tmp-clone");
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

        // Create a GitBackend without a token (local_only mode)
        let repo_path = dir.path().join("editor-repo");
        let config = CorpusConfig::new(&bare_url, &repo_path);
        // No .with_token() — triggers local_only
        let client = CorpusClient::new(config);
        let mut backend = GitBackend::new(client, None);
        assert!(backend.local_only);

        backend.ensure_ready().await.unwrap();

        // Write and persist
        backend
            .write_file(Path::new("test.feature"), "Feature: Test\n")
            .await
            .unwrap();
        backend
            .persist(&WriteContext {
                message: "add test scenario".to_string(),
            })
            .await
            .unwrap();

        // Verify local commit exists on a session branch
        let branch = Command::new("git")
            .args(["branch", "--show-current"])
            .current_dir(&repo_path)
            .output()
            .await
            .unwrap();
        let branch_str = String::from_utf8_lossy(&branch.stdout);
        assert!(
            branch_str.trim().starts_with("editor/session-"),
            "expected session branch, got: {branch_str}"
        );

        let log = Command::new("git")
            .args(["log", "--oneline", "-1"])
            .current_dir(&repo_path)
            .output()
            .await
            .unwrap();
        let log_str = String::from_utf8_lossy(&log.stdout);
        assert!(log_str.contains("add test scenario"));

        // Verify NOT pushed
        let remote_log = Command::new("git")
            .args(["log", "--oneline", "--all"])
            .current_dir(&bare_path)
            .output()
            .await
            .unwrap();
        let remote_str = String::from_utf8_lossy(&remote_log.stdout);
        assert!(!remote_str.contains("add test scenario"));
    }
}
