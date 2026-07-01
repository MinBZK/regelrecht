use std::path::Path;
use std::sync::Arc;

use crate::error::{CorpusError, Result};
use crate::models::{Source, SourceType};

/// Mints a short-lived, repo-scoped token for a source on demand, instead
/// of relying on a long-lived stored PAT.
///
/// One implementation per VCS provider — today
/// [`crate::github_app::GitHubAppAuth`] (GitHub App installation tokens).
/// Kept as an abstract trait here (not behind the `github` feature) so the
/// resolver, the [`ProviderAuthRegistry`] factory, and the editor-api
/// state can refer to it without pulling in reqwest; the concrete,
/// network-backed implementations live in their provider modules.
///
/// `token_for_source` returns `Ok(None)` when this provider can't serve
/// the source — the source is a different provider's kind, or the app has
/// no access (e.g. not installed on the owner). That's the signal for the
/// caller to fall back to the static env/file token chain rather than
/// fail.
#[async_trait::async_trait]
pub trait AppTokenMinter: Send + Sync {
    async fn token_for_source(&self, source: &Source) -> Result<Option<String>>;
}

/// Factory + registry of per-provider credential minters.
///
/// Built once from the environment ([`ProviderAuthRegistry::from_env`]):
/// each provider self-configures and is simply absent when its env isn't
/// set. [`minter_for`](Self::minter_for) routes a source to its provider's
/// minter by [`SourceType`], so adding GitLab (or any other provider)
/// later is a new field + a new match arm here — call sites don't change.
#[derive(Default, Clone)]
pub struct ProviderAuthRegistry {
    /// GitHub App minter. Other providers (GitLab, Azure DevOps, …) get
    /// their own field here when implemented.
    github: Option<Arc<dyn AppTokenMinter>>,
}

impl ProviderAuthRegistry {
    /// Load every supported provider's minter from the environment. A
    /// provider whose env isn't configured stays `None` (the source then
    /// falls back to the static token chain). Errors only when a provider
    /// is half-configured (e.g. GitHub app id set but key unreadable).
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            github: load_github_app_minter()?,
        })
    }

    /// The minter for `source`'s provider, or `None` when that provider
    /// has no app-auth configured / the source has no remote provider
    /// (local).
    pub fn minter_for(&self, source: &Source) -> Option<&dyn AppTokenMinter> {
        match &source.source_type {
            SourceType::GitHub { .. } => self.github.as_deref(),
            SourceType::Local { .. } => None,
        }
    }

    /// Whether any provider minter is configured — for startup logging so
    /// operators can confirm the app-auth path is active.
    pub fn is_configured(&self) -> bool {
        self.github.is_some()
    }
}

/// Build the GitHub App minter from the environment, or `None` when the
/// app isn't configured. Gated on the `github` feature; without it the
/// provider is simply unavailable.
#[cfg(feature = "github")]
fn load_github_app_minter() -> Result<Option<Arc<dyn AppTokenMinter>>> {
    Ok(crate::github_app::GitHubAppAuth::from_env()?
        .map(|a| Arc::new(a) as Arc<dyn AppTokenMinter>))
}

#[cfg(not(feature = "github"))]
fn load_github_app_minter() -> Result<Option<Arc<dyn AppTokenMinter>>> {
    Ok(None)
}

/// Resolve a token for a source, preferring a provider-minted short-lived
/// token (GitHub App, …) when one is available, then the static env/file
/// chain.
///
/// Resolution order:
/// 0. **Provider minter** (`providers`) — routed by [`SourceType`]. Mints
///    a short-lived, repo-scoped token; nothing long-lived is stored. A
///    `None` from the minter (provider not configured / app not installed)
///    falls through to the static chain.
/// 1.–N. the static chain via [`resolve_token_strict`] (when `strict`) or
///    [`resolve_token_for_source`] (otherwise).
///
/// `strict` mirrors the two static resolvers: pass `true` for any source
/// whose lookup key is derived from untrusted input (the writable-own
/// source of a traject), so an unknown key fails closed instead of
/// leaking the legacy `CORPUS_GIT_TOKEN`.
pub async fn resolve_token_async(
    source: &Source,
    auth_file: Option<&Path>,
    providers: Option<&ProviderAuthRegistry>,
    strict: bool,
) -> Result<Option<String>> {
    if let Some(providers) = providers {
        if let Some(minter) = providers.minter_for(source) {
            match minter.token_for_source(source).await {
                Ok(Some(token)) => return Ok(Some(token)),
                // The provider can't serve this source (not installed / repo
                // not in the install) — fall through to the static chain.
                Ok(None) => {}
                // A provider failure (transient GitHub 5xx/429, mint error,
                // expired key, …) must NOT hard-fail the source: the whole
                // point of the hybrid is that the static env/file token keeps
                // working. Log and fall through rather than propagate.
                Err(e) => {
                    tracing::warn!(
                        source_id = %source.id,
                        error = %e,
                        "provider token mint failed; falling back to static token chain"
                    );
                }
            }
        }
    }

    if strict {
        let key = source.auth_ref.as_deref().unwrap_or(&source.id);
        resolve_token_strict(key, auth_file)
    } else {
        resolve_token_for_source(&source.id, source.auth_ref.as_deref(), auth_file)
    }
}

/// Construct the per-source token env-var name from a slug.
///
/// Single source of truth — used by the resolver and surfaced in
/// operator-facing error messages so the name shown to the operator
/// matches what the resolver actually looks up. Keeping it in one place
/// guarantees that if the naming scheme ever changes (lowercase prefix,
/// extra separator, etc.) the resolver and the diagnostic log can't
/// silently drift apart and confuse an operator who follows the error
/// message.
pub fn token_env_name(auth_ref: &str) -> String {
    format!(
        "CORPUS_AUTH_{}_TOKEN",
        auth_ref.to_uppercase().replace('-', "_")
    )
}

/// Auth configuration for a corpus source, loaded from `corpus-auth.yaml`.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct AuthConfig {
    pub sources: Vec<SourceAuth>,
}

/// Auth entry for a single source.
#[derive(Clone, serde::Deserialize)]
pub struct SourceAuth {
    pub id: String,
    pub token: String,
}

impl std::fmt::Debug for SourceAuth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SourceAuth")
            .field("id", &self.id)
            .field("token", &"***")
            .finish()
    }
}

/// Look up a GitHub token for a source.
///
/// Uses `auth_ref` as the lookup key when provided, otherwise falls back
/// to `source_id`. This matches the RFC-010 design where `auth_ref` in the
/// manifest references an entry in the auth config.
///
/// Resolution order:
/// 1. Environment variable `CORPUS_AUTH_{KEY}_TOKEN` (uppercased, hyphens → underscores)
/// 2. `corpus-auth.yaml` file (if it exists)
/// 3. Legacy shared `CORPUS_GIT_TOKEN` (harvester-era fallback)
/// 4. None (unauthenticated, lower rate limits)
///
/// **Security**: the legacy fallback at step 3 returns the same token
/// for *any* `auth_ref`, which makes it unsafe whenever the `auth_ref`
/// is derived from user input (e.g. a user picking which repo a
/// traject points at). Use [`resolve_token_strict`] there instead —
/// it omits the legacy fallback so an unknown `auth_ref` cleanly
/// returns `None` rather than leaking the central token to a
/// user-chosen repo on the outgoing API call.
pub fn resolve_token_for_source(
    source_id: &str,
    auth_ref: Option<&str>,
    auth_file: Option<&Path>,
) -> Result<Option<String>> {
    let key = auth_ref.unwrap_or(source_id);
    resolve_token(key, auth_file)
}

/// Like [`resolve_token_for_source`] but only consults the per-source
/// env var and the auth file. Returns `None` when neither yields a
/// token — the legacy `CORPUS_GIT_TOKEN` fallback is intentionally
/// skipped.
///
/// Use this for any code path where the lookup key is derived from
/// untrusted input. Today that's the writable-own source of a traject
/// whose repo coordinates came from the create-request — without this
/// guard, an attacker-controlled `auth_ref` would fall through to the
/// shared `CORPUS_GIT_TOKEN` and ship it (via `Authorization: Bearer`)
/// to a repo the attacker controls, a token-exfiltration vector.
pub fn resolve_token_strict(auth_ref: &str, auth_file: Option<&Path>) -> Result<Option<String>> {
    // 1. Per-source environment variable
    let env_key = token_env_name(auth_ref);
    if let Ok(token) = std::env::var(&env_key) {
        if !token.is_empty() {
            return Ok(Some(token));
        }
    }

    // 2. Auth file. We deliberately do NOT fall through to
    // `CORPUS_GIT_TOKEN` afterwards — see `resolve_token` for the legacy
    // fallback path.
    find_in_auth_file(auth_ref, auth_file)
}

/// Read the auth file (if any) and look up `auth_ref` in it. Returns
/// `None` when the file is absent or the ref isn't listed. Shared
/// between [`resolve_token`] and [`resolve_token_strict`] so both
/// parsers stay in lock-step — a future change to the auth-file shape
/// only needs to land here.
fn find_in_auth_file(auth_ref: &str, auth_file: Option<&Path>) -> Result<Option<String>> {
    let Some(path) = auth_file else {
        return Ok(None);
    };
    if !path.exists() {
        return Ok(None);
    }
    let content = std::fs::read_to_string(path).map_err(|e| {
        CorpusError::Config(format!(
            "Failed to read auth file {}: {}",
            path.display(),
            e
        ))
    })?;
    let config: AuthConfig = serde_yaml_ng::from_str(&content).map_err(|e| {
        CorpusError::Config(format!(
            "Failed to parse auth file {}: {}",
            path.display(),
            e
        ))
    })?;
    Ok(config
        .sources
        .iter()
        .find(|s| s.id == auth_ref)
        .map(|entry| entry.token.clone()))
}

/// Look up a GitHub token by key.
///
/// Resolution order:
/// 1. Per-source environment variable `CORPUS_AUTH_{KEY}_TOKEN`
///    (uppercased, hyphens → underscores). Use this for multi-source
///    setups that need isolation between source repos.
/// 2. `corpus-auth.yaml` file (if it exists) — explicit per-source
///    entries, same purpose as (1) but file-backed.
/// 3. Legacy shared environment variable `CORPUS_GIT_TOKEN`. The
///    harvester era used this single global token for one upstream
///    repo. We accept it as a fallback so deployments that still set
///    only the legacy variable keep working for *every* GitHub source
///    without needing to duplicate the secret under each per-source
///    name. Per-source vars set above still win.
/// 4. None (unauthenticated, lower rate limits).
pub fn resolve_token(source_id: &str, auth_file: Option<&Path>) -> Result<Option<String>> {
    // 1. Per-source environment variable
    let env_key = token_env_name(source_id);
    if let Ok(token) = std::env::var(&env_key) {
        if !token.is_empty() {
            return Ok(Some(token));
        }
    }

    // 2. Auth file
    if let Some(token) = find_in_auth_file(source_id, auth_file)? {
        return Ok(Some(token));
    }

    // 3. Legacy shared CORPUS_GIT_TOKEN (harvester-era single token)
    if let Ok(token) = std::env::var("CORPUS_GIT_TOKEN") {
        if !token.is_empty() {
            return Ok(Some(token));
        }
    }

    // 4. No token
    Ok(None)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::sync::Mutex;
    use tempfile::NamedTempFile;

    // Tests in this module mutate the process-wide environment via
    // `CORPUS_GIT_TOKEN` / per-source vars. cargo's default parallel
    // test runner would then race two tests reading/writing the same
    // env vars and produce flakes. Serialize the env-mutating tests
    // behind one mutex so they take turns. Tests that only read a
    // unique-named var don't need it.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn test_resolve_token_from_env() {
        // Use a unique env var name to avoid test interference
        let key = "CORPUS_AUTH_TEST_SOURCE_123_TOKEN";
        unsafe { std::env::set_var(key, "env-token-value") };

        let result = resolve_token("test-source-123", None).unwrap();
        assert_eq!(result, Some("env-token-value".to_string()));

        unsafe { std::env::remove_var(key) };
    }

    #[test]
    fn test_resolve_token_from_file() {
        let yaml = r#"
sources:
  - id: amsterdam
    token: file-token-123
  - id: rotterdam
    token: file-token-456
"#;
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(yaml.as_bytes()).unwrap();

        let result = resolve_token("amsterdam", Some(file.path())).unwrap();
        assert_eq!(result, Some("file-token-123".to_string()));

        let result = resolve_token("rotterdam", Some(file.path())).unwrap();
        assert_eq!(result, Some("file-token-456".to_string()));
    }

    #[test]
    fn test_resolve_token_none() {
        let _g = ENV_LOCK.lock().unwrap();
        // Make sure the legacy CORPUS_GIT_TOKEN doesn't leak from the
        // test runner's environment and turn this assertion into a flake.
        let legacy_was_set = std::env::var("CORPUS_GIT_TOKEN").ok();
        unsafe { std::env::remove_var("CORPUS_GIT_TOKEN") };
        let result = resolve_token("nonexistent", None).unwrap();
        if let Some(prev) = legacy_was_set {
            unsafe { std::env::set_var("CORPUS_GIT_TOKEN", prev) };
        }
        assert_eq!(result, None);
    }

    #[test]
    fn test_resolve_token_falls_back_to_legacy_corpus_git_token() {
        let _g = ENV_LOCK.lock().unwrap();
        // No per-source var, no auth file → resolver picks up
        // CORPUS_GIT_TOKEN so deployments that still set only the legacy
        // single-token variable keep working across all sources.
        let prev = std::env::var("CORPUS_GIT_TOKEN").ok();
        unsafe { std::env::set_var("CORPUS_GIT_TOKEN", "legacy-token-value") };
        let result = resolve_token("any-source-here", None).unwrap();
        match prev {
            Some(v) => unsafe { std::env::set_var("CORPUS_GIT_TOKEN", v) },
            None => unsafe { std::env::remove_var("CORPUS_GIT_TOKEN") },
        }
        assert_eq!(result, Some("legacy-token-value".to_string()));
    }

    #[test]
    fn test_per_source_env_wins_over_legacy_corpus_git_token() {
        let _g = ENV_LOCK.lock().unwrap();
        let prev_legacy = std::env::var("CORPUS_GIT_TOKEN").ok();
        let per_source_key = "CORPUS_AUTH_PRIORITY_TEST_TOKEN";
        unsafe { std::env::set_var("CORPUS_GIT_TOKEN", "legacy-loses") };
        unsafe { std::env::set_var(per_source_key, "per-source-wins") };
        let result = resolve_token("priority-test", None).unwrap();
        unsafe { std::env::remove_var(per_source_key) };
        match prev_legacy {
            Some(v) => unsafe { std::env::set_var("CORPUS_GIT_TOKEN", v) },
            None => unsafe { std::env::remove_var("CORPUS_GIT_TOKEN") },
        }
        assert_eq!(result, Some("per-source-wins".to_string()));
    }

    #[test]
    fn test_resolve_token_missing_file_is_none() {
        let result = resolve_token("test", Some(Path::new("/nonexistent/auth.yaml"))).unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn resolve_token_strict_skips_legacy_corpus_git_token() {
        // The whole point of `resolve_token_strict`: an unknown auth_ref
        // must NOT fall through to `CORPUS_GIT_TOKEN`. If it did, an
        // attacker who can pick `auth_ref` (via repo coords on the
        // create-traject endpoint) would harvest the operator's
        // central token on the next outgoing GitHub call. This test
        // pins the strict behaviour so a future refactor of the
        // resolver can't silently regress it.
        let _g = ENV_LOCK.lock().unwrap();
        let prev = std::env::var("CORPUS_GIT_TOKEN").ok();
        unsafe { std::env::set_var("CORPUS_GIT_TOKEN", "central-secret") };
        let result = resolve_token_strict("attacker-supplied-key", None).unwrap();
        match prev {
            Some(v) => unsafe { std::env::set_var("CORPUS_GIT_TOKEN", v) },
            None => unsafe { std::env::remove_var("CORPUS_GIT_TOKEN") },
        }
        assert_eq!(
            result, None,
            "strict resolver must NOT leak the legacy token"
        );
    }

    #[test]
    fn resolve_token_strict_still_uses_per_source_env() {
        // The strict variant still honours the explicit per-source env
        // var — that's the path operators are supposed to configure.
        // Holds `ENV_LOCK` like the sibling strict test: even though
        // the key here is unique, env-mutating tests run serialised so
        // a concurrent test holding the lock and mutating arbitrary
        // env vars can't race with this one.
        let _g = ENV_LOCK.lock().unwrap();
        let key = "CORPUS_AUTH_STRICT_PER_SOURCE_OK_TOKEN";
        unsafe { std::env::set_var(key, "per-source-value") };
        let result = resolve_token_strict("strict-per-source-ok", None).unwrap();
        unsafe { std::env::remove_var(key) };
        assert_eq!(result, Some("per-source-value".to_string()));
    }

    /// A minter that returns a fixed answer, to exercise the resolver's
    /// provider tier without a network call.
    struct FakeMinter(Option<String>);

    #[async_trait::async_trait]
    impl AppTokenMinter for FakeMinter {
        async fn token_for_source(&self, _source: &Source) -> Result<Option<String>> {
            Ok(self.0.clone())
        }
    }

    /// A minter that always errors, to prove a provider failure doesn't
    /// hard-fail the source but falls through to the static chain.
    struct FailingMinter;

    #[async_trait::async_trait]
    impl AppTokenMinter for FailingMinter {
        async fn token_for_source(&self, _source: &Source) -> Result<Option<String>> {
            Err(CorpusError::Git(
                "simulated transient GitHub error".to_string(),
            ))
        }
    }

    fn github_probe(id: &str) -> Source {
        Source {
            id: id.to_string(),
            name: id.to_string(),
            source_type: SourceType::GitHub {
                github: crate::models::GitHubSource {
                    owner: "acme".to_string(),
                    repo: "corpus".to_string(),
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

    /// A default (unconfigured) registry has no minter for any source, so
    /// `resolve_token_async` behaves exactly like the static chain.
    #[test]
    fn default_registry_has_no_minter() {
        let registry = ProviderAuthRegistry::default();
        assert!(!registry.is_configured());
        assert!(registry.minter_for(&github_probe("x")).is_none());
    }

    /// When a provider minter yields a token it wins over the static
    /// env/file chain — the whole point of the app-auth tier.
    #[tokio::test]
    async fn resolve_token_async_prefers_provider_minter() {
        let registry = ProviderAuthRegistry {
            github: Some(Arc::new(FakeMinter(Some("minted-token".to_string())))),
        };
        let src = github_probe("provider-wins-src");
        let got = resolve_token_async(&src, None, Some(&registry), true)
            .await
            .unwrap();
        assert_eq!(got.as_deref(), Some("minted-token"));
    }

    /// When the minter declines (app not installed → `None`), the resolver
    /// falls back to the static per-source env var. Uses a unique key, so
    /// no `ENV_LOCK` and no legacy fallback are involved.
    #[tokio::test]
    async fn resolve_token_async_falls_back_when_minter_declines() {
        let key = "CORPUS_AUTH_PROVIDER_FALLBACK_SRC_TOKEN";
        unsafe { std::env::set_var(key, "env-fallback-token") };
        let registry = ProviderAuthRegistry {
            github: Some(Arc::new(FakeMinter(None))),
        };
        let src = github_probe("provider-fallback-src");
        let got = resolve_token_async(&src, None, Some(&registry), true)
            .await
            .unwrap();
        unsafe { std::env::remove_var(key) };
        assert_eq!(got.as_deref(), Some("env-fallback-token"));
    }

    /// A provider *error* (transient GitHub failure, mint error) must not
    /// hard-fail: the resolver logs and still falls through to the static
    /// per-source env var instead of propagating the error.
    #[tokio::test]
    async fn resolve_token_async_falls_back_when_minter_errors() {
        let key = "CORPUS_AUTH_PROVIDER_ERR_SRC_TOKEN";
        unsafe { std::env::set_var(key, "env-after-error") };
        let registry = ProviderAuthRegistry {
            github: Some(Arc::new(FailingMinter)),
        };
        let src = github_probe("provider-err-src");
        let got = resolve_token_async(&src, None, Some(&registry), true)
            .await
            .unwrap();
        unsafe { std::env::remove_var(key) };
        assert_eq!(got.as_deref(), Some("env-after-error"));
    }
}
