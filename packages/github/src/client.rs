//! The `GithubClient` service: one shared `reqwest::Client`, one header
//! builder, one base-url mechanism, and the ETag + rate-limit state that the
//! stateful read paths need.
//!
//! State (ETag cache, last-seen rate-limit remaining) lives behind a
//! `std::sync::Mutex` so every method takes `&self` (interior mutability).
//! The lock is only ever taken to read/replace a small `HashMap` entry or an
//! `Option<u32>` — never held across a `.await` (clippy's `await_holding_lock`
//! guards this), so it can't stall the async runtime.
//!
//! ## Conditional GETs
//!
//! Two shapes of ETag state live here:
//!
//! * [`cached_etag`](GithubClient::cached_etag) / [`store_etag`](GithubClient::store_etag)
//!   — an ETag with **no body**, for the Trees read, whose caller keeps the
//!   previously loaded data itself and only needs to be told "unchanged"
//!   (`Ok(None)` on 304).
//! * [`cached_response`](GithubClient::cached_response) /
//!   [`store_response`](GithubClient::store_response) — an ETag **together
//!   with the payload it was observed with**, for the Contents reads, whose
//!   callers need the content back on a 304. Storing the two as one unit is
//!   what makes the 304 path safe: there is never an ETag whose body was
//!   dropped, so a 304 can never degrade into "empty" or "does not exist".
//!
//! Both caches are keyed per (url, token identity): a conditional request
//! authenticated as a different principal must not be answered from another
//! principal's cache entry. The token itself is never stored — only a hash.
//!
//! The response cache never serves content without asking GitHub first: it
//! only turns a 200 into a 304, which costs no rate-limit quota. It can
//! therefore not go stale — a changed file changes its ETag and comes back
//! as a full 200. Its size is capped ([`RESPONSE_CACHE_BUDGET_BYTES`]) so a
//! corpus-wide read sequence cannot grow it without bound; the least
//! recently used entries are evicted first.

use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, VecDeque};
use std::hash::{Hash, Hasher};
use std::sync::Mutex;
use std::time::Duration;

use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION, USER_AGENT};

use crate::contents::DirectoryEntry;
use crate::error::{GithubError, Result};

/// Memory budget for the conditional-GET response cache. Entries are
/// evicted least-recently-used first once the summed payload size passes
/// this. 32 MiB holds a few thousand law bodies — enough for the paths an
/// editor session touches repeatedly, small enough that it can never be the
/// reason a container is OOM-killed.
pub const RESPONSE_CACHE_BUDGET_BYTES: usize = 32 * 1024 * 1024;

/// User-Agent sent on every request, so GitHub audit logs attribute reads and
/// writes to this client uniformly (the three hand-rolled predecessors each
/// sent `regelrecht-corpus/0.1`).
pub(crate) const USER_AGENT_VALUE: &str = concat!("regelrecht-github/", env!("CARGO_PKG_VERSION"));

/// GitHub REST API version header value pinned across all calls.
pub(crate) const GITHUB_API_VERSION: &str = "2022-11-28";

/// Payload cached alongside an ETag, per read shape. Kept as one enum so a
/// cache hit can never hand a directory listing to a file read (or vice
/// versa) — the reader matches on the variant it expects and treats
/// anything else as a miss.
#[derive(Debug, Clone)]
pub(crate) enum CachedPayload {
    /// A Contents file read: decoded body plus its blob sha.
    File { content: String, sha: String },
    /// A raw Contents file read (`application/vnd.github.raw+json`), which
    /// carries no sha.
    Raw(String),
    /// A Contents directory listing.
    Directory(Vec<DirectoryEntry>),
}

impl CachedPayload {
    /// Approximate heap footprint, for the cache budget. Exactness doesn't
    /// matter — the budget is a guard rail, not an accountant.
    fn size_bytes(&self) -> usize {
        match self {
            Self::File { content, sha } => content.len() + sha.len(),
            Self::Raw(content) => content.len(),
            Self::Directory(entries) => entries
                .iter()
                .map(|e| e.name.len() + e.entry_type.len() + 32)
                .sum(),
        }
    }
}

/// An ETag together with the payload that ETag was observed with. The two
/// are stored and evicted as one unit — see the module docs.
#[derive(Debug, Clone)]
pub(crate) struct CachedResponse {
    pub etag: String,
    pub payload: CachedPayload,
}

/// Mutable per-client state guarded by [`GithubClient::state`].
#[derive(Default)]
struct ClientState {
    /// ETag cache: cache key → last ETag value. Feeds `If-None-Match` on
    /// the Trees read so an unchanged tree comes back as a cheap 304.
    etag_cache: HashMap<String, String>,
    /// Conditional-GET cache for the Contents reads: cache key → ETag +
    /// the payload it belongs to.
    response_cache: HashMap<String, CachedResponse>,
    /// Cache keys in least-recently-used order (front = oldest touch), the
    /// eviction order for [`response_cache`](Self::response_cache).
    response_order: VecDeque<String>,
    /// Summed [`CachedPayload::size_bytes`] of `response_cache`.
    response_bytes: usize,
    /// Most recent `x-ratelimit-remaining` seen on any response.
    rate_limit_remaining: Option<u32>,
}

impl ClientState {
    /// Drop least-recently-used entries until the cache fits `budget`.
    fn evict_to(&mut self, budget: usize) {
        while self.response_bytes > budget {
            let Some(oldest) = self.response_order.pop_front() else {
                // Order and map disagree — reset rather than spin.
                self.response_cache.clear();
                self.response_bytes = 0;
                break;
            };
            if let Some(dropped) = self.response_cache.remove(&oldest) {
                self.response_bytes = self
                    .response_bytes
                    .saturating_sub(dropped.payload.size_bytes());
            }
        }
    }
}

/// One GitHub REST client shared by every regelrecht application.
pub struct GithubClient {
    pub(crate) client: reqwest::Client,
    /// API base URL — no trailing slash; every method prefixes its own
    /// `/...` path. Production default is `https://api.github.com`.
    pub(crate) api_base: String,
    /// Byte budget for the conditional-GET response cache. A field rather
    /// than a constant so a test can shrink it and prove eviction.
    cache_budget: usize,
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
            cache_budget: RESPONSE_CACHE_BUDGET_BYTES,
            state: Mutex::new(ClientState::default()),
        })
    }

    /// Shrink or grow the conditional-GET cache budget. Test seam: nothing
    /// in production changes it away from [`RESPONSE_CACHE_BUDGET_BYTES`].
    /// Takes `&mut self` so it can only be set before the client is shared.
    pub fn set_cache_budget_bytes(&mut self, budget: usize) {
        self.cache_budget = budget;
        if let Ok(mut state) = self.state.lock() {
            state.evict_to(budget);
        }
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

    /// Cache key for a conditional GET: the URL plus the identity of the
    /// token it is made with. Two principals reading the same URL get two
    /// entries, so a 304 can never hand one principal a body fetched under
    /// another's credential. The token is hashed, never stored.
    pub(crate) fn cache_key(url: &str, token: Option<&str>) -> String {
        match token {
            Some(token) => {
                let mut hasher = DefaultHasher::new();
                token.hash(&mut hasher);
                format!("{url}#{:016x}", hasher.finish())
            }
            None => format!("{url}#anon"),
        }
    }

    /// Read the cached ETag for `key`, if any. Guard scope is this call only.
    pub(crate) fn cached_etag(&self, key: &str) -> Option<String> {
        self.state
            .lock()
            .ok()
            .and_then(|s| s.etag_cache.get(key).cloned())
    }

    /// Store the ETag observed for `key`. Guard scope is this call only.
    pub(crate) fn store_etag(&self, key: &str, etag: &str) {
        if let Ok(mut state) = self.state.lock() {
            state.etag_cache.insert(key.to_string(), etag.to_string());
        }
    }

    /// Read the cached ETag **plus payload** for `key`, marking the entry
    /// as most recently used. Returns a clone: the caller holds the payload
    /// across its request, so a concurrent eviction can never leave it with
    /// an ETag whose body is gone.
    pub(crate) fn cached_response(&self, key: &str) -> Option<CachedResponse> {
        let mut state = self.state.lock().ok()?;
        let hit = state.response_cache.get(key).cloned()?;
        if let Some(pos) = state.response_order.iter().position(|k| k == key) {
            state.response_order.remove(pos);
        }
        state.response_order.push_back(key.to_string());
        Some(hit)
    }

    /// Store an ETag together with the payload it was observed with, then
    /// evict least-recently-used entries until the cache fits its budget.
    pub(crate) fn store_response(&self, key: &str, etag: &str, payload: CachedPayload) {
        let budget = self.cache_budget;
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        // A payload larger than the whole budget would be inserted and
        // immediately evicted; skip the churn.
        let size = payload.size_bytes();
        if size > budget {
            return;
        }
        if let Some(previous) = state.response_cache.remove(key) {
            state.response_bytes = state
                .response_bytes
                .saturating_sub(previous.payload.size_bytes());
            if let Some(pos) = state.response_order.iter().position(|k| k == key) {
                state.response_order.remove(pos);
            }
        }
        state.response_bytes += size;
        state.response_cache.insert(
            key.to_string(),
            CachedResponse {
                etag: etag.to_string(),
                payload,
            },
        );
        state.response_order.push_back(key.to_string());
        state.evict_to(budget);
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

    // `GITHUB_API_BASE` mutates process-global env, which every test in this
    // binary shares, so the env-sensitive assertions live in this ONE test.
    // No other test may read or write that var — not even defensively — because
    // a `remove_var` racing this test's `set_var` makes it fail intermittently.
    // Tests that need a specific base URL use `with_base_url`/`set_base_url`.
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
        let mut c = GithubClient::new().unwrap();
        c.set_base_url("http://127.0.0.1:1234/");
        assert_eq!(c.api_base, "http://127.0.0.1:1234");
        let c = c.with_base_url("http://127.0.0.1:9999///");
        assert_eq!(c.api_base, "http://127.0.0.1:9999");
    }

    #[test]
    fn default_headers_carry_the_shared_set() {
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
