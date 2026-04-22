//! Burger-demo LLM-uitlegproxy (RFC-016).
//!
//! One-route Axum service that translates a law-execution result into a
//! citizen-friendly Dutch explanation via the Anthropic API.
//!
//! - `ANTHROPIC_API_KEY` must be set (process aborts otherwise).
//! - `ALLOWED_ORIGIN` defaults to `http://localhost:7180` (the frontend-demo
//!   dev server). Set to the production demo URL in deploy.
//! - Rate-limited to 5 requests per minute per IP.

use std::env;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use tower_governor::governor::GovernorConfigBuilder;
use tower_governor::GovernorLayer;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;

mod explain;

use explain::{explain, ExplainRequest, ExplainResponse, ExplainerConfig};

#[derive(Clone)]
struct AppState {
    http: reqwest::Client,
    config: Arc<ExplainerConfig>,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let api_key = env::var("ANTHROPIC_API_KEY").unwrap_or_else(|_| {
        tracing::error!("ANTHROPIC_API_KEY is required");
        std::process::exit(1);
    });
    let model = env::var("ANTHROPIC_MODEL").unwrap_or_else(|_| "claude-opus-4-7".to_string());
    let allowed_origin =
        env::var("ALLOWED_ORIGIN").unwrap_or_else(|_| "http://localhost:7180".to_string());

    let state = AppState {
        http: reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .unwrap_or_else(|e| {
                tracing::error!(error = %e, "failed to build HTTP client");
                std::process::exit(1);
            }),
        config: Arc::new(ExplainerConfig { api_key, model }),
    };

    // 5 requests / minute / IP.
    let governor_conf = Arc::new(
        GovernorConfigBuilder::default()
            .per_second(12)
            .burst_size(5)
            .finish()
            .unwrap_or_else(|| {
                tracing::error!("invalid governor config");
                std::process::exit(1);
            }),
    );

    let cors = match allowed_origin.parse::<axum::http::HeaderValue>() {
        Ok(origin) => CorsLayer::new()
            .allow_origin(origin)
            .allow_methods([axum::http::Method::POST, axum::http::Method::OPTIONS])
            .allow_headers([axum::http::header::CONTENT_TYPE]),
        Err(e) => {
            tracing::error!(error = %e, origin = %allowed_origin, "invalid ALLOWED_ORIGIN");
            std::process::exit(1);
        }
    };

    let app = Router::new()
        .route("/health", get(|| async { "OK" }))
        .route("/api/explain", post(explain_handler))
        .layer(GovernorLayer::new(governor_conf))
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let port: u16 = env::var("PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(7181);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!("demo-api listening on {addr}");

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .unwrap_or_else(|e| {
            tracing::error!(error = %e, "failed to bind");
            std::process::exit(1);
        });
    if let Err(e) = axum::serve(listener, app).await {
        tracing::error!(error = %e, "server error");
        std::process::exit(1);
    }
}

async fn explain_handler(
    State(state): State<AppState>,
    Json(req): Json<ExplainRequest>,
) -> Result<Json<ExplainResponse>, (StatusCode, String)> {
    match explain(&state.http, &state.config, req).await {
        Ok(resp) => Ok(Json(resp)),
        Err(e) => {
            let status = e.status_code();
            tracing::warn!(error = %e, "explain failed");
            Err((status, e.to_string()))
        }
    }
}

impl IntoResponse for ExplainResponse {
    fn into_response(self) -> axum::response::Response {
        Json(self).into_response()
    }
}
