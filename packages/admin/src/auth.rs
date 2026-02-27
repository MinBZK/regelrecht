use std::borrow::Cow;

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Redirect, Response};
use axum::Json;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use openidconnect::core::CoreResponseType;
use openidconnect::{
    AuthenticationFlow, AuthorizationCode, CsrfToken, Nonce, OAuth2TokenResponse,
    PkceCodeChallenge, PkceCodeVerifier, RedirectUrl, Scope, TokenResponse,
};
use serde::Deserialize;
use serde::Serialize;
use tower_sessions::Session;

use crate::config::AppConfig;
use crate::state::AppState;

const SESSION_KEY_CSRF: &str = "oidc_csrf";
const SESSION_KEY_NONCE: &str = "oidc_nonce";
const SESSION_KEY_PKCE_VERIFIER: &str = "oidc_pkce_verifier";
pub(crate) const SESSION_KEY_AUTHENTICATED: &str = "authenticated";
const SESSION_KEY_SUB: &str = "person_sub";
const SESSION_KEY_EMAIL: &str = "person_email";
const SESSION_KEY_NAME: &str = "person_name";
const SESSION_KEY_ID_TOKEN: &str = "id_token_hint";

fn base_url_from_request(config: &AppConfig, headers: &HeaderMap) -> String {
    // Safety: startup validation in AppConfig::try_from_env ensures BASE_URL is
    // set when OIDC is enabled. If we somehow get here without it, fall back to
    // a safe default rather than panicking.
    let Some(base_url) = config.base_url.as_deref() else {
        tracing::error!("BASE_URL is not set — this should have been caught at startup");
        return "https://localhost".to_string();
    };

    if !config.base_url_allow_dynamic {
        return base_url.to_string();
    }

    // Dynamic mode: try to derive from headers, validate against BASE_URL
    match derive_url_from_headers(headers) {
        Some(derived) => match validate_derived_url(&derived, config) {
            Ok(()) => derived,
            Err(reason) => {
                tracing::warn!(
                    derived_url = %derived,
                    reason = %reason,
                    "rejected header-derived URL, falling back to BASE_URL"
                );
                base_url.to_string()
            }
        },
        None => base_url.to_string(),
    }
}

fn derive_url_from_headers(headers: &HeaderMap) -> Option<String> {
    let host = headers
        .get("x-forwarded-host")
        .or_else(|| headers.get("host"))
        .and_then(|v| v.to_str().ok())?;

    let scheme = headers
        .get("x-forwarded-proto")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("https");

    Some(format!("{scheme}://{host}"))
}

fn validate_derived_url(derived: &str, config: &AppConfig) -> Result<(), String> {
    // Parse scheme and host from derived URL
    let (derived_scheme, derived_host) = derived
        .split_once("://")
        .ok_or_else(|| "invalid URL format".to_string())?;

    // Reject suspicious characters that could enable URL confusion
    if derived_host.contains('@') || derived_host.contains('\\') {
        return Err(format!("suspicious characters in host: {derived_host}"));
    }

    // Validate scheme matches BASE_URL
    if let Some(base_scheme) = config.base_url_scheme() {
        if derived_scheme != base_scheme {
            return Err(format!(
                "scheme mismatch: derived={derived_scheme}, base={base_scheme}"
            ));
        }
    }

    // Validate domain suffix matches BASE_URL
    if let Some(base_suffix) = config.base_url_domain_suffix() {
        // Strip port from derived host for suffix comparison
        let derived_host_no_port = derived_host.split(':').next().unwrap_or(derived_host);
        let derived_suffix = derived_host_no_port
            .find('.')
            .map(|i| &derived_host_no_port[i..]);

        match derived_suffix {
            Some(suffix) if suffix == base_suffix => Ok(()),
            Some(suffix) => Err(format!(
                "domain suffix mismatch: derived={suffix}, base={base_suffix}"
            )),
            None => Err(format!(
                "no domain suffix in derived host: {derived_host_no_port}"
            )),
        }
    } else {
        Err("BASE_URL has no domain suffix to validate against".to_string())
    }
}

#[derive(Serialize)]
pub struct AuthStatus {
    pub authenticated: bool,
    pub oidc_configured: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub person: Option<PersonInfo>,
}

#[derive(Serialize)]
pub struct PersonInfo {
    pub sub: String,
    pub email: String,
    pub name: String,
}

#[derive(Deserialize)]
struct RealmAccess {
    roles: Vec<String>,
}

pub async fn login(
    State(state): State<AppState>,
    headers: HeaderMap,
    session: Session,
) -> Result<Response, StatusCode> {
    let client = state
        .oidc_client
        .as_ref()
        .ok_or(StatusCode::NOT_IMPLEMENTED)?;

    // Cycle the session ID to prevent session fixation attacks.
    session.cycle_id().await.map_err(|e| {
        tracing::error!(error = %e, "failed to cycle session ID");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let base_url = base_url_from_request(&state.config, &headers);
    let redirect_url = RedirectUrl::new(format!("{base_url}/auth/callback")).map_err(|e| {
        tracing::error!(error = %e, "invalid redirect URL");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();

    let (auth_url, csrf_token, nonce) = client
        .authorize_url(
            AuthenticationFlow::<CoreResponseType>::AuthorizationCode,
            CsrfToken::new_random,
            Nonce::new_random,
        )
        .set_redirect_uri(Cow::Owned(redirect_url))
        .add_scope(Scope::new("openid".to_string()))
        .add_scope(Scope::new("email".to_string()))
        .add_scope(Scope::new("profile".to_string()))
        .set_pkce_challenge(pkce_challenge)
        .url();

    session_insert(&session, SESSION_KEY_CSRF, csrf_token.secret().clone()).await?;
    session_insert(&session, SESSION_KEY_NONCE, nonce.secret().clone()).await?;
    session_insert(
        &session,
        SESSION_KEY_PKCE_VERIFIER,
        pkce_verifier.secret().clone(),
    )
    .await?;

    Ok(Redirect::temporary(auth_url.as_str()).into_response())
}

async fn session_insert(session: &Session, key: &str, value: String) -> Result<(), StatusCode> {
    session.insert(key, value).await.map_err(|e| {
        tracing::error!(key, error = %e, "failed to insert into session");
        StatusCode::INTERNAL_SERVER_ERROR
    })
}

#[derive(serde::Deserialize)]
pub struct CallbackQuery {
    pub code: Option<String>,
    pub state: String,
    pub error: Option<String>,
    pub error_description: Option<String>,
}

pub async fn callback(
    State(app_state): State<AppState>,
    headers: HeaderMap,
    session: Session,
    axum::extract::Query(params): axum::extract::Query<CallbackQuery>,
) -> Result<Response, StatusCode> {
    if let Some(ref error) = params.error {
        let description = params.error_description.as_deref().unwrap_or("unknown");
        tracing::warn!(error, description, "IdP returned error on callback");
        return Err(StatusCode::FORBIDDEN);
    }

    let code = params.code.ok_or_else(|| {
        tracing::warn!("callback missing authorization code");
        StatusCode::BAD_REQUEST
    })?;

    let client = app_state
        .oidc_client
        .as_ref()
        .ok_or(StatusCode::NOT_IMPLEMENTED)?;

    let base_url = base_url_from_request(&app_state.config, &headers);
    let redirect_url = RedirectUrl::new(format!("{base_url}/auth/callback")).map_err(|e| {
        tracing::error!(error = %e, "invalid redirect URL");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let stored_csrf: String = session
        .get(SESSION_KEY_CSRF)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to read CSRF from session");
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or_else(|| {
            tracing::warn!("no CSRF token in session");
            StatusCode::BAD_REQUEST
        })?;

    if params.state != stored_csrf {
        tracing::warn!("CSRF token mismatch");
        return Err(StatusCode::BAD_REQUEST);
    }

    let stored_nonce: String = session
        .get(SESSION_KEY_NONCE)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::BAD_REQUEST)?;

    let stored_pkce: String = session
        .get(SESSION_KEY_PKCE_VERIFIER)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::BAD_REQUEST)?;

    let http_client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let token_response = client
        .exchange_code(AuthorizationCode::new(code))
        .set_redirect_uri(Cow::Owned(redirect_url))
        .set_pkce_verifier(PkceCodeVerifier::new(stored_pkce))
        .request_async(&http_client)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "token exchange failed");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let id_token = token_response.id_token().ok_or_else(|| {
        tracing::error!("no ID token in response");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let nonce_verifier = openidconnect::Nonce::new(stored_nonce);
    let claims = id_token
        .claims(&client.id_token_verifier(), &nonce_verifier)
        .map_err(|e| {
            tracing::error!(error = %e, "ID token verification failed");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let required_role = &app_state
        .config
        .oidc
        .as_ref()
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?
        .required_role;

    // Extract realm roles from the access token. Keycloak includes
    // `realm_access` in the access token by default but not in the ID token.
    //
    // SAFETY: the access token is received directly from the token endpoint
    // over TLS, authenticated by client_secret + PKCE. We decode the payload
    // without cryptographic verification because the token endpoint itself is
    // the trust anchor — this is standard for confidential OIDC clients.
    // Do NOT pass untrusted/user-supplied JWTs to this function.
    let access_token_secret = get_access_token_secret(&token_response);
    let realm_roles = extract_realm_roles(access_token_secret);
    tracing::debug!(
        sub = %claims.subject().as_str(),
        roles = ?realm_roles,
        required = %required_role,
        "checking realm roles"
    );

    let has_role = realm_roles
        .as_ref()
        .map(|roles| roles.contains(required_role))
        .unwrap_or(false);

    if !has_role {
        tracing::warn!(role = %required_role, "user lacks required role");
        return Err(StatusCode::FORBIDDEN);
    }

    let sub = claims.subject().as_str().to_string();
    let email = claims.email().map(|e| (**e).clone()).unwrap_or_default();
    let name = claims
        .name()
        .and_then(|n| n.get(None).map(|v| (**v).clone()))
        .or_else(|| claims.preferred_username().map(|u| (**u).clone()))
        .unwrap_or_default();

    let id_token_jwt = id_token.to_string();

    session.cycle_id().await.map_err(|e| {
        tracing::error!(error = %e, "failed to rotate session ID");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let _ = session.remove::<String>(SESSION_KEY_CSRF).await;
    let _ = session.remove::<String>(SESSION_KEY_NONCE).await;
    let _ = session.remove::<String>(SESSION_KEY_PKCE_VERIFIER).await;

    session
        .insert(SESSION_KEY_AUTHENTICATED, true)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    session_insert(&session, SESSION_KEY_SUB, sub.clone()).await?;
    session_insert(&session, SESSION_KEY_EMAIL, email.clone()).await?;
    session_insert(&session, SESSION_KEY_NAME, name.clone()).await?;
    session_insert(&session, SESSION_KEY_ID_TOKEN, id_token_jwt).await?;

    tracing::debug!(email = %email, "OIDC login successful");

    Ok(Redirect::temporary(&format!("{base_url}/")).into_response())
}

pub async fn logout(
    State(state): State<AppState>,
    headers: HeaderMap,
    session: Session,
) -> Result<Response, StatusCode> {
    let id_token_hint: Option<String> = session.get(SESSION_KEY_ID_TOKEN).await.ok().flatten();

    session.flush().await.map_err(|e| {
        tracing::error!(error = %e, "failed to flush session");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let base_url = base_url_from_request(&state.config, &headers);

    if let (Some(ref end_session_url), Some(ref oidc)) =
        (&state.end_session_url, &state.config.oidc)
    {
        let mut params = vec![
            ("post_logout_redirect_uri", base_url),
            ("client_id", oidc.client_id.clone()),
        ];
        if let Some(ref hint) = id_token_hint {
            params.push(("id_token_hint", hint.clone()));
        }

        let query = params
            .iter()
            .map(|(k, v)| format!("{}={}", k, urlencoding::encode(v)))
            .collect::<Vec<_>>()
            .join("&");

        let redirect_url = format!("{end_session_url}?{query}");
        Ok(Redirect::temporary(&redirect_url).into_response())
    } else {
        Ok(Redirect::temporary(&format!("{base_url}/")).into_response())
    }
}

pub async fn status(State(state): State<AppState>, session: Session) -> Json<AuthStatus> {
    let oidc_configured = state.config.is_auth_enabled();

    let authenticated: bool = session
        .get(SESSION_KEY_AUTHENTICATED)
        .await
        .ok()
        .flatten()
        .unwrap_or(false);

    let person = if authenticated {
        let sub: String = session
            .get(SESSION_KEY_SUB)
            .await
            .ok()
            .flatten()
            .unwrap_or_default();
        let email: String = session
            .get(SESSION_KEY_EMAIL)
            .await
            .ok()
            .flatten()
            .unwrap_or_default();
        let name: String = session
            .get(SESSION_KEY_NAME)
            .await
            .ok()
            .flatten()
            .unwrap_or_default();
        Some(PersonInfo { sub, email, name })
    } else {
        None
    };

    Json(AuthStatus {
        authenticated,
        oidc_configured,
        person,
    })
}

#[derive(Deserialize)]
struct JwtPayload {
    realm_access: Option<RealmAccess>,
}

fn get_access_token_secret(resp: &impl OAuth2TokenResponse) -> &str {
    resp.access_token().secret()
}

/// Decode `realm_access.roles` from a JWT payload without signature verification.
///
/// SAFETY: must only be called on tokens received directly from the trusted
/// token endpoint over TLS. Never pass user-supplied or forwarded JWTs.
fn extract_realm_roles(jwt: &str) -> Option<Vec<String>> {
    let payload_b64 = jwt.split('.').nth(1)?;
    let payload_bytes = URL_SAFE_NO_PAD.decode(payload_b64).ok()?;
    let payload: JwtPayload = serde_json::from_slice(&payload_bytes).ok()?;
    Some(payload.realm_access?.roles)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::oidc::ConfiguredClient;
    use axum::body::Body;
    use axum::http::HeaderValue;
    use axum::routing::get;
    use axum::Router;
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use base64::Engine;
    use openidconnect::{ClientId, ClientSecret, ProviderMetadataWithLogout, TokenUrl};
    use sqlx::postgres::PgPoolOptions;
    use std::sync::Arc;
    use tower::ServiceExt;
    use tower_sessions::SessionManagerLayer;
    use tower_sessions_memory_store::MemoryStore;

    fn fake_jwt(payload_json: &str) -> String {
        let header = URL_SAFE_NO_PAD.encode(r#"{"alg":"RS256"}"#);
        let payload = URL_SAFE_NO_PAD.encode(payload_json);
        format!("{header}.{payload}.fake-signature")
    }

    // --- CallbackQuery deserialization ---

    #[test]
    fn callback_query_success_response() {
        let q: CallbackQuery =
            serde_json::from_str(r#"{"code":"abc123","state":"csrf-token"}"#).unwrap();
        assert_eq!(q.code.unwrap(), "abc123");
        assert_eq!(q.state, "csrf-token");
        assert!(q.error.is_none());
        assert!(q.error_description.is_none());
    }

    #[test]
    fn callback_query_error_response() {
        let q: CallbackQuery = serde_json::from_str(
            r#"{"state":"csrf-token","error":"access_denied","error_description":"User denied"}"#,
        )
        .unwrap();
        assert!(q.code.is_none());
        assert_eq!(q.error.unwrap(), "access_denied");
        assert_eq!(q.error_description.unwrap(), "User denied");
    }

    #[test]
    fn callback_query_missing_state_fails() {
        let result: Result<CallbackQuery, _> = serde_json::from_str(r#"{"code":"abc123"}"#);
        assert!(result.is_err());
    }

    // --- extract_realm_roles ---

    #[test]
    fn extract_roles_with_valid_realm_access() {
        let jwt = fake_jwt(r#"{"realm_access":{"roles":["allowed-user","viewer"]}}"#);
        let roles = extract_realm_roles(&jwt).unwrap();
        assert_eq!(roles, vec!["allowed-user", "viewer"]);
    }

    #[test]
    fn extract_roles_missing_realm_access() {
        let jwt = fake_jwt(r#"{"sub":"user1"}"#);
        assert!(extract_realm_roles(&jwt).is_none());
    }

    #[test]
    fn extract_roles_empty_roles_array() {
        let jwt = fake_jwt(r#"{"realm_access":{"roles":[]}}"#);
        let roles = extract_realm_roles(&jwt).unwrap();
        assert!(roles.is_empty());
    }

    #[test]
    fn extract_roles_invalid_jwt() {
        assert!(extract_realm_roles("not-a-jwt").is_none());
    }

    #[test]
    fn extract_roles_invalid_base64_payload() {
        assert!(extract_realm_roles("header.!!!invalid!!!.sig").is_none());
    }

    #[test]
    fn extract_roles_contains_check() {
        let jwt = fake_jwt(r#"{"realm_access":{"roles":["allowed-user","editor"]}}"#);
        let roles = extract_realm_roles(&jwt).unwrap();
        assert!(roles.contains(&"allowed-user".to_string()));
        assert!(!roles.contains(&"admin".to_string()));
    }

    // --- base_url_from_request ---

    fn config_static(url: &str) -> AppConfig {
        AppConfig {
            oidc: None,
            base_url: Some(url.to_string()),
            base_url_allow_dynamic: false,
        }
    }

    fn config_dynamic(url: &str) -> AppConfig {
        AppConfig {
            oidc: None,
            base_url: Some(url.to_string()),
            base_url_allow_dynamic: true,
        }
    }

    // --- Static mode tests ---

    #[test]
    fn static_mode_returns_base_url_ignoring_headers() {
        let config = config_static("https://admin.rig.example.nl");
        let mut headers = HeaderMap::new();
        headers.insert("host", HeaderValue::from_static("evil.attacker.com"));
        assert_eq!(
            base_url_from_request(&config, &headers),
            "https://admin.rig.example.nl"
        );
    }

    #[test]
    fn static_mode_returns_base_url_with_no_headers() {
        let config = config_static("https://admin.example.com");
        let headers = HeaderMap::new();
        assert_eq!(
            base_url_from_request(&config, &headers),
            "https://admin.example.com"
        );
    }

    // --- Dynamic mode tests ---

    #[test]
    fn dynamic_mode_accepts_matching_suffix() {
        let config = config_dynamic("https://admin.rig.example.nl");
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-forwarded-host",
            HeaderValue::from_static("pr42.rig.example.nl"),
        );
        headers.insert("x-forwarded-proto", HeaderValue::from_static("https"));
        assert_eq!(
            base_url_from_request(&config, &headers),
            "https://pr42.rig.example.nl"
        );
    }

    #[test]
    fn dynamic_mode_rejects_wrong_domain() {
        let config = config_dynamic("https://admin.rig.example.nl");
        let mut headers = HeaderMap::new();
        headers.insert("host", HeaderValue::from_static("evil.attacker.com"));
        // Should fall back to BASE_URL
        assert_eq!(
            base_url_from_request(&config, &headers),
            "https://admin.rig.example.nl"
        );
    }

    #[test]
    fn dynamic_mode_rejects_scheme_mismatch() {
        let config = config_dynamic("https://admin.rig.example.nl");
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-forwarded-host",
            HeaderValue::from_static("pr42.rig.example.nl"),
        );
        headers.insert("x-forwarded-proto", HeaderValue::from_static("http"));
        // Should fall back to BASE_URL
        assert_eq!(
            base_url_from_request(&config, &headers),
            "https://admin.rig.example.nl"
        );
    }

    #[test]
    fn dynamic_mode_rejects_at_sign_in_host() {
        let config = config_dynamic("https://admin.rig.example.nl");
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-forwarded-host",
            HeaderValue::from_static("user@evil.rig.example.nl"),
        );
        headers.insert("x-forwarded-proto", HeaderValue::from_static("https"));
        assert_eq!(
            base_url_from_request(&config, &headers),
            "https://admin.rig.example.nl"
        );
    }

    #[test]
    fn dynamic_mode_rejects_backslash_in_host() {
        let config = config_dynamic("https://admin.rig.example.nl");
        let mut headers = HeaderMap::new();
        headers.insert("host", HeaderValue::from_static("evil\\x.rig.example.nl"));
        assert_eq!(
            base_url_from_request(&config, &headers),
            "https://admin.rig.example.nl"
        );
    }

    #[test]
    fn dynamic_mode_no_headers_falls_back() {
        let config = config_dynamic("https://admin.rig.example.nl");
        let headers = HeaderMap::new();
        assert_eq!(
            base_url_from_request(&config, &headers),
            "https://admin.rig.example.nl"
        );
    }

    #[test]
    fn dynamic_mode_forwarded_host_takes_priority_over_host() {
        let config = config_dynamic("https://admin.rig.example.nl");
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-forwarded-host",
            HeaderValue::from_static("pr99.rig.example.nl"),
        );
        headers.insert("x-forwarded-proto", HeaderValue::from_static("https"));
        headers.insert("host", HeaderValue::from_static("internal:8000"));
        assert_eq!(
            base_url_from_request(&config, &headers),
            "https://pr99.rig.example.nl"
        );
    }

    // --- derive_url_from_headers ---

    #[test]
    fn derive_url_returns_none_without_host() {
        let headers = HeaderMap::new();
        assert!(derive_url_from_headers(&headers).is_none());
    }

    #[test]
    fn derive_url_defaults_to_https() {
        let mut headers = HeaderMap::new();
        headers.insert("host", HeaderValue::from_static("example.com"));
        assert_eq!(
            derive_url_from_headers(&headers).unwrap(),
            "https://example.com"
        );
    }

    // --- validate_derived_url ---

    #[test]
    fn validate_rejects_bare_hostname() {
        let config = config_dynamic("https://admin.rig.example.nl");
        let result = validate_derived_url("https://localhost", &config);
        assert!(result.is_err());
    }

    // =========================================================================
    // Integration test helpers
    // =========================================================================

    use std::sync::LazyLock;

    /// Shared RSA key pair generated once for all tests.
    static TEST_RSA_KEY: LazyLock<rsa::RsaPrivateKey> = LazyLock::new(|| {
        use rsa::pkcs1::DecodeRsaPrivateKey;
        rsa::RsaPrivateKey::from_pkcs1_pem(TEST_RSA_PEM).expect("valid test RSA key")
    });

    const TEST_RSA_PEM: &str = "\
-----BEGIN RSA PRIVATE KEY-----
MIIEpAIBAAKCAQEAtwt6Yii90rabfWrceTMAb6/lIkDXWywJZW5CGJBnm6ePnxdi
yeAJM3I4CGLXJb5mYN/ACLAWjrsac6M2PyBEIdPdwnJ1PcvwkVGOeqomT7GUKtCL
UwWshGP0wTIjFeY7RIyOmCd7I2rO5kMYuEOq+XfOBWXpWIhOSeFgyCOxjK0UC6Lq
aszFIPIg5CJdWmBKIJnqOvPfl7KJSgxdcEK/ETzutBP61VVOGC+3oOGQu3UYr91x
xHpvrebZ8G0InPrfPbfAB3jvXqK6qwIqbYs/9buKX5OQzKna5fp4725iYi6a0Eeg
qMuD3rESaE1EG0gMRUYEF3ECvdrSe8cSziHyKwIDAQABAoIBAAy4vf/oz/np722X
NI0x3RO7ba6PQ3MWi5f37Ue9cDinu891SyGNB2atcgqB1W0jSgSX7cX3eGHhdsms
Vr6qv0F7SEbVjfjGXfO474ZD9sIELVrlFUHRu6Hp5olaMt5jRXboA+28P2PV7lz0
3djJ+diObzb91GrER8NSaC0QxKwU/vN/BWWsKvkM/IJKvYCWOPbiuFNC/JbWzKaS
SP8DUf3X1Qwepwt6sQiLjZSz5qrd5Qr4GafBCNhnlBaXIILpKTPiiFr62jOej42A
VW3kgAgf0QdDHNDztxb1yb4rDrIg+FD9QdTrhzIx0VI4blI6xLUa/u24HXu8UjA5
8jm7D0kCgYEA+5uTAslPkE+wlzCDyFef37gR5+ERgzGoVj0vAMB1oPwxPZOES4Jy
vT0cc/WL0iE0O4DXjCXN0er6zePCy8TL6JrcfaQmqKRa6oerwy2jCmsQUFCcQioX
MS7iYhk4eQ3DjT8cBE86ZVLIS2f5exZbLFLEMKQ5i8hyS5k0RxVwhlkCgYEAuj1/
sPYlvqLaPauH4yAWPICV6s16d3+s1fI33ZCGTz4ADfEFKShHSGLXMaHT/taMJR+F
e4PJ6WWP5D9eH1EFlN3d6l8rWqm2tAq5/cxT00ylmQnyVCYWrKzAA/Rk3kGnyz0+
hircHfjSk2wtktH2QUpXtWDRFkb/3Es1WZRxtCMCgYEAlUlAl+WkHKb7yykQ9/zt
sgsALMoA3wvGqqyQx+xpnsQj3zo4w6i5tYid6jul416qJCgVPGVt0oCOoTzjZo30
wqWn77BG88bY3tDy29KnK1ZNDqpVnHhm3FrKHZSDSmgdQCBS2ke8CURt7Tfa8epY
3FqbZ5T5Q/QBxNM5DngtFLkCgYBUIhAbOzdV5W+9yE181zP0ZQpUpjqa3TyQ8fk2
yGFETvfrVGRGcYGyO6SHMVn5l6Z75r+ASsrd+xmDvPSiJRHmbEwh4phNPrngn6/h
7Xo4zDlK52lnhkVcADZGExO2K+bHM4WZSqdhitRl8MqtttgOKq1wrKoH7E8Nj5Qs
QZkUDQKBgQDuX3YCnHbbyk1fgJXX678uLuf7MvdpKgh7AdIeV0pKgJNGXFIg7h+Y
xDLWfAIUr3n54YRTUYWRFrzg60H3RWCBST5KE+oTtpljuRprs5Z6gOYxGLOCgwqY
FEs4SYxqDdCakQ9CV5M4uyyjLrxg+/Ra9BqycPcmJGQQrVhnTnBa2g==
-----END RSA PRIVATE KEY-----";

    /// Build a JWKS JSON object from the test RSA public key.
    fn test_jwks_json() -> serde_json::Value {
        use rsa::traits::PublicKeyParts;
        let pub_key = TEST_RSA_KEY.to_public_key();
        let n = URL_SAFE_NO_PAD.encode(pub_key.n().to_bytes_be());
        let e = URL_SAFE_NO_PAD.encode(pub_key.e().to_bytes_be());
        serde_json::json!({
            "keys": [{
                "kty": "RSA",
                "use": "sig",
                "alg": "RS256",
                "kid": "test-key",
                "n": n,
                "e": e
            }]
        })
    }

    /// Build an OIDC client for tests. `base_url` should be the wiremock
    /// server URI (or any URL — token and JWKS paths are appended).
    fn test_oidc_client(base_url: &str) -> ConfiguredClient {
        let token_url = &format!("{base_url}/token");
        let jwks_url = &format!("{base_url}/jwks");
        test_oidc_client_urls(token_url, jwks_url)
    }

    fn test_oidc_client_urls(token_url: &str, jwks_url: &str) -> ConfiguredClient {
        use openidconnect::core::CoreJsonWebKeySet;

        let token = TokenUrl::new(token_url.into()).expect("valid token URL");

        // Build provider metadata JSON and deserialize as ProviderMetadataWithLogout.
        // This avoids fighting the complex generic type parameters.
        let metadata_json = serde_json::json!({
            "issuer": "https://idp.test.example/realms/test",
            "authorization_endpoint": "https://idp.test.example/realms/test/protocol/openid-connect/auth",
            "token_endpoint": token_url,
            "jwks_uri": jwks_url,
            "response_types_supported": ["code"],
            "subject_types_supported": ["public"],
            "id_token_signing_alg_values_supported": ["RS256"]
        });

        let mut provider_with_logout: ProviderMetadataWithLogout =
            serde_json::from_value(metadata_json)
                .expect("deserialize as ProviderMetadataWithLogout");

        // The JWKS field is #[serde(skip)] so we must set it manually.
        // Without this, ID token signature verification would fail.
        let jwks: CoreJsonWebKeySet =
            serde_json::from_value(test_jwks_json()).expect("deserialize JWKS");
        provider_with_logout = provider_with_logout.set_jwks(jwks);

        openidconnect::core::CoreClient::from_provider_metadata(
            provider_with_logout,
            ClientId::new("test-client".into()),
            Some(ClientSecret::new("test-secret".into())),
        )
        .set_token_uri(token)
    }

    fn test_state_with_oidc(client: ConfiguredClient, end_session_url: Option<&str>) -> AppState {
        let config = AppConfig {
            oidc: Some(crate::config::OidcConfig {
                client_id: "test-client".into(),
                client_secret: "test-secret".into(),
                issuer_url: "https://idp.test.example/realms/test".into(),
                required_role: "allowed-user".into(),
            }),
            base_url: Some("https://admin.test.example".into()),
            base_url_allow_dynamic: false,
        };

        #[allow(clippy::expect_used)]
        let pool = PgPoolOptions::new()
            .connect_lazy("postgres://test@localhost/test")
            .expect("lazy pool");

        AppState {
            pool,
            oidc_client: Some(Arc::new(client)),
            end_session_url: end_session_url.map(String::from),
            config: Arc::new(config),
        }
    }

    fn test_state_no_oidc() -> AppState {
        let config = AppConfig {
            oidc: None,
            base_url: None,
            base_url_allow_dynamic: false,
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
        }
    }

    fn test_app(state: AppState) -> Router {
        let store = MemoryStore::default();
        let session_layer = SessionManagerLayer::new(store);

        Router::new()
            .route("/auth/login", get(login))
            .route("/auth/callback", get(callback))
            .route("/auth/logout", get(logout))
            .route("/auth/status", get(status))
            .route("/test/set-session", get(set_session_handler))
            .with_state(state)
            .layer(session_layer)
    }

    #[derive(Deserialize)]
    struct SetSessionQuery {
        csrf: Option<String>,
        nonce: Option<String>,
        pkce_verifier: Option<String>,
        authenticated: Option<bool>,
        sub: Option<String>,
        email: Option<String>,
        name: Option<String>,
        id_token_hint: Option<String>,
    }

    async fn set_session_handler(
        session: Session,
        axum::extract::Query(q): axum::extract::Query<SetSessionQuery>,
    ) -> &'static str {
        if let Some(v) = q.csrf {
            session
                .insert(SESSION_KEY_CSRF, v)
                .await
                .expect("insert csrf");
        }
        if let Some(v) = q.nonce {
            session
                .insert(SESSION_KEY_NONCE, v)
                .await
                .expect("insert nonce");
        }
        if let Some(v) = q.pkce_verifier {
            session
                .insert(SESSION_KEY_PKCE_VERIFIER, v)
                .await
                .expect("insert pkce");
        }
        if let Some(v) = q.authenticated {
            session
                .insert(SESSION_KEY_AUTHENTICATED, v)
                .await
                .expect("insert auth");
        }
        if let Some(v) = q.sub {
            session
                .insert(SESSION_KEY_SUB, v)
                .await
                .expect("insert sub");
        }
        if let Some(v) = q.email {
            session
                .insert(SESSION_KEY_EMAIL, v)
                .await
                .expect("insert email");
        }
        if let Some(v) = q.name {
            session
                .insert(SESSION_KEY_NAME, v)
                .await
                .expect("insert name");
        }
        if let Some(v) = q.id_token_hint {
            session
                .insert(SESSION_KEY_ID_TOKEN, v)
                .await
                .expect("insert id_token");
        }
        "ok"
    }

    fn extract_cookie(response: &axum::http::Response<Body>) -> String {
        response
            .headers()
            .get("set-cookie")
            .expect("set-cookie header")
            .to_str()
            .expect("cookie str")
            .to_string()
    }

    fn build_id_token(nonce: &str, _roles: &[&str]) -> String {
        use rsa::pkcs1v15::SigningKey;
        use rsa::signature::SignatureEncoding;
        use rsa::signature::Signer;

        let now = chrono::Utc::now();
        let iat = now.timestamp();
        let exp = (now + chrono::Duration::hours(1)).timestamp();

        let header = serde_json::json!({
            "alg": "RS256",
            "typ": "JWT",
            "kid": "test-key"
        });

        let payload = serde_json::json!({
            "iss": "https://idp.test.example/realms/test",
            "sub": "test-user-id",
            "aud": "test-client",
            "exp": exp,
            "iat": iat,
            "nonce": nonce,
            "email": "test@example.com",
            "preferred_username": "testuser",
            "name": "Test User"
        });

        let header_b64 = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&header).expect("header json"));
        let payload_b64 =
            URL_SAFE_NO_PAD.encode(serde_json::to_vec(&payload).expect("payload json"));

        let message = format!("{header_b64}.{payload_b64}");

        let signing_rsa_key = SigningKey::<sha2::Sha256>::new(TEST_RSA_KEY.clone());
        let signature = signing_rsa_key.sign(message.as_bytes());
        let sig_b64 = URL_SAFE_NO_PAD.encode(signature.to_vec());

        format!("{message}.{sig_b64}")
    }

    fn build_access_token(roles: &[&str]) -> String {
        let roles_json: Vec<String> = roles.iter().map(|r| format!("\"{r}\"")).collect();
        let payload = format!(
            r#"{{"realm_access":{{"roles":[{}]}}}}"#,
            roles_json.join(",")
        );
        fake_jwt(&payload)
    }

    fn build_token_response_json(id_token: &str, access_token: &str) -> serde_json::Value {
        serde_json::json!({
            "access_token": access_token,
            "token_type": "Bearer",
            "expires_in": 3600,
            "id_token": id_token
        })
    }

    // =========================================================================
    // Status handler tests
    // =========================================================================

    #[tokio::test]
    async fn status_unauthenticated_returns_false() {
        let client = test_oidc_client("https://idp.test.example");
        let state = test_state_with_oidc(client, None);
        let app = test_app(state);

        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/auth/status")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body");
        let json: serde_json::Value = serde_json::from_slice(&body).expect("json");
        assert_eq!(json["authenticated"], false);
        assert_eq!(json["oidc_configured"], true);
        assert!(json.get("person").is_none() || json["person"].is_null());
    }

    #[tokio::test]
    async fn status_authenticated_returns_person() {
        let client = test_oidc_client("https://idp.test.example");
        let state = test_state_with_oidc(client, None);
        let app = test_app(state);

        // Set up authenticated session
        let response = app
            .clone()
            .oneshot(
                axum::http::Request::builder()
                    .uri("/test/set-session?authenticated=true&sub=user-123&email=test%40example.com&name=Test%20User")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::OK);
        let cookie = extract_cookie(&response);

        // Check status with session cookie
        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/auth/status")
                    .header("cookie", &cookie)
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body");
        let json: serde_json::Value = serde_json::from_slice(&body).expect("json");
        assert_eq!(json["authenticated"], true);
        assert_eq!(json["oidc_configured"], true);
        assert_eq!(json["person"]["sub"], "user-123");
        assert_eq!(json["person"]["email"], "test@example.com");
        assert_eq!(json["person"]["name"], "Test User");
    }

    #[tokio::test]
    async fn status_oidc_disabled_returns_not_configured() {
        let state = test_state_no_oidc();
        let app = test_app(state);

        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/auth/status")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body");
        let json: serde_json::Value = serde_json::from_slice(&body).expect("json");
        assert_eq!(json["authenticated"], false);
        assert_eq!(json["oidc_configured"], false);
    }

    // =========================================================================
    // Login handler tests
    // =========================================================================

    #[tokio::test]
    async fn login_redirects_to_idp() {
        let client = test_oidc_client("https://idp.test.example");
        let state = test_state_with_oidc(client, None);
        let app = test_app(state);

        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/auth/login")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::TEMPORARY_REDIRECT);
        let location = response
            .headers()
            .get("location")
            .expect("location header")
            .to_str()
            .expect("location str");

        assert!(
            location
                .starts_with("https://idp.test.example/realms/test/protocol/openid-connect/auth"),
            "expected redirect to IdP auth URL, got: {location}"
        );
        assert!(
            location.contains("response_type=code"),
            "missing response_type=code"
        );
        assert!(
            location.contains("client_id=test-client"),
            "missing client_id"
        );
        assert!(location.contains("scope=openid"), "missing scope=openid");
    }

    #[tokio::test]
    async fn login_without_oidc_client_returns_501() {
        let state = test_state_no_oidc();
        let app = test_app(state);

        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/auth/login")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::NOT_IMPLEMENTED);
    }

    // =========================================================================
    // Logout handler tests
    // =========================================================================

    #[tokio::test]
    async fn logout_with_end_session_url_redirects_to_idp() {
        let client = test_oidc_client("https://idp.test.example");
        let state = test_state_with_oidc(
            client,
            Some("https://idp.test.example/realms/test/protocol/openid-connect/logout"),
        );
        let app = test_app(state);

        // Set up session with id_token_hint
        let response = app
            .clone()
            .oneshot(
                axum::http::Request::builder()
                    .uri("/test/set-session?authenticated=true&id_token_hint=some.jwt.token")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        let cookie = extract_cookie(&response);

        // Logout
        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/auth/logout")
                    .header("cookie", &cookie)
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::TEMPORARY_REDIRECT);
        let location = response
            .headers()
            .get("location")
            .expect("location header")
            .to_str()
            .expect("location str");

        assert!(
            location
                .starts_with("https://idp.test.example/realms/test/protocol/openid-connect/logout"),
            "expected redirect to IdP logout, got: {location}"
        );
        assert!(
            location.contains("post_logout_redirect_uri="),
            "missing post_logout_redirect_uri"
        );
        assert!(
            location.contains("client_id=test-client"),
            "missing client_id"
        );
        assert!(location.contains("id_token_hint="), "missing id_token_hint");
    }

    #[tokio::test]
    async fn logout_without_end_session_url_redirects_to_base() {
        let client = test_oidc_client("https://idp.test.example");
        let state = test_state_with_oidc(client, None);
        let app = test_app(state);

        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/auth/logout")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::TEMPORARY_REDIRECT);
        let location = response
            .headers()
            .get("location")
            .expect("location header")
            .to_str()
            .expect("location str");
        assert_eq!(location, "https://admin.test.example/");
    }

    #[tokio::test]
    async fn logout_flushes_session() {
        let client = test_oidc_client("https://idp.test.example");
        let state = test_state_with_oidc(client, None);
        let store = MemoryStore::default();
        let session_layer = SessionManagerLayer::new(store);

        let app = Router::new()
            .route("/auth/logout", get(logout))
            .route("/auth/status", get(status))
            .route("/test/set-session", get(set_session_handler))
            .with_state(state)
            .layer(session_layer);

        // Set up authenticated session
        let response = app
            .clone()
            .oneshot(
                axum::http::Request::builder()
                    .uri("/test/set-session?authenticated=true&sub=u1&email=e%40e.com&name=N")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        let cookie = extract_cookie(&response);

        // Logout
        let response = app
            .clone()
            .oneshot(
                axum::http::Request::builder()
                    .uri("/auth/logout")
                    .header("cookie", &cookie)
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::TEMPORARY_REDIRECT);

        // The old session cookie should now show unauthenticated
        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/auth/status")
                    .header("cookie", &cookie)
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body");
        let json: serde_json::Value = serde_json::from_slice(&body).expect("json");
        assert_eq!(json["authenticated"], false);
    }

    // =========================================================================
    // Callback handler tests
    // =========================================================================

    #[tokio::test]
    async fn callback_success_flow() {
        let mock_server = wiremock::MockServer::start().await;

        let nonce = "test-nonce-123";
        let csrf = "test-csrf-456";
        let id_token = build_id_token(nonce, &["allowed-user"]);
        let access_token = build_access_token(&["allowed-user"]);
        let token_body = build_token_response_json(&id_token, &access_token);

        wiremock::Mock::given(wiremock::matchers::method("POST"))
            .and(wiremock::matchers::path("/token"))
            .respond_with(
                wiremock::ResponseTemplate::new(200)
                    .set_body_json(&token_body)
                    .insert_header("content-type", "application/json"),
            )
            .mount(&mock_server)
            .await;

        let client = test_oidc_client(&mock_server.uri());
        let state = test_state_with_oidc(client, None);

        let store = MemoryStore::default();
        let session_layer = SessionManagerLayer::new(store);

        let app = Router::new()
            .route("/auth/callback", get(callback))
            .route("/auth/status", get(status))
            .route("/test/set-session", get(set_session_handler))
            .with_state(state)
            .layer(session_layer);

        // Pre-populate session with CSRF, nonce, PKCE verifier
        let pkce_verifier = "test-pkce-verifier";
        let set_uri =
            format!("/test/set-session?csrf={csrf}&nonce={nonce}&pkce_verifier={pkce_verifier}");
        let response = app
            .clone()
            .oneshot(
                axum::http::Request::builder()
                    .uri(&set_uri)
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::OK);
        let cookie = extract_cookie(&response);

        // Hit callback
        let callback_uri = format!("/auth/callback?code=test-code&state={csrf}");
        let response = app
            .clone()
            .oneshot(
                axum::http::Request::builder()
                    .uri(&callback_uri)
                    .header("cookie", &cookie)
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(
            response.status(),
            StatusCode::TEMPORARY_REDIRECT,
            "expected redirect after successful callback"
        );

        // Extract the new cookie (session was cycled)
        let new_cookie = extract_cookie(&response);

        // Verify session is authenticated via /auth/status
        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/auth/status")
                    .header("cookie", &new_cookie)
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body");
        let json: serde_json::Value = serde_json::from_slice(&body).expect("json");
        assert_eq!(json["authenticated"], true);
        assert_eq!(json["person"]["sub"], "test-user-id");
        assert_eq!(json["person"]["email"], "test@example.com");
    }

    #[tokio::test]
    async fn callback_error_from_idp_returns_403() {
        let client = test_oidc_client("https://idp.test.example");
        let state = test_state_with_oidc(client, None);
        let app = test_app(state);

        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/auth/callback?error=access_denied&error_description=User%20denied&state=some-csrf")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn callback_csrf_mismatch_returns_400() {
        let client = test_oidc_client("https://idp.test.example");
        let state = test_state_with_oidc(client, None);
        let app = test_app(state);

        // Set session with one CSRF
        let response = app
            .clone()
            .oneshot(
                axum::http::Request::builder()
                    .uri("/test/set-session?csrf=correct-csrf&nonce=n&pkce_verifier=p")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        let cookie = extract_cookie(&response);

        // Callback with wrong CSRF
        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/auth/callback?code=test-code&state=wrong-csrf")
                    .header("cookie", &cookie)
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn callback_missing_code_returns_400() {
        let client = test_oidc_client("https://idp.test.example");
        let state = test_state_with_oidc(client, None);
        let app = test_app(state);

        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/auth/callback?state=some-csrf")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn callback_missing_required_role_returns_403() {
        let mock_server = wiremock::MockServer::start().await;

        let nonce = "test-nonce-role";
        let csrf = "test-csrf-role";
        // Token has "viewer" role but NOT "allowed-user"
        let id_token = build_id_token(nonce, &["viewer"]);
        let access_token = build_access_token(&["viewer"]);
        let token_body = build_token_response_json(&id_token, &access_token);

        wiremock::Mock::given(wiremock::matchers::method("POST"))
            .and(wiremock::matchers::path("/token"))
            .respond_with(
                wiremock::ResponseTemplate::new(200)
                    .set_body_json(&token_body)
                    .insert_header("content-type", "application/json"),
            )
            .mount(&mock_server)
            .await;

        let client = test_oidc_client(&mock_server.uri());
        let state = test_state_with_oidc(client, None);

        let store = MemoryStore::default();
        let session_layer = SessionManagerLayer::new(store);

        let app = Router::new()
            .route("/auth/callback", get(callback))
            .route("/test/set-session", get(set_session_handler))
            .with_state(state)
            .layer(session_layer);

        // Pre-populate session
        let set_uri =
            format!("/test/set-session?csrf={csrf}&nonce={nonce}&pkce_verifier=test-pkce");
        let response = app
            .clone()
            .oneshot(
                axum::http::Request::builder()
                    .uri(&set_uri)
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        let cookie = extract_cookie(&response);

        // Callback with valid code but wrong roles
        let callback_uri = format!("/auth/callback?code=test-code&state={csrf}");
        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri(&callback_uri)
                    .header("cookie", &cookie)
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn callback_without_oidc_client_returns_501() {
        let state = test_state_no_oidc();
        let app = test_app(state);

        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/auth/callback?code=test&state=csrf")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::NOT_IMPLEMENTED);
    }
}
