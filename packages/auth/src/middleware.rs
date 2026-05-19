use std::future::Future;
use std::pin::Pin;

use axum::extract::{Request, State};
use axum::http::header;
use axum::http::{HeaderValue, StatusCode};
use axum::middleware::Next;
use axum::response::Response;
use tower_sessions::Session;

use crate::handlers::{SESSION_KEY_AUTHENTICATED, SESSION_KEY_ROLES, SESSION_KEY_SUB};
use crate::OidcAppState;

type RequireRoleFuture = Pin<Box<dyn Future<Output = Result<Response, StatusCode>> + Send>>;

/// Outcome of a session-based role check. Independent of any HTTP error type
/// so that callers in different crates can map this to their own error
/// (`StatusCode` here, `ApiError` in the admin crate).
pub enum RoleCheck {
    /// Session is authenticated and carries the required role.
    Allowed,
    /// Session has no authenticated marker (or it's false).
    NotAuthenticated,
    /// Session is authenticated but lacks the required role. `sub` is the
    /// Keycloak subject if present, included so callers can log it.
    MissingRole { sub: Option<String> },
}

/// Inspect a session and decide whether it satisfies the role requirement.
///
/// Returns [`RoleCheck::Allowed`] when the session holds an authenticated
/// marker *and* `SESSION_KEY_ROLES` contains `required_role`. The role list
/// is compared verbatim against the realm roles stored at login — Keycloak
/// composite expansion means a higher role's token automatically carries the
/// lower roles, so callers never need to check "role A or role B".
///
/// This helper is shared between [`require_role`] (returns 401/403 status
/// codes) and the admin crate's `require_auth` (which wraps the API-key
/// bypass around the session path). It does **not** consider whether auth
/// is enabled on the application — callers are expected to early-return
/// before reaching this when their `is_auth_enabled()` is false.
pub async fn check_session_role(session: &Session, required_role: &str) -> RoleCheck {
    let authenticated: bool = session
        .get(SESSION_KEY_AUTHENTICATED)
        .await
        .ok()
        .flatten()
        .unwrap_or(false);
    if !authenticated {
        return RoleCheck::NotAuthenticated;
    }

    // Distinguish "no roles key in session at all" (pre-RBAC session from
    // before this code shipped) from "key is present but empty" (legitimate
    // misconfiguration — Keycloak issued an empty `realm_access.roles`).
    //
    // The former must return `NotAuthenticated` so the caller maps to 401 and
    // triggers the OIDC re-login flow — that login repopulates the roles key
    // from the JWT and the session self-heals. Returning 403 here would leave
    // pre-existing sessions stuck (403 does not redirect to login), so during
    // a rolling deploy of this code every logged-in user would see "forbidden"
    // on routes they actually have permission to reach.
    //
    // An explicitly empty list still falls through to `MissingRole`, which is
    // the correct response for a user whose Keycloak roles are genuinely empty.
    let roles: Option<Vec<String>> = session.get(SESSION_KEY_ROLES).await.ok().flatten();
    match roles {
        None => RoleCheck::NotAuthenticated,
        Some(roles) => {
            if roles.iter().any(|r| r == required_role) {
                RoleCheck::Allowed
            } else {
                let sub: Option<String> = session.get(SESSION_KEY_SUB).await.ok().flatten();
                RoleCheck::MissingRole { sub }
            }
        }
    }
}

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

/// Session-based authentication middleware.
///
/// Passes through when auth is disabled. Returns 401 when the session
/// is not authenticated. Does NOT handle API key authentication — the
/// admin dashboard wraps this with its own API key check.
pub async fn require_session_auth<S: OidcAppState>(
    State(state): State<S>,
    session: Session,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    if !state.is_auth_enabled() {
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

/// Route-level RBAC gate.
///
/// Returns a middleware function that allows the request only when the
/// session holds `role` (compared verbatim against the realm roles stored
/// at login). Role hierarchy lives in Keycloak as composite roles, so a
/// higher role (e.g. `editor-admin`) automatically carries the lower roles
/// in its token — code never needs to check "role A or role B".
///
/// Behaviour:
/// - auth disabled → passthrough (dev/test convenience).
/// - not authenticated → 401.
/// - authenticated but lacks the role → 403.
pub fn require_role<S: OidcAppState>(
    role: &'static str,
) -> impl Fn(State<S>, Session, Request, Next) -> RequireRoleFuture + Clone + Send + Sync + 'static
{
    move |State(state): State<S>, session: Session, request: Request, next: Next| {
        Box::pin(async move {
            if !state.is_auth_enabled() {
                return Ok(next.run(request).await);
            }

            match check_session_role(&session, role).await {
                RoleCheck::Allowed => Ok(next.run(request).await),
                RoleCheck::NotAuthenticated => Err(StatusCode::UNAUTHORIZED),
                RoleCheck::MissingRole { sub } => {
                    tracing::warn!(
                        required = %role,
                        sub = ?sub,
                        "user lacks required role for route"
                    );
                    Err(StatusCode::FORBIDDEN)
                }
            }
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::config::OidcConfig;
    use crate::oidc::ConfiguredClient;
    use axum::body::Body;
    use axum::middleware as axum_middleware;
    use axum::routing::get;
    use axum::Router;
    use std::sync::Arc;
    use tower::ServiceExt;
    use tower_sessions::SessionManagerLayer;
    use tower_sessions_memory_store::MemoryStore;

    #[derive(Clone)]
    struct TestState {
        oidc_config: Option<OidcConfig>,
    }

    impl OidcAppState for TestState {
        fn oidc_client(&self) -> Option<&Arc<ConfiguredClient>> {
            None
        }
        fn end_session_url(&self) -> Option<&str> {
            None
        }
        fn oidc_config(&self) -> Option<&OidcConfig> {
            self.oidc_config.as_ref()
        }
        fn is_auth_enabled(&self) -> bool {
            self.oidc_config.is_some()
        }
        fn base_url(&self) -> Option<&str> {
            None
        }
        fn http_client(&self) -> &reqwest::Client {
            // Not called in middleware tests
            unimplemented!()
        }
    }

    fn test_state(auth_enabled: bool) -> TestState {
        let oidc_config = if auth_enabled {
            Some(OidcConfig {
                client_id: "test".into(),
                client_secret: "test".into(),
                issuer_url: "https://example.com".into(),
                required_role: "user".into(),
            })
        } else {
            None
        };
        TestState { oidc_config }
    }

    fn test_app(state: TestState) -> Router {
        let store = MemoryStore::default();
        let session_layer = SessionManagerLayer::new(store);

        Router::new()
            .route("/test", get(|| async { "ok" }))
            .route_layer(axum_middleware::from_fn_with_state(
                state.clone(),
                require_session_auth::<TestState>,
            ))
            .with_state(state)
            .layer(session_layer)
    }

    #[tokio::test]
    async fn security_headers_are_set() {
        let app = Router::new()
            .route("/test", get(|| async { "ok" }))
            .layer(axum_middleware::from_fn(security_headers));

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
        assert!(response
            .headers()
            .get("content-security-policy")
            .unwrap()
            .to_str()
            .unwrap()
            .contains("default-src 'self'"));
        assert!(response
            .headers()
            .get("permissions-policy")
            .unwrap()
            .to_str()
            .unwrap()
            .contains("geolocation=()"));
        assert!(response
            .headers()
            .get("strict-transport-security")
            .unwrap()
            .to_str()
            .unwrap()
            .contains("max-age="));
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

        let app = Router::new()
            .route("/test", get(|| async { "ok" }))
            .route_layer(axum_middleware::from_fn_with_state(
                state.clone(),
                require_session_auth::<TestState>,
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

    // --- require_role factory ---

    /// Build an app that gates `/test` with `require_role(role)` and exposes
    /// `/seed?roles=a,b,c` (via `test_utils::with_seed_route`) to set up a
    /// fully authenticated session with the given realm roles.
    fn role_test_app(state: TestState, role: &'static str) -> Router {
        let store = MemoryStore::default();
        let session_layer = SessionManagerLayer::new(store);

        let gated = Router::new()
            .route("/test", get(|| async { "ok" }))
            .route_layer(axum_middleware::from_fn_with_state(
                state.clone(),
                require_role::<TestState>(role),
            ));

        crate::test_utils::with_seed_route(gated)
            .with_state(state)
            .layer(session_layer)
    }

    use crate::test_utils::seed_session;

    async fn get_test(app: Router, cookie: &str) -> StatusCode {
        app.oneshot(
            axum::http::Request::builder()
                .uri("/test")
                .header("cookie", cookie)
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response")
        .status()
    }

    #[tokio::test]
    async fn require_role_passthrough_when_auth_disabled() {
        let app = role_test_app(test_state(false), "editor-writer");
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
    async fn require_role_unauthenticated_returns_401() {
        let app = role_test_app(test_state(true), "editor-writer");
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
    async fn reader_can_access_reader_route() {
        let app = role_test_app(test_state(true), "editor-reader");
        let cookie = seed_session(&app, "editor-reader").await;
        assert_eq!(get_test(app, &cookie).await, StatusCode::OK);
    }

    #[tokio::test]
    async fn reader_cannot_access_writer_route() {
        let app = role_test_app(test_state(true), "editor-writer");
        let cookie = seed_session(&app, "editor-reader").await;
        assert_eq!(get_test(app, &cookie).await, StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn writer_can_access_reader_route() {
        // Composite expansion: a writer's token contains the reader role too.
        let app = role_test_app(test_state(true), "editor-reader");
        let cookie = seed_session(&app, "editor-writer,editor-reader").await;
        assert_eq!(get_test(app, &cookie).await, StatusCode::OK);
    }

    #[tokio::test]
    async fn app_admin_can_access_all_routes_within_app() {
        // Editor-admin composite includes writer and reader. Test the
        // top-of-ladder role hits writer, reader, and a specific-right route.
        for required in ["editor-reader", "editor-writer", "editor-publish"] {
            let app = role_test_app(test_state(true), required);
            let cookie = seed_session(
                &app,
                "editor-admin,editor-writer,editor-reader,editor-publish",
            )
            .await;
            assert_eq!(
                get_test(app, &cookie).await,
                StatusCode::OK,
                "editor-admin should access {required}"
            );
        }
    }

    #[tokio::test]
    async fn regelrecht_admin_can_access_all_apps() {
        // Regelrecht-admin's token transitively contains every sub-role across
        // both apps; a check for any leaf role passes.
        for required in [
            "editor-reader",
            "editor-writer",
            "editor-admin",
            "harvester-reader",
            "harvester-writer",
            "harvester-admin",
        ] {
            let app = role_test_app(test_state(true), required);
            let cookie = seed_session(
                &app,
                "regelrecht-admin,editor-admin,editor-writer,editor-reader,harvester-admin,harvester-writer,harvester-reader",
            )
            .await;
            assert_eq!(
                get_test(app, &cookie).await,
                StatusCode::OK,
                "regelrecht-admin should access {required}"
            );
        }
    }

    #[tokio::test]
    async fn writer_without_specific_right_gets_403() {
        // editor-writer should NOT auto-inherit editor-publish — that's an
        // orthogonal right granted explicitly or via editor-admin composite.
        let app = role_test_app(test_state(true), "editor-publish");
        let cookie = seed_session(&app, "editor-writer,editor-reader").await;
        assert_eq!(get_test(app, &cookie).await, StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn writer_with_specific_right_passes() {
        let app = role_test_app(test_state(true), "editor-publish");
        let cookie = seed_session(&app, "editor-writer,editor-reader,editor-publish").await;
        assert_eq!(get_test(app, &cookie).await, StatusCode::OK);
    }

    #[tokio::test]
    async fn authenticated_with_no_roles_gets_403() {
        // A session that's authenticated but has no roles stored (edge case
        // from a misconfigured Keycloak that issued an empty realm_access)
        // must be denied, not allowed.
        let app = role_test_app(test_state(true), "editor-reader");
        let cookie = seed_session(&app, "").await;
        assert_eq!(get_test(app, &cookie).await, StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn pre_rbac_session_without_roles_key_returns_401() {
        // Simulates a session created BEFORE this PR shipped: the session
        // carries `SESSION_KEY_AUTHENTICATED = true` but the
        // `SESSION_KEY_ROLES` key was never written. The check must report
        // `NotAuthenticated` (→ 401) so the caller's response triggers an
        // OIDC re-login — that login then populates `SESSION_KEY_ROLES` from
        // the JWT and the session self-heals.
        //
        // Returning 403 here (what an `unwrap_or_default()` empty-Vec would
        // produce) would break the user experience during the rolling deploy
        // because 403 does not redirect to the login flow.
        let store = MemoryStore::default();
        let state = test_state(true);
        let session_layer = SessionManagerLayer::new(store);

        let gated = Router::new()
            .route("/test", get(|| async { "ok" }))
            .route_layer(axum_middleware::from_fn_with_state(
                state.clone(),
                require_role::<TestState>("editor-reader"),
            ));

        // Mount a `/seed-pre-rbac` route that ONLY sets
        // SESSION_KEY_AUTHENTICATED and deliberately omits SESSION_KEY_ROLES.
        let app = gated
            .route(
                "/seed-pre-rbac",
                get(|session: Session| async move {
                    session
                        .insert(SESSION_KEY_AUTHENTICATED, true)
                        .await
                        .expect("insert auth");
                    "seeded"
                }),
            )
            .with_state(state)
            .layer(session_layer);

        // Seed the pre-RBAC session.
        let response = app
            .clone()
            .oneshot(
                axum::http::Request::builder()
                    .uri("/seed-pre-rbac")
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

        // The gated request must return 401 (NotAuthenticated), not 403.
        assert_eq!(get_test(app, &cookie).await, StatusCode::UNAUTHORIZED);
    }
}
