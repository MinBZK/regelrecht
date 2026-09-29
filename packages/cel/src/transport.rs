//! Transport to a cell: how a process requests a lexostatus, asks for a
//! trial reduction or has a cell record.
//!
//! A process never reads a chronicle directly, not even that of the cell in
//! which it records. It asks, along the same route every consumer uses
//! (`/cells/<id>/api/lexostatus/<name>`). The runtime chooses the transport
//! (RFC-022 par. 4.3): if the cell runs in the same runtime, the request goes
//! internally through the router, without network; if there is a url, over
//! HTTP. Both give the same response.
//!
//! Only a process of the runtime itself may record and reduce on trial
//! (see [`RuntimeToken`]). The internal transport therefore sends the token of
//! the runtime along; an HTTP transport only if it was given one, and the
//! runtime never gives it to a transport to another runtime. Another runtime
//! with the shared read token ([`ReadToken`], `CELL_READ_TOKEN`) may also read
//! (the chronicle, a case, a lexostatus); an HTTP transport sends it along if
//! the runtime has one.

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

/// The header in which a process sends the runtime token.
pub const RUNTIME_TOKEN_HEADER: &str = "x-cell-runtime-token";

/// The header in which another runtime sends the read token.
pub const READ_TOKEN_HEADER: &str = "x-cell-read-token";

/// A secret the runtime creates anew at every start, known only to its own
/// processes: the internal transport sends it along, and a cell only records
/// (or reduces on trial) on a request that carries it. Reading also asks for
/// the token, or the read token ([`ReadToken`]). This is no security context
/// between organizations (RFC-022 par. 2); it only prevents anyone who
/// reaches the port from putting a gram in a chronicle or reading the
/// identity and the intake in a chronicle.
#[derive(Clone)]
pub struct RuntimeToken(Arc<str>);

impl RuntimeToken {
    /// A new token: 244 random bits from two UUIDs (v4).
    pub fn generate() -> Self {
        Self(
            format!(
                "{}{}",
                uuid::Uuid::new_v4().simple(),
                uuid::Uuid::new_v4().simple()
            )
            .into(),
        )
    }

    /// A token with a given value, such as the read token from the
    /// environment.
    pub fn out(text: &str) -> Self {
        Self(text.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Whether `offered` is this token. The comparison looks at every byte,
    /// also after the first difference.
    pub fn holds(&self, offered: &[u8]) -> bool {
        let own = self.0.as_bytes();
        own.len() == offered.len()
            && own.iter().zip(offered).fold(0u8, |v, (a, b)| v | (a ^ b)) == 0
    }
}

/// The read token: a secret shared by runtimes that trust each other
/// (`CELL_READ_TOKEN`), so that a process in one runtime can read a lexostatus
/// of a cell in the other. It grants only reading, never recording. Without
/// a read token only the own runtime reads.
pub type ReadToken = RuntimeToken;

impl std::fmt::Debug for RuntimeToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("RuntimeToken(..)")
    }
}

/// Why a request did not yield a lexostatus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransportError {
    /// No response: no connection, or not within the time limit.
    Unreachable(String),
    /// A response, but no lexostatus (an HTTP error status).
    Response { status: u16, error: String },
    /// A response with a good status, but not JSON or not the shape the
    /// requester expects; or a request that cannot be written as JSON.
    Json(String),
}

impl std::fmt::Display for TransportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TransportError::Unreachable(r) => write!(f, "unreachable: {r}"),
            TransportError::Response { status, error } => write!(f, "status {status}: {error}"),
            TransportError::Json(r) => write!(f, "unreadable: {r}"),
        }
    }
}

/// The response to a request, as a future.
pub type Response<'a> = Pin<Box<dyn Future<Output = Result<Value, TransportError>> + Send + 'a>>;

/// A way to request a route of a runtime.
pub trait Transport: Send + Sync {
    /// `internal` or `http`, for the provenance in a response.
    fn kind(&self) -> &'static str;

    /// `GET` on a path of the runtime, such as `/api/cells` or
    /// `/cells/<id>/api/lexostatus/<name>?<input>`, with JSON back.
    fn fetch<'a>(&'a self, path: &'a str) -> Response<'a>;

    /// `POST` with a JSON body on a path of the runtime, such as
    /// `/cells/<id>/api/grams`, with JSON back.
    fn send<'a>(&'a self, path: &'a str, body: &'a Value) -> Response<'a>;
}

/// Remembers the responses to `GET` of one or more transports, per
/// transport and path, for the duration of a request. The case screen
/// computes every action of a case on trial; those read the same lexostatuses
/// of the case and the same sources on the same reference date (the as-of is
/// in the path), and so the process asks for each of them once, even when the
/// trials run concurrently. A `POST` (a trial reduction with a draft, the
/// recording) always goes through. An error (an unreachable source) is also
/// remembered for the duration of the request: then all trials say the same.
/// The memory belongs to one request and is thrown away afterwards.
#[derive(Clone, Default)]
pub struct Remember(Arc<std::sync::Mutex<Memory>>);

type Memory =
    std::collections::HashMap<(usize, String), Arc<OnceCell<Result<Value, TransportError>>>>;

impl Remember {
    /// `within`, with the responses to `GET` remembered.
    pub fn wrap(&self, within: Arc<dyn Transport>) -> Arc<dyn Transport> {
        Arc::new(Remembering {
            within,
            memory: self.clone(),
        })
    }

    fn place(
        &self,
        within: &Arc<dyn Transport>,
        path: &str,
    ) -> Arc<OnceCell<Result<Value, TransportError>>> {
        // The same transport is the same target; the key is its address.
        // That is unique as long as the transport lives, and every
        // `Remembering` holds on to its own as long as the memory is used.
        let who = Arc::as_ptr(within).cast::<()>() as usize;
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .entry((who, path.to_string()))
            .or_default()
            .clone()
    }
}

struct Remembering {
    within: Arc<dyn Transport>,
    memory: Remember,
}

impl Transport for Remembering {
    fn kind(&self) -> &'static str {
        self.within.kind()
    }

    fn fetch<'a>(&'a self, path: &'a str) -> Response<'a> {
        Box::pin(async move {
            let place = self.memory.place(&self.within, path);
            place.get_or_init(|| self.within.fetch(path)).await.clone()
        })
    }

    fn send<'a>(&'a self, path: &'a str, body: &'a Value) -> Response<'a> {
        self.within.send(path, body)
    }
}

/// Request within a time limit. Too late is unreachable.
pub async fn fetch(
    transport: &dyn Transport,
    path: &str,
    limit: Duration,
) -> Result<Value, TransportError> {
    match tokio::time::timeout(limit, transport.fetch(path)).await {
        Ok(response) => response,
        Err(_) => Err(TransportError::Unreachable(format!(
            "no response within {} s",
            limit.as_secs_f32()
        ))),
    }
}

/// An error response `{"error": "..."}` in words. A response without that
/// shape is passed on as it is (truncated), preceded by the reason of the status.
fn error_text(status: StatusCode, body: &[u8]) -> TransportError {
    let reason = status.canonical_reason().unwrap_or("error");
    let as_error = serde_json::from_slice::<Value>(body)
        .ok()
        .and_then(|v| v.get("error")?.as_str().map(str::to_string));
    let error = match as_error {
        Some(f) => f,
        None => {
            let text: String = String::from_utf8_lossy(body)
                .trim()
                .chars()
                .take(200)
                .collect();
            if text.is_empty() {
                reason.to_string()
            } else {
                format!("{reason}: {text}")
            }
        }
    };
    TransportError::Response {
        status: status.as_u16(),
        error,
    }
}

/// Internal: the same call as the HTTP route, through the router of the own
/// runtime, without network, with the token of the runtime. The router only
/// exists once all cells are loaded; until then the source is unreachable.
#[derive(Clone)]
pub struct Internal {
    router: Arc<OnceLock<Router>>,
    token: RuntimeToken,
}

impl Internal {
    pub fn new(router: Arc<OnceLock<Router>>, token: RuntimeToken) -> Self {
        Self { router, token }
    }
}

impl Transport for Internal {
    fn kind(&self) -> &'static str {
        "internal"
    }

    fn fetch<'a>(&'a self, path: &'a str) -> Response<'a> {
        Box::pin(async move {
            let req = Request::get(path)
                .header(RUNTIME_TOKEN_HEADER, self.token.as_str())
                .body(Body::empty())
                .map_err(|e| TransportError::Unreachable(e.to_string()))?;
            self.request(req).await
        })
    }

    fn send<'a>(&'a self, path: &'a str, body: &'a Value) -> Response<'a> {
        Box::pin(async move {
            let req = Request::post(path)
                .header(header::CONTENT_TYPE, "application/json")
                .header(RUNTIME_TOKEN_HEADER, self.token.as_str())
                .body(Body::from(body.to_string()))
                .map_err(|e| TransportError::Unreachable(e.to_string()))?;
            self.request(req).await
        })
    }
}

impl Internal {
    async fn request(&self, req: Request<Body>) -> Result<Value, TransportError> {
        let router =
            self.router.get().cloned().ok_or_else(|| {
                TransportError::Unreachable("the runtime is not running yet".into())
            })?;
        let resp = router
            .oneshot(req)
            .await
            .map_err(|e| TransportError::Unreachable(e.to_string()))?;
        let status = resp.status();
        let body = resp
            .into_body()
            .collect()
            .await
            .map_err(|e| TransportError::Unreachable(e.to_string()))?
            .to_bytes();
        if !status.is_success() {
            return Err(error_text(status, &body));
        }
        serde_json::from_slice(&body).map_err(|e| TransportError::Json(format!("not JSON: {e}")))
    }
}

/// Over HTTP to a runtime at `basis` (for example `http://host:7172`).
/// There is no security context: no signing and no authorization.
/// Only to the own runtime is a runtime token sent along
/// ([`Http::with_runtime_token`]).
#[derive(Clone)]
pub struct Http {
    basis: String,
    client: reqwest::Client,
    token: Option<RuntimeToken>,
    read_token: Option<ReadToken>,
}

impl Http {
    pub fn new(basis: &str, limit: Duration) -> Result<Self, String> {
        let client = reqwest::Client::builder()
            .timeout(limit)
            .build()
            .map_err(|e| e.to_string())?;
        Ok(Self {
            basis: basis.trim_end_matches('/').to_string(),
            client,
            token: None,
            read_token: None,
        })
    }

    /// Send the runtime token along: only for a transport to the own
    /// runtime, never to someone else's.
    pub fn with_runtime_token(mut self, token: RuntimeToken) -> Self {
        self.token = Some(token);
        self
    }

    /// Send the read token along: reading at a runtime that has the same
    /// read token.
    pub fn with_read_token(mut self, token: Option<ReadToken>) -> Self {
        self.read_token = token;
        self
    }
}

impl Transport for Http {
    fn kind(&self) -> &'static str {
        "http"
    }

    fn fetch<'a>(&'a self, path: &'a str) -> Response<'a> {
        Box::pin(async move {
            let url = format!("{}{path}", self.basis);
            self.response(&url, self.client.get(&url)).await
        })
    }

    fn send<'a>(&'a self, path: &'a str, body: &'a Value) -> Response<'a> {
        Box::pin(async move {
            let url = format!("{}{path}", self.basis);
            self.response(
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
    async fn response(
        &self,
        url: &str,
        mut request: reqwest::RequestBuilder,
    ) -> Result<Value, TransportError> {
        if let Some(t) = &self.token {
            request = request.header(RUNTIME_TOKEN_HEADER, t.as_str());
        }
        if let Some(t) = &self.read_token {
            request = request.header(READ_TOKEN_HEADER, t.as_str());
        }
        let resp = request
            .send()
            .await
            .map_err(|e| TransportError::Unreachable(format!("{url}: {e}")))?;
        let status = StatusCode::from_u16(resp.status().as_u16())
            .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        let body = resp
            .bytes()
            .await
            .map_err(|e| TransportError::Unreachable(format!("{url}: {e}")))?;
        if !status.is_success() {
            return Err(error_text(status, &body));
        }
        serde_json::from_slice(&body).map_err(|e| TransportError::Json(format!("not JSON: {e}")))
    }
}

/// A transport for tests: every request gets the same response, and the
/// requests are remembered.
#[cfg(test)]
#[allow(clippy::unwrap_used)]
pub(crate) mod trial {
    use super::*;
    use std::sync::Mutex;

    pub(crate) struct Fixed {
        response: Result<Value, TransportError>,
        ask: Mutex<Vec<String>>,
    }

    impl Fixed {
        pub(crate) fn new(response: Result<Value, TransportError>) -> Self {
            Self {
                response,
                ask: Mutex::new(Vec::new()),
            }
        }

        /// The requested paths, in order.
        pub(crate) fn ask(&self) -> Vec<String> {
            self.ask.lock().unwrap().clone()
        }
    }

    impl Transport for Fixed {
        fn kind(&self) -> &'static str {
            "internal"
        }
        fn fetch<'a>(&'a self, path: &'a str) -> Response<'a> {
            self.ask.lock().unwrap().push(path.to_string());
            let a = self.response.clone();
            Box::pin(async move { a })
        }
        fn send<'a>(&'a self, path: &'a str, _body: &'a Value) -> Response<'a> {
            self.fetch(path)
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
                get(|| async { (StatusCode::NOT_FOUND, Json(json!({"error": "weg"}))) }),
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

    /// The memory requests each path once per transport, also with
    /// concurrent requests; another transport or a POST goes through.
    #[tokio::test]
    async fn remembering_requests_each_path_once() {
        let a = Arc::new(trial::Fixed::new(Ok(json!({"a": 1}))));
        let b = Arc::new(trial::Fixed::new(Ok(json!({"b": 2}))));
        let memory = Remember::default();
        let (ta, tb) = (memory.wrap(a.clone()), memory.wrap(b.clone()));
        let (x, y) = tokio::join!(ta.fetch("/l?p=1"), ta.fetch("/l?p=1"));
        assert_eq!((x.unwrap(), y.unwrap()), (json!({"a": 1}), json!({"a": 1})));
        assert_eq!(tb.fetch("/l?p=1").await.unwrap(), json!({"b": 2}));
        ta.fetch("/l?p=2").await.unwrap();
        assert_eq!(a.ask(), ["/l?p=1", "/l?p=2"]);
        assert_eq!(b.ask(), ["/l?p=1"]);
        ta.send("/l?p=1", &json!({})).await.unwrap();
        ta.send("/l?p=1", &json!({})).await.unwrap();
        assert_eq!(a.ask().len(), 4);
    }

    #[tokio::test]
    async fn internal_through_the_router() {
        let lock = Arc::new(OnceLock::new());
        let t = Internal::new(lock.clone(), RuntimeToken::generate());
        assert!(matches!(
            t.fetch("/goed").await,
            Err(TransportError::Unreachable(_))
        ));
        lock.set(router()).ok();
        assert_eq!(t.fetch("/goed").await.unwrap(), json!({"a": 1}));
        assert_eq!(
            t.fetch("/fout").await.unwrap_err(),
            TransportError::Response {
                status: 404,
                error: "weg".into()
            }
        );
    }

    #[tokio::test]
    async fn http_and_the_time_limit() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, router()).await });
        let t = Http::new(&format!("http://{address}/"), Duration::from_secs(10)).unwrap();
        assert_eq!(t.fetch("/goed").await.unwrap(), json!({"a": 1}));
        assert!(matches!(
            t.fetch("/fout").await,
            Err(TransportError::Response { status: 404, .. })
        ));
        // A good status with an unreadable response is not an empty value.
        assert!(matches!(
            t.fetch("/geen-json").await,
            Err(TransportError::Json(r)) if r.contains("not JSON")
        ));
        let error = fetch(&t, "/traag", Duration::from_millis(200))
            .await
            .unwrap_err();
        assert!(matches!(error, TransportError::Unreachable(r) if r.contains("within")));
    }

    /// A route that returns the header with the runtime token.
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
    async fn the_runtime_token_is_only_sent_when_present() {
        let token = RuntimeToken::generate();
        let lock = Arc::new(OnceLock::new());
        lock.set(echo()).ok();
        let internal = Internal::new(lock, token.clone());
        assert_eq!(
            internal.fetch("/token").await.unwrap(),
            json!(token.as_str())
        );

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, echo()).await });
        let url = format!("http://{address}");
        let foreign = Http::new(&url, Duration::from_secs(5)).unwrap();
        assert_eq!(foreign.fetch("/token").await.unwrap(), Value::Null);
        let own = foreign.with_runtime_token(token.clone());
        assert_eq!(own.fetch("/token").await.unwrap(), json!(token.as_str()));
    }

    #[test]
    fn the_token_matches_only_exactly() {
        let t = RuntimeToken::generate();
        assert_eq!(t.as_str().len(), 64);
        assert!(t.holds(t.as_str().as_bytes()));
        assert!(!t.holds(&t.as_str().as_bytes()[..63]));
        assert!(!t.holds(RuntimeToken::generate().as_str().as_bytes()));
        assert!(!format!("{t:?}").contains(t.as_str()));
    }

    #[tokio::test]
    async fn http_without_server_is_unreachable() {
        // A port that was just free is (almost certainly) still closed.
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        drop(listener);
        let t = Http::new(&format!("http://{address}"), Duration::from_secs(2)).unwrap();
        assert!(matches!(
            t.fetch("/goed").await,
            Err(TransportError::Unreachable(_))
        ));
    }
}
