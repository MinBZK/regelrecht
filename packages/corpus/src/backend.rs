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

    fn resolve(&self, relative: &Path) -> PathBuf {
        self.root.join(relative)
    }
}

#[async_trait]
impl RepoBackend for LocalBackend {
    async fn read_file(&self, relative_path: &Path) -> Result<Option<String>> {
        let abs = self.resolve(relative_path);
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
        let abs = self.resolve(relative_path);
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
        let abs = self.resolve(relative_path);
        match tokio::fs::remove_file(&abs).await {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }

    async fn list_files(&self, dir: &Path, extension: Option<&str>) -> Result<Vec<FileEntry>> {
        let abs = self.resolve(dir);
        let mut entries = Vec::new();

        let mut read_dir = match tokio::fs::read_dir(&abs).await {
            Ok(rd) => rd,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(entries),
            Err(e) => return Err(e.into()),
        };

        while let Some(entry) = read_dir.next_entry().await? {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
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
                    let _ = tokio::fs::remove_file(&probe).await;
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
pub struct GitBackend {
    client: CorpusClient,
    /// Sub-path within the repo that corresponds to the source root
    /// (e.g. "regulation/nl").
    repo_subpath: Option<String>,
    /// Files written since the last persist, as absolute paths.
    dirty_files: tokio::sync::Mutex<Vec<PathBuf>>,
}

impl GitBackend {
    pub fn new(client: CorpusClient, repo_subpath: Option<String>) -> Self {
        Self {
            client,
            repo_subpath,
            dirty_files: tokio::sync::Mutex::new(Vec::new()),
        }
    }

    /// Resolve a source-relative path to an absolute path in the checkout.
    fn resolve(&self, relative: &Path) -> PathBuf {
        let base = match &self.repo_subpath {
            Some(sub) => self.client.repo_path().join(sub),
            None => self.client.repo_path().to_path_buf(),
        };
        base.join(relative)
    }
}

#[async_trait]
impl RepoBackend for GitBackend {
    async fn read_file(&self, relative_path: &Path) -> Result<Option<String>> {
        let abs = self.resolve(relative_path);
        match tokio::fs::read_to_string(&abs).await {
            Ok(content) => Ok(Some(content)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    async fn write_file(&self, relative_path: &Path, content: &str) -> Result<()> {
        let abs = self.resolve(relative_path);
        if let Some(parent) = abs.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::write(&abs, content).await?;

        self.dirty_files.lock().await.push(abs);
        Ok(())
    }

    async fn delete_file(&self, relative_path: &Path) -> Result<()> {
        let abs = self.resolve(relative_path);
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
        let abs = self.resolve(dir);
        let mut entries = Vec::new();

        let mut read_dir = match tokio::fs::read_dir(&abs).await {
            Ok(rd) => rd,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(entries),
            Err(e) => return Err(e.into()),
        };

        while let Some(entry) = read_dir.next_entry().await? {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
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

        self.client.commit_and_push(&paths, &ctx.message).await
    }

    async fn ensure_ready(&mut self) -> Result<()> {
        self.client.ensure_repo().await
    }

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
pub fn create_backend(source: &Source, auth_token: Option<&str>) -> Result<Box<dyn RepoBackend>> {
    match &source.source_type {
        SourceType::Local { local } => Ok(Box::new(LocalBackend::new(local.path.clone(), true))),
        SourceType::GitHub { github } => {
            let repo_url = format!("https://github.com/{}/{}.git", github.owner, github.repo);
            let repo_path = PathBuf::from(format!("/tmp/corpus-editor/{}", source.id));
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
    async fn local_ensure_ready_fails_for_missing_dir() {
        let mut backend = LocalBackend::new(PathBuf::from("/nonexistent/path"), true);
        let result = backend.ensure_ready().await;
        assert!(result.is_err());
    }
}
