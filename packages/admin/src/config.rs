use std::env;

use sha2::{Digest, Sha256};

pub use regelrecht_auth::OidcConfig;

#[derive(Clone)]
pub struct AppConfig {
    pub oidc: Option<OidcConfig>,
    pub base_url: Option<String>,
    pub allowed_hosts: Vec<String>,
    pub api_key: Option<String>,
    /// Pre-computed SHA-256 hash of the API key (avoids re-hashing on every request).
    pub api_key_hash: Option<[u8; 32]>,
}

impl std::fmt::Debug for AppConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppConfig")
            .field("oidc", &self.oidc)
            .field("base_url", &self.base_url)
            .field("api_key", &self.api_key.as_ref().map(|_| "[REDACTED]"))
            .field("api_key_hash", &self.api_key_hash.map(|_| "[REDACTED]"))
            .finish()
    }
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
                "OIDC authentication is DISABLED — admin panel is unprotected. \
                 Configure OIDC environment variables to enable authentication."
            );
        }

        let base_url = regelrecht_auth::parse_base_url()?;
        if base_url.is_none() && oidc.is_some() {
            tracing::info!(
                "BASE_URL is not set — OIDC redirect URLs will be derived from request headers"
            );
        }

        let api_key = env::var("ADMIN_API_KEY").ok().filter(|s| !s.is_empty());
        if let Some(ref key) = api_key {
            if key.len() < 32 {
                tracing::warn!(
                    "ADMIN_API_KEY is shorter than 32 characters — consider using a longer key"
                );
            }
            tracing::info!("API key authentication is enabled (GET + DELETE)");
        }

        let api_key_hash = api_key
            .as_ref()
            .map(|k| Sha256::digest(k.as_bytes()).into());

        let allowed_hosts = regelrecht_auth::parse_allowed_hosts();

        Ok(Self {
            oidc,
            base_url,
            allowed_hosts,
            api_key,
            api_key_hash,
        })
    }

    pub fn is_auth_enabled(&self) -> bool {
        self.oidc.is_some()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    const OIDC_VARS: &[&str] = &[
        "OIDC_CLIENT_ID",
        "OIDC_CLIENT_SECRET",
        "OIDC_DISCOVERY_URL",
        "KEYCLOAK_BASE_URL",
        "KEYCLOAK_REALM",
        "OIDC_REQUIRED_ROLE",
    ];

    fn clear_env() {
        for var in OIDC_VARS {
            env::remove_var(var);
        }
        env::remove_var("ADMIN_API_KEY");
        env::remove_var("BASE_URL");
    }

    fn set_complete_oidc_env() {
        env::set_var("OIDC_CLIENT_ID", "test-client");
        env::set_var("OIDC_CLIENT_SECRET", "secret");
        env::set_var("KEYCLOAK_BASE_URL", "https://keycloak.example.com");
        env::set_var("KEYCLOAK_REALM", "test-realm");
    }

    #[test]
    fn no_oidc_vars_disables_auth() {
        let _lock = ENV_LOCK.lock();
        clear_env();

        let config = AppConfig::try_from_env().expect("should succeed");
        assert!(config.oidc.is_none());
        assert!(!config.is_auth_enabled());

        clear_env();
    }

    #[test]
    fn complete_keycloak_vars_enables_auth() {
        let _lock = ENV_LOCK.lock();
        clear_env();
        set_complete_oidc_env();

        let config = AppConfig::try_from_env().expect("should succeed");
        assert!(config.is_auth_enabled());
        let oidc = config.oidc.unwrap();
        assert_eq!(oidc.client_id, "test-client");

        clear_env();
    }

    #[test]
    fn api_key_from_env() {
        let _lock = ENV_LOCK.lock();
        clear_env();
        env::set_var("ADMIN_API_KEY", "test-secret-key");

        let config = AppConfig::try_from_env().expect("should succeed");
        assert_eq!(config.api_key.as_deref(), Some("test-secret-key"));

        clear_env();
    }

    #[test]
    fn empty_api_key_is_none() {
        let _lock = ENV_LOCK.lock();
        clear_env();
        env::set_var("ADMIN_API_KEY", "");

        let config = AppConfig::try_from_env().expect("should succeed");
        assert!(config.api_key.is_none());

        clear_env();
    }

    #[test]
    fn no_api_key_is_none() {
        let _lock = ENV_LOCK.lock();
        clear_env();

        let config = AppConfig::try_from_env().expect("should succeed");
        assert!(config.api_key.is_none());

        clear_env();
    }
}
