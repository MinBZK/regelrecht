use axum::extract::{Request, State};
use axum::http::header;
use axum::http::{HeaderValue, StatusCode};
use axum::middleware::Next;
use axum::response::Response;
use tower_sessions::Session;

use crate::auth::SESSION_KEY_AUTHENTICATED;
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

pub async fn require_auth(
    State(state): State<AppState>,
    session: Session,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
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
        Err(StatusCode::UNAUTHORIZED)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::config::AppConfig;
    use crate::state::AppState;
    use axum::body::Body;
    use axum::middleware as axum_middleware;
    use axum::routing::get;
    use axum::Router;
    use sqlx::postgres::PgPoolOptions;
    use std::sync::Arc;
    use tower::ServiceExt;
    use tower_sessions::SessionManagerLayer;
    use tower_sessions_memory_store::MemoryStore;

    fn test_state(auth_enabled: bool) -> AppState {
        let config = if auth_enabled {
            AppConfig {
                oidc: Some(crate::config::OidcConfig {
                    client_id: "test".into(),
                    client_secret: "test".into(),
                    issuer_url: "https://example.com".into(),
                    required_role: "user".into(),
                }),
                base_url: None,
            }
        } else {
            AppConfig {
                oidc: None,
                base_url: None,
            }
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

    fn test_app(state: AppState) -> Router {
        let store = MemoryStore::default();
        let session_layer = SessionManagerLayer::new(store);

        Router::new()
            .route("/test", get(|| async { "ok" }))
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
}
