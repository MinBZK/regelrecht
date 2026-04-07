use axum::extract::{Request, State};
use axum::http::header;
use axum::http::{HeaderValue, Method};
use axum::middleware::Next;
use axum::response::Response;
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use tower_sessions::Session;

use crate::auth::SESSION_KEY_AUTHENTICATED;
use crate::error::ApiError;
use crate::state::AppState;

pub async fn security_headers(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
    headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("strict-origin-when-cross-origin"),
    );
    headers.insert(
        "content-security-policy",
        HeaderValue::from_static(
            "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; font-src 'self'; connect-src 'self'; frame-ancestors 'none'",
        ),
    );
    headers.insert(
        "permissions-policy",
        HeaderValue::from_static("geolocation=(), camera=(), microphone=()"),
    );
    headers.insert(
        header::STRICT_TRANSPORT_SECURITY,
        HeaderValue::from_static("max-age=31536000; includeSubDomains"),
    );
    response
}

/// Methods allowed via API key authentication (no OIDC session required).
const API_KEY_ALLOWED_METHODS: &[Method] = &[Method::GET, Method::DELETE];

pub async fn require_auth(
    State(state): State<AppState>,
    session: Session,
    request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    // Check bearer token first (fast path for programmatic access).
    if let Some(ref key_hash) = state.config.api_key_hash {
        if let Some(token) = extract_bearer_token(&request) {
            // Compare SHA-256 digests in constant time to prevent
            // timing leaks of both key content and length.
            let token_hash = Sha256::digest(token.as_bytes());
            let token_matches = token_hash.ct_eq(key_hash).into();
            if token_matches {
                if !API_KEY_ALLOWED_METHODS.contains(request.method()) {
                    tracing::warn!(
                        method = %request.method(),
                        uri = %request.uri(),
                        "API key auth: method not allowed"
                    );
                    return Err(ApiError::Forbidden("method not allowed".to_string()));
                }
                return Ok(next.run(request).await);
            }
            // Invalid bearer token — reject immediately, don't fall through to session.
            tracing::warn!(uri = %request.uri(), "API key auth: invalid bearer token");
            return Err(ApiError::Unauthorized("invalid bearer token".to_string()));
        }
    }

    // Fall through to OIDC/session authentication.
    if !state.config.is_auth_enabled() {
        return Ok(next.run(request).await);
    }

    let authenticated: bool = session
        .get(SESSION_KEY_AUTHENTICATED)
        .await
        .ok()
        .flatten()
        .unwrap_or(false);

    if authenticated {
        Ok(next.run(request).await)
    } else {
        Err(ApiError::Unauthorized(
            "authentication required".to_string(),
        ))
    }
}

fn extract_bearer_token(request: &Request) -> Option<String> {
    let value = request
        .headers()
        .get(header::AUTHORIZATION)?
        .to_str()
        .ok()?;
    // RFC 7235: auth-scheme is case-insensitive.
    if value.len() > 7 && value[..7].eq_ignore_ascii_case("bearer ") {
        Some(value[7..].to_string())
    } else {
        None
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::config::AppConfig;
    use crate::state::AppState;
    use axum::body::Body;
    use axum::http::StatusCode;
    use axum::middleware as axum_middleware;
    use axum::routing::get;
    use axum::Router;
    use sqlx::postgres::PgPoolOptions;
    use std::sync::Arc;
    use tower::ServiceExt;
    use tower_sessions::SessionManagerLayer;
    use tower_sessions_memory_store::MemoryStore;

    fn test_state_with_api_key(auth_enabled: bool, api_key: Option<&str>) -> AppState {
        let config = AppConfig {
            oidc: if auth_enabled {
                Some(crate::config::OidcConfig {
                    client_id: "test".into(),
                    client_secret: "test".into(),
                    issuer_url: "https://example.com".into(),
                    required_role: "user".into(),
                })
            } else {
                None
            },
            base_url: None,
            api_key: api_key.map(String::from),
            api_key_hash: api_key.map(|k| {
                use sha2::{Digest, Sha256};
                Sha256::digest(k.as_bytes()).into()
            }),
        };

        #[allow(clippy::expect_used)]
        let pool = PgPoolOptions::new()
            .connect_lazy("postgres://test@localhost/test")
            .expect("lazy pool");

        AppState {
            pool,
            oidc_client: None,
            end_session_url: None,
            config: Arc::new(config),
            metrics_cache: Arc::new(crate::metrics::new_cache()),
            http_client: reqwest::Client::new(),
            corpus: Arc::new(tokio::sync::RwLock::new(crate::state::CorpusState::empty())),
        }
    }

    fn test_state(auth_enabled: bool) -> AppState {
        test_state_with_api_key(auth_enabled, None)
    }

    fn test_app(state: AppState) -> Router {
        let store = MemoryStore::default();
        let session_layer = SessionManagerLayer::new(store);

        Router::new()
            .route(
                "/test",
                get(|| async { "ok" })
                    .post(|| async { "ok" })
                    .delete(|| async { "ok" }),
            )
            .route_layer(axum_middleware::from_fn_with_state(
                state.clone(),
                require_auth,
            ))
            .with_state(state)
            .layer(session_layer)
    }

    #[tokio::test]
    async fn security_headers_are_set() {
        let app = Router::new()
            .route("/test", get(|| async { "ok" }))
            .layer(axum_middleware::from_fn(super::security_headers));

        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/test")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get("x-content-type-options").unwrap(),
            "nosniff"
        );
        assert_eq!(response.headers().get("x-frame-options").unwrap(), "DENY");
        assert_eq!(
            response.headers().get("referrer-policy").unwrap(),
            "strict-origin-when-cross-origin"
        );
        assert_eq!(
            response.headers().get("content-security-policy").unwrap(),
            "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; font-src 'self'; connect-src 'self'; frame-ancestors 'none'"
        );
        assert_eq!(
            response.headers().get("permissions-policy").unwrap(),
            "geolocation=(), camera=(), microphone=()"
        );
        assert_eq!(
            response.headers().get("strict-transport-security").unwrap(),
            "max-age=31536000; includeSubDomains"
        );
    }

    #[tokio::test]
    async fn auth_disabled_passes_through() {
        let app = test_app(test_state(false));

        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/test")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn unauthenticated_returns_401() {
        let app = test_app(test_state(true));

        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/test")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(
            response.headers().get("www-authenticate").unwrap(),
            "Bearer"
        );
    }

    #[tokio::test]
    async fn authenticated_passes_through() {
        let store = MemoryStore::default();
        let state = test_state(true);
        let session_layer = SessionManagerLayer::new(store);

        // Add a helper endpoint that sets the session authenticated flag.
        let app = Router::new()
            .route("/test", get(|| async { "ok" }))
            .route_layer(axum_middleware::from_fn_with_state(
                state.clone(),
                require_auth,
            ))
            .route(
                "/set-auth",
                get(|session: Session| async move {
                    session
                        .insert(SESSION_KEY_AUTHENTICATED, true)
                        .await
                        .expect("insert");
                    "set"
                }),
            )
            .with_state(state)
            .layer(session_layer);

        // Hit the helper to create an authenticated session.
        let response = app
            .clone()
            .oneshot(
                axum::http::Request::builder()
                    .uri("/set-auth")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::OK);

        let cookie = response
            .headers()
            .get("set-cookie")
            .expect("set-cookie header")
            .to_str()
            .expect("cookie str")
            .to_string();

        // Use the session cookie on the protected endpoint.
        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/test")
                    .header("cookie", &cookie)
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn api_key_valid_get_passes() {
        let state = test_state_with_api_key(true, Some("test-key"));
        let app = test_app(state);

        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/test")
                    .header("authorization", "Bearer test-key")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn api_key_valid_delete_passes() {
        let state = test_state_with_api_key(true, Some("test-key"));
        let app = test_app(state);

        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .method("DELETE")
                    .uri("/test")
                    .header("authorization", "Bearer test-key")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn api_key_valid_post_returns_403() {
        let state = test_state_with_api_key(true, Some("test-key"));
        let app = test_app(state);

        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/test")
                    .header("authorization", "Bearer test-key")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn api_key_invalid_returns_401() {
        let state = test_state_with_api_key(true, Some("test-key"));
        let app = test_app(state);

        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/test")
                    .header("authorization", "Bearer wrong-key")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn api_key_not_configured_ignores_bearer() {
        // No API key configured, OIDC disabled — should pass through regardless of bearer header.
        let state = test_state_with_api_key(false, None);
        let app = test_app(state);

        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/test")
                    .header("authorization", "Bearer some-token")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn api_key_without_oidc_rejects_invalid_token() {
        // API key configured but OIDC disabled — invalid bearer should still be rejected.
        let state = test_state_with_api_key(false, Some("test-key"));
        let app = test_app(state);

        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/test")
                    .header("authorization", "Bearer wrong-key")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn api_key_without_oidc_no_bearer_falls_through() {
        // API key configured, OIDC disabled, no bearer header — auth disabled passthrough.
        let state = test_state_with_api_key(false, Some("test-key"));
        let app = test_app(state);

        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/test")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::OK);
    }
}
