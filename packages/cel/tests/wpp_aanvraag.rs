//! A political party submits an application for a subsidy (Wpp art. 102) and
//! the cell of the Autoriteit records it as a gram. The cell gets the shape of the application by executing the law:
//! Wpp 102 establishes the application, Wpp 107 decides on it, and Awb 4:2
//! and 4:13 hook onto it.
//!
//! Only cells, regulation and a data directory: no channels, forms,
//! synthesis or registers. That corpus is not in this repository, so the
//! test runs only when `CEL_WPP_CORPUS` points at its root (with `cells/`
//! and `regulation/`):
//!
//! ```text
//! CEL_WPP_CORPUS=<root> cargo test -p regelrecht-cel --test wpp_aanvraag
//! ```

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::PathBuf;
use std::sync::Arc;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use chrono::DateTime;
use http_body_util::BodyExt;
use regelrecht_cel::config::{Config, DEFAULT_PORT};
use regelrecht_cel::runtime::Runtime;
use regelrecht_cel::schema::{self, Kind};
use regelrecht_cel::transport::RUNTIME_TOKEN_HEADER;
use serde_json::{json, Value};
use tower::ServiceExt;

const CELL: &str = "/cells/autoriteit_politieke_partijen";
const AUTHORITY: &str = "nederlandse_autoriteit_politieke_partijen";

fn corpus() -> Option<PathBuf> {
    let root = PathBuf::from(std::env::var_os("CEL_WPP_CORPUS")?);
    assert!(
        root.join("cells").is_dir() && root.join("regulation").is_dir(),
        "CEL_WPP_CORPUS={} has no cells/ and regulation/",
        root.display()
    );
    Some(root)
}

async fn as_runtime(
    rt: &Runtime,
    method: &str,
    uri: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let req = Request::builder()
        .method(method)
        .uri(uri)
        .header(RUNTIME_TOKEN_HEADER, rt.runtime_token.as_str());
    let req = match body {
        Some(b) => req
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(b.to_string()))
            .unwrap(),
        None => req.body(Body::empty()).unwrap(),
    };
    let resp = rt.router.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

#[tokio::test]
async fn a_party_submits_an_application_and_the_authority_records_it() {
    let Some(root) = corpus() else {
        eprintln!("CEL_WPP_CORPUS not set: skipped");
        return;
    };
    let data = tempfile::tempdir().unwrap();
    let config = Config {
        cells_path: root.join("cells"),
        regulation_path: root.join("regulation"),
        data_dir: data.path().to_path_buf(),
        port: DEFAULT_PORT,
        read_token: None,
        read_token_sources: Vec::new(),
        reduction: Default::default(),
        registers: None,
        channels: None,
        synthesis: None,
        examples: None,
    };
    let clock = Arc::new(|| DateTime::parse_from_rfc3339("2027-03-15T09:30:00+01:00").unwrap());
    let rt = Runtime::load(&config, clock).unwrap_or_else(|e| panic!("{e:#?}"));

    // The application, with what the law asks: Wpp 102 (lid 1, 3 and 4) and
    // Awb 4:2 lid 1, through its hook.
    let request = json!({
        "actor": AUTHORITY,
        "stream": "napp_aanvragen",
        "event": "aanvraag_ontvangen",
        "intake": {"channel": "aanvrager"},
        "external": {
            "subsidiejaar": 2027,
            "statutaire_naam": "Vereniging Voorbeeld",
            "geregistreerde_aanduiding": "VOORBEELD",
            "adres_aanvrager": "Voorbeeldstraat 1, 2511 AA Den Haag",
            "dagtekening": "2027-03-14",
            "ondertekening": "A. Voorzitter",
        },
    });
    let (status, body) = as_runtime(&rt, "POST", &format!("{CELL}/api/grams"), Some(request)).await;
    assert_eq!(status, StatusCode::CREATED, "{body:#}");
    let gram = &body["gram"];
    schema::validate(Kind::Gram, gram).unwrap();

    assert_eq!(gram["recording_actor"], AUTHORITY);
    // Awb 4:13 lid 1: the receipt is the moment that counts.
    assert_eq!(gram["effective_at"], "2027-03-15T09:30:00+01:00");
    // The fields are what the executed law asks: Wpp 102 lid 1, 3 and 4 and
    // Awb 4:2 lid 1. The name of the applicant (4:2 lid 1 onder a) is the
    // statutory name (102 lid 3 onder a), and the decision asked for (4:2
    // lid 1 onder c) is the one Wpp 107 takes on it: neither is a field of
    // its own for the party.
    assert_eq!(
        gram["fields"],
        json!({
            "subsidiejaar": 2027,
            "statutaire_naam": "Vereniging Voorbeeld",
            "geregistreerde_aanduiding": "VOORBEELD",
            "adres_aanvrager": "Voorbeeldstraat 1, 2511 AA Den Haag",
            "dagtekening": "2027-03-14",
            "gevraagde_beschikking": "wet_op_de_politieke_partijen#107",
            "ondertekening": "A. Voorzitter",
            "organen": null,
            "registratie": null,
        })
    );
    assert_eq!(gram["subtype"], "aanvraag");
    assert_eq!(gram["stage"], "AANVRAAG");

    // One line in the chronicle of the Autoriteit.
    let chronicle = data.path().join("autoriteit_politieke_partijen/napp.jsonl");
    assert_eq!(
        std::fs::read_to_string(chronicle).unwrap().lines().count(),
        1
    );

    // A field the law does not ask is refused.
    let unknown = json!({
        "actor": AUTHORITY, "stream": "napp_aanvragen", "event": "aanvraag_ontvangen",
        "intake": {"channel": "aanvrager"},
        "external": {"subsidiejaar": 2027, "schoenmaat": 44},
    });
    let (status, body) = as_runtime(&rt, "POST", &format!("{CELL}/api/grams"), Some(unknown)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body:#}");
}
