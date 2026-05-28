//! Pre-flight checks for a GitHub repository before we commit a traject
//! to use it as a write target.
//!
//! Two questions to answer:
//! 1. Does the supplied token actually have push access to this repo?
//!    (Without it, the very first save would fail at push time with a
//!    cryptic "could not read Username" or a 403; checking up-front gives
//!    a clean error at traject-create time.)
//! 2. Does the configured `base_branch` exist? (We branch off this on
//!    first persist — a missing base would also fail late.)

#[cfg(feature = "github")]
mod inner {
    use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION, USER_AGENT};
    use reqwest::StatusCode;
    use serde::Deserialize;

    /// Distinct failure modes for repo access validation. The editor-api
    /// caller maps each variant onto a specific HTTP status / user-facing
    /// message — keep them disjoint so the matching is exhaustive.
    #[derive(Debug)]
    pub enum RepoAccessError {
        /// 401 on either call. Token rejected by GitHub (revoked, expired,
        /// or simply wrong).
        Unauthorized,
        /// 404 on `GET /repos/{owner}/{repo}`. Either the repo really
        /// doesn't exist, or it's private and the token can't see it —
        /// GitHub returns 404 in both cases to avoid leaking existence.
        RepoNotFound,
        /// 404 on `GET /repos/{owner}/{repo}/branches/{base}`. Repo is
        /// reachable but the base branch the operator named is wrong.
        BranchNotFound,
        /// Repo reads fine, but the token's user/installation has
        /// `permissions.push == false`. Pushing the traject branch later
        /// would fail with 403 — better to refuse the traject now.
        NoPushAccess,
        /// Transport-level failure (DNS, TLS, timeout, connection reset).
        /// Worth retrying or surfacing as 503.
        Transport(String),
        /// Any other response we can't classify (5xx, surprise 4xx, JSON
        /// parse failure). Includes the status + a short error blurb for
        /// diagnostics — never the token.
        Other(String),
    }

    impl std::fmt::Display for RepoAccessError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Self::Unauthorized => write!(f, "token is not authorised for this repo"),
                Self::RepoNotFound => write!(f, "repository not found or token cannot see it"),
                Self::BranchNotFound => write!(f, "base branch does not exist on the repo"),
                Self::NoPushAccess => write!(f, "token has no push access to the repo"),
                Self::Transport(msg) => write!(f, "transport error talking to GitHub: {}", msg),
                Self::Other(msg) => write!(f, "unexpected GitHub response: {}", msg),
            }
        }
    }

    impl std::error::Error for RepoAccessError {}

    /// Information we glean from a successful validation call. The
    /// caller might surface `default_branch` (e.g. as a hint when the
    /// operator typed the wrong base) or use `is_private` to drive
    /// frontend hints later.
    #[derive(Debug, Clone)]
    pub struct RepoInfo {
        pub default_branch: String,
        pub is_private: bool,
    }

    /// Minimal subset of the `/repos/{owner}/{repo}` response we care
    /// about. GitHub returns many more fields; ignore them.
    #[derive(Debug, Deserialize)]
    struct RepoResponse {
        default_branch: String,
        #[serde(default)]
        private: bool,
        permissions: Option<RepoPermissions>,
    }

    #[derive(Debug, Deserialize)]
    struct RepoPermissions {
        #[serde(default)]
        push: bool,
    }

    /// Pre-flight check before letting a traject use this repo as its
    /// write target. Performs two API calls in sequence: repo lookup
    /// (also returns the permission bits) and branch existence.
    ///
    /// `base_url` is exposed for tests pointing at a wiremock server;
    /// production callers pass `"https://api.github.com"`.
    pub async fn validate_repo_access(
        base_url: &str,
        owner: &str,
        repo: &str,
        base_branch: &str,
        token: &str,
    ) -> Result<RepoInfo, RepoAccessError> {
        let client = reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(10))
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| RepoAccessError::Transport(e.to_string()))?;

        let info = check_repo(&client, base_url, owner, repo, token).await?;
        check_branch(&client, base_url, owner, repo, base_branch, token).await?;
        Ok(info)
    }

    async fn check_repo(
        client: &reqwest::Client,
        base_url: &str,
        owner: &str,
        repo: &str,
        token: &str,
    ) -> Result<RepoInfo, RepoAccessError> {
        let url = format!("{}/repos/{}/{}", base_url, owner, repo);
        let headers = default_headers(token)?;
        let response = client
            .get(&url)
            .headers(headers)
            .send()
            .await
            .map_err(|e| RepoAccessError::Transport(e.to_string()))?;

        match response.status() {
            StatusCode::OK => {}
            StatusCode::UNAUTHORIZED => return Err(RepoAccessError::Unauthorized),
            StatusCode::NOT_FOUND => return Err(RepoAccessError::RepoNotFound),
            // 403 from the repo lookup means the token can see the repo
            // exists but isn't allowed to read it (rare; usually masks
            // as 404). Treat as Unauthorized — the operator needs to fix
            // the token either way.
            StatusCode::FORBIDDEN => return Err(RepoAccessError::Unauthorized),
            other => {
                let body = response.text().await.unwrap_or_default();
                return Err(RepoAccessError::Other(format!(
                    "{}: {}",
                    other,
                    truncate(&body, 200)
                )));
            }
        }

        let parsed: RepoResponse = response
            .json()
            .await
            .map_err(|e| RepoAccessError::Other(format!("parse repo response: {}", e)))?;

        // No `permissions` block on the response usually means an
        // unauthenticated request — we always send a token, so missing
        // permissions here is a real "no access" signal, not a quirk to
        // shrug off.
        let pushable = parsed.permissions.as_ref().map(|p| p.push).unwrap_or(false);
        if !pushable {
            return Err(RepoAccessError::NoPushAccess);
        }

        Ok(RepoInfo {
            default_branch: parsed.default_branch,
            is_private: parsed.private,
        })
    }

    async fn check_branch(
        client: &reqwest::Client,
        base_url: &str,
        owner: &str,
        repo: &str,
        base_branch: &str,
        token: &str,
    ) -> Result<(), RepoAccessError> {
        // Branch names can legitimately contain `/` (e.g. `feature/foo`,
        // `release/1.0`). Spliced raw into the path, the slash collapses
        // into the URL router so GitHub matches `branch = "feature"` with
        // a trailing path segment and returns 404. Percent-encode the
        // segment so the branch reaches the API as-typed.
        let url = format!(
            "{}/repos/{}/{}/branches/{}",
            base_url,
            owner,
            repo,
            percent_encode_path_segment(base_branch)
        );
        let headers = default_headers(token)?;
        let response = client
            .get(&url)
            .headers(headers)
            .send()
            .await
            .map_err(|e| RepoAccessError::Transport(e.to_string()))?;

        match response.status() {
            StatusCode::OK => Ok(()),
            StatusCode::NOT_FOUND => Err(RepoAccessError::BranchNotFound),
            StatusCode::UNAUTHORIZED => Err(RepoAccessError::Unauthorized),
            // Per-branch 403 = the token can resolve the repo (passed the
            // `/repos/{owner}/{repo}` check above) but the fine-grained PAT
            // scopes don't include `contents:read`. Surface as "no push
            // access" since the operator's fix is identical: widen the
            // token. Branch protection itself only governs writes, so it
            // can't be the cause of a 403 on this GET.
            StatusCode::FORBIDDEN => Err(RepoAccessError::NoPushAccess),
            other => {
                let body = response.text().await.unwrap_or_default();
                Err(RepoAccessError::Other(format!(
                    "{}: {}",
                    other,
                    truncate(&body, 200)
                )))
            }
        }
    }

    /// Build the default header set for every GitHub API call, including
    /// the `Authorization` header. Returns `RepoAccessError::Other` when
    /// the token contains bytes that aren't valid in an HTTP header value
    /// (BOM, CR/LF, non-ASCII) — silently dropping the header would send
    /// an unauthenticated request and surface as a misleading 401, which
    /// the operator would chase as "GitHub rejected the token" while the
    /// real cause is a malformed env var.
    fn default_headers(token: &str) -> Result<HeaderMap, RepoAccessError> {
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
        let auth_value = HeaderValue::from_str(&format!("Bearer {}", token)).map_err(|_| {
            RepoAccessError::Other(
                "token contains characters not valid in an HTTP header value \
                 — check the env var for whitespace/BOM/non-ASCII"
                    .to_string(),
            )
        })?;
        headers.insert(AUTHORIZATION, auth_value);
        Ok(headers)
    }

    /// UTF-8-safe slice up to `max` *bytes*, walking back to the
    /// nearest char boundary so a non-ASCII body from GitHub (a 5xx
    /// HTML page, say) doesn't panic the formatter on a multi-byte
    /// split. `s[..max]` would panic in that case — only safe for pure
    /// ASCII, which we can't assume on the unhappy path.
    fn truncate(s: &str, max: usize) -> String {
        if s.len() <= max {
            return s.to_string();
        }
        // Walk back from `max` until we land on a char boundary. `str`
        // guarantees that byte 0 is a boundary, so this terminates.
        let mut end = max;
        while !s.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}…", &s[..end])
    }

    /// Percent-encode a single URL path segment per RFC 3986 §2.3:
    /// unreserved chars (alphanumeric, `-._~`) pass through, everything
    /// else becomes `%XX`. In particular `/`, `#`, `?`, and `%` are
    /// encoded — without that, a branch like `feature/foo` collapses
    /// into the URL path and GitHub matches `branch = "feature"` with a
    /// trailing segment instead of looking up the real ref.
    ///
    /// Inline rather than via a crate dependency: the call is tiny,
    /// the rule is fixed by the RFC, and the corpus crate already runs
    /// without the `percent_encoding` / `url` crates as direct deps.
    fn percent_encode_path_segment(s: &str) -> String {
        use std::fmt::Write;
        let mut out = String::with_capacity(s.len());
        for &b in s.as_bytes() {
            if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
                out.push(b as char);
            } else {
                // `write!` into a String can't fail; ignore the result.
                let _ = write!(out, "%{:02X}", b);
            }
        }
        out
    }

    #[cfg(test)]
    #[allow(clippy::unwrap_used)]
    mod tests {
        use super::*;
        use wiremock::matchers::{header, method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        fn ok_repo_body(default_branch: &str, push: bool, private: bool) -> serde_json::Value {
            serde_json::json!({
                "default_branch": default_branch,
                "private": private,
                "permissions": { "push": push, "pull": true, "admin": false }
            })
        }

        #[tokio::test]
        async fn happy_path_returns_info() {
            let server = MockServer::start().await;
            Mock::given(method("GET"))
                .and(path("/repos/acme/foo"))
                .and(header("Authorization", "Bearer t"))
                .respond_with(
                    ResponseTemplate::new(200).set_body_json(ok_repo_body("main", true, true)),
                )
                .expect(1)
                .mount(&server)
                .await;
            Mock::given(method("GET"))
                .and(path("/repos/acme/foo/branches/main"))
                .and(header("Authorization", "Bearer t"))
                .respond_with(
                    ResponseTemplate::new(200).set_body_json(serde_json::json!({"name":"main"})),
                )
                .expect(1)
                .mount(&server)
                .await;

            let info = validate_repo_access(&server.uri(), "acme", "foo", "main", "t")
                .await
                .unwrap();
            assert_eq!(info.default_branch, "main");
            assert!(info.is_private);
        }

        #[tokio::test]
        async fn slashed_branch_is_percent_encoded() {
            // `feature/foo` would collapse into the URL path if we spliced
            // it raw: GitHub's router would match `branch = "feature"` with
            // a trailing path segment and return 404. We must percent-
            // encode the slash so the API sees `branches/feature%2Ffoo`.
            let server = MockServer::start().await;
            Mock::given(method("GET"))
                .and(path("/repos/acme/foo"))
                .respond_with(
                    ResponseTemplate::new(200).set_body_json(ok_repo_body("main", true, true)),
                )
                .expect(1)
                .mount(&server)
                .await;
            // wiremock matches against the raw (still-encoded) request
            // path. Asserting on the literal `%2F` form proves the
            // client is sending the encoded variant — without the
            // percent-encoding fix, the request would go out as
            // `branches/feature/foo` and the mock would not match.
            Mock::given(method("GET"))
                .and(path("/repos/acme/foo/branches/feature%2Ffoo"))
                .respond_with(
                    ResponseTemplate::new(200)
                        .set_body_json(serde_json::json!({"name":"feature/foo"})),
                )
                .expect(1)
                .mount(&server)
                .await;
            // Negative-path catch-all: a regression that drops the
            // encoding would hit this route, return 200 (with a body
            // that does not match the route), and the positive mock's
            // `.expect(1)` would still fail with "expected 1, got 0".
            // The catch-all just makes the failure mode louder by
            // letting the bogus route succeed visibly.
            Mock::given(method("GET"))
                .and(path("/repos/acme/foo/branches/feature"))
                .respond_with(ResponseTemplate::new(404))
                .expect(0)
                .mount(&server)
                .await;

            validate_repo_access(&server.uri(), "acme", "foo", "feature/foo", "t")
                .await
                .expect("slashed branch should resolve");
        }

        #[test]
        fn percent_encode_path_segment_examples() {
            // Spot-checks for the inline encoder so a future refactor
            // (e.g. swapping to a crate) is forced to preserve the
            // contract.
            assert_eq!(percent_encode_path_segment("main"), "main");
            assert_eq!(percent_encode_path_segment("feature/foo"), "feature%2Ffoo");
            assert_eq!(
                percent_encode_path_segment("release-1.0_rc~final"),
                "release-1.0_rc~final"
            );
            assert_eq!(percent_encode_path_segment("a b"), "a%20b");
            assert_eq!(percent_encode_path_segment("a#b"), "a%23b");
            assert_eq!(percent_encode_path_segment("a?b"), "a%3Fb");
            // Multi-byte UTF-8: encoded as raw bytes per RFC 3986.
            // `é` is 0xC3 0xA9 in UTF-8.
            assert_eq!(percent_encode_path_segment("é"), "%C3%A9");
        }

        #[tokio::test]
        async fn missing_repo_is_not_found() {
            let server = MockServer::start().await;
            Mock::given(method("GET"))
                .and(path("/repos/acme/foo"))
                .respond_with(ResponseTemplate::new(404))
                .mount(&server)
                .await;

            let err = validate_repo_access(&server.uri(), "acme", "foo", "main", "t")
                .await
                .expect_err("404 should error");
            assert!(matches!(err, RepoAccessError::RepoNotFound));
        }

        #[tokio::test]
        async fn bad_token_is_unauthorized() {
            let server = MockServer::start().await;
            Mock::given(method("GET"))
                .and(path("/repos/acme/foo"))
                .respond_with(ResponseTemplate::new(401))
                .mount(&server)
                .await;

            let err = validate_repo_access(&server.uri(), "acme", "foo", "main", "t")
                .await
                .expect_err("401 should error");
            assert!(matches!(err, RepoAccessError::Unauthorized));
        }

        #[tokio::test]
        async fn read_only_token_has_no_push_access() {
            let server = MockServer::start().await;
            Mock::given(method("GET"))
                .and(path("/repos/acme/foo"))
                .respond_with(
                    ResponseTemplate::new(200).set_body_json(ok_repo_body("main", false, true)),
                )
                .expect(1)
                .mount(&server)
                .await;
            // Branch call must NOT be made — fail-fast on no-push.
            Mock::given(method("GET"))
                .and(path("/repos/acme/foo/branches/main"))
                .respond_with(ResponseTemplate::new(200))
                .expect(0)
                .mount(&server)
                .await;

            let err = validate_repo_access(&server.uri(), "acme", "foo", "main", "t")
                .await
                .expect_err("no push must error");
            assert!(matches!(err, RepoAccessError::NoPushAccess));
        }

        #[tokio::test]
        async fn missing_branch_is_branch_not_found() {
            let server = MockServer::start().await;
            Mock::given(method("GET"))
                .and(path("/repos/acme/foo"))
                .respond_with(
                    ResponseTemplate::new(200).set_body_json(ok_repo_body("main", true, false)),
                )
                .mount(&server)
                .await;
            Mock::given(method("GET"))
                .and(path("/repos/acme/foo/branches/wibble"))
                .respond_with(ResponseTemplate::new(404))
                .mount(&server)
                .await;

            let err = validate_repo_access(&server.uri(), "acme", "foo", "wibble", "t")
                .await
                .expect_err("missing branch should error");
            assert!(matches!(err, RepoAccessError::BranchNotFound));
        }

        #[tokio::test]
        async fn malformed_token_surfaces_as_other_not_silent_unauth() {
            // A token with bytes that aren't valid in an HTTP header
            // value (here: an embedded newline) used to silently drop
            // the Authorization header and surface as 401, sending the
            // operator on a wild "token rejected by GitHub" chase. Now
            // it returns RepoAccessError::Other so the operator-facing
            // message names the real cause (corrupt env var).
            //
            // No server interaction expected — the failure happens
            // *before* we hit the network. We still spin one up so the
            // signature is consistent with the other tests; assert that
            // it isn't touched.
            let server = MockServer::start().await;
            Mock::given(method("GET"))
                .and(path("/repos/acme/foo"))
                .respond_with(ResponseTemplate::new(200))
                .expect(0)
                .mount(&server)
                .await;

            let err = validate_repo_access(&server.uri(), "acme", "foo", "main", "bad\ntoken")
                .await
                .expect_err("malformed token must error");
            match err {
                RepoAccessError::Other(msg) => assert!(
                    msg.contains("not valid in an HTTP header value"),
                    "unexpected Other body: {msg}"
                ),
                other => panic!("expected RepoAccessError::Other, got {other:?}"),
            }
        }

        #[tokio::test]
        async fn missing_permissions_block_is_no_push_access() {
            // We always send a token, so a response without a permissions
            // block isn't an "unauth read shape" — it's an unusual one we
            // should reject rather than fall through.
            let server = MockServer::start().await;
            Mock::given(method("GET"))
                .and(path("/repos/acme/foo"))
                .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "default_branch": "main",
                    "private": true
                })))
                .mount(&server)
                .await;

            let err = validate_repo_access(&server.uri(), "acme", "foo", "main", "t")
                .await
                .expect_err("missing perms should error");
            assert!(matches!(err, RepoAccessError::NoPushAccess));
        }
    }
}

#[cfg(feature = "github")]
pub use inner::*;
