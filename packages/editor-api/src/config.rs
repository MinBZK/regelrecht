pub use regelrecht_auth::OidcConfig;

use crate::github_oauth::GithubOAuth;

#[derive(Clone)]
pub struct AppConfig {
    pub oidc: Option<OidcConfig>,
    pub base_url: Option<String>,
    /// GitHub user-OAuth config (spike). `None` disables the link flow and
    /// keeps the corpus write path on its existing service/App token.
    pub github_oauth: Option<GithubOAuth>,
    /// LLM provider used for traject enrich-op-aanvragen (the taak-flow
    /// enrich endpoint). Overridable per deployment via `TASK_ENRICH_PROVIDER`.
    /// Defaults to `"claude"` — deliberately not the worker's own
    /// `LLM_PROVIDER` default (`"opencode"`, see `enrich.rs`): a human is
    /// waiting on this result, and `claude` is the provider the enrich
    /// pipeline treats as "with provenance" (see reset-exhausted docs).
    pub task_enrich_provider: String,
}

impl AppConfig {
    pub fn from_env() -> Self {
        match Self::try_from_env() {
            Ok(config) => config,
            Err(e) => {
                tracing::error!("{e}");
                std::process::exit(1);
            }
        }
    }

    fn try_from_env() -> Result<Self, String> {
        let oidc = regelrecht_auth::parse_oidc_from_env()?;

        if oidc.is_some() {
            tracing::info!("OIDC authentication is enabled");
        } else {
            tracing::warn!(
                "OIDC authentication is DISABLED — editor is unprotected. \
                 All routes (editor-reader/writer/admin tiers) bypass auth checks. \
                 Do NOT run this configuration in production."
            );
        }

        let base_url = regelrecht_auth::parse_base_url()?;
        if base_url.is_none() && oidc.is_some() {
            tracing::info!(
                "BASE_URL is not set — OIDC redirect URLs will be derived from request headers"
            );
        }

        let github_oauth = GithubOAuth::from_env()?;
        if let Some(gh) = github_oauth.as_ref() {
            tracing::info!(
                require_user_token = gh.require_user_token,
                "GitHub user-OAuth is enabled (per-user traject writes)"
            );
        }

        let task_enrich_provider = std::env::var("TASK_ENRICH_PROVIDER")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "claude".to_string());
        if !regelrecht_pipeline::enrich::ENRICH_PROVIDERS.contains(&task_enrich_provider.as_str()) {
            return Err(format!(
                "TASK_ENRICH_PROVIDER={task_enrich_provider:?} is geen geldige provider \
                 (verwacht een van {:?}). Een typo hier valt in de worker stil terug op de \
                 default provider en omzeilt de per-provider-uurcap — fail-fast bij opstarten.",
                regelrecht_pipeline::enrich::ENRICH_PROVIDERS
            ));
        }

        Ok(Self {
            oidc,
            base_url,
            github_oauth,
            task_enrich_provider,
        })
    }

    pub fn is_auth_enabled(&self) -> bool {
        self.oidc.is_some()
    }
}
