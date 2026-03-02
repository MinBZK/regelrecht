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
//! ## Base URL & host validation
//!
//! | Variable                | Required | Description                                              |
//! |-------------------------|----------|----------------------------------------------------------|
//! | `BASE_URL`              | yes*     | Canonical base URL. Required when OIDC is enabled.       |
//! | `BASE_URL_ALLOW_DYNAMIC`| no       | `true`/`1` to allow header-derived URLs within the       |
//! |                         |          | `BASE_URL` domain suffix (for PR preview deploys).       |

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
    pub base_url_allow_dynamic: bool,
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

        let base_url = env::var("BASE_URL").ok();

        let base_url_allow_dynamic = matches!(
            env::var("BASE_URL_ALLOW_DYNAMIC").as_deref(),
            Ok("true" | "1")
        );

        if oidc.is_some() {
            if base_url.is_none() {
                return Err("OIDC is enabled but BASE_URL is not set. \
                     BASE_URL is required when OIDC authentication is enabled \
                     to prevent host header injection attacks."
                    .to_string());
            }
            tracing::info!("OIDC authentication is enabled");
            if base_url_allow_dynamic {
                tracing::info!(
                    "BASE_URL_ALLOW_DYNAMIC is enabled; request-derived hosts \
                     sharing the BASE_URL domain suffix will be accepted"
                );
            }
        } else {
            tracing::warn!("OIDC authentication is DISABLED — admin panel is unprotected");
        }

        if base_url_allow_dynamic && base_url.is_none() {
            return Err("BASE_URL_ALLOW_DYNAMIC=true but BASE_URL is not set. \
                 BASE_URL is required as the trust anchor for dynamic host validation."
                .to_string());
        }

        Ok(Self {
            oidc,
            base_url,
            base_url_allow_dynamic,
        })
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

    /// Returns the domain suffix of `BASE_URL` (everything from the first `.` in the host).
    /// E.g. `https://admin.rig.example.nl` → `.rig.example.nl`
    pub fn base_url_domain_suffix(&self) -> Option<&str> {
        let base = self.base_url.as_deref()?;
        let host = base
            .strip_prefix("https://")
            .or_else(|| base.strip_prefix("http://"))?;
        // Strip port if present
        let host = host.split(':').next().unwrap_or(host);
        host.find('.').map(|i| &host[i..])
    }

    /// Returns the scheme of `BASE_URL` (`"https"` or `"http"`).
    pub fn base_url_scheme(&self) -> Option<&str> {
        let base = self.base_url.as_deref()?;
        base.split("://").next()
    }
}

#[cfg(test)]
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
        "BASE_URL",
        "BASE_URL_ALLOW_DYNAMIC",
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
        env::set_var("BASE_URL", "https://admin.example.com");
    }

    #[test]
    fn no_oidc_vars_disables_auth() {
        let _lock = ENV_LOCK.lock();
        clear_oidc_env();

        let config = AppConfig::try_from_env().expect("should succeed");
        assert!(config.oidc.is_none());
        assert!(!config.is_auth_enabled());
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
        env::set_var("BASE_URL", "https://admin.example.com");
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

    #[test]
    fn default_base_url_is_none() {
        let _lock = ENV_LOCK.lock();
        clear_oidc_env();

        let config = AppConfig::try_from_env().expect("should succeed");
        assert!(config.base_url.is_none());
    }

    #[test]
    fn custom_base_url() {
        let _lock = ENV_LOCK.lock();
        clear_oidc_env();
        env::set_var("BASE_URL", "https://admin.example.com");

        let config = AppConfig::try_from_env().expect("should succeed");
        assert_eq!(config.base_url.unwrap(), "https://admin.example.com");

        clear_oidc_env();
    }

    #[test]
    fn oidc_without_base_url_fails() {
        let _lock = ENV_LOCK.lock();
        clear_oidc_env();
        env::set_var("OIDC_CLIENT_ID", "test-client");
        env::set_var("OIDC_CLIENT_SECRET", "secret");
        env::set_var("KEYCLOAK_BASE_URL", "https://keycloak.example.com");
        env::set_var("KEYCLOAK_REALM", "test-realm");
        // No BASE_URL set

        let result = AppConfig::try_from_env();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("BASE_URL"));

        clear_oidc_env();
    }

    #[test]
    fn allow_dynamic_without_base_url_fails() {
        let _lock = ENV_LOCK.lock();
        clear_oidc_env();
        env::set_var("BASE_URL_ALLOW_DYNAMIC", "true");

        let result = AppConfig::try_from_env();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("BASE_URL_ALLOW_DYNAMIC"));

        clear_oidc_env();
    }

    #[test]
    fn allow_dynamic_parsed_correctly() {
        let _lock = ENV_LOCK.lock();
        clear_oidc_env();
        env::set_var("BASE_URL", "https://admin.example.com");
        env::set_var("BASE_URL_ALLOW_DYNAMIC", "true");

        let config = AppConfig::try_from_env().expect("should succeed");
        assert!(config.base_url_allow_dynamic);

        clear_oidc_env();
    }

    #[test]
    fn allow_dynamic_accepts_1() {
        let _lock = ENV_LOCK.lock();
        clear_oidc_env();
        env::set_var("BASE_URL", "https://admin.example.com");
        env::set_var("BASE_URL_ALLOW_DYNAMIC", "1");

        let config = AppConfig::try_from_env().expect("should succeed");
        assert!(config.base_url_allow_dynamic);

        clear_oidc_env();
    }

    #[test]
    fn allow_dynamic_defaults_to_false() {
        let _lock = ENV_LOCK.lock();
        clear_oidc_env();

        let config = AppConfig::try_from_env().expect("should succeed");
        assert!(!config.base_url_allow_dynamic);

        clear_oidc_env();
    }

    #[test]
    fn base_url_domain_suffix_extracts_correctly() {
        let config = AppConfig {
            oidc: None,
            base_url: Some("https://admin.rig.prd1.gn2.quattro.rijksapps.nl".to_string()),
            base_url_allow_dynamic: false,
        };
        assert_eq!(
            config.base_url_domain_suffix(),
            Some(".rig.prd1.gn2.quattro.rijksapps.nl")
        );
    }

    #[test]
    fn base_url_domain_suffix_with_port() {
        let config = AppConfig {
            oidc: None,
            base_url: Some("https://admin.example.com:8443".to_string()),
            base_url_allow_dynamic: false,
        };
        assert_eq!(config.base_url_domain_suffix(), Some(".example.com"));
    }

    #[test]
    fn base_url_domain_suffix_none_when_no_base_url() {
        let config = AppConfig {
            oidc: None,
            base_url: None,
            base_url_allow_dynamic: false,
        };
        assert_eq!(config.base_url_domain_suffix(), None);
    }

    #[test]
    fn base_url_scheme_extracts_correctly() {
        let config = AppConfig {
            oidc: None,
            base_url: Some("https://admin.example.com".to_string()),
            base_url_allow_dynamic: false,
        };
        assert_eq!(config.base_url_scheme(), Some("https"));
    }

    #[test]
    fn base_url_scheme_http() {
        let config = AppConfig {
            oidc: None,
            base_url: Some("http://localhost:8000".to_string()),
            base_url_allow_dynamic: false,
        };
        assert_eq!(config.base_url_scheme(), Some("http"));
    }
}
