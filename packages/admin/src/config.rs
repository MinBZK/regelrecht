use std::env;

use sha2::{Digest, Sha256};

pub use regelrecht_auth::OidcConfig;

#[derive(Clone)]
pub struct AppConfig {
    pub oidc: Option<OidcConfig>,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    /// Pre-computed SHA-256 hash of the API key (avoids re-hashing on every request).
    pub api_key_hash: Option<[u8; 32]>,
    /// Pre-computed SHA-256 hash of the metrics auth token (sent by Prometheus).
    pub metrics_token_hash: Option<[u8; 32]>,
    /// Enable test SSO for PR deployments. Blocked in production.
    pub test_sso: bool,
}

impl std::fmt::Debug for AppConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppConfig")
            .field("oidc", &self.oidc)
            .field("base_url", &self.base_url)
            .field("api_key", &self.api_key.as_ref().map(|_| "[REDACTED]"))
            .field("api_key_hash", &self.api_key_hash.map(|_| "[REDACTED]"))
            .field(
                "metrics_token_hash",
                &self.metrics_token_hash.map(|_| "[REDACTED]"),
            )
            .field("test_sso", &self.test_sso)
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

        let metrics_token = env::var("METRICS_AUTH_TOKEN")
            .ok()
            .filter(|s| !s.is_empty());
        if metrics_token.is_some() {
            tracing::info!("Metrics endpoint authentication is enabled (METRICS_AUTH_TOKEN)");
        }
        let metrics_token_hash = metrics_token
            .as_ref()
            .map(|k| Sha256::digest(k.as_bytes()).into());

        let deployment = env::var("DEPLOYMENT_NAME").unwrap_or_default();
        let is_preview = deployment.starts_with("pr")
            && deployment.len() > 2
            && deployment[2..].bytes().all(|b| b.is_ascii_digit());

        let test_sso_explicit = env::var("TEST_SSO")
            .ok()
            .filter(|s| !s.is_empty())
            .map(|v| match v.to_ascii_lowercase().as_str() {
                "true" | "1" | "yes" => Ok(true),
                "false" | "0" | "no" => Ok(false),
                _ => Err(format!(
                    "Invalid TEST_SSO value '{v}'. Use 'true' or 'false'."
                )),
            })
            .transpose()?;

        // Auto-enable for PR deployments, allow explicit override via TEST_SSO=false.
        let test_sso = match test_sso_explicit {
            Some(val) => val,
            None => is_preview,
        };

        // Allowlist: test SSO may only be enabled for confirmed PR deployments (pr<N>).
        if test_sso && !is_preview {
            return Err(format!(
                "TEST_SSO may only be enabled for PR deployments (DEPLOYMENT_NAME=pr<N>). \
                 Current DEPLOYMENT_NAME is '{deployment}'."
            ));
        }

        if test_sso {
            tracing::warn!("TEST SSO is enabled — for test/PR deployments only");
        }

        Ok(Self {
            oidc,
            base_url,
            api_key,
            api_key_hash,
            metrics_token_hash,
            test_sso,
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
        env::remove_var("METRICS_AUTH_TOKEN");
        env::remove_var("TEST_SSO");
        env::remove_var("DEPLOYMENT_NAME");
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

    #[test]
    fn metrics_token_from_env() {
        let _lock = ENV_LOCK.lock();
        clear_env();
        env::set_var("METRICS_AUTH_TOKEN", "prom-secret");

        let config = AppConfig::try_from_env().expect("should succeed");
        assert!(config.metrics_token_hash.is_some());

        clear_env();
    }

    #[test]
    fn no_metrics_token_is_none() {
        let _lock = ENV_LOCK.lock();
        clear_env();

        let config = AppConfig::try_from_env().expect("should succeed");
        assert!(config.metrics_token_hash.is_none());

        clear_env();
    }

    #[test]
    fn test_sso_enabled_from_env() {
        let _lock = ENV_LOCK.lock();
        clear_env();
        env::set_var("TEST_SSO", "true");
        env::set_var("DEPLOYMENT_NAME", "pr42");

        let config = AppConfig::try_from_env().expect("should succeed");
        assert!(config.test_sso);

        clear_env();
    }

    #[test]
    fn test_sso_disabled_by_default() {
        let _lock = ENV_LOCK.lock();
        clear_env();

        let config = AppConfig::try_from_env().expect("should succeed");
        assert!(!config.test_sso);

        clear_env();
    }

    #[test]
    fn test_sso_blocks_production() {
        let _lock = ENV_LOCK.lock();
        clear_env();
        env::set_var("TEST_SSO", "true");
        env::set_var("DEPLOYMENT_NAME", "regelrecht");

        let result = AppConfig::try_from_env();
        assert!(result.is_err());

        clear_env();
    }

    #[test]
    fn test_sso_blocks_non_pr_deployment() {
        let _lock = ENV_LOCK.lock();
        clear_env();
        env::set_var("TEST_SSO", "true");
        env::set_var("DEPLOYMENT_NAME", "staging");

        let result = AppConfig::try_from_env();
        assert!(result.is_err());

        clear_env();
    }

    #[test]
    fn test_sso_blocks_when_deployment_unset() {
        let _lock = ENV_LOCK.lock();
        clear_env();
        env::set_var("TEST_SSO", "true");

        let result = AppConfig::try_from_env();
        assert!(result.is_err());

        clear_env();
    }

    #[test]
    fn test_sso_case_insensitive() {
        let _lock = ENV_LOCK.lock();
        clear_env();
        env::set_var("TEST_SSO", "True");
        env::set_var("DEPLOYMENT_NAME", "pr99");

        let config = AppConfig::try_from_env().expect("should succeed");
        assert!(config.test_sso);

        clear_env();
    }

    #[test]
    fn test_sso_accepts_1_as_true() {
        let _lock = ENV_LOCK.lock();
        clear_env();
        env::set_var("TEST_SSO", "1");
        env::set_var("DEPLOYMENT_NAME", "pr10");

        let config = AppConfig::try_from_env().expect("should succeed");
        assert!(config.test_sso);

        clear_env();
    }

    #[test]
    fn test_sso_rejects_invalid_value() {
        let _lock = ENV_LOCK.lock();
        clear_env();
        env::set_var("TEST_SSO", "banana");
        env::set_var("DEPLOYMENT_NAME", "pr10");

        let result = AppConfig::try_from_env();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("Invalid TEST_SSO value"));

        clear_env();
    }

    #[test]
    fn test_sso_false_string() {
        let _lock = ENV_LOCK.lock();
        clear_env();
        env::set_var("TEST_SSO", "false");

        let config = AppConfig::try_from_env().expect("should succeed");
        assert!(!config.test_sso);

        clear_env();
    }

    #[test]
    fn test_sso_auto_enabled_for_pr_deployment() {
        let _lock = ENV_LOCK.lock();
        clear_env();
        env::set_var("DEPLOYMENT_NAME", "pr42");

        let config = AppConfig::try_from_env().expect("should succeed");
        assert!(config.test_sso);

        clear_env();
    }

    #[test]
    fn test_sso_not_auto_enabled_for_preview_deployment() {
        let _lock = ENV_LOCK.lock();
        clear_env();
        env::set_var("DEPLOYMENT_NAME", "preview");

        let config = AppConfig::try_from_env().expect("should succeed");
        assert!(!config.test_sso);

        clear_env();
    }

    #[test]
    fn test_sso_not_auto_enabled_for_production() {
        let _lock = ENV_LOCK.lock();
        clear_env();
        env::set_var("DEPLOYMENT_NAME", "regelrecht");

        let config = AppConfig::try_from_env().expect("should succeed");
        assert!(!config.test_sso);

        clear_env();
    }

    #[test]
    fn test_sso_explicit_false_overrides_pr_auto() {
        let _lock = ENV_LOCK.lock();
        clear_env();
        env::set_var("DEPLOYMENT_NAME", "pr42");
        env::set_var("TEST_SSO", "false");

        let config = AppConfig::try_from_env().expect("should succeed");
        assert!(!config.test_sso);

        clear_env();
    }
}
