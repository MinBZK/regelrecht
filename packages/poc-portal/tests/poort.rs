//! The gate, through the real router.
//!
//! The unit tests prove the cookie cannot be forged; these prove the router
//! actually asks. Those are different claims, and the second is the one that
//! breaks when a route is registered in the wrong order or a layer is added
//! after the fallback.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use tower::ServiceExt as _;

use regelrecht_poc_portal::app::{router, AppState};
use regelrecht_poc_portal::config::Config;
use regelrecht_poc_portal::gate;

const SECRET: &str = "0123456789abcdef0123456789abcdef";

const REGISTER: &str = r#"
pocs:
  - slug: alfa
    titel: Alfa
    samenvatting: De eerste
    soort: statisch
    bron: poc-alfa
  - slug: beta
    titel: Beta
    samenvatting: De tweede
    soort: statisch
    bron: poc-beta
"#;

/// Build a portal over a register of two static PoCs.
///
/// The env vars are process-wide, so every test here runs against the same
/// set; they are only ever set to these values.
fn app() -> axum::Router {
    std::env::set_var("POC_COOKIE_SECRET", SECRET);
    std::env::set_var("POC_PW_ALFA", "alfa-geheim");
    std::env::set_var("POC_PW_BETA", "beta-geheim");
    std::env::set_var("POC_STATIC_DIR", "/nonexistent");
    let config = Config::from_env(REGISTER).expect("config");
    router(AppState::new(config))
}

fn cookie_voor(slug: &str) -> String {
    let sleutel = gate::Sleutel::new(SECRET).expect("key");
    format!(
        "{}={}",
        gate::cookie_naam(slug),
        gate::maak_cookie(&sleutel, slug, time::OffsetDateTime::now_utc())
    )
}

async fn get(app: &axum::Router, pad: &str, cookie: Option<&str>) -> (StatusCode, String) {
    let mut req = Request::builder().uri(pad).method("GET");
    if let Some(c) = cookie {
        req = req.header(header::COOKIE, c);
    }
    let response = app
        .clone()
        .oneshot(req.body(Body::empty()).expect("request"))
        .await
        .expect("response");
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), 1 << 20)
        .await
        .expect("body");
    (status, String::from_utf8_lossy(&bytes).into_owned())
}

#[tokio::test]
async fn the_index_is_public() {
    let (status, body) = get(&app(), "/", None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("Alfa") && body.contains("Beta"));
}

#[tokio::test]
async fn health_is_public() {
    let (status, body) = get(&app(), "/health", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, "OK");
}

#[tokio::test]
async fn a_poc_without_a_cookie_asks_for_the_password() {
    let (status, body) = get(&app(), "/alfa/", None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert!(body.contains("Wachtwoord"));
    assert!(body.contains(r#"action="/_toegang/alfa""#));
}

#[tokio::test]
async fn a_deep_link_keeps_its_address_so_signing_in_continues_there() {
    // The reason for 401-with-a-body rather than a redirect.
    let (status, body) = get(&app(), "/alfa/beleid/detail", None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert!(body.contains(r#"value="/alfa/beleid/detail""#));
}

#[tokio::test]
async fn one_pocs_cookie_does_not_open_another() {
    // The property the whole per-PoC design exists for, verified through the
    // router rather than only over the signing function.
    let app = app();
    let (status, _) = get(&app, "/beta/", Some(&cookie_voor("alfa"))).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn a_valid_cookie_gets_past_the_gate() {
    // POC_STATIC_DIR points at nothing, so "past the gate" shows up as a 404
    // from the file service rather than the 401 the gate would return.
    let app = app();
    let (status, _) = get(&app, "/alfa/", Some(&cookie_voor("alfa"))).await;
    assert_ne!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn an_unknown_poc_is_not_found_rather_than_a_password_prompt() {
    let (status, _) = get(&app(), "/bestaat-niet/", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

async fn post_wachtwoord(app: &axum::Router, slug: &str, body: &str) -> axum::response::Response {
    app.clone()
        .oneshot(
            Request::builder()
                .uri(format!("/_toegang/{slug}"))
                .method("POST")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(body.to_string()))
                .expect("request"),
        )
        .await
        .expect("response")
}

#[tokio::test]
async fn the_right_password_sets_a_cookie_scoped_to_that_poc() {
    let response =
        post_wachtwoord(&app(), "alfa", "wachtwoord=alfa-geheim&verder=%2Falfa%2F").await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);

    let cookie = response
        .headers()
        .get(header::SET_COOKIE)
        .and_then(|v| v.to_str().ok())
        .expect("set-cookie");
    assert!(cookie.starts_with("poc_alfa="));
    assert!(cookie.contains("Path=/alfa/"), "{cookie}");
    assert!(cookie.contains("HttpOnly"), "{cookie}");
    assert!(cookie.contains("Secure"), "{cookie}");
    assert!(cookie.contains("SameSite=Lax"), "{cookie}");

    assert_eq!(
        response
            .headers()
            .get(header::LOCATION)
            .and_then(|v| v.to_str().ok()),
        Some("/alfa/")
    );
}

#[tokio::test]
async fn the_wrong_password_sets_no_cookie() {
    let response = post_wachtwoord(&app(), "alfa", "wachtwoord=fout&verder=%2Falfa%2F").await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert!(response.headers().get(header::SET_COOKIE).is_none());
}

#[tokio::test]
async fn another_pocs_password_does_not_open_this_one() {
    let response = post_wachtwoord(&app(), "alfa", "wachtwoord=beta-geheim").await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert!(response.headers().get(header::SET_COOKIE).is_none());
}

#[tokio::test]
async fn the_return_path_cannot_send_a_visitor_off_this_host() {
    // `verder` comes from the form, so it is attacker-controlled. Anything
    // that is not a path inside this PoC falls back to the index.
    for kwaad in [
        "https%3A%2F%2Fkwaadaardig.nl",
        "%2F%2Fkwaadaardig.nl",
        "%2Fbeta%2F",
        "%2Fetc%2Fpasswd",
    ] {
        let response = post_wachtwoord(
            &app(),
            "alfa",
            &format!("wachtwoord=alfa-geheim&verder={kwaad}"),
        )
        .await;
        assert_eq!(
            response
                .headers()
                .get(header::LOCATION)
                .and_then(|v| v.to_str().ok()),
            Some("/"),
            "verder={kwaad} should not be followed",
        );
    }
}

#[tokio::test]
async fn every_answer_carries_the_security_headers() {
    // The layer sits over the fallback too, which is where the PoCs are
    // served — the mistake `serve_static_and_secure` in editor-api documents.
    let app = app();
    for (pad, cookie) in [
        ("/", None),
        ("/alfa/", None),
        ("/alfa/", Some(cookie_voor("alfa"))),
    ] {
        let mut req = Request::builder().uri(pad).method("GET");
        if let Some(c) = cookie {
            req = req.header(header::COOKIE, c);
        }
        let response = app
            .clone()
            .oneshot(req.body(Body::empty()).expect("request"))
            .await
            .expect("response");
        let csp = response
            .headers()
            .get(header::CONTENT_SECURITY_POLICY)
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default();
        assert!(csp.contains("worker-src 'self' blob:"), "{pad}: {csp}");
        assert!(
            response.headers().contains_key("x-content-type-options"),
            "{pad}"
        );
    }
}

#[tokio::test]
async fn a_config_without_a_password_refuses_to_start() {
    // The failure mode this guards is a PoC in the register that is served to
    // anyone because its env var was forgotten.
    std::env::set_var("POC_COOKIE_SECRET", SECRET);
    std::env::remove_var("POC_PW_GAMMA");
    let register = format!("{REGISTER}  - slug: gamma\n    titel: G\n    samenvatting: S\n    soort: statisch\n    bron: b\n");
    // `Config` holds the signing key and every password, so it deliberately
    // does not derive Debug — hence matching rather than `expect_err`.
    match Config::from_env(&register) {
        Ok(_) => panic!("a PoC without a password must not start"),
        Err(e) => assert!(e.to_string().contains("POC_PW_GAMMA"), "{e}"),
    }
}
