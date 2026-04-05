//! Application configuration, loaded from environment variables.
//!
//! # Environment variables
//!
//! ## OIDC authentication (all required when `OIDC_CLIENT_ID` is set)
//!
//! | Variable              | Required | Description                                                |
//! |-----------------------|----------|------------------------------------------------------------|
//! | `OIDC_CLIENT_ID`      | yes*     | OAuth 2 client ID. Enables OIDC when set.                  |
//! | `OIDC_CLIENT_SECRET`  | yes*     | OAuth 2 client secret.                                     |
//! | `OIDC_DISCOVERY_URL`  | no       | Full OIDC discovery URL; takes priority over Keycloak vars.|
//! | `KEYCLOAK_BASE_URL`   | no       | Keycloak base URL (fallback issuer construction).          |
//! | `KEYCLOAK_REALM`      | no       | Keycloak realm (fallback issuer construction).             |
//! | `OIDC_REQUIRED_ROLE`  | no       | Realm role required for access (default: `allowed-user`).  |
//!
//! *Required together — if `OIDC_CLIENT_ID` is set, `OIDC_CLIENT_SECRET` must also be set,
//! and either `OIDC_DISCOVERY_URL` or both `KEYCLOAK_BASE_URL` + `KEYCLOAK_REALM`.
//!
//! ## Base URL
//!
//! | Variable   | Required | Description                                                     |
//! |------------|----------|-----------------------------------------------------------------|
//! | `BASE_URL` | no       | Static base URL (e.g. `https://admin.example.com`). When set,   |
//! |            |          | used for all OIDC redirect/logout URLs instead of request        |
//! |            |          | headers. Prevents open-redirect via `X-Forwarded-Host`.          |

use std::env;

#[derive(Clone)]
pub struct OidcConfig {
    pub client_id: String,
    pub client_secret: String,
    pub issuer_url: String,
    pub required_role: String,
}

impl std::fmt::Debug for OidcConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OidcConfig")
            .field("client_id", &self.client_id)
            .field("client_secret", &"[REDACTED]")
            .field("issuer_url", &self.issuer_url)
            .field("required_role", &self.required_role)
            .finish()
    }
}

#[derive(Clone, Debug)]
pub struct AppConfig {
    pub oidc: Option<OidcConfig>,
    pub base_url: Option<String>,
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
        let oidc = match env::var("OIDC_CLIENT_ID").ok() {
            None => None,
            Some(client_id) => Some(Self::parse_oidc_config(client_id)?),
        };

        if oidc.is_some() {
            tracing::info!("OIDC authentication is enabled");
        } else {
            tracing::warn!(
                "OIDC authentication is DISABLED — admin panel is unprotected. \
                 Configure OIDC environment variables to enable authentication."
            );
        }

        let base_url = env::var("BASE_URL")
            .ok()
            .filter(|s| !s.is_empty())
            .map(|s| {
                let trimmed = s.trim_end_matches('/').to_string();
                url::Url::parse(&trimmed)
                    .map_err(|e| format!("BASE_URL is not a valid URL: {e}"))?;
                tracing::info!("BASE_URL configured: {trimmed}");
                Ok::<String, String>(trimmed)
            })
            .transpose()?;
        if base_url.is_none() && oidc.is_some() {
            tracing::warn!(
                "BASE_URL is not set — OIDC redirect URLs will be derived from request headers. \
                 Set BASE_URL to prevent open-redirect attacks."
            );
        }

        Ok(Self { oidc, base_url })
    }

    fn parse_oidc_config(client_id: String) -> Result<OidcConfig, String> {
        let client_secret = env::var("OIDC_CLIENT_SECRET").unwrap_or_default();
        if client_secret.is_empty() {
            return Err("OIDC_CLIENT_ID is set but OIDC_CLIENT_SECRET is missing. \
                 Refusing to start without complete OIDC configuration."
                .to_string());
        }

        let issuer_url = Self::resolve_issuer_url()?;

        let required_role =
            env::var("OIDC_REQUIRED_ROLE").unwrap_or_else(|_| "allowed-user".to_string());

        Ok(OidcConfig {
            client_id,
            client_secret,
            issuer_url,
            required_role,
        })
    }

    fn resolve_issuer_url() -> Result<String, String> {
        // OIDC_DISCOVERY_URL takes priority (RIG-style injection)
        if let Ok(discovery_url) = env::var("OIDC_DISCOVERY_URL") {
            if !discovery_url.is_empty() {
                // Strip /.well-known/openid-configuration suffix if present
                let issuer = discovery_url
                    .strip_suffix("/.well-known/openid-configuration")
                    .unwrap_or(&discovery_url);
                tracing::info!("using OIDC_DISCOVERY_URL for issuer: {issuer}");
                return Ok(issuer.to_string());
            }
        }

        // Fallback: construct from KEYCLOAK_BASE_URL + KEYCLOAK_REALM
        let base = env::var("KEYCLOAK_BASE_URL").unwrap_or_default();
        let realm = env::var("KEYCLOAK_REALM").unwrap_or_default();

        if !base.is_empty() && !realm.is_empty() {
            let issuer = format!("{}/realms/{}", base.trim_end_matches('/'), realm);
            tracing::info!("using KEYCLOAK_BASE_URL + KEYCLOAK_REALM for issuer: {issuer}");
            return Ok(issuer);
        }

        Err("OIDC_CLIENT_ID is set but no issuer could be determined. \
             Set OIDC_DISCOVERY_URL, or both KEYCLOAK_BASE_URL and KEYCLOAK_REALM."
            .to_string())
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

    fn clear_oidc_env() {
        for var in OIDC_VARS {
            env::remove_var(var);
        }
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
        clear_oidc_env();

        let config = AppConfig::try_from_env().expect("should succeed");
        assert!(config.oidc.is_none());
        assert!(!config.is_auth_enabled());

        clear_oidc_env();
    }

    #[test]
    fn complete_keycloak_vars_enables_auth() {
        let _lock = ENV_LOCK.lock();
        clear_oidc_env();
        set_complete_oidc_env();

        let config = AppConfig::try_from_env().expect("should succeed");
        assert!(config.is_auth_enabled());
        let oidc = config.oidc.unwrap();
        assert_eq!(oidc.client_id, "test-client");
        assert_eq!(oidc.client_secret, "secret");
        assert_eq!(
            oidc.issuer_url,
            "https://keycloak.example.com/realms/test-realm"
        );

        clear_oidc_env();
    }

    #[test]
    fn discovery_url_takes_priority_over_keycloak_vars() {
        let _lock = ENV_LOCK.lock();
        clear_oidc_env();
        set_complete_oidc_env();
        env::set_var(
            "OIDC_DISCOVERY_URL",
            "https://idp.example.com/realms/my-realm/.well-known/openid-configuration",
        );

        let config = AppConfig::try_from_env().expect("should succeed");
        let oidc = config.oidc.unwrap();
        assert_eq!(oidc.issuer_url, "https://idp.example.com/realms/my-realm");

        clear_oidc_env();
    }

    #[test]
    fn discovery_url_without_wellknown_suffix() {
        let _lock = ENV_LOCK.lock();
        clear_oidc_env();
        env::set_var("OIDC_CLIENT_ID", "test-client");
        env::set_var("OIDC_CLIENT_SECRET", "secret");
        env::set_var(
            "OIDC_DISCOVERY_URL",
            "https://idp.example.com/realms/myrealm",
        );

        let config = AppConfig::try_from_env().expect("should succeed");
        let oidc = config.oidc.unwrap();
        assert_eq!(oidc.issuer_url, "https://idp.example.com/realms/myrealm");

        clear_oidc_env();
    }

    #[test]
    fn missing_secret_is_hard_error() {
        let _lock = ENV_LOCK.lock();
        clear_oidc_env();
        env::set_var("OIDC_CLIENT_ID", "test-client");

        let result = AppConfig::try_from_env();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("OIDC_CLIENT_SECRET"));

        clear_oidc_env();
    }

    #[test]
    fn missing_issuer_is_hard_error() {
        let _lock = ENV_LOCK.lock();
        clear_oidc_env();
        env::set_var("OIDC_CLIENT_ID", "test-client");
        env::set_var("OIDC_CLIENT_SECRET", "secret");

        let result = AppConfig::try_from_env();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("OIDC_DISCOVERY_URL"));
        assert!(err.contains("KEYCLOAK_BASE_URL"));

        clear_oidc_env();
    }

    #[test]
    fn default_role_is_allowed_user() {
        let _lock = ENV_LOCK.lock();
        clear_oidc_env();
        set_complete_oidc_env();

        let config = AppConfig::try_from_env().expect("should succeed");
        assert_eq!(config.oidc.unwrap().required_role, "allowed-user");

        clear_oidc_env();
    }

    #[test]
    fn custom_role_from_env() {
        let _lock = ENV_LOCK.lock();
        clear_oidc_env();
        set_complete_oidc_env();
        env::set_var("OIDC_REQUIRED_ROLE", "super-admin");

        let config = AppConfig::try_from_env().expect("should succeed");
        assert_eq!(config.oidc.unwrap().required_role, "super-admin");

        clear_oidc_env();
    }
}
