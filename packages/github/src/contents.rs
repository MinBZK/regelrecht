//! Contents API: read a file (raw or with its sha), list a directory, and
//! upsert / delete a file. Writes carry optimistic-concurrency semantics
//! (409 → [`GithubError::Conflict`]) and permission semantics (genuine 403 →
//! [`GithubError::WriteDenied`], rate-limit 403 stays generic).
//!
//! ## Absence versus failure
//!
//! The reads here answer "not there" as `Ok(None)` / an empty listing, and
//! callers act on that: a missing file is created, a missing directory has
//! no colliding names in it. So a *failure* dressed up as absence is not a
//! degraded read — it overwrites work. Only a 404 that GitHub attributes to
//! the **path** is absence; a 404 for a ref that doesn't exist (a branch
//! never created, a deleted preview branch) means the answer is unknown and
//! comes back as an error. See [`not_found_is_missing_ref`].
//!
//! ## Conditional GETs
//!
//! All three reads carry `If-None-Match` from the client's response cache,
//! which stores each ETag together with the body it belongs to. A 304 is
//! answered from that body, so it saves rate-limit quota (304s don't count)
//! without ever turning into an empty answer. A 304 arriving without a
//! cached body — the server ignoring the absence of our header — is retried
//! unconditionally rather than reported as "gone".

use base64::Engine;
use reqwest::header::{HeaderValue, ACCEPT, CONTENT_TYPE, IF_NONE_MATCH};
use serde::{Deserialize, Serialize};

use crate::client::{CachedPayload, GithubClient};
use crate::error::{GithubError, Result};

/// Whether a 404 body says the **ref** is missing rather than the path.
/// GitHub answers a read at a non-existent branch/sha with `"No commit
/// found for the ref"`; a genuinely missing file inside a live ref answers
/// the generic `"Not Found"`. Only the latter is absence.
fn not_found_is_missing_ref(body: &str) -> bool {
    body.contains("No commit found for the ref")
}

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

/// What a conditional GET produced: a cache hit, a body to parse, or a
/// 404 that is genuinely about the path.
enum ConditionalOutcome {
    /// HTTP 304 — the cached payload is current and is the answer.
    NotModified(CachedPayload),
    /// A fresh 2xx body, with the key its payload should be stored under.
    Fresh {
        response: reqwest::Response,
        cache_key: String,
    },
    /// HTTP 404 attributed to the path: the thing is not there.
    Absent,
}

impl GithubClient {
    /// One conditional Contents GET: attaches `If-None-Match` when a cached
    /// payload exists, resolves 304 against that cache, separates
    /// path-absence from ref-absence, and turns every other non-success
    /// status into an error.
    ///
    /// A 304 without a cached payload cannot be answered (there is no body
    /// to return, and reporting absence would be a lie), so the request is
    /// retried once without the conditional header. Only a server that 304s
    /// an unconditional request reaches the error at the end.
    async fn conditional_contents_get(
        &self,
        url: &str,
        accept: Option<&'static str>,
        token: Option<&str>,
        what: &str,
    ) -> Result<ConditionalOutcome> {
        let cache_key = Self::cache_key(url, token);

        for attempt in 0..2 {
            let cached = if attempt == 0 {
                self.cached_response(&cache_key)
            } else {
                None
            };

            let mut headers = self.default_headers(token)?;
            if let Some(accept) = accept {
                headers.insert(ACCEPT, HeaderValue::from_static(accept));
            }
            if let Some(hit) = &cached {
                if let Ok(value) = HeaderValue::from_str(&hit.etag) {
                    headers.insert(IF_NONE_MATCH, value);
                }
            }

            let response = self
                .client
                .get(url)
                .headers(headers)
                .send()
                .await
                .map_err(|e| GithubError::Transport(format!("GitHub API request failed: {e}")))?;
            self.track_rate_limit(&response);
            tracing::debug!(status = %response.status(), what, "gh contents GET response");

            if response.status() == reqwest::StatusCode::NOT_MODIFIED {
                match cached {
                    Some(hit) => return Ok(ConditionalOutcome::NotModified(hit.payload)),
                    // Unprompted 304: fall through to the unconditional retry.
                    None => continue,
                }
            }

            if response.status() == reqwest::StatusCode::NOT_FOUND {
                let body = response.text().await.unwrap_or_default();
                if not_found_is_missing_ref(&body) {
                    return Err(GithubError::Api {
                        status: 404,
                        message: format!(
                            "Contents API for {what}: the ref does not exist ({body})"
                        ),
                    });
                }
                return Ok(ConditionalOutcome::Absent);
            }

            if !response.status().is_success() {
                let status = response.status().as_u16();
                let body = response.text().await.unwrap_or_default();
                return Err(GithubError::Api {
                    status,
                    message: format!("Contents API for {what}: {body}"),
                });
            }

            return Ok(ConditionalOutcome::Fresh {
                response,
                cache_key,
            });
        }

        Err(GithubError::Api {
            status: 304,
            message: format!(
                "Contents API for {what}: answered 304 to a request that carried no \
                 If-None-Match, so there is no body to serve"
            ),
        })
    }

    /// Read the `ETag` header off a fresh response, if it has one.
    fn response_etag(response: &reqwest::Response) -> Option<String> {
        response
            .headers()
            .get("etag")
            .and_then(|v| v.to_str().ok())
            .map(str::to_string)
    }

    /// Fetch a single file's content via the Contents API, requesting the raw
    /// representation (`application/vnd.github.raw+json`) so there is no
    /// base64 to decode — and, unlike the JSON representation, no 1 MiB
    /// ceiling. Errors on any non-success status, 404 included.
    pub async fn fetch_file_raw(
        &self,
        repo: &str,
        git_ref: &str,
        path: &str,
        token: Option<&str>,
    ) -> Result<String> {
        self.fetch_file_raw_opt(repo, git_ref, path, token)
            .await?
            .ok_or_else(|| GithubError::Api {
                status: 404,
                message: format!("Contents API for {path}: not found"),
            })
    }

    /// [`fetch_file_raw`](Self::fetch_file_raw) with absence as a value:
    /// `Ok(None)` when the file is not in the ref. A missing *ref* still
    /// errors — see the module docs.
    #[tracing::instrument(name = "gh_http", skip_all, fields(method = "GET", kind = "contents_raw", repo = %repo))]
    pub async fn fetch_file_raw_opt(
        &self,
        repo: &str,
        git_ref: &str,
        path: &str,
        token: Option<&str>,
    ) -> Result<Option<String>> {
        let url = format!(
            "{}/repos/{}/contents/{}?ref={}",
            self.api_base, repo, path, git_ref
        );
        let outcome = self
            .conditional_contents_get(&url, Some("application/vnd.github.raw+json"), token, path)
            .await?;
        let (response, cache_key) = match outcome {
            ConditionalOutcome::Absent => return Ok(None),
            ConditionalOutcome::NotModified(CachedPayload::Raw(content)) => {
                return Ok(Some(content))
            }
            // A cached payload of another shape cannot answer this read.
            // It can only happen if a URL served two representations; treat
            // it as a miss rather than guessing.
            ConditionalOutcome::NotModified(_) => {
                return Err(GithubError::Decode(format!(
                    "cached response for {path} is not a raw file body"
                )))
            }
            ConditionalOutcome::Fresh {
                response,
                cache_key,
            } => (response, cache_key),
        };

        let etag = Self::response_etag(&response);
        let content = response
            .text()
            .await
            .map_err(|e| GithubError::Transport(format!("failed to read response body: {e}")))?;
        if let Some(etag) = etag {
            self.store_response(&cache_key, &etag, CachedPayload::Raw(content.clone()));
        }
        Ok(Some(content))
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
        let outcome = self
            .conditional_contents_get(&url, None, token, path)
            .await?;
        let (response, cache_key) = match outcome {
            ConditionalOutcome::Absent => return Ok(None),
            ConditionalOutcome::NotModified(CachedPayload::File { content, sha }) => {
                return Ok(Some((content, sha)))
            }
            ConditionalOutcome::NotModified(_) => {
                return Err(GithubError::Decode(format!(
                    "cached response for {path} is not a file body"
                )))
            }
            ConditionalOutcome::Fresh {
                response,
                cache_key,
            } => (response, cache_key),
        };

        let etag = Self::response_etag(&response);
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
        if let Some(etag) = etag {
            self.store_response(
                &cache_key,
                &etag,
                CachedPayload::File {
                    content: content.clone(),
                    sha: item.sha.clone(),
                },
            );
        }
        Ok(Some((content, item.sha)))
    }

    /// List a directory via the Contents API. Returns an empty list for a
    /// missing directory (404) — the "nothing here yet" path. Non-array
    /// responses (someone listed a file path) also yield an empty list.
    /// A 404 for a missing **ref** is an error, not an empty directory.
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
        let outcome = self
            .conditional_contents_get(&url, None, token, dir)
            .await?;
        let (response, cache_key) = match outcome {
            ConditionalOutcome::Absent => return Ok(Vec::new()),
            ConditionalOutcome::NotModified(CachedPayload::Directory(entries)) => {
                return Ok(entries)
            }
            ConditionalOutcome::NotModified(_) => {
                return Err(GithubError::Decode(format!(
                    "cached response for {dir} is not a directory listing"
                )))
            }
            ConditionalOutcome::Fresh {
                response,
                cache_key,
            } => (response, cache_key),
        };

        let etag = Self::response_etag(&response);
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
        let entries: Vec<DirectoryEntry> = items
            .into_iter()
            .map(|i| DirectoryEntry {
                name: i.name,
                entry_type: i.entry_type,
            })
            .collect();
        if let Some(etag) = etag {
            self.store_response(&cache_key, &etag, CachedPayload::Directory(entries.clone()));
        }
        Ok(entries)
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

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use base64::Engine;
    use wiremock::matchers::{header, header_exists, method, path as path_matcher};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;

    /// GitHub's 404 body when the branch/sha in `?ref=` does not exist.
    const MISSING_REF_BODY: &str =
        r#"{"message":"No commit found for the ref pr9999","status":"404"}"#;
    /// GitHub's 404 body for a path that is simply not in the ref.
    const MISSING_PATH_BODY: &str = r#"{"message":"Not Found","status":"404"}"#;

    fn client(server: &MockServer) -> GithubClient {
        GithubClient::new().unwrap().with_base_url(server.uri())
    }

    /// A Contents API file response body carrying `content` for `path`.
    fn file_body(path: &str, sha: &str, content: &str) -> serde_json::Value {
        serde_json::json!({
            "name": path.rsplit('/').next().unwrap_or(path),
            "path": path,
            "sha": sha,
            "type": "file",
            "encoding": "base64",
            "content": base64::engine::general_purpose::STANDARD.encode(content.as_bytes()),
        })
    }

    fn dir_body(names: &[(&str, &str)]) -> serde_json::Value {
        serde_json::Value::Array(
            names
                .iter()
                .map(|(name, kind)| {
                    serde_json::json!({
                        "name": name,
                        "path": name,
                        "sha": "s",
                        "type": kind,
                    })
                })
                .collect(),
        )
    }

    /// A listing that 404s because the *ref* is gone must not read as "this
    /// directory is empty": the caller derives collision-free filenames from
    /// that list, so an empty answer silently authorises an overwrite.
    #[tokio::test]
    async fn missing_ref_does_not_read_as_an_empty_directory() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path_matcher("/repos/acme/corpus/contents/documents"))
            .respond_with(ResponseTemplate::new(404).set_body_string(MISSING_REF_BODY))
            .mount(&server)
            .await;

        let err = client(&server)
            .list_directory("acme/corpus", "pr9999", "documents", None)
            .await
            .expect_err("a missing ref must fail the listing");
        assert!(
            matches!(err, GithubError::Api { status: 404, .. }),
            "unexpected error: {err}"
        );
    }

    /// The other side of that coin: a directory that genuinely isn't there
    /// yet is absence, and stays an empty listing.
    #[tokio::test]
    async fn missing_path_still_lists_empty() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path_matcher("/repos/acme/corpus/contents/documents"))
            .respond_with(ResponseTemplate::new(404).set_body_string(MISSING_PATH_BODY))
            .mount(&server)
            .await;

        let entries = client(&server)
            .list_directory("acme/corpus", "main", "documents", None)
            .await
            .unwrap();
        assert!(entries.is_empty());
    }

    /// Same rule for a file read: "the branch is gone" must not arrive as
    /// "the file does not exist", which a caller would answer by creating it.
    #[tokio::test]
    async fn missing_ref_does_not_read_as_a_missing_file() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path_matcher("/repos/acme/corpus/contents/wet/a.yaml"))
            .respond_with(ResponseTemplate::new(404).set_body_string(MISSING_REF_BODY))
            .mount(&server)
            .await;

        let err = client(&server)
            .fetch_file_with_sha("acme/corpus", "pr9999", "wet/a.yaml", None)
            .await
            .expect_err("a missing ref must fail the read");
        assert!(
            matches!(err, GithubError::Api { status: 404, .. }),
            "unexpected error: {err}"
        );
    }

    #[tokio::test]
    async fn missing_file_is_still_none() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path_matcher("/repos/acme/corpus/contents/wet/a.yaml"))
            .respond_with(ResponseTemplate::new(404).set_body_string(MISSING_PATH_BODY))
            .mount(&server)
            .await;

        assert!(client(&server)
            .fetch_file_with_sha("acme/corpus", "main", "wet/a.yaml", None)
            .await
            .unwrap()
            .is_none());
    }

    /// The rate-limit payoff: the second read of an unchanged file carries
    /// `If-None-Match`, GitHub answers 304 (free), and the caller still gets
    /// the body — never `None`.
    #[tokio::test]
    async fn repeat_file_read_revalidates_and_304_serves_the_cached_body() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path_matcher("/repos/acme/corpus/contents/wet/a.yaml"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("etag", "\"v1\"")
                    .set_body_json(file_body("wet/a.yaml", "blob1", "$id: a\n")),
            )
            .up_to_n_times(1)
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path_matcher("/repos/acme/corpus/contents/wet/a.yaml"))
            .and(header("if-none-match", "\"v1\""))
            .respond_with(ResponseTemplate::new(304))
            .expect(1)
            .mount(&server)
            .await;

        let c = client(&server);
        let first = c
            .fetch_file_with_sha("acme/corpus", "main", "wet/a.yaml", None)
            .await
            .unwrap();
        assert_eq!(first, Some(("$id: a\n".to_string(), "blob1".to_string())));

        let second = c
            .fetch_file_with_sha("acme/corpus", "main", "wet/a.yaml", None)
            .await
            .unwrap();
        assert_eq!(
            second,
            Some(("$id: a\n".to_string(), "blob1".to_string())),
            "a 304 must serve the cached body, not an empty answer"
        );
    }

    /// A 304 on a *listing* is the dangerous one: read as an empty body it
    /// becomes an empty directory, and the collision check goes blind.
    #[tokio::test]
    async fn repeat_directory_listing_304_serves_the_cached_entries() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path_matcher("/repos/acme/corpus/contents/documents"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("etag", "\"d1\"")
                    .set_body_json(dir_body(&[("rapport.md", "file"), ("bijlagen", "dir")])),
            )
            .up_to_n_times(1)
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path_matcher("/repos/acme/corpus/contents/documents"))
            .and(header("if-none-match", "\"d1\""))
            .respond_with(ResponseTemplate::new(304))
            .expect(1)
            .mount(&server)
            .await;

        let c = client(&server);
        let first = c
            .list_directory("acme/corpus", "main", "documents", None)
            .await
            .unwrap();
        assert_eq!(first.len(), 2);

        let second = c
            .list_directory("acme/corpus", "main", "documents", None)
            .await
            .unwrap();
        let names: Vec<&str> = second.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(
            names,
            vec!["rapport.md", "bijlagen"],
            "a 304 must serve the cached listing, not an empty directory"
        );
    }

    /// The raw representation (used for the >1 MiB implements index) has the
    /// same contract.
    #[tokio::test]
    async fn repeat_raw_read_304_serves_the_cached_body() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path_matcher("/repos/acme/corpus/contents/index.json"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("etag", "\"r1\"")
                    .set_body_string("{\"version\":1}"),
            )
            .up_to_n_times(1)
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path_matcher("/repos/acme/corpus/contents/index.json"))
            .and(header("if-none-match", "\"r1\""))
            .respond_with(ResponseTemplate::new(304))
            .expect(1)
            .mount(&server)
            .await;

        let c = client(&server);
        assert_eq!(
            c.fetch_file_raw("acme/corpus", "main", "index.json", None)
                .await
                .unwrap(),
            "{\"version\":1}"
        );
        assert_eq!(
            c.fetch_file_raw("acme/corpus", "main", "index.json", None)
                .await
                .unwrap(),
            "{\"version\":1}",
            "a 304 must serve the cached raw body"
        );
    }

    /// A 304 answered to a request that carried no `If-None-Match` leaves us
    /// with nothing to serve. Retry unconditionally rather than report the
    /// file as gone; only a server that 304s that retry too gets an error.
    #[tokio::test]
    async fn unprompted_304_is_retried_instead_of_read_as_absent() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path_matcher("/repos/acme/corpus/contents/wet/a.yaml"))
            .respond_with(ResponseTemplate::new(304))
            .up_to_n_times(1)
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path_matcher("/repos/acme/corpus/contents/wet/a.yaml"))
            .respond_with(ResponseTemplate::new(200).set_body_json(file_body(
                "wet/a.yaml",
                "b1",
                "$id: a\n",
            )))
            .expect(1)
            .mount(&server)
            .await;

        let got = client(&server)
            .fetch_file_with_sha("acme/corpus", "main", "wet/a.yaml", None)
            .await
            .unwrap();
        assert_eq!(got, Some(("$id: a\n".to_string(), "b1".to_string())));
    }

    /// A server that keeps 304-ing an unconditional request has given us no
    /// body at all — that is an error, never "the file does not exist".
    #[tokio::test]
    async fn persistent_unprompted_304_errors_rather_than_reporting_absence() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path_matcher("/repos/acme/corpus/contents/wet/a.yaml"))
            .respond_with(ResponseTemplate::new(304))
            .expect(2)
            .mount(&server)
            .await;

        let err = client(&server)
            .fetch_file_with_sha("acme/corpus", "main", "wet/a.yaml", None)
            .await
            .expect_err("a bodyless 304 is not an answer");
        assert!(
            matches!(err, GithubError::Api { status: 304, .. }),
            "unexpected error: {err}"
        );
    }

    /// Cache entries are per token identity: a read authenticated as another
    /// principal must not revalidate against — let alone be answered from —
    /// the first principal's entry.
    #[tokio::test]
    async fn a_second_token_does_not_reuse_the_first_tokens_cache_entry() {
        let server = MockServer::start().await;
        // Any conditional request on token-b is a bug; assert it never happens.
        Mock::given(method("GET"))
            .and(path_matcher("/repos/acme/corpus/contents/wet/a.yaml"))
            .and(header("authorization", "Bearer token-b"))
            .and(header_exists("if-none-match"))
            .respond_with(ResponseTemplate::new(500))
            .expect(0)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path_matcher("/repos/acme/corpus/contents/wet/a.yaml"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("etag", "\"v1\"")
                    .set_body_json(file_body("wet/a.yaml", "b1", "$id: a\n")),
            )
            .mount(&server)
            .await;

        let c = client(&server);
        c.fetch_file_with_sha("acme/corpus", "main", "wet/a.yaml", Some("token-a"))
            .await
            .unwrap();
        c.fetch_file_with_sha("acme/corpus", "main", "wet/a.yaml", Some("token-b"))
            .await
            .unwrap();
    }

    /// The cache is bounded: a body that no longer fits the budget is
    /// evicted, and the next read goes out unconditionally (a full 200)
    /// instead of revalidating an ETag whose body we threw away.
    #[tokio::test]
    async fn an_evicted_body_is_re_fetched_without_a_conditional_header() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path_matcher("/repos/acme/corpus/contents/wet/a.yaml"))
            .and(header_exists("if-none-match"))
            .respond_with(ResponseTemplate::new(304))
            .expect(0)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path_matcher("/repos/acme/corpus/contents/wet/a.yaml"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("etag", "\"v1\"")
                    .set_body_json(file_body("wet/a.yaml", "b1", "$id: a\n")),
            )
            .expect(2)
            .mount(&server)
            .await;

        let mut c = client(&server);
        // Budget smaller than the body: it is never worth caching.
        c.set_cache_budget_bytes(4);
        for _ in 0..2 {
            let got = c
                .fetch_file_with_sha("acme/corpus", "main", "wet/a.yaml", None)
                .await
                .unwrap();
            assert_eq!(got, Some(("$id: a\n".to_string(), "b1".to_string())));
        }
    }

    /// A subtree sha is the identity a precomputed index is checked against,
    /// so "the ref/path is unknown" (`Ok(None)`) must stay distinguishable
    /// from a real sha — and cost one call per path component.
    #[tokio::test]
    async fn subtree_sha_walks_components_and_reports_an_unknown_path_as_none() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path_matcher("/repos/acme/corpus/git/trees/main"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "sha": "roottree",
                "truncated": false,
                "tree": [
                    {"path": "regulation", "type": "tree", "sha": "regtree"},
                    {"path": "README.md", "type": "blob", "sha": "readme"},
                ]
            })))
            .mount(&server)
            .await;

        let c = client(&server);
        assert_eq!(
            c.subtree_sha("acme/corpus", "main", "regulation", None)
                .await
                .unwrap(),
            Some("regtree".to_string())
        );
        assert_eq!(
            c.subtree_sha("acme/corpus", "main", "elders", None)
                .await
                .unwrap(),
            None,
            "an absent directory has no sha; that is not the same as a mismatch"
        );
        assert_eq!(
            c.subtree_sha("acme/corpus", "main", "", None)
                .await
                .unwrap(),
            Some("roottree".to_string())
        );
    }

    /// A subtree lookup that fails for any other reason (rate limit, 5xx)
    /// must error: an unreadable sha is not a differing sha.
    #[tokio::test]
    async fn subtree_sha_surfaces_a_rate_limited_lookup_as_an_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path_matcher("/repos/acme/corpus/git/trees/main"))
            .respond_with(
                ResponseTemplate::new(403)
                    .insert_header("x-ratelimit-remaining", "0")
                    .set_body_string("API rate limit exceeded"),
            )
            .mount(&server)
            .await;

        let err = client(&server)
            .subtree_sha("acme/corpus", "main", "regulation", None)
            .await
            .expect_err("a 403 is not an answer");
        assert!(
            matches!(err, GithubError::Api { status: 403, .. }),
            "unexpected error: {err}"
        );
    }

    /// A missing ref has no tree at all — `Ok(None)`, so callers fall back
    /// rather than believing a sha they never got.
    #[tokio::test]
    async fn subtree_sha_of_a_missing_ref_is_none() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path_matcher("/repos/acme/corpus/git/trees/pr9999"))
            .respond_with(ResponseTemplate::new(404).set_body_string(MISSING_PATH_BODY))
            .mount(&server)
            .await;

        assert_eq!(
            client(&server)
                .subtree_sha("acme/corpus", "pr9999", "regulation", None)
                .await
                .unwrap(),
            None
        );
    }
}
