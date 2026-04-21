/// Shared state for the pipeline REST API.
#[derive(Clone)]
pub struct ApiState {
    pub pool: sqlx::PgPool,
    pub http_client: reqwest::Client,
}
