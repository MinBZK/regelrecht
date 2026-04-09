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
use subtle::ConstantTimeEq;
use tower_sessions::Session;

use crate::OidcAppState;

pub const SESSION_KEY_CSRF: &str = "oidc_csrf";
pub const SESSION_KEY_NONCE: &str = "oidc_nonce";
pub const SESSION_KEY_PKCE_VERIFIER: &str = "oidc_pkce_verifier";
pub const SESSION_KEY_AUTHENTICATED: &str = "authenticated";
pub const SESSION_KEY_SUB: &str = "person_sub";
pub const SESSION_KEY_EMAIL: &str = "person_email";
pub const SESSION_KEY_NAME: &str = "person_name";
pub const SESSION_KEY_ID_TOKEN: &str = "id_token_hint";
const SESSION_KEY_BASE_URL: &str = "oidc_base_url";

/// Derive the base URL from `BASE_URL` env or request headers.
///
/// Used only during `/auth/login` to construct the initial `redirect_uri`.
/// The result is stored in the session so that `/auth/callback` and
/// `/auth/logout` don't need to trust request headers again — Keycloak's
/// "Valid redirect URIs" check validates the URL at login time.
fn base_url_from_config_or_request<S: OidcAppState>(state: &S, headers: &HeaderMap) -> String {
    if let Some(base_url) = state.base_url() {
        return base_url.to_string();
    }

    let host = headers
        .get("x-forwarded-host")
        .or_else(|| headers.get("host"))
        .and_then(|v| v.to_str().ok())
        .unwrap_or("localhost");

    let scheme = headers
        .get("x-forwarded-proto")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("https");

    format!("{scheme}://{host}")
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

pub async fn login<S: OidcAppState>(
    State(state): State<S>,
    headers: HeaderMap,
    session: Session,
) -> Result<Response, StatusCode> {
    let client = state.oidc_client().ok_or(StatusCode::NOT_IMPLEMENTED)?;

    session.cycle_id().await.map_err(|e| {
        tracing::error!(error = %e, "failed to cycle session ID");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let base_url = base_url_from_config_or_request(&state, &headers);
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
    session_insert(&session, SESSION_KEY_BASE_URL, base_url).await?;

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

pub async fn callback<S: OidcAppState>(
    State(app_state): State<S>,
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

    let client = app_state.oidc_client().ok_or(StatusCode::NOT_IMPLEMENTED)?;

    // Retrieve the base_url that was stored during /auth/login.
    // This is the URL that Keycloak validated against its redirect URI allowlist,
    // so we know it's trusted — no need to re-derive from request headers.
    let base_url: String = session
        .get(SESSION_KEY_BASE_URL)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to read base_url from session");
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or_else(|| {
            tracing::warn!("no base_url in session — session may have expired");
            StatusCode::BAD_REQUEST
        })?;

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

    // Constant-time comparison to prevent timing side-channels.
    let csrf_matches: bool = params.state.as_bytes().ct_eq(stored_csrf.as_bytes()).into();
    if !csrf_matches {
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

    let token_response = client
        .exchange_code(AuthorizationCode::new(code))
        .set_redirect_uri(Cow::Owned(redirect_url))
        .set_pkce_verifier(PkceCodeVerifier::new(stored_pkce))
        .request_async(app_state.http_client())
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
        .oidc_config()
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?
        .required_role;

    // Extract realm roles, preferring the already-verified ID token.
    // Keycloak includes `realm_access` in the access token by default; to use
    // the more secure ID-token path, add a "realm roles" mapper to the client
    // scopes in Keycloak so that `realm_access` is included in the ID token.
    let id_token_jwt = id_token.to_string();
    let realm_roles = extract_realm_roles(&id_token_jwt).or_else(|| {
        tracing::info!(
            "realm_access not found in ID token — falling back to access token. \
             Configure a Keycloak client mapper to include realm_access in the ID token."
        );
        let access_token_secret = get_access_token_secret(&token_response);
        extract_realm_roles(access_token_secret)
    });
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

    session.cycle_id().await.map_err(|e| {
        tracing::error!(error = %e, "failed to rotate session ID");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    if let Err(e) = session.remove::<String>(SESSION_KEY_CSRF).await {
        tracing::warn!(error = %e, "failed to remove CSRF from session");
    }
    if let Err(e) = session.remove::<String>(SESSION_KEY_NONCE).await {
        tracing::warn!(error = %e, "failed to remove nonce from session");
    }
    if let Err(e) = session.remove::<String>(SESSION_KEY_PKCE_VERIFIER).await {
        tracing::warn!(error = %e, "failed to remove PKCE verifier from session");
    }
    if let Err(e) = session.remove::<String>(SESSION_KEY_BASE_URL).await {
        tracing::warn!(error = %e, "failed to remove base_url from session");
    }

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

pub async fn logout<S: OidcAppState>(
    State(state): State<S>,
    session: Session,
) -> Result<Response, StatusCode> {
    let id_token_hint: Option<String> = session.get(SESSION_KEY_ID_TOKEN).await.ok().flatten();

    // For the post-logout redirect, use the explicit BASE_URL config.
    // When not configured, fall back to "/" (relative redirect) to avoid
    // trusting request headers for the redirect target.
    let base_url = state.base_url().unwrap_or_default().to_string();

    session.flush().await.map_err(|e| {
        tracing::error!(error = %e, "failed to flush session");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    if let (Some(end_session_url), Some(oidc)) = (state.end_session_url(), state.oidc_config()) {
        let mut url = url::Url::parse(end_session_url).map_err(|e| {
            tracing::error!(error = %e, "invalid end_session_endpoint URL");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
        url.query_pairs_mut()
            .append_pair("post_logout_redirect_uri", &base_url)
            .append_pair("client_id", &oidc.client_id);
        if let Some(ref hint) = id_token_hint {
            url.query_pairs_mut().append_pair("id_token_hint", hint);
        }

        Ok(Redirect::temporary(url.as_str()).into_response())
    } else {
        Ok(Redirect::temporary(&format!("{base_url}/")).into_response())
    }
}

pub async fn status<S: OidcAppState>(State(state): State<S>, session: Session) -> Json<AuthStatus> {
    let oidc_configured = state.is_auth_enabled();

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

/// Decode `realm_access.roles` from a JWT payload.
///
/// Called first on the signature-verified ID token, then as fallback on the
/// access token received directly from the trusted token endpoint over TLS.
pub fn extract_realm_roles(jwt: &str) -> Option<Vec<String>> {
    let payload_b64 = jwt.split('.').nth(1)?;
    let payload_bytes = URL_SAFE_NO_PAD.decode(payload_b64).ok()?;
    let payload: JwtPayload = serde_json::from_slice(&payload_bytes).ok()?;
    Some(payload.realm_access?.roles)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

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
    fn extract_roles_contains_check() {
        let jwt = fake_jwt(r#"{"realm_access":{"roles":["allowed-user","editor"]}}"#);
        let roles = extract_realm_roles(&jwt).unwrap();
        assert!(roles.contains(&"allowed-user".to_string()));
        assert!(!roles.contains(&"admin".to_string()));
    }
}
