use std::sync::Arc;

use sqlx::PgPool;

use crate::config::AppConfig;
use crate::oidc::ConfiguredClient;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub oidc_client: Option<Arc<ConfiguredClient>>,
    pub end_session_url: Option<String>,
    pub config: Arc<AppConfig>,
}
