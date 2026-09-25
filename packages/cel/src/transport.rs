//! Transport naar een cel: hoe een proces een lexostatus opvraagt, een
//! proefreductie vraagt of een cel laat vastleggen.
//!
//! Een proces leest nooit rechtstreeks in een kroniek, ook niet in die van de
//! cel waarin het vastlegt. Het vraagt, langs dezelfde route die elke afnemer
//! gebruikt (`/cellen/<id>/api/lexostatus/<naam>`). De runtime kiest het
//! transport (RFC-022 par. 4.3): draait de cel in dezelfde runtime, dan gaat
//! de vraag intern door de router, zonder netwerk; staat er een url, dan over
//! HTTP. Beide leveren hetzelfde antwoord.

use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt;

/// Waarom een vraag geen lexostatus opleverde.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransportFout {
    /// Geen antwoord: geen verbinding, of niet binnen de tijdslimiet.
    Onbereikbaar(String),
    /// Wel een antwoord, maar geen lexostatus (een HTTP-foutstatus).
    Antwoord { status: u16, fout: String },
    /// Een antwoord met een goede status, maar geen JSON of niet de vorm die
    /// de vrager verwacht; of een verzoek dat niet als JSON te schrijven is.
    Json(String),
}

impl std::fmt::Display for TransportFout {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TransportFout::Onbereikbaar(r) => write!(f, "onbereikbaar: {r}"),
            TransportFout::Antwoord { status, fout } => write!(f, "status {status}: {fout}"),
            TransportFout::Json(r) => write!(f, "onleesbaar: {r}"),
        }
    }
}

/// Het antwoord op een vraag, als toekomst.
pub type Antwoord<'a> = Pin<Box<dyn Future<Output = Result<Value, TransportFout>> + Send + 'a>>;

/// Een manier om een route van een runtime op te vragen.
pub trait Transport: Send + Sync {
    /// `intern` of `http`, voor de herkomst in een antwoord.
    fn soort(&self) -> &'static str;

    /// `GET` op een pad van de runtime, zoals `/api/cellen` of
    /// `/cellen/<id>/api/lexostatus/<naam>?<invoer>`, met JSON terug.
    fn haal<'a>(&'a self, pad: &'a str) -> Antwoord<'a>;

    /// `POST` met een JSON-body op een pad van de runtime, zoals
    /// `/cellen/<id>/api/grammen`, met JSON terug.
    fn stuur<'a>(&'a self, pad: &'a str, body: &'a Value) -> Antwoord<'a>;
}

/// Vraag binnen een tijdslimiet. Te laat is onbereikbaar.
pub async fn haal_binnen(
    transport: &dyn Transport,
    pad: &str,
    limiet: Duration,
) -> Result<Value, TransportFout> {
    match tokio::time::timeout(limiet, transport.haal(pad)).await {
        Ok(antwoord) => antwoord,
        Err(_) => Err(TransportFout::Onbereikbaar(format!(
            "geen antwoord binnen {} s",
            limiet.as_secs_f32()
        ))),
    }
}

/// Een fout-antwoord `{"fout": "..."}` in woorden. Een antwoord zonder die
/// vorm gaat mee zoals het is (ingekort), met de reden van de status ervoor.
fn fouttekst(status: StatusCode, body: &[u8]) -> TransportFout {
    let reden = status.canonical_reason().unwrap_or("fout");
    let als_fout = serde_json::from_slice::<Value>(body)
        .ok()
        .and_then(|v| v.get("fout")?.as_str().map(str::to_string));
    let fout = match als_fout {
        Some(f) => f,
        None => {
            let tekst: String = String::from_utf8_lossy(body)
                .trim()
                .chars()
                .take(200)
                .collect();
            if tekst.is_empty() {
                reden.to_string()
            } else {
                format!("{reden}: {tekst}")
            }
        }
    };
    TransportFout::Antwoord {
        status: status.as_u16(),
        fout,
    }
}

/// Intern: dezelfde aanroep als de HTTP-route, door de router van de eigen
/// runtime, zonder netwerk. De router bestaat pas als alle cellen geladen
/// zijn; tot dan is de bron onbereikbaar.
#[derive(Clone, Default)]
pub struct Intern {
    router: Arc<OnceLock<Router>>,
}

impl Intern {
    pub fn new(router: Arc<OnceLock<Router>>) -> Self {
        Self { router }
    }
}

impl Transport for Intern {
    fn soort(&self) -> &'static str {
        "intern"
    }

    fn haal<'a>(&'a self, pad: &'a str) -> Antwoord<'a> {
        Box::pin(async move {
            let req = Request::get(pad)
                .body(Body::empty())
                .map_err(|e| TransportFout::Onbereikbaar(e.to_string()))?;
            self.vraag(req).await
        })
    }

    fn stuur<'a>(&'a self, pad: &'a str, body: &'a Value) -> Antwoord<'a> {
        Box::pin(async move {
            let req = Request::post(pad)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body.to_string()))
                .map_err(|e| TransportFout::Onbereikbaar(e.to_string()))?;
            self.vraag(req).await
        })
    }
}

impl Intern {
    async fn vraag(&self, req: Request<Body>) -> Result<Value, TransportFout> {
        let router = self
            .router
            .get()
            .cloned()
            .ok_or_else(|| TransportFout::Onbereikbaar("de runtime draait nog niet".into()))?;
        let resp = router
            .oneshot(req)
            .await
            .map_err(|e| TransportFout::Onbereikbaar(e.to_string()))?;
        let status = resp.status();
        let body = resp
            .into_body()
            .collect()
            .await
            .map_err(|e| TransportFout::Onbereikbaar(e.to_string()))?
            .to_bytes();
        if !status.is_success() {
            return Err(fouttekst(status, &body));
        }
        serde_json::from_slice(&body).map_err(|e| TransportFout::Json(format!("geen JSON: {e}")))
    }
}

/// Over HTTP naar een runtime op `basis` (bijvoorbeeld `http://host:7172`).
/// Er is geen veiligheidscontext: geen ondertekening en geen autorisatie.
#[derive(Clone)]
pub struct Http {
    basis: String,
    client: reqwest::Client,
}

impl Http {
    pub fn new(basis: &str, limiet: Duration) -> Result<Self, String> {
        let client = reqwest::Client::builder()
            .timeout(limiet)
            .build()
            .map_err(|e| e.to_string())?;
        Ok(Self {
            basis: basis.trim_end_matches('/').to_string(),
            client,
        })
    }
}

impl Transport for Http {
    fn soort(&self) -> &'static str {
        "http"
    }

    fn haal<'a>(&'a self, pad: &'a str) -> Antwoord<'a> {
        Box::pin(async move {
            let url = format!("{}{pad}", self.basis);
            self.antwoord(&url, self.client.get(&url)).await
        })
    }

    fn stuur<'a>(&'a self, pad: &'a str, body: &'a Value) -> Antwoord<'a> {
        Box::pin(async move {
            let url = format!("{}{pad}", self.basis);
            self.antwoord(
                &url,
                self.client
                    .post(&url)
                    .header(reqwest::header::CONTENT_TYPE, "application/json")
                    .body(body.to_string()),
            )
            .await
        })
    }
}

impl Http {
    async fn antwoord(
        &self,
        url: &str,
        verzoek: reqwest::RequestBuilder,
    ) -> Result<Value, TransportFout> {
        let resp = verzoek
            .send()
            .await
            .map_err(|e| TransportFout::Onbereikbaar(format!("{url}: {e}")))?;
        let status = StatusCode::from_u16(resp.status().as_u16())
            .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        let body = resp
            .bytes()
            .await
            .map_err(|e| TransportFout::Onbereikbaar(format!("{url}: {e}")))?;
        if !status.is_success() {
            return Err(fouttekst(status, &body));
        }
        serde_json::from_slice(&body).map_err(|e| TransportFout::Json(format!("geen JSON: {e}")))
    }
}

/// Een transport voor tests: elke vraag krijgt hetzelfde antwoord, en de
/// vragen worden onthouden.
#[cfg(test)]
#[allow(clippy::unwrap_used)]
pub(crate) mod proef {
    use super::*;
    use std::sync::Mutex;

    pub(crate) struct Vast {
        antwoord: Result<Value, TransportFout>,
        vragen: Mutex<Vec<String>>,
    }

    impl Vast {
        pub(crate) fn new(antwoord: Result<Value, TransportFout>) -> Self {
            Self {
                antwoord,
                vragen: Mutex::new(Vec::new()),
            }
        }

        /// De gevraagde paden, in volgorde.
        pub(crate) fn vragen(&self) -> Vec<String> {
            self.vragen.lock().unwrap().clone()
        }
    }

    impl Transport for Vast {
        fn soort(&self) -> &'static str {
            "intern"
        }
        fn haal<'a>(&'a self, pad: &'a str) -> Antwoord<'a> {
            self.vragen.lock().unwrap().push(pad.to_string());
            let a = self.antwoord.clone();
            Box::pin(async move { a })
        }
        fn stuur<'a>(&'a self, pad: &'a str, _body: &'a Value) -> Antwoord<'a> {
            self.haal(pad)
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use axum::routing::get;
    use axum::Json;
    use serde_json::json;

    fn router() -> Router {
        Router::new()
            .route("/goed", get(|| async { Json(json!({"a": 1})) }))
            .route(
                "/fout",
                get(|| async { (StatusCode::NOT_FOUND, Json(json!({"fout": "weg"}))) }),
            )
            .route("/geen-json", get(|| async { "geen json" }))
            .route(
                "/traag",
                get(|| async {
                    tokio::time::sleep(Duration::from_secs(5)).await;
                    Json(json!({}))
                }),
            )
    }

    #[tokio::test]
    async fn intern_door_de_router() {
        let lock = Arc::new(OnceLock::new());
        let t = Intern::new(lock.clone());
        assert!(matches!(
            t.haal("/goed").await,
            Err(TransportFout::Onbereikbaar(_))
        ));
        lock.set(router()).ok();
        assert_eq!(t.haal("/goed").await.unwrap(), json!({"a": 1}));
        assert_eq!(
            t.haal("/fout").await.unwrap_err(),
            TransportFout::Antwoord {
                status: 404,
                fout: "weg".into()
            }
        );
    }

    #[tokio::test]
    async fn http_en_de_tijdslimiet() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let adres = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, router()).await });
        let t = Http::new(&format!("http://{adres}/"), Duration::from_secs(10)).unwrap();
        assert_eq!(t.haal("/goed").await.unwrap(), json!({"a": 1}));
        assert!(matches!(
            t.haal("/fout").await,
            Err(TransportFout::Antwoord { status: 404, .. })
        ));
        // Een goede status met een onleesbaar antwoord is geen lege waarde.
        assert!(matches!(
            t.haal("/geen-json").await,
            Err(TransportFout::Json(r)) if r.contains("geen JSON")
        ));
        let fout = haal_binnen(&t, "/traag", Duration::from_millis(200))
            .await
            .unwrap_err();
        assert!(matches!(fout, TransportFout::Onbereikbaar(r) if r.contains("binnen")));
    }

    #[tokio::test]
    async fn http_zonder_server_is_onbereikbaar() {
        // Een poort die net vrij was, is (vrijwel zeker) nog steeds dicht.
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let adres = listener.local_addr().unwrap();
        drop(listener);
        let t = Http::new(&format!("http://{adres}"), Duration::from_secs(2)).unwrap();
        assert!(matches!(
            t.haal("/goed").await,
            Err(TransportFout::Onbereikbaar(_))
        ));
    }
}
