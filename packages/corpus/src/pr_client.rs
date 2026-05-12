//! GitHub Pull Request API client.
//!
//! A small, focused client for the three Pulls API calls the editor's
//! write-back path needs:
//! - `GET /repos/{owner}/{repo}/pulls?head={owner}:{branch}&state=open`
//!   to look for an open PR on a session branch.
//! - `POST /repos/{owner}/{repo}/pulls` to open a new PR.
//! - `PATCH /repos/{owner}/{repo}/pulls/{number}` to refresh title/body
//!   when a session adds more commits to an existing PR.
//!
//! Hand-rolled with `reqwest` rather than pulling in `octocrab` because the
//! surface is tiny and the corpus crate already speaks `reqwest` for the
//! Trees/Contents API. Reconsider if PR comments / review state get added.

#[cfg(feature = "github")]
mod inner {
    use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION, USER_AGENT};
    use serde::{Deserialize, Serialize};

    use crate::backend::PrInfo;
    use crate::error::{CorpusError, Result};

    /// Hand-rolled GitHub Pulls API client.
    pub struct PullRequestClient {
        client: reqwest::Client,
        /// Base URL of the GitHub API. Always `https://api.github.com` in
        /// production; overridable via [`PullRequestClient::with_base_url`]
        /// for tests that point at a wiremock server.
        base_url: String,
    }

    #[derive(Debug, Serialize)]
    struct CreatePrBody<'a> {
        title: &'a str,
        body: &'a str,
        head: &'a str,
        base: &'a str,
    }

    #[derive(Debug, Serialize)]
    struct UpdatePrBody<'a> {
        title: &'a str,
        body: &'a str,
    }

    #[derive(Debug, Deserialize)]
    struct PrResponse {
        number: u64,
        html_url: String,
    }

    impl PullRequestClient {
        /// Build a client. Uses the standard `regelrecht-corpus/0.1` UA so
        /// requests are traceable in GitHub audit logs the same way reads are.
        pub fn new() -> Result<Self> {
            // User-Agent is set per-request in `default_headers` so requests
            // remain traceable in GitHub audit logs without configuring it
            // twice on the client.
            let client = reqwest::Client::builder()
                .connect_timeout(std::time::Duration::from_secs(30))
                .timeout(std::time::Duration::from_secs(60))
                .build()
                .map_err(|e| {
                    CorpusError::Config(format!("failed to create PR HTTP client: {}", e))
                })?;
            Ok(Self {
                client,
                base_url: "https://api.github.com".to_string(),
            })
        }

        /// Override the API base URL. Intended for tests that point at a
        /// wiremock server. `base_url` should not include a trailing slash.
        pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
            self.base_url = base_url.into();
            self
        }

        /// Idempotently ensure there is an open PR for `head` against `base`.
        ///
        /// - If an open PR for `head` already exists, refresh its title/body
        ///   (so subsequent saves in the same session can update the
        ///   description with new context) and return its info.
        /// - Otherwise open a new PR.
        ///
        /// Returns the resulting [`PrInfo`]. Errors when the API call fails
        /// or the response can't be parsed; the caller surfaces those as a
        /// 5xx on the save endpoint.
        ///
        /// `head` is the branch name on the source repo (no `owner:` prefix
        /// — this client always pushes to and PRs from the same repo, so a
        /// fork-style `owner:branch` reference is unnecessary). The
        /// `head=...` query parameter on the list call is built internally
        /// using `repo_owner:head` per the GitHub API spec.
        // The seven-arg signature mirrors the GitHub Pulls API shape; bundling
        // them into a struct would just shuffle the same data into named
        // fields without making call sites clearer (there's only one caller,
        // and it builds the args from named struct fields anyway).
        #[allow(clippy::too_many_arguments)]
        pub async fn ensure_pr(
            &self,
            owner: &str,
            repo: &str,
            head: &str,
            base: &str,
            title: &str,
            body: &str,
            token: &str,
        ) -> Result<PrInfo> {
            if let Some(existing) = self.find_open_pr(owner, repo, head, token).await? {
                self.update_pr(owner, repo, existing.number, title, body, token)
                    .await
            } else {
                self.create_pr(owner, repo, head, base, title, body, token)
                    .await
            }
        }

        async fn find_open_pr(
            &self,
            owner: &str,
            repo: &str,
            head: &str,
            token: &str,
        ) -> Result<Option<PrResponse>> {
            // Per GitHub spec, `head` filter must be `owner:branch`.
            let url = format!(
                "{}/repos/{}/{}/pulls?state=open&head={}:{}",
                self.base_url, owner, repo, owner, head
            );

            let response = self
                .client
                .get(&url)
                .headers(self.default_headers(token))
                .send()
                .await
                .map_err(|e| CorpusError::Git(format!("PR list request failed: {}", e)))?;

            if !response.status().is_success() {
                return Err(CorpusError::Git(format!(
                    "GitHub PR list returned {}: {}",
                    response.status(),
                    response.text().await.unwrap_or_default()
                )));
            }

            let prs: Vec<PrResponse> = response
                .json()
                .await
                .map_err(|e| CorpusError::Git(format!("failed to parse PR list: {}", e)))?;

            // Multiple open PRs for the same head shouldn't happen in
            // practice (GitHub blocks duplicates), but if it does we pick
            // the first to keep behaviour deterministic.
            Ok(prs.into_iter().next())
        }

        #[allow(clippy::too_many_arguments)]
        async fn create_pr(
            &self,
            owner: &str,
            repo: &str,
            head: &str,
            base: &str,
            title: &str,
            body: &str,
            token: &str,
        ) -> Result<PrInfo> {
            let url = format!("{}/repos/{}/{}/pulls", self.base_url, owner, repo);
            let payload = CreatePrBody {
                title,
                body,
                head,
                base,
            };

            let response = self
                .client
                .post(&url)
                .headers(self.default_headers(token))
                .json(&payload)
                .send()
                .await
                .map_err(|e| CorpusError::Git(format!("PR create request failed: {}", e)))?;

            if !response.status().is_success() {
                return Err(CorpusError::Git(format!(
                    "GitHub PR create returned {}: {}",
                    response.status(),
                    response.text().await.unwrap_or_default()
                )));
            }

            let pr: PrResponse = response.json().await.map_err(|e| {
                CorpusError::Git(format!("failed to parse PR create response: {}", e))
            })?;

            Ok(PrInfo {
                number: pr.number,
                html_url: pr.html_url,
            })
        }

        async fn update_pr(
            &self,
            owner: &str,
            repo: &str,
            number: u64,
            title: &str,
            body: &str,
            token: &str,
        ) -> Result<PrInfo> {
            let url = format!(
                "{}/repos/{}/{}/pulls/{}",
                self.base_url, owner, repo, number
            );
            let payload = UpdatePrBody { title, body };

            let response = self
                .client
                .patch(&url)
                .headers(self.default_headers(token))
                .json(&payload)
                .send()
                .await
                .map_err(|e| CorpusError::Git(format!("PR update request failed: {}", e)))?;

            if !response.status().is_success() {
                return Err(CorpusError::Git(format!(
                    "GitHub PR update returned {}: {}",
                    response.status(),
                    response.text().await.unwrap_or_default()
                )));
            }

            let pr: PrResponse = response.json().await.map_err(|e| {
                CorpusError::Git(format!("failed to parse PR update response: {}", e))
            })?;

            Ok(PrInfo {
                number: pr.number,
                html_url: pr.html_url,
            })
        }

        fn default_headers(&self, token: &str) -> HeaderMap {
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
            if let Ok(val) = HeaderValue::from_str(&format!("Bearer {}", token)) {
                headers.insert(AUTHORIZATION, val);
            }
            headers
        }
    }

    #[cfg(test)]
    #[allow(clippy::unwrap_used)]
    mod tests {
        use super::*;
        use wiremock::matchers::{body_partial_json, header, method, path, query_param};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        fn client_for(server: &MockServer) -> PullRequestClient {
            PullRequestClient::new()
                .unwrap()
                .with_base_url(server.uri())
        }

        #[tokio::test]
        async fn ensure_pr_creates_when_none_exists() {
            let server = MockServer::start().await;

            // No existing PR for this head
            Mock::given(method("GET"))
                .and(path("/repos/org/repo/pulls"))
                .and(query_param("state", "open"))
                .and(query_param("head", "org:editor/session-abc"))
                .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
                .expect(1)
                .mount(&server)
                .await;

            // Create call returns 201 + new PR
            Mock::given(method("POST"))
                .and(path("/repos/org/repo/pulls"))
                .and(header("Authorization", "Bearer test-token"))
                .and(body_partial_json(serde_json::json!({
                    "head": "editor/session-abc",
                    "base": "main",
                    "title": "Editor session abc",
                })))
                .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
                    "number": 42,
                    "html_url": "https://github.com/org/repo/pull/42",
                })))
                .expect(1)
                .mount(&server)
                .await;

            let pr = client_for(&server)
                .ensure_pr(
                    "org",
                    "repo",
                    "editor/session-abc",
                    "main",
                    "Editor session abc",
                    "Submitted via Regelrecht editor",
                    "test-token",
                )
                .await
                .unwrap();

            assert_eq!(pr.number, 42);
            assert_eq!(pr.html_url, "https://github.com/org/repo/pull/42");
        }

        #[tokio::test]
        async fn ensure_pr_updates_existing_open_pr() {
            let server = MockServer::start().await;

            // Existing open PR returned by list call
            Mock::given(method("GET"))
                .and(path("/repos/org/repo/pulls"))
                .and(query_param("head", "org:editor/session-xyz"))
                .respond_with(
                    ResponseTemplate::new(200).set_body_json(serde_json::json!([{
                        "number": 7,
                        "html_url": "https://github.com/org/repo/pull/7",
                    }])),
                )
                .expect(1)
                .mount(&server)
                .await;

            // PATCH refreshes title/body and returns the same PR
            Mock::given(method("PATCH"))
                .and(path("/repos/org/repo/pulls/7"))
                .and(body_partial_json(serde_json::json!({
                    "title": "Editor session xyz (3 edits)",
                })))
                .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "number": 7,
                    "html_url": "https://github.com/org/repo/pull/7",
                })))
                .expect(1)
                .mount(&server)
                .await;

            // Reject any POST (create) — we must NOT open a new PR
            Mock::given(method("POST"))
                .and(path("/repos/org/repo/pulls"))
                .respond_with(ResponseTemplate::new(500))
                .expect(0)
                .mount(&server)
                .await;

            let pr = client_for(&server)
                .ensure_pr(
                    "org",
                    "repo",
                    "editor/session-xyz",
                    "main",
                    "Editor session xyz (3 edits)",
                    "updated body",
                    "test-token",
                )
                .await
                .unwrap();

            assert_eq!(pr.number, 7);
        }

        #[tokio::test]
        async fn ensure_pr_propagates_api_error() {
            let server = MockServer::start().await;

            Mock::given(method("GET"))
                .and(path("/repos/org/repo/pulls"))
                .respond_with(ResponseTemplate::new(403).set_body_string("forbidden"))
                .mount(&server)
                .await;

            let err = client_for(&server)
                .ensure_pr(
                    "org",
                    "repo",
                    "editor/session-foo",
                    "main",
                    "title",
                    "body",
                    "no-perms-token",
                )
                .await
                .expect_err("403 must error");

            let msg = err.to_string();
            assert!(msg.contains("403"), "error should include status: {msg}");
        }
    }
}

#[cfg(feature = "github")]
pub use inner::*;
