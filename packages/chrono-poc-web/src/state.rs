//! De state die elke route deelt.

use std::sync::Arc;

use regelrecht_auth::{ConfiguredClient, OidcAppState, OidcConfig};

use crate::config::AppConfig;
use crate::worlds::WorldRegistry;

/// Wat elke handler bij zich heeft: de configuratie, de werelden, en wat de
/// login nodig heeft.
///
/// Geen pool en geen sessie-store: er is geen database (de sessies staan in
/// geheugen, de werelden ook), en dat is de grens die deze opstelling expliciet
/// stelt.
#[derive(Clone)]
pub struct AppState {
    /// De configuratie, gelezen bij het starten.
    pub config: Arc<AppConfig>,
    /// De werelden van alle sessies.
    pub worlds: Arc<WorldRegistry>,
    /// De OIDC-client, als de login aan staat.
    pub oidc_client: Option<Arc<ConfiguredClient>>,
    /// De logout-URL van de IdP, als die er is.
    pub end_session_url: Option<String>,
    /// De HTTP-client waarmee de login met de IdP praat.
    pub http_client: reqwest::Client,
}

impl OidcAppState for AppState {
    fn oidc_client(&self) -> Option<&Arc<ConfiguredClient>> {
        self.oidc_client.as_ref()
    }
    fn end_session_url(&self) -> Option<&str> {
        self.end_session_url.as_deref()
    }
    fn oidc_config(&self) -> Option<&OidcConfig> {
        self.config.oidc.as_ref()
    }
    fn is_auth_enabled(&self) -> bool {
        self.config.is_auth_enabled()
    }
    fn base_url(&self) -> Option<&str> {
        self.config.base_url.as_deref()
    }
    fn http_client(&self) -> &reqwest::Client {
        &self.http_client
    }
}
