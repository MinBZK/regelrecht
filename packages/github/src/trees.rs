//! Git Trees API: list every blob in a repo at a ref in one call, with ETag
//! caching so an unchanged tree comes back as a cheap 304.

use serde::Deserialize;

use crate::client::GithubClient;
use crate::error::{GithubError, Result};

/// One blob discovered in a repo tree: its repo-relative path plus the blob
/// sha the listing reported. The sha is the file's content identity — two
/// listings reporting the same sha are byte-identical.
#[derive(Debug, Clone)]
pub struct TreeEntryFile {
    pub path: String,
    /// GitHub always sends the blob sha; kept optional so a missing field
    /// degrades to "no content identity" rather than failing the whole parse.
    pub sha: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TreeResponse {
    tree: Vec<TreeEntry>,
    truncated: bool,
}

#[derive(Debug, Deserialize)]
struct TreeEntry {
    path: String,
    #[serde(rename = "type")]
    entry_type: String,
    #[serde(default)]
    sha: Option<String>,
}

impl GithubClient {
    /// List every **blob** (file) in `repo` at `git_ref` via the Trees API
    /// (`GET /repos/{repo}/git/trees/{ref}?recursive=1`), one call for the
    /// whole tree.
    ///
    /// Uses `If-None-Match` from the per-URL ETag cache: returns `Ok(None)`
    /// when the tree is unchanged (HTTP 304) so callers can preserve
    /// previously loaded data. Non-blob entries (trees, submodules, symlinks)
    /// are filtered out; the caller narrows further (e.g. by extension or
    /// sub-path). A truncated response (repo too large for one page) is an
    /// error rather than a silent partial list.
    pub async fn list_tree_files(
        &self,
        repo: &str,
        git_ref: &str,
        token: Option<&str>,
    ) -> Result<Option<Vec<TreeEntryFile>>> {
        let url = format!(
            "{}/repos/{}/git/trees/{}?recursive=1",
            self.api_base, repo, git_ref
        );

        let mut headers = self.default_headers(token)?;
        if let Some(etag) = self.cached_etag(&url) {
            if let Ok(val) = reqwest::header::HeaderValue::from_str(&etag) {
                headers.insert(reqwest::header::IF_NONE_MATCH, val);
            }
        }

        let response = self
            .client
            .get(&url)
            .headers(headers)
            .send()
            .await
            .map_err(|e| GithubError::Transport(format!("GitHub Trees API request failed: {e}")))?;
        self.track_rate_limit(&response);

        if response.status() == reqwest::StatusCode::NOT_MODIFIED {
            tracing::debug!(repo = %repo, "Tree unchanged (ETag match)");
            return Ok(None);
        }

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(GithubError::Api {
                status,
                message: format!("Trees API for {repo}@{git_ref}: {body}"),
            });
        }

        if let Some(etag) = response.headers().get("etag").and_then(|v| v.to_str().ok()) {
            self.store_etag(&url, etag);
        }

        let tree: TreeResponse = response
            .json()
            .await
            .map_err(|e| GithubError::Decode(format!("failed to parse tree response: {e}")))?;

        if tree.truncated {
            return Err(GithubError::Api {
                status: 200,
                message: format!(
                    "Trees API response for '{repo}' was truncated — repository has too \
                     many files. Narrow the listing (a sub-path) or reduce the file count."
                ),
            });
        }

        let files = tree
            .tree
            .into_iter()
            .filter(|e| e.entry_type == "blob")
            .map(|e| TreeEntryFile {
                path: e.path,
                sha: e.sha,
            })
            .collect();
        Ok(Some(files))
    }
}
