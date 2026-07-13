pub use regelrecht_auth::OidcConfig;

use crate::github_oauth::GithubOAuth;

#[derive(Clone)]
pub struct AppConfig {
    pub oidc: Option<OidcConfig>,
    pub base_url: Option<String>,
    /// GitHub user-OAuth config (spike). `None` disables the link flow and
    /// keeps the corpus write path on its existing service/App token.
    pub github_oauth: Option<GithubOAuth>,
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

        Ok(Self {
            oidc,
            base_url,
            github_oauth,
        })
    }

    pub fn is_auth_enabled(&self) -> bool {
        self.oidc.is_some()
    }
}
