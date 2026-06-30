//! GitHub App authentication: mint short-lived installation access
//! tokens on demand instead of storing a long-lived PAT per source repo.
//!
//! ## Why
//!
//! The per-source `CORPUS_AUTH_{REF}_TOKEN` env var (see [`crate::auth`])
//! means an operator must register a new secret for every private repo a
//! traject points at. A GitHub App replaces all of those with a single
//! secret — the app's private key — set once. For any repo the app is
//! installed on, this module mints a 1-hour installation token scoped to
//! that repo on demand, so nothing long-lived is stored anywhere.
//!
//! ## The token dance
//!
//! 1. Sign a short-lived **app JWT** (RS256) with the private key — pure
//!    crypto, no network. Proves "I am this app".
//! 2. Look up the **installation** of the app on the repo's owner
//!    (`GET /orgs/{owner}/installation`, falling back to the user
//!    endpoint for personal accounts). No installation → the app has no
//!    access to that owner; we return `None` so the caller falls back to
//!    the env/file token chain.
//! 3. Mint an **installation access token**
//!    (`POST /app/installations/{id}/access_tokens`), scoped to the one
//!    repo with `contents:write` + `metadata:read`. Lives ~1 hour.
//! 4. Hand the token back; downstream code uses it exactly like a PAT
//!    (`Authorization: Bearer`, or via `GIT_ASKPASS` for clones).
//!
//! Minted tokens are cached per `(owner, repo)` and re-minted shortly
//! before expiry, so a burst of reads/writes shares one token.

#[cfg(feature = "github")]
mod inner {
    use std::collections::HashMap;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    use async_trait::async_trait;
    use base64::Engine;
    use jsonwebtoken::{Algorithm, EncodingKey, Header};
    use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION, USER_AGENT};
    use serde::{Deserialize, Serialize};
    use tokio::sync::Mutex;

    use crate::auth::AppTokenMinter;
    use crate::error::{CorpusError, Result};
    use crate::models::{Source, SourceType};

    /// An installation token is valid for one hour. We treat it as expired
    /// a little early so a token never expires mid-request after we handed
    /// it out.
    const TOKEN_TTL: Duration = Duration::from_secs(55 * 60);
    /// The app JWT is short-lived; GitHub caps it at 10 minutes. Use 9 to
    /// leave headroom for clock skew on both sides.
    const JWT_TTL_SECS: u64 = 9 * 60;
    /// How long an installation-lookup result (id *or* "not installed") is
    /// trusted before re-checking. Short, so that installing the App on a
    /// new owner — or uninstalling/reinstalling (which changes the
    /// installation id) — takes effect within minutes instead of requiring
    /// an editor restart.
    const INSTALLATION_TTL: Duration = Duration::from_secs(5 * 60);

    /// JWT claims for the app-level token (`iss` = app id).
    #[derive(Serialize)]
    struct AppClaims {
        iat: u64,
        exp: u64,
        iss: String,
    }

    /// `GET /orgs|users/{owner}/installation` — we only need the id.
    #[derive(Deserialize)]
    struct InstallationResponse {
        id: u64,
    }

    /// `POST /app/installations/{id}/access_tokens` — we only need the token.
    #[derive(Deserialize)]
    struct AccessTokenResponse {
        token: String,
    }

    /// A cached token-resolution result with its expiry: `Some(token)` is a
    /// live installation token, `None` the negative "repo not in this
    /// install" verdict.
    type CachedToken = (Option<String>, SystemTime);
    /// A cached installation-lookup result with its expiry: `Some(id)` when
    /// installed on the owner, `None` for the "not installed" verdict.
    type CachedInstallation = (Option<u64>, SystemTime);

    /// Authenticates to GitHub as a GitHub App and mints short-lived
    /// per-repo installation tokens. Construct once at startup
    /// ([`GitHubAppAuth::from_env`]) and share via `Arc`.
    pub struct GitHubAppAuth {
        app_id: String,
        encoding_key: EncodingKey,
        client: reqwest::Client,
        /// API base; overridable for tests. No trailing slash.
        api_base: String,
        /// lowercased owner → (installation id, expiry). A `None` id means
        /// "looked up, app is not installed on this owner" so we don't
        /// re-hit the API on every read for an owner the app can't see; the
        /// expiry ([`INSTALLATION_TTL`]) bounds how long that verdict (and a
        /// positive id, in case of uninstall+reinstall) is trusted. Keyed
        /// lowercase because GitHub owners are case-insensitive.
        installations: Mutex<HashMap<String, CachedInstallation>>,
        /// (lowercased owner, lowercased repo) → (token-or-negative, expiry).
        /// `Some(token)` is a live installation token; `None` is the negative
        /// verdict "repo not in this install" (422), cached briefly so it
        /// isn't re-minted on every call. Keyed lowercase because GitHub
        /// owner/repo names are case-insensitive, so differently-cased
        /// references share one cached entry.
        tokens: Mutex<HashMap<(String, String), CachedToken>>,
    }

    impl GitHubAppAuth {
        /// Build from an app id and a PEM-encoded RSA private key.
        pub fn new(app_id: impl Into<String>, private_key_pem: &[u8]) -> Result<Self> {
            let encoding_key = EncodingKey::from_rsa_pem(private_key_pem).map_err(|e| {
                CorpusError::Config(format!("invalid GitHub App private key (RSA PEM): {e}"))
            })?;
            let client = reqwest::Client::builder()
                .user_agent("regelrecht-corpus/0.1")
                .connect_timeout(Duration::from_secs(30))
                .timeout(Duration::from_secs(60))
                .build()
                .map_err(|e| CorpusError::Config(format!("failed to create HTTP client: {e}")))?;
            Ok(Self {
                app_id: app_id.into(),
                encoding_key,
                client,
                api_base: "https://api.github.com".to_string(),
                installations: Mutex::new(HashMap::new()),
                tokens: Mutex::new(HashMap::new()),
            })
        }

        /// Construct from the environment, or `None` when the GitHub App is
        /// not configured (so deployments without it transparently fall
        /// back to the env/file token chain).
        ///
        /// - `GITHUB_APP_ID` — the numeric app id (or client id).
        /// - `GITHUB_APP_PRIVATE_KEY` — the PEM contents **or its base64
        ///   encoding**, **or** `GITHUB_APP_PRIVATE_KEY_PATH` — a path to
        ///   the PEM file.
        ///
        /// The base64 form exists because a raw PEM is multiline, and some
        /// deployment platforms (e.g. ZAD passes env vars as a single
        /// newline-separated `KEY=VALUE` blob) corrupt a value that itself
        /// contains newlines. `base64 -w0 key.pem` gives a safe single-line
        /// value.
        ///
        /// Returns `Ok(None)` when `GITHUB_APP_ID` is unset (app simply not
        /// configured). Returns `Err` when the id is set but the key is
        /// missing/unreadable, so a half-configured app surfaces loudly
        /// rather than parsing to a silent no-op — the caller decides whether
        /// that `Err` is fatal (the editor logs it at error level and
        /// continues on the static token chain rather than refusing to boot).
        pub fn from_env() -> Result<Option<Self>> {
            let Ok(app_id) = std::env::var("GITHUB_APP_ID") else {
                return Ok(None);
            };
            if app_id.trim().is_empty() {
                return Ok(None);
            }
            let raw = if let Ok(inline) = std::env::var("GITHUB_APP_PRIVATE_KEY") {
                if inline.trim().is_empty() {
                    return Err(CorpusError::Config(
                        "GITHUB_APP_ID is set but GITHUB_APP_PRIVATE_KEY is empty".to_string(),
                    ));
                }
                inline.into_bytes()
            } else if let Ok(path) = std::env::var("GITHUB_APP_PRIVATE_KEY_PATH") {
                std::fs::read(&path).map_err(|e| {
                    CorpusError::Config(format!(
                        "failed to read GITHUB_APP_PRIVATE_KEY_PATH ({path}): {e}"
                    ))
                })?
            } else {
                return Err(CorpusError::Config(
                    "GITHUB_APP_ID is set but neither GITHUB_APP_PRIVATE_KEY nor \
                     GITHUB_APP_PRIVATE_KEY_PATH is configured"
                        .to_string(),
                ));
            };
            let pem = normalize_private_key(raw)?;
            Ok(Some(Self::new(app_id, &pem)?))
        }

        /// Mint a repo-scoped installation token for `owner/repo`, or
        /// `Ok(None)` when the app is not installed on `owner`. Inherent
        /// helper so call sites that already hold `owner`/`repo` directly
        /// (e.g. the create-traject preflight) don't need to build a
        /// [`Source`].
        pub async fn token_for(&self, owner: &str, repo: &str) -> Result<Option<String>> {
            // Cache key is lowercased (GitHub names are case-insensitive);
            // the original casing is still sent on the API calls below.
            let key = (owner.to_lowercase(), repo.to_lowercase());

            // Serve a still-valid cached result without any network call.
            // A cached `None` is the negative verdict ("repo not in this
            // install"); both are honoured until they expire.
            if let Some((cached, expiry)) = self.tokens.lock().await.get(&key) {
                if *expiry > SystemTime::now() {
                    return Ok(cached.clone());
                }
            }

            // One JWT for the whole cold mint — the installation lookup and
            // the token mint that follow are milliseconds apart and the JWT
            // is valid for minutes, so signing it once (rather than per
            // helper) avoids a redundant RSA signing operation.
            let jwt = self.generate_jwt()?;

            // No installation on this owner → the app can't help here; let
            // the caller fall back to the env/file token chain.
            let Some(installation_id) = self.installation_id(owner, &jwt).await? else {
                return Ok(None);
            };

            // 422 (repo not in this installation's selected set) → the app
            // can't serve this repo; fall back to the static chain. Negative-
            // cache it for the short installation TTL (mirroring the
            // not-installed verdict) so a persistently-not-selected repo
            // doesn't re-mint and 422 on every corpus operation, while a
            // later repo-selection change still takes effect within minutes.
            let Some(token) = self.mint_token(installation_id, repo, &jwt).await? else {
                // Log so an operator can tell a deliberately-excluded repo
                // apart from a typo'd `owner/repo` — both surface as 422, and
                // without this the App bypass is silent for the TTL window.
                tracing::warn!(
                    owner = %owner,
                    repo = %repo,
                    "GitHub App: repo not in installation's selected repositories (422); \
                     falling back to static token chain"
                );
                self.tokens
                    .lock()
                    .await
                    .insert(key, (None, SystemTime::now() + INSTALLATION_TTL));
                return Ok(None);
            };
            let expiry = SystemTime::now() + TOKEN_TTL;
            self.tokens
                .lock()
                .await
                .insert(key, (Some(token.clone()), expiry));
            Ok(Some(token))
        }

        /// Point the client at a different API base — for tests against a
        /// wiremock server. Production uses [`GitHubAppAuth::new`].
        pub fn with_api_base(mut self, base_url: impl Into<String>) -> Self {
            self.api_base = base_url.into().trim_end_matches('/').to_string();
            self
        }

        /// Sign a fresh app-level JWT (RS256, `iss` = app id, ~9 min).
        fn generate_jwt(&self) -> Result<String> {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|e| CorpusError::Config(format!("system clock before unix epoch: {e}")))?
                .as_secs();
            let claims = AppClaims {
                // Backdate `iat` 60s to tolerate minor clock skew between
                // us and GitHub (GitHub rejects a future `iat`).
                iat: now.saturating_sub(60),
                exp: now + JWT_TTL_SECS,
                iss: self.app_id.clone(),
            };
            jsonwebtoken::encode(&Header::new(Algorithm::RS256), &claims, &self.encoding_key)
                .map_err(|e| CorpusError::Config(format!("failed to sign GitHub App JWT: {e}")))
        }

        /// Headers for an app-JWT-authenticated request.
        fn jwt_headers(&self, jwt: &str) -> Result<HeaderMap> {
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
            let auth = HeaderValue::from_str(&format!("Bearer {jwt}"))
                .map_err(|e| CorpusError::Config(format!("invalid JWT header value: {e}")))?;
            headers.insert(AUTHORIZATION, auth);
            Ok(headers)
        }

        /// Resolve (and cache) the installation id of this app on `owner`.
        /// `Ok(None)` means the app is not installed on that owner. Cached
        /// for [`INSTALLATION_TTL`] (both the id and the "not installed"
        /// verdict) so a later install/uninstall is picked up without a
        /// process restart.
        async fn installation_id(&self, owner: &str, jwt: &str) -> Result<Option<u64>> {
            let cache_key = owner.to_lowercase();
            if let Some((id, expiry)) = self.installations.lock().await.get(&cache_key) {
                if *expiry > SystemTime::now() {
                    return Ok(*id);
                }
            }

            // Try the org endpoint first, then the user endpoint — a
            // personal-account install is reachable only via `/users/...`.
            let id = match self.fetch_installation(jwt, "orgs", owner).await? {
                Some(id) => Some(id),
                None => self.fetch_installation(jwt, "users", owner).await?,
            };

            self.installations
                .lock()
                .await
                .insert(cache_key, (id, SystemTime::now() + INSTALLATION_TTL));
            Ok(id)
        }

        /// One installation lookup against `/{kind}/{owner}/installation`.
        /// 404 → `Ok(None)` (not installed / wrong account kind).
        async fn fetch_installation(
            &self,
            jwt: &str,
            kind: &str,
            owner: &str,
        ) -> Result<Option<u64>> {
            let url = format!("{}/{}/{}/installation", self.api_base, kind, owner);
            let response = self
                .client
                .get(&url)
                .headers(self.jwt_headers(jwt)?)
                .send()
                .await
                .map_err(|e| {
                    CorpusError::Git(format!("GitHub App installation lookup failed: {e}"))
                })?;

            if response.status() == reqwest::StatusCode::NOT_FOUND {
                return Ok(None);
            }
            if !response.status().is_success() {
                return Err(CorpusError::Git(format!(
                    "GitHub App installation lookup for {owner} returned {}: {}",
                    response.status(),
                    response.text().await.unwrap_or_default()
                )));
            }
            let parsed: InstallationResponse = response.json().await.map_err(|e| {
                CorpusError::Git(format!("failed to parse installation response: {e}"))
            })?;
            Ok(Some(parsed.id))
        }

        /// Mint an installation token scoped to a single repo with
        /// `contents:write` + `metadata:read`.
        ///
        /// Returns `Ok(None)` when the repo is **not part of this
        /// installation's selected repositories** (GitHub answers 422). That
        /// is the normal case for a least-privilege "only select
        /// repositories" install, not an error — the caller then falls back
        /// to the static token chain instead of hard-failing the source.
        async fn mint_token(
            &self,
            installation_id: u64,
            repo: &str,
            jwt: &str,
        ) -> Result<Option<String>> {
            let url = format!(
                "{}/app/installations/{}/access_tokens",
                self.api_base, installation_id
            );
            let body = serde_json::json!({
                "repositories": [repo],
                "permissions": { "contents": "write", "metadata": "read" },
            });
            let response = self
                .client
                .post(&url)
                .headers(self.jwt_headers(jwt)?)
                .json(&body)
                .send()
                .await
                .map_err(|e| CorpusError::Git(format!("GitHub App token mint failed: {e}")))?;

            // 422 = at least one requested repo isn't accessible to this
            // installation (the "only select repositories" case). The app
            // genuinely can't serve this repo → signal fall-through, don't
            // error.
            if response.status() == reqwest::StatusCode::UNPROCESSABLE_ENTITY {
                return Ok(None);
            }
            if !response.status().is_success() {
                return Err(CorpusError::Git(format!(
                    "GitHub App token mint for installation {installation_id} returned {}: {}",
                    response.status(),
                    response.text().await.unwrap_or_default()
                )));
            }
            let parsed: AccessTokenResponse = response.json().await.map_err(|e| {
                CorpusError::Git(format!("failed to parse access-token response: {e}"))
            })?;
            Ok(Some(parsed.token))
        }
    }

    /// Accept the private key either as a raw PEM (multiline) or as the
    /// base64 encoding of a PEM (single line). A real PEM always carries the
    /// dashed `-----BEGIN ` marker, and `-` is not in the base64 alphabet,
    /// so the marker's presence unambiguously distinguishes the two — no
    /// guessing. The base64 path tolerates embedded whitespace (wrapped
    /// base64) by stripping it before decoding.
    fn normalize_private_key(raw: Vec<u8>) -> Result<Vec<u8>> {
        if raw.windows(11).any(|w| w == b"-----BEGIN ") {
            return Ok(raw);
        }
        let cleaned: Vec<u8> = raw
            .into_iter()
            .filter(|b| !b.is_ascii_whitespace())
            .collect();
        base64::engine::general_purpose::STANDARD
            .decode(&cleaned)
            .map_err(|e| {
                CorpusError::Config(format!(
                    "GITHUB_APP_PRIVATE_KEY is neither a PEM (no -----BEGIN marker) \
                     nor valid base64: {e}"
                ))
            })
    }

    #[async_trait]
    impl AppTokenMinter for GitHubAppAuth {
        /// Mint a token for a GitHub source. Returns `Ok(None)` for non-
        /// GitHub sources (a different provider's minter handles those) and
        /// when the app is not installed on the repo's owner.
        async fn token_for_source(&self, source: &Source) -> Result<Option<String>> {
            let SourceType::GitHub { github } = &source.source_type else {
                return Ok(None);
            };
            self.token_for(&github.owner, &github.repo).await
        }
    }

    #[cfg(test)]
    mod tests {
        #![allow(clippy::unwrap_used)]

        use base64::Engine;
        use wiremock::matchers::{method, path as path_matcher};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        use super::{normalize_private_key, GitHubAppAuth};
        use crate::auth::AppTokenMinter;
        use crate::models::{LocalSource, Source, SourceType};

        /// A throwaway RSA key generated solely for these unit tests — it
        /// authenticates nothing (the mock server never verifies the JWT)
        /// and is not used by any deployment.
        const TEST_PRIVATE_KEY: &str = "-----BEGIN PRIVATE KEY-----
MIIEvQIBADANBgkqhkiG9w0BAQEFAASCBKcwggSjAgEAAoIBAQC0++TFH/Dgo3/y
0XlFxJsNellsFDs6PLPNvuWvXHIkdkPlPtl1usDpd1G/J0k5DlQ0doE1BjXh9+Yw
uPGsfLoHNhrQY63BVdvqwQw20ryfKfF62FuMtb7NG++LmVRTF6bBxgN0/8wmhRHw
oHacnWhlDnLZnFvnh5NyDw4N5+I5vAilxQc9UPjk7WseiEGnDI5tihH1/qQWqjn0
h0jTc3JPtbNwg2+tAHQQTX4zUSiWbiO0s/TPdwz9HpnlnvZejzEA5DLRLHN8HFSk
UCTK28Pch+QabF6ShPe9MT8YGca4H7q5o46GISs0+MjXPYTA3oQdlv62n901eBH5
v1fkk30VAgMBAAECggEABW//+pAwG3+uC2yRuS/j/K6tWxrsgJ5sRIU0v2UGpOPM
vGl7/RaETz9ffF/Aym8Mxpq83Qv3uHdLOjGESUtiB29vtM0Z3lInDlGIfWktc5a+
A5PWhE69kcoCE26i4vA1+WJqnixFRO8Aj/syNRhhl4+ska8p77XaDzY2lOJfoJ5h
krzu0UxQhxo09JM3GdwZUOIpk0/8AJAhFU5SI5dlZgm03uWLiEQvMeZy931UcUw0
IpKPIn4Wob7xsPZGxDMJEhxhr9HJmpuUIbl5I2Hf1OVhrVUXOAdhlxHszzsxJ82T
eFBv6sNrBuJczDY2HDhW5j1pppHG9w2O+W8OaiHrbQKBgQD2Yso4AMwucZx+Dalp
E32aslLCPrINWux+u/fxWgO6UI9SzaiCVWR5WEMqUH0JZyP5NAiBQToWha+OZt71
b+EuaXF2cRUoxG8coTT/VvG0cOa6MrtJ+G38cVOeyOe7iqcjeSidWG5MxdSZbfXr
+3h7U/rr6qBiOiBc4ILux/RkvwKBgQC8C8mXbgmPG/r6fF7z//cSSYVCyGNuOjqX
ikl3NaD+7bbWF2y/mfQzInBoLSij+oytdQIUYLQrqhCY1Xlw0EgsTrX64vRkigR+
LlKM9rU/CS4n3KutnUMbOK+/GkdyJnwyHUIYasMl5zmMjrkO5hqcJkUbgp0ekCRZ
DH2RZFivKwKBgQDTjOlSgqTOL/CNbw+J0BllzT0v2YMp4mrzOlPeoEpZHDijgT/x
gH5/jiBFYcyqWSvTGjE/QhEtK2YcYAmKNaDkJ9crOldPpLI+o9AMecuZAeOp9ktH
bQ6K1YdV6+zE4301AR+1UiuKscYkYvznvQiq4+Wr0M4a6QvGk2L4wSj/owKBgBBg
xnIV92crfLSMWIjP5mkFVkH2yhIzqB7CwJtNZHRPp/kFmUcm1YoOmdO4+y0tCUui
QUgdFBQpf8CP9z/IJEEXqensEnUfQDztM+trIWYYGpkGMz2v0MRyL3xpgYeDqpWC
ztrpkY2fkfeYBq4xhGfNPX+j5KNg0ome+ODM6Jx5AoGAKuk2voq6qiK6GK+OjAUn
Dowzu2cOag3v0urvaYlmKAjlN0T03sm3PHb6luiaFGiADbtgAe+Qa+4Xsy4csFVQ
Bz/gbfBkAPp8C+YDnkQV0dls16CS9wWhTrg8eJustV6QJ96Uw9h4+WZdPZCo9D9s
/qtQyRgVfnPLqQdv00kx3gw=
-----END PRIVATE KEY-----";

        fn app(server: &MockServer) -> GitHubAppAuth {
            GitHubAppAuth::new("123456", TEST_PRIVATE_KEY.as_bytes())
                .unwrap()
                .with_api_base(server.uri())
        }

        fn github_source(owner: &str, repo: &str) -> Source {
            Source {
                id: format!("{owner}-{repo}"),
                name: format!("{owner}/{repo}"),
                source_type: SourceType::GitHub {
                    github: crate::models::GitHubSource {
                        owner: owner.to_string(),
                        repo: repo.to_string(),
                        branch: "main".to_string(),
                        path: None,
                        git_ref: None,
                    },
                },
                scopes: Vec::new(),
                priority: 0,
                auth_ref: None,
            }
        }

        /// The private key may be given as raw PEM (multiline) or as base64
        /// of the PEM (single line, for env vars that can't carry
        /// newlines). Both must reach the same key bytes; pure garbage is an
        /// error, not a silent empty key.
        #[test]
        fn normalize_private_key_accepts_raw_pem_and_base64() {
            let pem = TEST_PRIVATE_KEY.as_bytes().to_vec();
            assert_eq!(normalize_private_key(pem.clone()).unwrap(), pem);

            let b64 = base64::engine::general_purpose::STANDARD.encode(&pem);
            assert_eq!(normalize_private_key(b64.into_bytes()).unwrap(), pem);

            assert!(normalize_private_key(b"neither pem nor base64 !!!".to_vec()).is_err());
        }

        /// A non-GitHub source is not this provider's concern: the minter
        /// returns `None` (so the resolver falls back to the static chain)
        /// without any network call.
        #[tokio::test]
        async fn non_github_source_returns_none() {
            let server = MockServer::start().await;
            let auth = app(&server);
            let local = Source {
                id: "local".to_string(),
                name: "Local".to_string(),
                source_type: SourceType::Local {
                    local: LocalSource {
                        path: "corpus".into(),
                    },
                },
                scopes: Vec::new(),
                priority: 0,
                auth_ref: None,
            };
            assert_eq!(auth.token_for_source(&local).await.unwrap(), None);
        }

        /// Happy path: look up the org installation, mint a repo-scoped
        /// token, and cache it — a second call serves from cache with no
        /// extra network round-trips (both mocks `.expect(1)`).
        #[tokio::test]
        async fn mints_and_caches_installation_token() {
            let server = MockServer::start().await;

            Mock::given(method("GET"))
                .and(path_matcher("/orgs/acme/installation"))
                .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "id": 42
                })))
                .expect(1)
                .mount(&server)
                .await;
            Mock::given(method("POST"))
                .and(path_matcher("/app/installations/42/access_tokens"))
                .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
                    "token": "ghs_installation_token"
                })))
                .expect(1)
                .mount(&server)
                .await;

            let auth = app(&server);
            let src = github_source("acme", "private-corpus");

            let first = auth.token_for_source(&src).await.unwrap();
            assert_eq!(first.as_deref(), Some("ghs_installation_token"));
            // Second call must be served from the cache (no extra requests).
            let second = auth.token_for_source(&src).await.unwrap();
            assert_eq!(second.as_deref(), Some("ghs_installation_token"));
        }

        /// "Only select repositories" install: the owner has an
        /// installation, but the target repo isn't in its selected set, so
        /// the mint returns 422. That must surface as `Ok(None)` (the app
        /// can't serve this repo → fall back to the static chain), not an
        /// error that would break the fallback.
        #[tokio::test]
        async fn repo_not_in_installation_returns_none() {
            let server = MockServer::start().await;
            Mock::given(method("GET"))
                .and(path_matcher("/orgs/acme/installation"))
                .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "id": 42
                })))
                .mount(&server)
                .await;
            // Exactly one mint: the 422 verdict must be negative-cached so a
            // second resolution doesn't hit the API again.
            Mock::given(method("POST"))
                .and(path_matcher("/app/installations/42/access_tokens"))
                .respond_with(ResponseTemplate::new(422))
                .expect(1)
                .mount(&server)
                .await;

            let auth = app(&server);
            assert_eq!(
                auth.token_for("acme", "not-granted-repo").await.unwrap(),
                None
            );
            assert_eq!(
                auth.token_for("acme", "not-granted-repo").await.unwrap(),
                None
            );
        }

        /// A personal account exposes its install only via `/users/...`;
        /// the org lookup 404s and we transparently fall through.
        #[tokio::test]
        async fn falls_back_to_user_installation() {
            let server = MockServer::start().await;

            Mock::given(method("GET"))
                .and(path_matcher("/orgs/octocat/installation"))
                .respond_with(ResponseTemplate::new(404))
                .mount(&server)
                .await;
            Mock::given(method("GET"))
                .and(path_matcher("/users/octocat/installation"))
                .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "id": 7
                })))
                .mount(&server)
                .await;
            Mock::given(method("POST"))
                .and(path_matcher("/app/installations/7/access_tokens"))
                .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
                    "token": "ghs_user_token"
                })))
                .mount(&server)
                .await;

            let auth = app(&server);
            let token = auth.token_for("octocat", "personal-repo").await.unwrap();
            assert_eq!(token.as_deref(), Some("ghs_user_token"));
        }

        /// The app is installed on neither the org nor the user: both
        /// lookups 404, so we return `None` and let the caller fall back to
        /// the env/file token chain — we never attempt a mint.
        #[tokio::test]
        async fn not_installed_returns_none() {
            let server = MockServer::start().await;

            Mock::given(method("GET"))
                .and(path_matcher("/orgs/stranger/installation"))
                .respond_with(ResponseTemplate::new(404))
                .mount(&server)
                .await;
            Mock::given(method("GET"))
                .and(path_matcher("/users/stranger/installation"))
                .respond_with(ResponseTemplate::new(404))
                .mount(&server)
                .await;

            let auth = app(&server);
            assert_eq!(auth.token_for("stranger", "repo").await.unwrap(), None);
        }
    }
}

#[cfg(feature = "github")]
pub use inner::GitHubAppAuth;
