//! API-only [`RepoBackend`] implementation against the GitHub REST API.
//!
//! No local git clone, no `/tmp` working tree. Reads go straight through
//! the Contents API; writes buffer in memory and flush as one Contents
//! API PUT/DELETE per file in [`persist`]. The branch is created lazily
//! in [`ensure_ready`] when it doesn't yet exist on the remote
//! ("traject branch on first activation").
//!
//! ## Atomicity
//!
//! The Contents API commits each PUT/DELETE separately. All current
//! editor flows ([`save_law`], [`save_scenario`], [`delete_scenario`],
//! [`save_annotations`]) write exactly one file per persist, so they are
//! effectively atomic. A future multi-file save would need the Git Data
//! API (blob → tree → commit → ref update) to land both files in a
//! single commit; the implementation here would surface partial-failure
//! state.
//!
//! ## Optimistic concurrency
//!
//! Each pending write carries the blob SHA that was current when the
//! caller last read the file (or `None` if it was never read — e.g. a
//! brand-new file). On the PUT, GitHub returns 409 if the SHA is stale.
//! `persist` then re-reads the SHA and retries **once**; a second 409 is
//! surfaced as [`CorpusError::Conflict`] for the caller to deal with.
//! For `save_annotations`'s append-only flow this is safe because dedup
//! happens against the freshly-read base before re-writing.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use async_trait::async_trait;
use tokio::sync::Mutex;

use crate::backend::{FileEntry, PersistOutcome, RecursiveFileEntry, RepoBackend, WriteContext};
use crate::error::{CorpusError, Result};
use crate::github::{Committer, GitHubFetcher};
use crate::models::GitHubSource;

/// Pending change buffered between `write_file`/`delete_file` and
/// `persist`. `base_sha` is `Some` when the caller read the file first
/// (handlers that do read-modify-write, like `save_annotations`); for
/// blind writes/deletes the backend resolves the SHA lazily at persist.
#[derive(Debug, Clone)]
struct PendingWrite {
    op: PendingOp,
    base_sha: Option<String>,
}

#[derive(Debug, Clone)]
enum PendingOp {
    Upsert(String),
    Delete,
}

/// Mutable state guarded by the backend's mutex. The fetcher lives here
/// because its methods take `&mut self` (ETag cache + rate-limit
/// tracking), and the SHA cache + pending buffer must be accessed from
/// `&self` callbacks on the trait.
struct Inner {
    fetcher: GitHubFetcher,
    /// Map from source-relative path → most recently observed blob SHA.
    /// Populated by `read_file`. On persist: entries for paths that were
    /// written are refreshed with the post-commit SHA; entries for paths
    /// that were deleted are removed. Stale entries for paths neither
    /// written nor deleted may linger — the next write's 409/retry path
    /// covers that, so it stays correct.
    sha_cache: HashMap<PathBuf, String>,
    /// Buffered writes/deletes, in insertion order.
    pending: Vec<(PathBuf, PendingWrite)>,
}

pub struct GitHubApiBackend {
    owner: String,
    repo: String,
    branch: String,
    /// Branch to seed the target branch from when it doesn't exist yet
    /// (default for the editor traject flow: the writable repo's
    /// default branch — `main` for the regelrecht-corpus repo).
    base_branch: Option<String>,
    /// Path prefix inside the repo (same role as `repo_subpath` on
    /// `GitBackend`). Source-relative paths are joined under this.
    sub_path: Option<String>,
    /// OAuth/PAT token for the API. `None` makes the backend read-only.
    token: Option<String>,
    inner: Mutex<Inner>,
}

impl GitHubApiBackend {
    pub fn new(
        github: &GitHubSource,
        base_branch: Option<String>,
        token: Option<String>,
    ) -> Result<Self> {
        Ok(Self {
            owner: github.owner.clone(),
            repo: github.repo.clone(),
            branch: github.effective_ref().to_string(),
            base_branch,
            sub_path: github.path.clone(),
            token,
            inner: Mutex::new(Inner {
                fetcher: GitHubFetcher::new()?,
                sha_cache: HashMap::new(),
                pending: Vec::new(),
            }),
        })
    }

    /// Repoint the backend's underlying fetcher at a different API base
    /// URL. Production code never calls this — it's the seam our
    /// wiremock-backed unit tests use to avoid hitting the real GitHub.
    /// Must be called before any other backend method (taking `&mut self`
    /// makes that easy to enforce at the type level).
    pub fn with_api_base(mut self, base_url: impl Into<String>) -> Self {
        // `get_mut` is fine because we still hold &mut self, so the mutex
        // isn't contended. Mutate the fetcher in place rather than
        // rebuilding it (which would propagate a fallible
        // `GitHubFetcher::new` call for what is meant to be a trivial
        // test-only seam).
        self.inner.get_mut().fetcher.set_base_url(base_url);
        self
    }

    fn full_repo(&self) -> String {
        format!("{}/{}", self.owner, self.repo)
    }

    /// Resolve a source-relative path to the in-repo path the GitHub API
    /// expects (with `sub_path` prefix). Forward slashes always — GitHub
    /// is OS-agnostic.
    fn api_path(&self, relative: &Path) -> Result<String> {
        validate_relative(relative)?;
        let rel = relative
            .to_str()
            .ok_or_else(|| {
                CorpusError::Config(format!("path is not valid UTF-8: {}", relative.display()))
            })?
            .replace('\\', "/");
        Ok(match &self.sub_path {
            Some(sub) if !sub.is_empty() => format!("{}/{}", sub.trim_end_matches('/'), rel),
            _ => rel,
        })
    }

    /// Fetch the current SHA for a path on the target branch. Used by
    /// `persist` when the caller never read the file first (blind write
    /// to an existing file). Returns `Ok(None)` on 404 — the caller can
    /// then treat the PUT as a create.
    async fn fetch_sha(
        inner: &mut Inner,
        repo: &str,
        branch: &str,
        path: &str,
        token: Option<&str>,
    ) -> Result<Option<String>> {
        match inner
            .fetcher
            .fetch_file_with_sha(repo, branch, path, token)
            .await?
        {
            Some((_, sha)) => Ok(Some(sha)),
            None => Ok(None),
        }
    }
}

fn validate_relative(path: &Path) -> Result<()> {
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
impl RepoBackend for GitHubApiBackend {
    async fn read_file(&self, relative_path: &Path) -> Result<Option<String>> {
        let api_path = self.api_path(relative_path)?;
        let mut inner = self.inner.lock().await;
        let outcome = inner
            .fetcher
            .fetch_file_with_sha(
                &self.full_repo(),
                &self.branch,
                &api_path,
                self.token.as_deref(),
            )
            .await?;
        match outcome {
            Some((content, sha)) => {
                inner.sha_cache.insert(relative_path.to_path_buf(), sha);
                Ok(Some(content))
            }
            None => {
                // Remove any stale SHA from a previous existence — a
                // later write will (correctly) be treated as a create.
                inner.sha_cache.remove(relative_path);
                Ok(None)
            }
        }
    }

    async fn write_file(&self, relative_path: &Path, content: &str) -> Result<()> {
        validate_relative(relative_path)?;
        if self.token.is_none() {
            return Err(CorpusError::ReadOnly(
                "GitHubApiBackend has no push token".to_string(),
            ));
        }
        let mut inner = self.inner.lock().await;
        let base_sha = inner.sha_cache.get(relative_path).cloned();
        inner.pending.push((
            relative_path.to_path_buf(),
            PendingWrite {
                op: PendingOp::Upsert(content.to_string()),
                base_sha,
            },
        ));
        Ok(())
    }

    async fn delete_file(&self, relative_path: &Path) -> Result<()> {
        validate_relative(relative_path)?;
        if self.token.is_none() {
            return Err(CorpusError::ReadOnly(
                "GitHubApiBackend has no push token".to_string(),
            ));
        }
        let mut inner = self.inner.lock().await;
        let base_sha = inner.sha_cache.get(relative_path).cloned();
        inner.pending.push((
            relative_path.to_path_buf(),
            PendingWrite {
                op: PendingOp::Delete,
                base_sha,
            },
        ));
        Ok(())
    }

    async fn list_files(&self, dir: &Path, extension: Option<&str>) -> Result<Vec<FileEntry>> {
        let api_dir = self.api_path(dir)?;
        let mut inner = self.inner.lock().await;
        let entries = inner
            .fetcher
            .list_directory(
                &self.full_repo(),
                &self.branch,
                &api_dir,
                self.token.as_deref(),
            )
            .await?;
        let mut out: Vec<FileEntry> = entries
            .into_iter()
            .filter(|e| e.entry_type == "file")
            .filter(|e| match extension {
                None => true,
                Some(ext) => Path::new(&e.name)
                    .extension()
                    .and_then(|s| s.to_str())
                    .is_some_and(|s| s == ext),
            })
            .map(|e| FileEntry { name: e.name })
            .collect();
        out.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(out)
    }

    async fn list_files_recursive(
        &self,
        dir: &Path,
        extension: Option<&str>,
    ) -> Result<Vec<RecursiveFileEntry>> {
        let api_root = self.api_path(dir)?;
        let mut inner = self.inner.lock().await;

        // Iterative BFS over Contents-API directory pages. Each tuple is
        // (`relative path under the listing root`, `full API path`); the
        // empty prefix on the seed means the seed directory's direct
        // children appear with bare filenames in the output.
        //
        // **GitHub limit**: the Contents API caps each directory listing
        // at 1000 entries with no pagination. A `documents/<traject>/`
        // folder is extremely unlikely to hit that, but if it ever does
        // the listing truncates silently. Switching to the Git Trees API
        // (`/git/trees/{sha}?recursive=1`) is the proper fix when the
        // need arises — it returns the entire subtree in one call.
        let mut queue: Vec<(String, String)> = vec![(String::new(), api_root)];
        let mut out: Vec<RecursiveFileEntry> = Vec::new();

        while let Some((rel_prefix, api_dir)) = queue.pop() {
            let entries = inner
                .fetcher
                .list_directory(
                    &self.full_repo(),
                    &self.branch,
                    &api_dir,
                    self.token.as_deref(),
                )
                .await?;
            for e in entries {
                let child_rel = if rel_prefix.is_empty() {
                    e.name.clone()
                } else {
                    format!("{}/{}", rel_prefix, e.name)
                };
                match e.entry_type.as_str() {
                    "file" => {
                        if let Some(ext) = extension {
                            let matches = Path::new(&e.name)
                                .extension()
                                .and_then(|s| s.to_str())
                                .is_some_and(|s| s == ext);
                            if !matches {
                                continue;
                            }
                        }
                        out.push(RecursiveFileEntry {
                            relative_path: child_rel,
                        });
                    }
                    "dir" => {
                        let child_api = if api_dir.is_empty() {
                            e.name
                        } else {
                            format!("{}/{}", api_dir.trim_end_matches('/'), e.name)
                        };
                        queue.push((child_rel, child_api));
                    }
                    _ => continue,
                }
            }
        }

        out.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
        Ok(out)
    }

    async fn persist(&self, ctx: &WriteContext) -> Result<PersistOutcome> {
        let pending: Vec<(PathBuf, PendingWrite)> = {
            let mut inner = self.inner.lock().await;
            std::mem::take(&mut inner.pending)
        };
        if pending.is_empty() {
            return Ok(PersistOutcome::default());
        }

        // Committer falls back to a service identity when no human is
        // attached — same shape as `GitBackend` (the trailer/co-author
        // is left empty rather than spoofed).
        let committer = Committer {
            name: ctx
                .author
                .as_ref()
                .map(|a| a.name.clone())
                .unwrap_or_else(|| "regelrecht-editor".to_string()),
            email: ctx
                .author
                .as_ref()
                .map(|a| a.email.clone())
                .unwrap_or_else(|| "noreply@regelrecht.local".to_string()),
        };

        let token = self.token.as_deref();
        let repo = self.full_repo();
        let mut new_shas: HashMap<PathBuf, String> = HashMap::new();

        // Take one lock guard for the whole loop so the &mut GitHubFetcher
        // calls inside don't pay re-acquire cost per-write. The pending
        // buffer was already drained above; if any write fails we propagate
        // via `?` and the remaining (still-untaken-from-buffer) entries are
        // dropped — fine in practice because each handler only enqueues a
        // single write before calling persist, so there is no partially-
        // applied multi-write batch to recover here.
        let mut inner = self.inner.lock().await;
        for (path, pw) in pending {
            let api_path = self.api_path(&path)?;
            match pw.op {
                PendingOp::Upsert(content) => {
                    let new_sha = try_put(
                        &mut inner.fetcher,
                        &repo,
                        &self.branch,
                        &api_path,
                        &content,
                        pw.base_sha.as_deref(),
                        &committer,
                        &ctx.message,
                        token,
                    )
                    .await?;
                    new_shas.insert(path, new_sha);
                }
                PendingOp::Delete => {
                    let sha_for_delete = match &pw.base_sha {
                        Some(s) => s.clone(),
                        None => {
                            match Self::fetch_sha(&mut inner, &repo, &self.branch, &api_path, token)
                                .await?
                            {
                                Some(s) => s,
                                // Already gone: treat as a successful delete,
                                // same shape as `LocalBackend::delete_file`.
                                None => continue,
                            }
                        }
                    };
                    try_delete(
                        &mut inner.fetcher,
                        &repo,
                        &self.branch,
                        &api_path,
                        &sha_for_delete,
                        &committer,
                        &ctx.message,
                        token,
                    )
                    .await?;
                    // Drop the cached SHA so a next read sees the file as
                    // gone (or rebuilt) without holding a stale value.
                    inner.sha_cache.remove(&path);
                }
            }
        }

        // Refresh the SHA cache so a follow-up read-modify-write cycle in
        // the same backend instance starts from the post-commit SHA.
        for (path, sha) in new_shas {
            inner.sha_cache.insert(path, sha);
        }

        // Contents API commits straight to the configured branch — no
        // PR is opened. The trajectflow already accepted `pr: None`
        // from the previous `GitBackend` impl, so this is wire-
        // compatible with the existing save handlers.
        Ok(PersistOutcome::default())
    }

    async fn ensure_ready(&mut self) -> Result<()> {
        // Read-only backends have nothing to bootstrap — the branch
        // either exists (reads work) or it doesn't (reads 404 as
        // they'd 404 on a missing file), and we don't try to mint a
        // branch without a write token.
        if self.token.is_none() {
            return Ok(());
        }
        let inner = self.inner.get_mut();
        let repo = format!("{}/{}", self.owner, self.repo);
        let exists = inner
            .fetcher
            .branch_exists(&repo, &self.branch, self.token.as_deref())
            .await?;
        if exists {
            return Ok(());
        }
        let base = self.base_branch.as_deref().ok_or_else(|| {
            CorpusError::Config(format!(
                "branch '{}' does not exist on {}/{} and no base_branch \
                 was configured to seed it from",
                self.branch, self.owner, self.repo
            ))
        })?;
        // TOCTOU on lazy branch creation: between our `branch_exists`
        // returning false and this POST, another activation (different
        // backend instance, same traject) can win the race and create
        // the branch first. GitHub then 422s us with "Reference already
        // exists". Re-check `branch_exists` on any create_branch failure;
        // if the branch is present now the desired post-condition holds
        // and we treat the create as a benign no-op.
        match inner
            .fetcher
            .create_branch(&repo, &self.branch, base, self.token.as_deref())
            .await
        {
            Ok(()) => {
                tracing::info!(
                    repo = %repo,
                    branch = %self.branch,
                    base = %base,
                    "GitHubApiBackend: created traject branch from base"
                );
            }
            Err(e) => {
                let now_exists = inner
                    .fetcher
                    .branch_exists(&repo, &self.branch, self.token.as_deref())
                    .await
                    .unwrap_or(false);
                if now_exists {
                    tracing::info!(
                        repo = %repo,
                        branch = %self.branch,
                        "GitHubApiBackend: create_branch lost a benign race; branch already exists"
                    );
                } else {
                    return Err(e);
                }
            }
        }
        Ok(())
    }

    fn is_writable(&self) -> bool {
        self.token.is_some()
    }
}

/// PUT with one optimistic-concurrency retry: on 409 the SHA is refreshed
/// (the file moved between our last read and this PUT) and the put is
/// reattempted exactly once with the new SHA. A second 409 propagates so
/// the caller can decide between abort and a higher-level retry.
#[allow(clippy::too_many_arguments)]
async fn try_put(
    fetcher: &mut GitHubFetcher,
    repo: &str,
    branch: &str,
    path: &str,
    content: &str,
    base_sha: Option<&str>,
    committer: &Committer,
    message: &str,
    token: Option<&str>,
) -> Result<String> {
    match fetcher
        .put_file(
            repo, branch, path, content, base_sha, committer, message, token,
        )
        .await
    {
        Ok(sha) => Ok(sha),
        Err(CorpusError::Conflict(_)) => {
            tracing::debug!(repo = %repo, path = %path, "PUT 409 — refreshing sha and retrying");
            let fresh = fetcher
                .fetch_file_with_sha(repo, branch, path, token)
                .await?
                .map(|(_, sha)| sha);
            fetcher
                .put_file(
                    repo,
                    branch,
                    path,
                    content,
                    fresh.as_deref(),
                    committer,
                    message,
                    token,
                )
                .await
        }
        Err(e) if is_unsigned_existing_file(&e) => {
            // A PUT without `sha` against an existing file returns 422
            // ("sha was not supplied"). Resolve the SHA and retry once
            // — covers `save_law` / `save_scenario` which call
            // `write_file` without a preceding `read_file`.
            tracing::debug!(repo = %repo, path = %path, "PUT 422 — fetching sha and retrying as update");
            let fresh = fetcher
                .fetch_file_with_sha(repo, branch, path, token)
                .await?
                .map(|(_, sha)| sha);
            if fresh.is_none() {
                // 422 wasn't about an existing file after all — propagate
                // the original error so the operator can diagnose.
                return Err(e);
            }
            fetcher
                .put_file(
                    repo,
                    branch,
                    path,
                    content,
                    fresh.as_deref(),
                    committer,
                    message,
                    token,
                )
                .await
        }
        Err(e) => Err(e),
    }
}

/// DELETE with one optimistic-concurrency retry: same shape as `try_put`.
#[allow(clippy::too_many_arguments)]
async fn try_delete(
    fetcher: &mut GitHubFetcher,
    repo: &str,
    branch: &str,
    path: &str,
    sha: &str,
    committer: &Committer,
    message: &str,
    token: Option<&str>,
) -> Result<()> {
    match fetcher
        .delete_file_via_api(repo, branch, path, sha, committer, message, token)
        .await
    {
        Ok(()) => Ok(()),
        Err(CorpusError::Conflict(_)) => {
            tracing::debug!(repo = %repo, path = %path, "DELETE 409 — refreshing sha and retrying");
            let fresh = fetcher
                .fetch_file_with_sha(repo, branch, path, token)
                .await?
                .map(|(_, sha)| sha);
            match fresh {
                Some(s) => {
                    fetcher
                        .delete_file_via_api(repo, branch, path, &s, committer, message, token)
                        .await
                }
                // Race: file was deleted between our 409 and this
                // refetch. Treat as a successful delete.
                None => Ok(()),
            }
        }
        Err(e) => Err(e),
    }
}

/// Best-effort detection that a PUT failed with 422 because we omitted
/// `sha` while the file already exists. We don't have a structured error
/// variant for 422 (it's just `CorpusError::Git`), so we sniff the
/// message — the GitHub response text reliably mentions `sha`.
fn is_unsigned_existing_file(e: &CorpusError) -> bool {
    match e {
        CorpusError::Git(msg) => msg.contains(" 422") && msg.contains("sha"),
        _ => false,
    }
}
