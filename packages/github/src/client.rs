//! The `GithubClient` service: one shared `reqwest::Client`, one header
//! builder, one base-url mechanism, and the ETag + rate-limit state that the
//! stateful read paths need.
//!
//! State (ETag cache, last-seen rate-limit remaining) lives behind a
//! `std::sync::Mutex` so every method takes `&self` (interior mutability).
//! The lock is only ever taken to read/replace a small `HashMap` entry or an
//! `Option<u32>` — never held across a `.await` (clippy's `await_holding_lock`
//! guards this), so it can't stall the async runtime.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION, USER_AGENT};

use crate::error::{GithubError, Result};

/// User-Agent sent on every request, so GitHub audit logs attribute reads and
/// writes to this client uniformly (the three hand-rolled predecessors each
/// sent `regelrecht-corpus/0.1`).
pub(crate) const USER_AGENT_VALUE: &str = concat!("regelrecht-github/", env!("CARGO_PKG_VERSION"));

/// GitHub REST API version header value pinned across all calls.
pub(crate) const GITHUB_API_VERSION: &str = "2022-11-28";

/// Mutable per-client state guarded by [`GithubClient::state`].
#[derive(Default)]
struct ClientState {
    /// ETag cache: request URL → last ETag value. Feeds `If-None-Match` on
    /// the Trees read so an unchanged tree comes back as a cheap 304.
    etag_cache: HashMap<String, String>,
    /// Most recent `x-ratelimit-remaining` seen on any response.
    rate_limit_remaining: Option<u32>,
}

/// One GitHub REST client shared by every regelrecht application.
pub struct GithubClient {
    pub(crate) client: reqwest::Client,
    /// API base URL — no trailing slash; every method prefixes its own
    /// `/...` path. Production default is `https://api.github.com`.
    pub(crate) api_base: String,
    state: Mutex<ClientState>,
}

impl GithubClient {
    /// Build a client pointed at `api.github.com`, or at whatever
    /// `GITHUB_API_BASE` names when that env var is set.
    ///
    /// The env override is read **once, here at construction**. It is the
    /// load-bearing test seam: the client is built deep inside backend /
    /// registry code with no config plumbing to inject a base URL, so the
    /// integration tests stand up a wiremock GitHub and point every client in
    /// the process at it via this env var. It doubles as a GitHub Enterprise
    /// seam; production deployments leave it unset.
    pub fn new() -> Result<Self> {
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(30))
            .timeout(Duration::from_secs(60))
            .build()
            .map_err(|e| GithubError::Config(format!("failed to create HTTP client: {e}")))?;

        let api_base = std::env::var("GITHUB_API_BASE")
            .ok()
            .map(|s| s.trim().trim_end_matches('/').to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "https://api.github.com".to_string());

        Ok(Self {
            client,
            api_base,
            state: Mutex::new(ClientState::default()),
        })
    }

    /// Override the API base URL, consuming self — for callers that build a
    /// client and immediately point it at a wiremock server (or a specific
    /// enterprise host). Trailing slashes are trimmed so callers can pass a
    /// server URI verbatim.
    #[must_use]
    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.set_base_url(base_url);
        self
    }

    /// In-place variant of [`with_base_url`](Self::with_base_url) for call
    /// sites that already hold the client by `&mut` (e.g. a backend reaching
    /// through its guard to repoint the client at a test server).
    pub fn set_base_url(&mut self, base_url: impl Into<String>) {
        self.api_base = base_url.into().trim_end_matches('/').to_string();
    }

    /// Most recent `x-ratelimit-remaining` value observed on a response, if
    /// any has been seen yet.
    pub fn rate_limit_remaining(&self) -> Option<u32> {
        self.state
            .lock()
            .map(|s| s.rate_limit_remaining)
            .unwrap_or(None)
    }

    /// Build the default header set every GitHub call shares (User-Agent,
    /// Accept, API version) plus the `Authorization` header when a token is
    /// given.
    ///
    /// Returns [`GithubError::InvalidToken`] when the token can't form a valid
    /// header value, rather than dropping the header and sending an
    /// unauthenticated request (which would surface as a misleading 401).
    pub(crate) fn default_headers(&self, token: Option<&str>) -> Result<HeaderMap> {
        let mut headers = HeaderMap::new();
        headers.insert(USER_AGENT, HeaderValue::from_static(USER_AGENT_VALUE));
        headers.insert(
            ACCEPT,
            HeaderValue::from_static("application/vnd.github+json"),
        );
        headers.insert(
            "X-GitHub-Api-Version",
            HeaderValue::from_static(GITHUB_API_VERSION),
        );
        if let Some(token) = token {
            let auth = HeaderValue::from_str(&format!("Bearer {token}"))
                .map_err(|e| GithubError::InvalidToken(e.to_string()))?;
            headers.insert(AUTHORIZATION, auth);
        }
        Ok(headers)
    }

    /// Record `x-ratelimit-remaining` from a response and warn when it runs
    /// low. Takes the guard, mutates, drops it — never spans an await.
    pub(crate) fn track_rate_limit(&self, response: &reqwest::Response) {
        if let Some(remaining) = response
            .headers()
            .get("x-ratelimit-remaining")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u32>().ok())
        {
            if let Ok(mut state) = self.state.lock() {
                state.rate_limit_remaining = Some(remaining);
            }
            if remaining < 100 {
                tracing::warn!(remaining, "GitHub API rate limit running low");
            }
        }
    }

    /// Read the cached ETag for `url`, if any. Guard scope is this call only.
    pub(crate) fn cached_etag(&self, url: &str) -> Option<String> {
        self.state
            .lock()
            .ok()
            .and_then(|s| s.etag_cache.get(url).cloned())
    }

    /// Store the ETag observed for `url`. Guard scope is this call only.
    pub(crate) fn store_etag(&self, url: &str, etag: &str) {
        if let Ok(mut state) = self.state.lock() {
            state.etag_cache.insert(url.to_string(), etag.to_string());
        }
    }

    /// True when a 403 response is GitHub *rate limiting* rather than a
    /// permission refusal: primary exhaustion answers 403 with
    /// `x-ratelimit-remaining: 0`, secondary limits answer 403 with a
    /// `retry-after` header. Write methods keep those on the generic
    /// [`GithubError::Api`] path instead of [`GithubError::WriteDenied`] — a
    /// "no write access" message for a transient limit would mislead.
    pub(crate) fn forbidden_is_rate_limit(response: &reqwest::Response) -> bool {
        response.headers().contains_key("retry-after")
            || response
                .headers()
                .get("x-ratelimit-remaining")
                .and_then(|v| v.to_str().ok())
                .map(str::trim)
                == Some("0")
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    // `GITHUB_API_BASE` mutates process-global env, so the env-sensitive
    // assertions share one serialized test to avoid cross-test races.
    #[test]
    fn env_base_url_is_read_at_construction_and_trimmed() {
        // Unset: falls back to api.github.com.
        std::env::remove_var("GITHUB_API_BASE");
        let c = GithubClient::new().unwrap();
        assert_eq!(c.api_base, "https://api.github.com");

        // Set with a trailing slash: trimmed.
        std::env::set_var("GITHUB_API_BASE", "https://ghe.example.test/api/v3/");
        let c = GithubClient::new().unwrap();
        assert_eq!(c.api_base, "https://ghe.example.test/api/v3");

        // Blank / whitespace-only: ignored, back to the default.
        std::env::set_var("GITHUB_API_BASE", "   ");
        let c = GithubClient::new().unwrap();
        assert_eq!(c.api_base, "https://api.github.com");

        std::env::remove_var("GITHUB_API_BASE");
    }

    #[test]
    fn set_base_url_trims_trailing_slash() {
        std::env::remove_var("GITHUB_API_BASE");
        let mut c = GithubClient::new().unwrap();
        c.set_base_url("http://127.0.0.1:1234/");
        assert_eq!(c.api_base, "http://127.0.0.1:1234");
        let c = c.with_base_url("http://127.0.0.1:9999///");
        assert_eq!(c.api_base, "http://127.0.0.1:9999");
    }

    #[test]
    fn default_headers_carry_the_shared_set() {
        std::env::remove_var("GITHUB_API_BASE");
        let c = GithubClient::new().unwrap();
        let headers = c.default_headers(Some("tok")).unwrap();
        assert_eq!(headers.get(USER_AGENT).unwrap(), USER_AGENT_VALUE);
        assert_eq!(headers.get(ACCEPT).unwrap(), "application/vnd.github+json");
        assert_eq!(
            headers.get("X-GitHub-Api-Version").unwrap(),
            GITHUB_API_VERSION
        );
        assert_eq!(headers.get(AUTHORIZATION).unwrap(), "Bearer tok");

        // Without a token there is no Authorization header at all.
        let anon = c.default_headers(None).unwrap();
        assert!(anon.get(AUTHORIZATION).is_none());
    }

    #[test]
    fn malformed_token_is_invalid_token_error() {
        std::env::remove_var("GITHUB_API_BASE");
        let c = GithubClient::new().unwrap();
        // An embedded newline can't form a valid header value.
        let err = c
            .default_headers(Some("bad\ntoken"))
            .expect_err("malformed token must error");
        assert!(matches!(err, GithubError::InvalidToken(_)));
        assert!(
            err.to_string()
                .contains("not valid in an HTTP header value"),
            "message must name the real cause: {err}"
        );
    }

    #[tokio::test]
    async fn etag_roundtrip_sends_if_none_match_and_handles_304() {
        std::env::remove_var("GITHUB_API_BASE");
        let server = MockServer::start().await;

        // First response carries an ETag; second request must echo it back
        // via If-None-Match and gets a 304. The first mock is capped at one
        // hit (`up_to_n_times`) so the second request — which also matches its
        // looser matcher — falls through to the header-specific 304 mock.
        Mock::given(method("GET"))
            .and(path("/probe"))
            .respond_with(ResponseTemplate::new(200).insert_header("etag", "\"abc\""))
            .up_to_n_times(1)
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/probe"))
            .and(header("if-none-match", "\"abc\""))
            .respond_with(ResponseTemplate::new(304))
            .expect(1)
            .mount(&server)
            .await;

        let client = GithubClient::new().unwrap().with_base_url(server.uri());
        let url = format!("{}/probe", client.api_base);

        // First call: no cached etag, store the one we get back.
        assert!(client.cached_etag(&url).is_none());
        let headers = client.default_headers(None).unwrap();
        let resp = client
            .client
            .get(&url)
            .headers(headers)
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status().as_u16(), 200);
        if let Some(etag) = resp.headers().get("etag").and_then(|v| v.to_str().ok()) {
            client.store_etag(&url, etag);
        }
        assert_eq!(client.cached_etag(&url).as_deref(), Some("\"abc\""));

        // Second call: send the cached etag; server answers 304.
        let mut headers = client.default_headers(None).unwrap();
        let etag = client.cached_etag(&url).unwrap();
        headers.insert(
            reqwest::header::IF_NONE_MATCH,
            HeaderValue::from_str(&etag).unwrap(),
        );
        let resp = client
            .client
            .get(&url)
            .headers(headers)
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status().as_u16(), 304);
    }
}
