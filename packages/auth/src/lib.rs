//! Shared OIDC/SSO authentication for RegelRecht services.

use std::sync::Arc;

use axum::routing::get;
use axum::Router;

pub mod config;
pub mod handlers;
pub mod middleware;
pub mod oidc;

pub use config::{
    host_is_allowed, parse_allowed_hosts, parse_base_url, parse_oidc_from_env, OidcConfig,
};
pub use handlers::{AuthStatus, PersonInfo, SESSION_KEY_AUTHENTICATED};
pub use middleware::{require_session_auth, security_headers};
pub use oidc::{discover_client, ConfiguredClient, DiscoveryResult};

/// Trait implemented by each service's `AppState` to provide OIDC context
/// to the shared auth handlers and middleware.
pub trait OidcAppState: Clone + Send + Sync + 'static {
    fn oidc_client(&self) -> Option<&Arc<ConfiguredClient>>;
    fn end_session_url(&self) -> Option<&str>;
    fn oidc_config(&self) -> Option<&OidcConfig>;
    fn is_auth_enabled(&self) -> bool;
    fn base_url(&self) -> Option<&str>;
    fn allowed_hosts(&self) -> &[String];
    fn http_client(&self) -> &reqwest::Client;
}

/// Build the standard auth routes (login, callback, logout, status)
/// for any `AppState` that implements [`OidcAppState`].
pub fn auth_routes<S: OidcAppState>() -> Router<S> {
    Router::new()
        .route("/auth/login", get(handlers::login::<S>))
        .route("/auth/callback", get(handlers::callback::<S>))
        .route("/auth/logout", get(handlers::logout::<S>))
        .route("/auth/status", get(handlers::status::<S>))
}
