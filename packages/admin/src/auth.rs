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
    use axum::http::HeaderValue;
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use base64::Engine;

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
}
