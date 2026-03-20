//! GitHub API client for fetching regulation files from remote repositories.
//!
//! Uses the GitHub Trees API for directory listing and Contents API for file content.
//! Supports ETag-based caching and rate limit tracking.

#[cfg(feature = "github")]
mod inner {
    use std::collections::HashMap;

    use reqwest::header::{
        HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION, IF_NONE_MATCH, USER_AGENT,
    };
    use serde::Deserialize;

    use crate::error::{CorpusError, Result};
    use crate::models::GitHubSource;

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
                .build()
                .map_err(|e| CorpusError::Config(format!("Failed to create HTTP client: {}", e)))?;

            Ok(Self {
                client,
                etag_cache: HashMap::new(),
                rate_limit_remaining: None,
            })
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
                "https://api.github.com/repos/{}/git/trees/{}?recursive=1",
                repo, branch
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
                tracing::warn!(
                    repo = %repo,
                    "Tree response was truncated — some files may be missing"
                );
            }

            let yaml_files: Vec<String> = tree
                .tree
                .into_iter()
                .filter(|e| {
                    e.entry_type == "blob"
                        && e.path.ends_with(".yaml")
                        && (base_path.is_empty() || e.path.starts_with(base_path))
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
                "https://api.github.com/repos/{}/contents/{}?ref={}",
                repo, path, branch
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
    }
}

#[cfg(feature = "github")]
pub use inner::*;
