//! Contents API: read a file (raw or with its sha), list a directory, and
//! upsert / delete a file. Writes carry optimistic-concurrency semantics
//! (409 → [`GithubError::Conflict`]) and permission semantics (genuine 403 →
//! [`GithubError::WriteDenied`], rate-limit 403 stays generic).

use base64::Engine;
use reqwest::header::{HeaderValue, ACCEPT, CONTENT_TYPE};
use serde::{Deserialize, Serialize};

use crate::client::GithubClient;
use crate::error::{GithubError, Result};

/// Commit identity for Contents / Git Data API writes. Both `committer` and
/// `author` accept this shape; callers set them to the same value so the human
/// editor shows on both sides of the commit when authenticating with a shared
/// service token. When authenticating with the acting user's own GitHub token,
/// callers pass `None`: the Contents API then defaults author and committer to
/// the authenticated user, attributing the commit to their real account.
#[derive(Debug, Clone, Serialize)]
pub struct Committer {
    pub name: String,
    pub email: String,
}

/// Single entry from a Contents API directory listing. Only the fields the
/// backend needs; GitHub returns more (url, html_url, size, …).
#[derive(Debug, Clone)]
pub struct DirectoryEntry {
    pub name: String,
    /// `"file"` or `"dir"` (GitHub also reports `"submodule"` / `"symlink"`).
    pub entry_type: String,
}

/// Raw Contents API response for a single path — used for file reads
/// (`type == "file"`) and, as a JSON array, for directory listings.
#[derive(Debug, Deserialize)]
struct ContentsItem {
    name: String,
    path: String,
    sha: String,
    #[serde(rename = "type")]
    entry_type: String,
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    encoding: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PutResponse {
    content: PutContent,
}
#[derive(Debug, Deserialize)]
struct PutContent {
    sha: String,
}

impl GithubClient {
    /// Fetch a single file's content via the Contents API, requesting the raw
    /// representation (`application/vnd.github.raw+json`) so there is no
    /// base64 to decode. Errors on any non-success status.
    pub async fn fetch_file_raw(
        &self,
        repo: &str,
        git_ref: &str,
        path: &str,
        token: Option<&str>,
    ) -> Result<String> {
        let url = format!(
            "{}/repos/{}/contents/{}?ref={}",
            self.api_base, repo, path, git_ref
        );
        let mut headers = self.default_headers(token)?;
        headers.insert(
            ACCEPT,
            HeaderValue::from_static("application/vnd.github.raw+json"),
        );

        let response = self
            .client
            .get(&url)
            .headers(headers)
            .send()
            .await
            .map_err(|e| GithubError::Transport(format!("GitHub API request failed: {e}")))?;
        self.track_rate_limit(&response);

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(GithubError::Api {
                status,
                message: format!("Contents API for {path}: {body}"),
            });
        }

        response
            .text()
            .await
            .map_err(|e| GithubError::Transport(format!("failed to read response body: {e}")))
    }

    /// Fetch a file's content **plus** its blob sha (the value a later update
    /// PUT needs for optimistic concurrency). Returns `Ok(None)` on 404.
    #[tracing::instrument(name = "gh_http", skip_all, fields(method = "GET", kind = "contents", repo = %repo))]
    pub async fn fetch_file_with_sha(
        &self,
        repo: &str,
        git_ref: &str,
        path: &str,
        token: Option<&str>,
    ) -> Result<Option<(String, String)>> {
        let url = format!(
            "{}/repos/{}/contents/{}?ref={}",
            self.api_base, repo, path, git_ref
        );
        let headers = self.default_headers(token)?;
        let response = self
            .client
            .get(&url)
            .headers(headers)
            .send()
            .await
            .map_err(|e| GithubError::Transport(format!("GitHub API request failed: {e}")))?;
        self.track_rate_limit(&response);
        tracing::debug!(status = %response.status(), "gh contents GET response");

        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(GithubError::Api {
                status,
                message: format!("Contents API for {path}: {body}"),
            });
        }

        let item: ContentsItem = response
            .json()
            .await
            .map_err(|e| GithubError::Decode(format!("failed to parse contents response: {e}")))?;
        if item.entry_type != "file" {
            return Err(GithubError::Api {
                status: 200,
                message: format!("Path '{path}' is a {}, not a file", item.entry_type),
            });
        }
        let content = decode_contents_payload(&item)?;
        Ok(Some((content, item.sha)))
    }

    /// List a directory via the Contents API. Returns an empty list for a
    /// missing directory (404) — the "nothing here yet" path. Non-array
    /// responses (someone listed a file path) also yield an empty list.
    #[tracing::instrument(name = "gh_http", skip_all, fields(method = "GET", kind = "contents_dir", repo = %repo))]
    pub async fn list_directory(
        &self,
        repo: &str,
        git_ref: &str,
        dir: &str,
        token: Option<&str>,
    ) -> Result<Vec<DirectoryEntry>> {
        let url = format!(
            "{}/repos/{}/contents/{}?ref={}",
            self.api_base, repo, dir, git_ref
        );
        let headers = self.default_headers(token)?;
        let response = self
            .client
            .get(&url)
            .headers(headers)
            .send()
            .await
            .map_err(|e| GithubError::Transport(format!("GitHub API request failed: {e}")))?;
        self.track_rate_limit(&response);

        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(Vec::new());
        }
        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(GithubError::Api {
                status,
                message: format!("Contents API for {dir}: {body}"),
            });
        }

        let body = response.text().await.map_err(|e| {
            GithubError::Transport(format!("failed to read directory listing: {e}"))
        })?;
        let trimmed = body.trim_start();
        if !trimmed.starts_with('[') {
            tracing::debug!(dir = %dir, "list_directory: path is not a directory");
            return Ok(Vec::new());
        }
        let items: Vec<ContentsItem> = serde_json::from_str(&body)
            .map_err(|e| GithubError::Decode(format!("failed to parse directory listing: {e}")))?;
        Ok(items
            .into_iter()
            .map(|i| DirectoryEntry {
                name: i.name,
                entry_type: i.entry_type,
            })
            .collect())
    }

    /// Upsert a file via Contents API PUT. `base_sha = None` creates a new
    /// file, `Some(sha)` updates an existing one. Returns the new blob sha so
    /// callers can chain writes without a re-read.
    ///
    /// Maps 409 → [`GithubError::Conflict`] (stale sha; caller can retry) and
    /// a genuine 403 → [`GithubError::WriteDenied`] — except rate-limit 403s,
    /// which stay on the generic [`GithubError::Api`] path.
    #[allow(clippy::too_many_arguments)]
    #[tracing::instrument(name = "gh_http", skip_all, fields(method = "PUT", kind = "contents", repo = %repo))]
    pub async fn put_file(
        &self,
        repo: &str,
        branch: &str,
        path: &str,
        content: &str,
        base_sha: Option<&str>,
        committer: Option<&Committer>,
        message: &str,
        token: Option<&str>,
    ) -> Result<String> {
        let url = format!("{}/repos/{}/contents/{}", self.api_base, repo, path);
        let mut body = serde_json::json!({
            "message": message,
            "content": base64::engine::general_purpose::STANDARD.encode(content.as_bytes()),
            "branch": branch,
        });
        if let Some(committer) = committer {
            body["committer"] = serde_json::json!(committer);
            body["author"] = serde_json::json!(committer);
        }
        if let Some(sha) = base_sha {
            body["sha"] = serde_json::Value::String(sha.to_string());
        }

        let mut headers = self.default_headers(token)?;
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

        let response = self
            .client
            .put(&url)
            .headers(headers)
            .body(body.to_string())
            .send()
            .await
            .map_err(|e| GithubError::Transport(format!("GitHub API request failed: {e}")))?;
        self.track_rate_limit(&response);

        let status = response.status();
        tracing::debug!(status = %status, "gh contents PUT response");
        if status == reqwest::StatusCode::CONFLICT {
            return Err(GithubError::Conflict(format!(
                "Contents API PUT {path} hit a 409 (stale sha)"
            )));
        }
        if status == reqwest::StatusCode::FORBIDDEN && !Self::forbidden_is_rate_limit(&response) {
            let body = response.text().await.unwrap_or_default();
            return Err(GithubError::WriteDenied(format!(
                "Contents API PUT {path} returned 403: {body}"
            )));
        }
        if !status.is_success() {
            let code = status.as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(GithubError::Api {
                status: code,
                message: format!("Contents API PUT {path}: {body}"),
            });
        }
        let parsed: PutResponse = response
            .json()
            .await
            .map_err(|e| GithubError::Decode(format!("failed to parse PUT response: {e}")))?;
        Ok(parsed.content.sha)
    }

    /// Delete a file via Contents API DELETE. Requires the current blob sha.
    /// 404 is treated as "already gone" (idempotent). 409 → Conflict, genuine
    /// 403 → WriteDenied, same as [`put_file`](Self::put_file).
    #[allow(clippy::too_many_arguments)]
    #[tracing::instrument(name = "gh_http", skip_all, fields(method = "DELETE", kind = "contents", repo = %repo))]
    pub async fn delete_file(
        &self,
        repo: &str,
        branch: &str,
        path: &str,
        sha: &str,
        committer: Option<&Committer>,
        message: &str,
        token: Option<&str>,
    ) -> Result<()> {
        let url = format!("{}/repos/{}/contents/{}", self.api_base, repo, path);
        let mut body = serde_json::json!({
            "message": message,
            "sha": sha,
            "branch": branch,
        });
        if let Some(committer) = committer {
            body["committer"] = serde_json::json!(committer);
            body["author"] = serde_json::json!(committer);
        }

        let mut headers = self.default_headers(token)?;
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

        let response = self
            .client
            .delete(&url)
            .headers(headers)
            .body(body.to_string())
            .send()
            .await
            .map_err(|e| GithubError::Transport(format!("GitHub API request failed: {e}")))?;
        self.track_rate_limit(&response);

        let status = response.status();
        if status == reqwest::StatusCode::NOT_FOUND {
            return Ok(());
        }
        if status == reqwest::StatusCode::CONFLICT {
            return Err(GithubError::Conflict(format!(
                "Contents API DELETE {path} hit a 409 (stale sha)"
            )));
        }
        if status == reqwest::StatusCode::FORBIDDEN && !Self::forbidden_is_rate_limit(&response) {
            let body = response.text().await.unwrap_or_default();
            return Err(GithubError::WriteDenied(format!(
                "Contents API DELETE {path} returned 403: {body}"
            )));
        }
        if !status.is_success() {
            let code = status.as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(GithubError::Api {
                status: code,
                message: format!("Contents API DELETE {path}: {body}"),
            });
        }
        Ok(())
    }
}

/// Decode a Contents-API response's content payload. The JSON path always
/// gives base64 (default `encoding: "base64"`); files >1 MiB come back without
/// `content` (encoding `"none"`), for which the Git Blob API is the route.
fn decode_contents_payload(item: &ContentsItem) -> Result<String> {
    let encoding = item.encoding.as_deref().unwrap_or("base64");
    if encoding != "base64" {
        return Err(GithubError::Decode(format!(
            "Contents API returned unsupported encoding '{encoding}' for {} \
             (large file? use the Blob API)",
            item.path
        )));
    }
    let content = item.content.as_deref().ok_or_else(|| {
        GithubError::Decode(format!(
            "Contents API returned no content for {} (possibly >1 MiB)",
            item.path
        ))
    })?;
    // The API wraps base64 at 60 chars/line — strip whitespace first.
    let cleaned: String = content
        .chars()
        .filter(|c| !c.is_ascii_whitespace())
        .collect();
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(cleaned.as_bytes())
        .map_err(|e| GithubError::Decode(format!("base64 decode failed for {}: {e}", item.path)))?;
    String::from_utf8(bytes)
        .map_err(|e| GithubError::Decode(format!("UTF-8 decode failed for {}: {e}", item.path)))
}
