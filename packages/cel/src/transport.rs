//! Transport naar een cel: hoe een proces een lexostatus opvraagt, een
//! proefreductie vraagt of een cel laat vastleggen.
//!
//! Een proces leest nooit rechtstreeks in een kroniek, ook niet in die van de
//! cel waarin het vastlegt. Het vraagt, langs dezelfde route die elke afnemer
//! gebruikt (`/cellen/<id>/api/lexostatus/<naam>`). De runtime kiest het
//! transport (RFC-022 par. 4.3): draait de cel in dezelfde runtime, dan gaat
//! de vraag intern door de router, zonder netwerk; staat er een url, dan over
//! HTTP. Beide leveren hetzelfde antwoord.
//!
//! Vastleggen en op proef reduceren mag alleen een proces van de runtime zelf
//! (zie [`RuntimeToken`]). Het interne transport stuurt daarom het token van
//! de runtime mee; een HTTP-transport alleen als het er een meekreeg, en de
//! runtime geeft het nooit aan een transport naar een andere runtime. Lezen
//! (de kroniek, een zaak, een lexostatus) mag ook een andere runtime met het
//! gedeelde leestoken ([`LeesToken`], `CEL_LEES_TOKEN`); een HTTP-transport
//! stuurt het mee als de runtime er een heeft.

use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use serde_json::Value;
use tokio::sync::OnceCell;
use tower::ServiceExt;

/// De header waarin een proces het runtime-token meestuurt.
pub const RUNTIME_TOKEN_HEADER: &str = "x-cel-runtime-token";

/// De header waarin een andere runtime het leestoken meestuurt.
pub const LEES_TOKEN_HEADER: &str = "x-cel-lees-token";

/// Een geheim dat de runtime bij elke start nieuw maakt, en dat alleen haar
/// eigen processen kennen: het interne transport stuurt het mee, en een cel
/// legt alleen vast (of reduceert op proef) op een verzoek dat het draagt.
/// Lezen vraagt het token ook, of het leestoken ([`LeesToken`]). Dit is geen
/// beveiligingscontext tussen organisaties (RFC-022 par. 2); het voorkomt
/// alleen dat iedereen die de poort bereikt een gram in een kroniek kan
/// zetten of de identiteit en de intake in een kroniek kan lezen.
#[derive(Clone)]
pub struct RuntimeToken(Arc<str>);

impl RuntimeToken {
    /// Een nieuw token: 244 willekeurige bits uit twee UUID's (v4).
    pub fn nieuw() -> Self {
        Self(
            format!(
                "{}{}",
                uuid::Uuid::new_v4().simple(),
                uuid::Uuid::new_v4().simple()
            )
            .into(),
        )
    }

    /// Een token met een gegeven waarde, zoals het leestoken uit de
    /// omgeving.
    pub fn uit(tekst: &str) -> Self {
        Self(tekst.into())
    }

    pub fn als_str(&self) -> &str {
        &self.0
    }

    /// Of `aangeboden` dit token is. De vergelijking kijkt naar elke byte,
    /// ook na het eerste verschil.
    pub fn klopt(&self, aangeboden: &[u8]) -> bool {
        let eigen = self.0.as_bytes();
        eigen.len() == aangeboden.len()
            && eigen
                .iter()
                .zip(aangeboden)
                .fold(0u8, |v, (a, b)| v | (a ^ b))
                == 0
    }
}

/// Het leestoken: een geheim dat runtimes die elkaar vertrouwen delen
/// (`CEL_LEES_TOKEN`), zodat een proces in de ene runtime een lexostatus
/// van een cel in de andere kan lezen. Het geeft alleen lezen, nooit
/// vastleggen. Zonder leestoken leest alleen de eigen runtime.
pub type LeesToken = RuntimeToken;

impl std::fmt::Debug for RuntimeToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("RuntimeToken(..)")
    }
}

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

/// Onthoudt de antwoorden op `GET` van een of meer transporten, per
/// transport en pad, voor de duur van een vraag. Het zaakscherm rekent elke
/// handeling van een zaak op proef uit; die lezen dezelfde lexostatussen van
/// de zaak en dezelfde bronnen op dezelfde peildatum (het peil staat in het
/// pad), en zo vraagt het proces elk daarvan een keer, ook als de proeven
/// tegelijk lopen. Een `POST` (een proefreductie met een concept, het
/// vastleggen) gaat altijd door. Ook een fout (een onbereikbare bron) blijft
/// voor de duur van de vraag onthouden: dan zeggen alle proeven hetzelfde.
/// Het geheugen hoort bij een vraag en wordt daarna weggegooid.
#[derive(Clone, Default)]
pub struct Onthouden(Arc<std::sync::Mutex<Geheugen>>);

type Geheugen =
    std::collections::HashMap<(usize, String), Arc<OnceCell<Result<Value, TransportFout>>>>;

impl Onthouden {
    /// `binnen`, met de antwoorden op `GET` onthouden.
    pub fn om(&self, binnen: Arc<dyn Transport>) -> Arc<dyn Transport> {
        Arc::new(Onthoudend {
            binnen,
            geheugen: self.clone(),
        })
    }

    fn plek(
        &self,
        binnen: &Arc<dyn Transport>,
        pad: &str,
    ) -> Arc<OnceCell<Result<Value, TransportFout>>> {
        // Hetzelfde transport is hetzelfde doel; de sleutel is zijn adres.
        // Dat is uniek zolang het transport leeft, en elk `Onthoudend` houdt
        // het zijne vast zolang het geheugen gebruikt wordt.
        let wie = Arc::as_ptr(binnen).cast::<()>() as usize;
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .entry((wie, pad.to_string()))
            .or_default()
            .clone()
    }
}

struct Onthoudend {
    binnen: Arc<dyn Transport>,
    geheugen: Onthouden,
}

impl Transport for Onthoudend {
    fn soort(&self) -> &'static str {
        self.binnen.soort()
    }

    fn haal<'a>(&'a self, pad: &'a str) -> Antwoord<'a> {
        Box::pin(async move {
            let plek = self.geheugen.plek(&self.binnen, pad);
            plek.get_or_init(|| self.binnen.haal(pad)).await.clone()
        })
    }

    fn stuur<'a>(&'a self, pad: &'a str, body: &'a Value) -> Antwoord<'a> {
        self.binnen.stuur(pad, body)
    }
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
/// runtime, zonder netwerk, met het token van de runtime. De router bestaat
/// pas als alle cellen geladen zijn; tot dan is de bron onbereikbaar.
#[derive(Clone)]
pub struct Intern {
    router: Arc<OnceLock<Router>>,
    token: RuntimeToken,
}

impl Intern {
    pub fn new(router: Arc<OnceLock<Router>>, token: RuntimeToken) -> Self {
        Self { router, token }
    }
}

impl Transport for Intern {
    fn soort(&self) -> &'static str {
        "intern"
    }

    fn haal<'a>(&'a self, pad: &'a str) -> Antwoord<'a> {
        Box::pin(async move {
            let req = Request::get(pad)
                .header(RUNTIME_TOKEN_HEADER, self.token.als_str())
                .body(Body::empty())
                .map_err(|e| TransportFout::Onbereikbaar(e.to_string()))?;
            self.vraag(req).await
        })
    }

    fn stuur<'a>(&'a self, pad: &'a str, body: &'a Value) -> Antwoord<'a> {
        Box::pin(async move {
            let req = Request::post(pad)
                .header(header::CONTENT_TYPE, "application/json")
                .header(RUNTIME_TOKEN_HEADER, self.token.als_str())
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
/// Alleen naar de eigen runtime gaat een runtime-token mee
/// ([`Http::met_runtime_token`]).
#[derive(Clone)]
pub struct Http {
    basis: String,
    client: reqwest::Client,
    token: Option<RuntimeToken>,
    lees_token: Option<LeesToken>,
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
            token: None,
            lees_token: None,
        })
    }

    /// Stuur het runtime-token mee: alleen voor een transport naar de eigen
    /// runtime, nooit naar die van een ander.
    pub fn met_runtime_token(mut self, token: RuntimeToken) -> Self {
        self.token = Some(token);
        self
    }

    /// Stuur het leestoken mee: lezen bij een runtime die hetzelfde
    /// leestoken heeft.
    pub fn met_lees_token(mut self, token: Option<LeesToken>) -> Self {
        self.lees_token = token;
        self
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
        mut verzoek: reqwest::RequestBuilder,
    ) -> Result<Value, TransportFout> {
        if let Some(t) = &self.token {
            verzoek = verzoek.header(RUNTIME_TOKEN_HEADER, t.als_str());
        }
        if let Some(t) = &self.lees_token {
            verzoek = verzoek.header(LEES_TOKEN_HEADER, t.als_str());
        }
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

    /// Het geheugen vraagt elk pad een keer per transport, ook bij
    /// gelijktijdige vragen; een ander transport of een POST gaat door.
    #[tokio::test]
    async fn onthouden_vraagt_elk_pad_een_keer() {
        let a = Arc::new(proef::Vast::new(Ok(json!({"a": 1}))));
        let b = Arc::new(proef::Vast::new(Ok(json!({"b": 2}))));
        let geheugen = Onthouden::default();
        let (ta, tb) = (geheugen.om(a.clone()), geheugen.om(b.clone()));
        let (x, y) = tokio::join!(ta.haal("/l?p=1"), ta.haal("/l?p=1"));
        assert_eq!((x.unwrap(), y.unwrap()), (json!({"a": 1}), json!({"a": 1})));
        assert_eq!(tb.haal("/l?p=1").await.unwrap(), json!({"b": 2}));
        ta.haal("/l?p=2").await.unwrap();
        assert_eq!(a.vragen(), ["/l?p=1", "/l?p=2"]);
        assert_eq!(b.vragen(), ["/l?p=1"]);
        ta.stuur("/l?p=1", &json!({})).await.unwrap();
        ta.stuur("/l?p=1", &json!({})).await.unwrap();
        assert_eq!(a.vragen().len(), 4);
    }

    #[tokio::test]
    async fn intern_door_de_router() {
        let lock = Arc::new(OnceLock::new());
        let t = Intern::new(lock.clone(), RuntimeToken::nieuw());
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

    /// Een route die de header met het runtime-token teruggeeft.
    fn echo() -> Router {
        Router::new().route(
            "/token",
            get(|h: axum::http::HeaderMap| async move {
                Json(json!(h
                    .get(RUNTIME_TOKEN_HEADER)
                    .and_then(|v| v.to_str().ok())))
            }),
        )
    }

    #[tokio::test]
    async fn het_runtime_token_gaat_alleen_mee_als_het_er_is() {
        let token = RuntimeToken::nieuw();
        let lock = Arc::new(OnceLock::new());
        lock.set(echo()).ok();
        let intern = Intern::new(lock, token.clone());
        assert_eq!(intern.haal("/token").await.unwrap(), json!(token.als_str()));

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let adres = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, echo()).await });
        let url = format!("http://{adres}");
        let vreemd = Http::new(&url, Duration::from_secs(5)).unwrap();
        assert_eq!(vreemd.haal("/token").await.unwrap(), Value::Null);
        let eigen = vreemd.met_runtime_token(token.clone());
        assert_eq!(eigen.haal("/token").await.unwrap(), json!(token.als_str()));
    }

    #[test]
    fn het_token_klopt_alleen_precies() {
        let t = RuntimeToken::nieuw();
        assert_eq!(t.als_str().len(), 64);
        assert!(t.klopt(t.als_str().as_bytes()));
        assert!(!t.klopt(&t.als_str().as_bytes()[..63]));
        assert!(!t.klopt(RuntimeToken::nieuw().als_str().as_bytes()));
        assert!(!format!("{t:?}").contains(t.als_str()));
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
