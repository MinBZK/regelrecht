//! GitHub API client for fetching regulation files from remote repositories.
//!
//! Uses the GitHub Trees API for directory listing and Contents API for file content.
//! Supports ETag-based caching and rate limit tracking.

#[cfg(feature = "github")]
mod inner {
    use std::collections::{HashMap, HashSet};

    use base64::Engine;
    use reqwest::header::{
        HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION, CONTENT_TYPE, IF_NONE_MATCH, USER_AGENT,
    };
    use serde::{Deserialize, Serialize};

    use crate::error::{CorpusError, Result};
    use crate::models::GitHubSource;

    /// Commit identity used on Contents/Git Data API writes. Both
    /// `committer` and `author` accept this shape — currently we set them
    /// to the same value so the human editor shows up on both sides of the
    /// git commit, and rely on the GitHub token's account for the actual
    /// push credentials.
    #[derive(Debug, Clone, Serialize)]
    pub struct Committer {
        pub name: String,
        pub email: String,
    }

    /// Single entry returned by a Contents API directory listing. Only the
    /// fields the backend needs are pulled off the JSON; GitHub returns
    /// quite a bit more (url, html_url, size, …) that we don't use.
    #[derive(Debug, Clone)]
    pub struct DirectoryEntry {
        pub name: String,
        pub path: String,
        /// `"file"` or `"dir"`. GitHub also reports `"submodule"` and
        /// `"symlink"`; the backend filters to `"file"` for listing.
        pub entry_type: String,
    }

    /// Raw shape of the Contents API response for a single path. Used both
    /// for file reads (where `type == "file"`) and directory listings
    /// (returned as a JSON array of these).
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

    /// Subset of the Contents-API PUT response we care about. The full
    /// response carries `content` and `commit` blocks; we only need the
    /// new file SHA so callers can chain another write without re-reading.
    #[derive(Debug, Deserialize)]
    struct PutResponse {
        content: PutContent,
    }
    #[derive(Debug, Deserialize)]
    struct PutContent {
        sha: String,
    }

    /// Refs-API response for `GET /git/ref/heads/{branch}`. We only need
    /// the object SHA to use as the base for a new branch.
    #[derive(Debug, Deserialize)]
    struct RefResponse {
        object: RefObject,
    }
    #[derive(Debug, Deserialize)]
    struct RefObject {
        sha: String,
    }

    /// Result of fetching a GitHub source.
    #[derive(Debug)]
    pub enum FetchResult {
        /// New or updated content was fetched.
        Fetched(Vec<FetchedFile>),
        /// Content has not changed since last fetch (HTTP 304).
        NotModified,
    }

    /// A fetched file from GitHub.
    #[derive(Debug, Clone)]
    pub struct FetchedFile {
        pub path: String,
        pub content: String,
    }

    /// GitHub API response for the Trees endpoint.
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
    }

    /// GitHub fetcher with ETag caching and rate limit awareness.
    pub struct GitHubFetcher {
        client: reqwest::Client,
        /// API base URL — overridable for tests against a wiremock server.
        /// Production default is `"https://api.github.com"`. No trailing
        /// slash; all callers prefix their `/...` path themselves.
        api_base: String,
        /// ETag cache: URL → ETag value
        etag_cache: HashMap<String, String>,
        /// Remaining API calls before rate limit
        rate_limit_remaining: Option<u32>,
    }

    impl GitHubFetcher {
        /// Create a new fetcher.
        pub fn new() -> Result<Self> {
            let client = reqwest::Client::builder()
                .user_agent("regelrecht-corpus/0.1")
                .connect_timeout(std::time::Duration::from_secs(30))
                .timeout(std::time::Duration::from_secs(60))
                .build()
                .map_err(|e| CorpusError::Config(format!("Failed to create HTTP client: {}", e)))?;

            Ok(Self {
                client,
                api_base: "https://api.github.com".to_string(),
                etag_cache: HashMap::new(),
                rate_limit_remaining: None,
            })
        }

        /// Override the API base URL — for tests that point at a wiremock
        /// server. Production callers use [`GitHubFetcher::new`] which
        /// already points at `api.github.com`.
        pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
            self.set_base_url(base_url);
            self
        }

        /// In-place variant of [`with_base_url`] for call sites that already
        /// hold the fetcher by `&mut` (e.g. `GitHubApiBackend::with_api_base`
        /// reaching through its `Mutex<Inner>`).
        pub fn set_base_url(&mut self, base_url: impl Into<String>) {
            self.api_base = base_url.into().trim_end_matches('/').to_string();
        }

        /// Fetch all YAML regulation files from a GitHub source.
        ///
        /// Returns `FetchResult::NotModified` when the tree has not changed
        /// (HTTP 304) so callers can preserve previously loaded data.
        pub async fn fetch_source(
            &mut self,
            source: &GitHubSource,
            token: Option<&str>,
        ) -> Result<FetchResult> {
            let base_path = source.path.as_deref().unwrap_or("");

            // Step 1: Get the tree to find all YAML files
            let yaml_paths = match self
                .list_yaml_files(
                    &source.full_repo(),
                    source.effective_ref(),
                    base_path,
                    token,
                )
                .await?
            {
                Some(paths) => paths,
                None => return Ok(FetchResult::NotModified),
            };

            if yaml_paths.is_empty() {
                return Ok(FetchResult::Fetched(Vec::new()));
            }

            // Step 2: Fetch each YAML file's content
            let mut files = Vec::new();
            for path in &yaml_paths {
                match self
                    .fetch_file_content(&source.full_repo(), source.effective_ref(), path, token)
                    .await
                {
                    Ok(content) => {
                        files.push(FetchedFile {
                            path: path.clone(),
                            content,
                        });
                    }
                    Err(e) => {
                        tracing::warn!(path = %path, error = %e, "Failed to fetch file, skipping");
                    }
                }
            }

            Ok(FetchResult::Fetched(files))
        }

        /// Fetch only laws matching the given `$id` set from a GitHub source.
        ///
        /// Uses the Trees API (1 call) to discover file paths, matches them
        /// against `law_ids` by extracting the law directory name from the path
        /// (`{base}/{layer}/{law_id}/{date}.yaml`), picks the best version per
        /// law (latest `valid_from` ≤ today), and fetches only those files.
        pub async fn fetch_source_filtered(
            &mut self,
            source: &GitHubSource,
            token: Option<&str>,
            law_ids: &HashSet<String>,
        ) -> Result<FetchResult> {
            if law_ids.is_empty() {
                return Ok(FetchResult::Fetched(Vec::new()));
            }

            let base_path = source.path.as_deref().unwrap_or("");

            let all_paths = match self
                .list_yaml_files(
                    &source.full_repo(),
                    source.effective_ref(),
                    base_path,
                    token,
                )
                .await?
            {
                Some(paths) => paths,
                None => return Ok(FetchResult::NotModified),
            };

            // Group paths by law_id, keeping only those in the filter set.
            // Path format: {base_path}/{layer}/{law_id}/{date}.yaml
            let prefix = if base_path.is_empty() {
                String::new()
            } else {
                format!("{}/", base_path)
            };

            let today = crate::source_map::today_str();
            let mut best_per_law: HashMap<String, String> = HashMap::new();

            for path in &all_paths {
                let rel = if prefix.is_empty() {
                    path.as_str()
                } else {
                    match path.strip_prefix(&prefix) {
                        Some(r) => r,
                        None => continue,
                    }
                };

                let parts: Vec<&str> = rel.split('/').collect();
                if parts.len() < 3 {
                    continue;
                }

                let law_id = parts[parts.len() - 2];
                if !law_ids.contains(law_id) {
                    continue;
                }

                // Extract date from filename (YYYY-MM-DD.yaml)
                let filename = parts[parts.len() - 1];
                let new_date = filename.strip_suffix(".yaml");

                if let Some(existing_path) = best_per_law.get(law_id) {
                    let existing_filename = existing_path.rsplit('/').next().unwrap_or("");
                    let existing_date = existing_filename.strip_suffix(".yaml");

                    let new_wins =
                        crate::source_map::pick_best_version(existing_date, new_date, &today);

                    if new_wins {
                        best_per_law.insert(law_id.to_string(), path.clone());
                    }
                } else {
                    best_per_law.insert(law_id.to_string(), path.clone());
                }
            }

            tracing::info!(
                matched = best_per_law.len(),
                requested = law_ids.len(),
                "fetching filtered laws from GitHub"
            );

            let mut files = Vec::new();
            for path in best_per_law.values() {
                match self
                    .fetch_file_content(&source.full_repo(), source.effective_ref(), path, token)
                    .await
                {
                    Ok(content) => {
                        files.push(FetchedFile {
                            path: path.clone(),
                            content,
                        });
                    }
                    Err(e) => {
                        tracing::warn!(path = %path, error = %e, "Failed to fetch file, skipping");
                    }
                }
            }

            Ok(FetchResult::Fetched(files))
        }

        /// List all YAML files in a repo path using the Trees API.
        ///
        /// Returns `None` when the server responds with 304 Not Modified,
        /// indicating the tree has not changed since the last fetch.
        async fn list_yaml_files(
            &mut self,
            repo: &str,
            branch: &str,
            base_path: &str,
            token: Option<&str>,
        ) -> Result<Option<Vec<String>>> {
            let url = format!(
                "{}/repos/{}/git/trees/{}?recursive=1",
                self.api_base, repo, branch
            );

            let mut headers = self.default_headers(token);

            // Use ETag for caching
            if let Some(etag) = self.etag_cache.get(&url) {
                headers.insert(
                    IF_NONE_MATCH,
                    HeaderValue::from_str(etag).unwrap_or_else(|_| HeaderValue::from_static("")),
                );
            }

            let response = self
                .client
                .get(&url)
                .headers(headers)
                .send()
                .await
                .map_err(|e| CorpusError::Git(format!("GitHub API request failed: {}", e)))?;

            self.track_rate_limit(&response);

            if response.status() == reqwest::StatusCode::NOT_MODIFIED {
                tracing::debug!(repo = %repo, "Tree unchanged (ETag match)");
                return Ok(None);
            }

            if !response.status().is_success() {
                return Err(CorpusError::Git(format!(
                    "GitHub Trees API returned {}: {}",
                    response.status(),
                    response.text().await.unwrap_or_default()
                )));
            }

            // Store new ETag
            if let Some(etag) = response.headers().get("etag") {
                if let Ok(etag_str) = etag.to_str() {
                    self.etag_cache.insert(url.clone(), etag_str.to_string());
                }
            }

            let tree: TreeResponse = response
                .json()
                .await
                .map_err(|e| CorpusError::Git(format!("Failed to parse tree response: {}", e)))?;

            if tree.truncated {
                return Err(CorpusError::Git(format!(
                    "GitHub Trees API response for '{}' was truncated — repository has too many files. \
                     Reduce the number of files or use a narrower `path` in the registry manifest.",
                    repo
                )));
            }

            let yaml_files: Vec<String> = tree
                .tree
                .into_iter()
                .filter(|e| {
                    e.entry_type == "blob"
                        && e.path.ends_with(".yaml")
                        && (base_path.is_empty()
                            || e.path == base_path
                            || e.path.starts_with(&format!("{}/", base_path)))
                })
                .map(|e| e.path)
                .collect();

            tracing::debug!(
                repo = %repo,
                count = yaml_files.len(),
                "Found YAML files in tree"
            );

            Ok(Some(yaml_files))
        }

        /// Fetch a single file's content using the Contents API.
        async fn fetch_file_content(
            &mut self,
            repo: &str,
            branch: &str,
            path: &str,
            token: Option<&str>,
        ) -> Result<String> {
            let url = format!(
                "{}/repos/{}/contents/{}?ref={}",
                self.api_base, repo, path, branch
            );

            let mut headers = self.default_headers(token);
            // Request raw content to avoid base64 decoding
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
                .map_err(|e| CorpusError::Git(format!("GitHub API request failed: {}", e)))?;

            self.track_rate_limit(&response);

            if !response.status().is_success() {
                return Err(CorpusError::Git(format!(
                    "GitHub Contents API returned {} for {}: {}",
                    response.status(),
                    path,
                    response.text().await.unwrap_or_default()
                )));
            }

            response
                .text()
                .await
                .map_err(|e| CorpusError::Git(format!("Failed to read response body: {}", e)))
        }

        /// Build default headers for GitHub API requests.
        fn default_headers(&self, token: Option<&str>) -> HeaderMap {
            let mut headers = HeaderMap::new();
            headers.insert(
                USER_AGENT,
                HeaderValue::from_static("regelrecht-corpus/0.1"),
            );
            headers.insert(
                ACCEPT,
                HeaderValue::from_static("application/vnd.github+json"),
            );
            headers.insert(
                "X-GitHub-Api-Version",
                HeaderValue::from_static("2022-11-28"),
            );

            if let Some(token) = token {
                if let Ok(val) = HeaderValue::from_str(&format!("Bearer {}", token)) {
                    headers.insert(AUTHORIZATION, val);
                }
            }

            headers
        }

        /// Track rate limit from response headers.
        fn track_rate_limit(&mut self, response: &reqwest::Response) {
            if let Some(remaining) = response.headers().get("x-ratelimit-remaining") {
                if let Ok(val) = remaining.to_str() {
                    if let Ok(n) = val.parse::<u32>() {
                        self.rate_limit_remaining = Some(n);
                        if n < 100 {
                            tracing::warn!(remaining = n, "GitHub API rate limit running low");
                        }
                    }
                }
            }
        }

        /// Get the current rate limit remaining (if known).
        pub fn rate_limit_remaining(&self) -> Option<u32> {
            self.rate_limit_remaining
        }

        // -----------------------------------------------------------------
        // Backend-oriented API (used by GitHubApiBackend; no ETag cache —
        // backend reads want the current state of the branch on every
        // call, not the cached one).
        // -----------------------------------------------------------------

        /// Fetch a single file's content **plus** its blob SHA. The SHA is
        /// what the Contents API expects on a subsequent update PUT for
        /// optimistic concurrency. Returns `Ok(None)` on 404.
        pub async fn fetch_file_with_sha(
            &mut self,
            repo: &str,
            branch: &str,
            path: &str,
            token: Option<&str>,
        ) -> Result<Option<(String, String)>> {
            let url = format!(
                "{}/repos/{}/contents/{}?ref={}",
                self.api_base, repo, path, branch
            );
            let headers = self.default_headers(token);
            let response = self
                .client
                .get(&url)
                .headers(headers)
                .send()
                .await
                .map_err(|e| CorpusError::Git(format!("GitHub API request failed: {}", e)))?;
            self.track_rate_limit(&response);

            if response.status() == reqwest::StatusCode::NOT_FOUND {
                return Ok(None);
            }
            if !response.status().is_success() {
                return Err(CorpusError::Git(format!(
                    "GitHub Contents API returned {} for {}: {}",
                    response.status(),
                    path,
                    response.text().await.unwrap_or_default()
                )));
            }

            let item: ContentsItem = response.json().await.map_err(|e| {
                CorpusError::Git(format!("Failed to parse contents response: {}", e))
            })?;
            if item.entry_type != "file" {
                return Err(CorpusError::Git(format!(
                    "Path '{}' is a {}, not a file",
                    path, item.entry_type
                )));
            }
            let content = decode_contents_payload(&item)?;
            Ok(Some((content, item.sha)))
        }

        /// List a directory via the Contents API. For a directory the
        /// response is a JSON array of [`ContentsItem`]; for a missing
        /// directory we return an empty list (404). Files only — sub-
        /// directories, symlinks and submodules are filtered out by the
        /// caller via [`DirectoryEntry::entry_type`].
        pub async fn list_directory(
            &mut self,
            repo: &str,
            branch: &str,
            dir: &str,
            token: Option<&str>,
        ) -> Result<Vec<DirectoryEntry>> {
            let url = format!(
                "{}/repos/{}/contents/{}?ref={}",
                self.api_base, repo, dir, branch
            );
            let headers = self.default_headers(token);
            let response = self
                .client
                .get(&url)
                .headers(headers)
                .send()
                .await
                .map_err(|e| CorpusError::Git(format!("GitHub API request failed: {}", e)))?;
            self.track_rate_limit(&response);

            // 404 on a directory listing is the "no scenarios yet" path —
            // same shape as the local LocalBackend.list_files when the
            // directory doesn't exist.
            if response.status() == reqwest::StatusCode::NOT_FOUND {
                return Ok(Vec::new());
            }
            if !response.status().is_success() {
                return Err(CorpusError::Git(format!(
                    "GitHub Contents API returned {} for {}: {}",
                    response.status(),
                    dir,
                    response.text().await.unwrap_or_default()
                )));
            }

            // The endpoint returns an array for directories and a single
            // object for files. We only call this for directories, but if
            // someone calls it on a file path we still return Ok([]) (the
            // file is not a directory).
            let body = response.text().await.map_err(|e| {
                CorpusError::Git(format!("Failed to read directory listing: {}", e))
            })?;
            let trimmed = body.trim_start();
            if !trimmed.starts_with('[') {
                tracing::debug!(dir = %dir, "list_directory: path is not a directory");
                return Ok(Vec::new());
            }
            let items: Vec<ContentsItem> = serde_json::from_str(&body).map_err(|e| {
                CorpusError::Git(format!("Failed to parse directory listing: {}", e))
            })?;
            Ok(items
                .into_iter()
                .map(|i| DirectoryEntry {
                    name: i.name,
                    path: i.path,
                    entry_type: i.entry_type,
                })
                .collect())
        }

        /// Upsert a file via Contents API PUT. Pass `base_sha = None` to
        /// create a new file, `Some(sha)` to update an existing one. The
        /// branch must exist (see [`ensure_branch`]). Returns the new blob
        /// SHA so callers can chain writes without an extra GET.
        ///
        /// Maps 409 to [`CorpusError::Conflict`] so backends can detect a
        /// concurrent-write race and retry; everything else is `Git`.
        #[allow(clippy::too_many_arguments)]
        pub async fn put_file(
            &mut self,
            repo: &str,
            branch: &str,
            path: &str,
            content: &str,
            base_sha: Option<&str>,
            committer: &Committer,
            message: &str,
            token: Option<&str>,
        ) -> Result<String> {
            let url = format!("{}/repos/{}/contents/{}", self.api_base, repo, path);
            let mut body = serde_json::json!({
                "message": message,
                "content": base64::engine::general_purpose::STANDARD.encode(content.as_bytes()),
                "branch": branch,
                "committer": committer,
                "author": committer,
            });
            if let Some(sha) = base_sha {
                body["sha"] = serde_json::Value::String(sha.to_string());
            }

            let mut headers = self.default_headers(token);
            headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

            let response = self
                .client
                .put(&url)
                .headers(headers)
                .body(body.to_string())
                .send()
                .await
                .map_err(|e| CorpusError::Git(format!("GitHub API request failed: {}", e)))?;
            self.track_rate_limit(&response);

            let status = response.status();
            if status == reqwest::StatusCode::CONFLICT {
                return Err(CorpusError::Conflict(format!(
                    "Contents API PUT {} hit a 409 (stale sha)",
                    path
                )));
            }
            if !status.is_success() {
                return Err(CorpusError::Git(format!(
                    "GitHub Contents API PUT {} returned {}: {}",
                    path,
                    status,
                    response.text().await.unwrap_or_default()
                )));
            }
            let parsed: PutResponse = response
                .json()
                .await
                .map_err(|e| CorpusError::Git(format!("Failed to parse PUT response: {}", e)))?;
            Ok(parsed.content.sha)
        }

        /// Delete a file via Contents API DELETE. Requires the current
        /// blob SHA. 404 is treated as "already gone" (idempotent — same
        /// shape as [`crate::backend::RepoBackend::delete_file`]).
        #[allow(clippy::too_many_arguments)]
        pub async fn delete_file_via_api(
            &mut self,
            repo: &str,
            branch: &str,
            path: &str,
            sha: &str,
            committer: &Committer,
            message: &str,
            token: Option<&str>,
        ) -> Result<()> {
            let url = format!("{}/repos/{}/contents/{}", self.api_base, repo, path);
            let body = serde_json::json!({
                "message": message,
                "sha": sha,
                "branch": branch,
                "committer": committer,
                "author": committer,
            });

            let mut headers = self.default_headers(token);
            headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

            let response = self
                .client
                .delete(&url)
                .headers(headers)
                .body(body.to_string())
                .send()
                .await
                .map_err(|e| CorpusError::Git(format!("GitHub API request failed: {}", e)))?;
            self.track_rate_limit(&response);

            let status = response.status();
            if status == reqwest::StatusCode::NOT_FOUND {
                return Ok(());
            }
            if status == reqwest::StatusCode::CONFLICT {
                return Err(CorpusError::Conflict(format!(
                    "Contents API DELETE {} hit a 409 (stale sha)",
                    path
                )));
            }
            if !status.is_success() {
                return Err(CorpusError::Git(format!(
                    "GitHub Contents API DELETE {} returned {}: {}",
                    path,
                    status,
                    response.text().await.unwrap_or_default()
                )));
            }
            Ok(())
        }

        /// Check whether a branch exists. Returns `Ok(true)` on 200,
        /// `Ok(false)` on 404, error on anything else.
        pub async fn branch_exists(
            &mut self,
            repo: &str,
            branch: &str,
            token: Option<&str>,
        ) -> Result<bool> {
            let url = format!("{}/repos/{}/git/ref/heads/{}", self.api_base, repo, branch);
            let headers = self.default_headers(token);
            let response = self
                .client
                .get(&url)
                .headers(headers)
                .send()
                .await
                .map_err(|e| CorpusError::Git(format!("GitHub API request failed: {}", e)))?;
            self.track_rate_limit(&response);

            let status = response.status();
            if status.is_success() {
                return Ok(true);
            }
            if status == reqwest::StatusCode::NOT_FOUND {
                return Ok(false);
            }
            Err(CorpusError::Git(format!(
                "GitHub Refs API returned {} for {}@{}: {}",
                status,
                repo,
                branch,
                response.text().await.unwrap_or_default()
            )))
        }

        /// Create `branch` pointing at the tip of `base_branch`. The base
        /// branch must already exist; the target branch must NOT exist
        /// (GitHub returns 422 otherwise — surfaced as `Git`).
        pub async fn create_branch(
            &mut self,
            repo: &str,
            branch: &str,
            base_branch: &str,
            token: Option<&str>,
        ) -> Result<()> {
            // 1) resolve the base ref's SHA
            let base_url = format!(
                "{}/repos/{}/git/ref/heads/{}",
                self.api_base, repo, base_branch
            );
            let headers = self.default_headers(token);
            let response = self
                .client
                .get(&base_url)
                .headers(headers)
                .send()
                .await
                .map_err(|e| CorpusError::Git(format!("GitHub API request failed: {}", e)))?;
            self.track_rate_limit(&response);
            if !response.status().is_success() {
                return Err(CorpusError::Git(format!(
                    "Could not resolve base branch {}@{}: {}",
                    repo,
                    base_branch,
                    response.text().await.unwrap_or_default()
                )));
            }
            let parsed: RefResponse = response
                .json()
                .await
                .map_err(|e| CorpusError::Git(format!("Failed to parse base ref: {}", e)))?;

            // 2) POST a new ref pointing at the same SHA
            let post_url = format!("{}/repos/{}/git/refs", self.api_base, repo);
            let body = serde_json::json!({
                "ref": format!("refs/heads/{}", branch),
                "sha": parsed.object.sha,
            });
            let mut headers = self.default_headers(token);
            headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
            let response = self
                .client
                .post(&post_url)
                .headers(headers)
                .body(body.to_string())
                .send()
                .await
                .map_err(|e| CorpusError::Git(format!("GitHub API request failed: {}", e)))?;
            self.track_rate_limit(&response);
            if !response.status().is_success() {
                return Err(CorpusError::Git(format!(
                    "GitHub Refs API POST returned {} for {}@{}: {}",
                    response.status(),
                    repo,
                    branch,
                    response.text().await.unwrap_or_default()
                )));
            }
            Ok(())
        }
    }

    /// Decode a Contents-API response's content payload. The API returns
    /// either base64-encoded content (default `encoding: "base64"`) or a
    /// raw string when `application/vnd.github.raw+json` was requested —
    /// but the JSON path always gives us base64, so we only handle that.
    /// Files larger than 1 MiB come back without `content` (encoding
    /// `"none"`); for those the Git Blob API is the documented route.
    fn decode_contents_payload(item: &ContentsItem) -> Result<String> {
        let encoding = item.encoding.as_deref().unwrap_or("base64");
        if encoding != "base64" {
            return Err(CorpusError::Git(format!(
                "Contents API returned unsupported encoding '{}' for {} \
                 (large file? use the Blob API)",
                encoding, item.path
            )));
        }
        let content = item.content.as_deref().ok_or_else(|| {
            CorpusError::Git(format!(
                "Contents API returned no content for {} (possibly >1 MiB)",
                item.path
            ))
        })?;
        // The API wraps the base64 at 60 chars per line — strip whitespace
        // before decoding.
        let cleaned: String = content
            .chars()
            .filter(|c| !c.is_ascii_whitespace())
            .collect();
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(cleaned.as_bytes())
            .map_err(|e| {
                CorpusError::Git(format!("Base64 decode failed for {}: {}", item.path, e))
            })?;
        String::from_utf8(bytes)
            .map_err(|e| CorpusError::Git(format!("UTF-8 decode failed for {}: {}", item.path, e)))
    }
}

#[cfg(feature = "github")]
pub use inner::*;
