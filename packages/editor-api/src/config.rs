pub use regelrecht_auth::OidcConfig;

#[derive(Clone, Debug)]
pub struct AppConfig {
    pub oidc: Option<OidcConfig>,
    pub base_url: Option<String>,
    pub allowed_hosts: Vec<String>,
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
            tracing::warn!("OIDC authentication is DISABLED — editor is unprotected");
        }

        let base_url = regelrecht_auth::parse_base_url()?;
        if base_url.is_none() && oidc.is_some() {
            tracing::info!(
                "BASE_URL is not set — OIDC redirect URLs will be derived from request headers"
            );
        }

        let allowed_hosts = regelrecht_auth::parse_allowed_hosts();

        Ok(Self {
            oidc,
            base_url,
            allowed_hosts,
        })
    }

    pub fn is_auth_enabled(&self) -> bool {
        self.oidc.is_some()
    }
}
