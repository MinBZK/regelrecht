//! Burger-demo LLM-uitlegproxy (RFC-016).
//!
//! One-route Axum service that translates a law-execution result into a
//! citizen-friendly Dutch explanation via the Anthropic API.
//!
//! - `ANTHROPIC_API_KEY` must be set (process aborts otherwise).
//! - `ALLOWED_ORIGINS` is a comma-separated allow-list. Each entry is either an
//!   exact origin (e.g. `https://demo.regelrecht.rijks.app`) or a `*.suffix`
//!   wildcard that matches any subdomain (e.g. `*.regelrecht.rijks.app` matches
//!   `https://demo-pr1.regelrecht.rijks.app` but NOT the bare apex). Defaults to
//!   `http://localhost:7180` (frontend-demo dev server). The legacy
//!   `ALLOWED_ORIGIN` (singular) is still honoured.
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
use tower_governor::key_extractor::PeerIpKeyExtractor;
use tower_governor::GovernorLayer;
use tower_http::cors::{AllowOrigin, CorsLayer};
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
    // Default to Haiku for cost: this proxy renders 4-6 zinnen B1-Nederlands,
    // a workload Haiku handles fine for ~5% of the per-token cost of Opus.
    // Operators can override to Sonnet/Opus via the env var.
    let model =
        env::var("ANTHROPIC_MODEL").unwrap_or_else(|_| "claude-haiku-4-5-20251001".to_string());
    let raw_origins = env::var("ALLOWED_ORIGINS")
        .or_else(|_| env::var("ALLOWED_ORIGIN"))
        .unwrap_or_else(|_| "http://localhost:7180".to_string());
    let allowed_origins: Vec<String> = raw_origins
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    if allowed_origins.is_empty() {
        tracing::error!("ALLOWED_ORIGINS resolved to an empty list");
        std::process::exit(1);
    }

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

    // 5 requests / minute, keyed on the connecting peer IP. The default
    // extractor (`SmartIpKeyExtractor`) trusts the leftmost `X-Forwarded-For`,
    // which is attacker-controlled when an upstream proxy appends rather than
    // replaces the header — so a hostile client can rotate fake IPs and bypass
    // the limit. We deliberately use `PeerIpKeyExtractor` (the socket peer
    // address) so the only way to get a fresh quota is from a fresh L4 source.
    // Behind a single ingress this rate-limits per-ingress; that's acceptable
    // here because the limit's job is "cap LLM spend", not per-user fairness.
    let governor_conf = Arc::new(
        GovernorConfigBuilder::default()
            .per_second(12)
            .burst_size(5)
            .key_extractor(PeerIpKeyExtractor)
            .finish()
            .unwrap_or_else(|| {
                tracing::error!("invalid governor config");
                std::process::exit(1);
            }),
    );

    tracing::info!(origins = ?allowed_origins, "CORS allow-list");
    let allowed_for_predicate = allowed_origins.clone();
    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::predicate(move |origin, _| {
            origin
                .to_str()
                .map(|s| {
                    allowed_for_predicate
                        .iter()
                        .any(|pat| origin_matches(s, pat))
                })
                .unwrap_or(false)
        }))
        .allow_methods([axum::http::Method::POST, axum::http::Method::OPTIONS])
        .allow_headers([axum::http::header::CONTENT_TYPE]);

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

/// Match an Origin header against an allow-list pattern.
/// Patterns starting with `*.` are subdomain wildcards: `*.example.com`
/// matches `https://foo.example.com` and `https://a.b.example.com` but NOT
/// the bare `https://example.com`.
fn origin_matches(origin: &str, pattern: &str) -> bool {
    if let Some(suffix) = pattern.strip_prefix("*.") {
        let Some((_, after_scheme)) = origin.split_once("://") else {
            return false;
        };
        let host = after_scheme.split(['/', ':']).next().unwrap_or("");
        host.ends_with(&format!(".{suffix}"))
    } else {
        origin == pattern
    }
}

#[cfg(test)]
mod tests {
    use super::origin_matches;

    #[test]
    fn exact_match() {
        assert!(origin_matches(
            "http://localhost:7180",
            "http://localhost:7180"
        ));
        assert!(!origin_matches(
            "http://localhost:7181",
            "http://localhost:7180"
        ));
    }

    #[test]
    fn subdomain_wildcard() {
        assert!(origin_matches(
            "https://demo-pr1.regelrecht.rijks.app",
            "*.regelrecht.rijks.app"
        ));
        assert!(origin_matches(
            "https://demo.regelrecht.rijks.app",
            "*.regelrecht.rijks.app"
        ));
    }

    #[test]
    fn wildcard_does_not_match_apex() {
        assert!(!origin_matches(
            "https://regelrecht.rijks.app",
            "*.regelrecht.rijks.app"
        ));
    }

    #[test]
    fn wildcard_ignores_port_and_path() {
        assert!(origin_matches(
            "https://demo-pr1.regelrecht.rijks.app:8443",
            "*.regelrecht.rijks.app"
        ));
    }

    #[test]
    fn wildcard_does_not_match_evil_lookalike() {
        // `evil-regelrecht.rijks.app` ends in "regelrecht.rijks.app" but is
        // not a subdomain of it, so the leading dot in `.{suffix}` matters.
        assert!(!origin_matches(
            "https://evil-regelrecht.rijks.app",
            "*.regelrecht.rijks.app"
        ));
    }
}
