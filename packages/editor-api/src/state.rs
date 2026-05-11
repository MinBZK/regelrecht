use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use regelrecht_auth::{ConfiguredClient, OidcAppState, OidcConfig};
use regelrecht_corpus::auth::resolve_token_for_source;
use regelrecht_corpus::backend::{RepoBackend, SessionGitBackend};
use regelrecht_corpus::config::CorpusConfig;
use regelrecht_corpus::models::SourceType;
use regelrecht_corpus::{CorpusClient, SourceMap};
use sqlx::PgPool;
use tokio::sync::{Mutex, RwLock};

use crate::config::AppConfig;

#[derive(Clone)]
pub struct AppState {
    /// Loaded corpus sources with provenance metadata.
    pub corpus: Arc<RwLock<CorpusState>>,
    pub oidc_client: Option<Arc<ConfiguredClient>>,
    pub end_session_url: Option<String>,
    pub config: Arc<AppConfig>,
    pub http_client: reqwest::Client,
    /// Database connection pool (available when auth is enabled).
    pub pool: Option<PgPool>,
    /// Base URL of the pipeline-api service (e.g. "http://pipeline-api:8001").
    /// When set, `/api/harvest/*` requests are proxied to this service.
    pub pipeline_api_url: Option<String>,
    /// Single-flight gate for `POST /api/corpus/reload`. The reload fans out
    /// to GitHub (one API call per law source); without this gate an
    /// authenticated user could burn through the 5000 req/hr token quota
    /// with a single `xargs -P` and block corpus reads for everyone.
    pub reload_lock: Arc<Mutex<()>>,
    /// Per-(editor session, source) write-back backends used by the
    /// federated PR write-path. Lazily created on first save.
    pub sessions: Arc<SessionRegistry>,
}

impl OidcAppState for AppState {
    fn oidc_client(&self) -> Option<&Arc<ConfiguredClient>> {
        self.oidc_client.as_ref()
    }
    fn end_session_url(&self) -> Option<&str> {
        self.end_session_url.as_deref()
    }
    fn oidc_config(&self) -> Option<&OidcConfig> {
        self.config.oidc.as_ref()
    }
    fn is_auth_enabled(&self) -> bool {
        self.config.is_auth_enabled()
    }
    fn base_url(&self) -> Option<&str> {
        self.config.base_url.as_deref()
    }
    fn http_client(&self) -> &reqwest::Client {
        &self.http_client
    }
}

/// A registered backend along with its writability flag, captured at init
/// time after [`RepoBackend::ensure_ready`] (so a local source on a
/// read-only filesystem is recorded as `writable: false`).
pub struct BackendEntry {
    pub backend: Arc<Mutex<Box<dyn RepoBackend>>>,
    pub writable: bool,
}

/// State for the corpus subsystem.
pub struct CorpusState {
    pub registry: regelrecht_corpus::CorpusRegistry,
    pub source_map: SourceMap,
    /// Backends keyed by source ID. Read-only backends are also registered
    /// here so reads (`get_scenario`, `list_scenarios`) can route through
    /// the same abstraction as writes — preventing read/write path
    /// mismatches when a fallback writable backend is used.
    pub backends: HashMap<String, BackendEntry>,
    /// Path to corpus-auth.yaml for GitHub authentication during reload.
    pub auth_file: Option<PathBuf>,
}

impl CorpusState {
    #[allow(dead_code)]
    pub fn empty() -> Self {
        Self {
            registry: regelrecht_corpus::CorpusRegistry::empty(),
            source_map: SourceMap::new(),
            backends: HashMap::new(),
            auth_file: None,
        }
    }
}

/// Outcome of resolving a write target through the [`SessionRegistry`].
///
/// For GitHub-sourced laws the registry hands back a per-(session, source)
/// [`SessionGitBackend`] that pushes to a feature branch and opens/updates
/// a PR upstream. For local-sourced laws it returns the existing global
/// backend so the on-disk dev workflow keeps working unchanged.
pub struct ResolvedSessionBackend {
    pub backend: Arc<Mutex<Box<dyn RepoBackend>>>,
    /// `true` when the resolved backend opens a PR on `persist`. The
    /// editor-api uses this flag to decide whether the JSON response for
    /// `save_law` / `save_scenario` should expose `pr_url` / `pr_number`.
    pub uses_session_pr: bool,
}

/// Composite key used by [`SessionRegistry`]: `(editor session id, source
/// id)`. Both are stable strings; the tuple lets us look up "the backend
/// session X uses to write to source Y" without separate maps.
type SessionBackendKey = (String, String);

/// Aliased to keep the [`SessionRegistry`] field type readable. The shape
/// matches the existing read-path `BackendEntry.backend` so the trait
/// object goes through the same `Arc<Mutex<Box<dyn RepoBackend>>>` chain.
type SharedBackend = Arc<Mutex<Box<dyn RepoBackend>>>;

/// Lazy registry of per-(session, source) backends used for federated
/// write-back.
///
/// Each editor session (a UUID minted in the browser, sent as
/// `X-Editor-Session`) gets its own [`SessionGitBackend`] per source it
/// writes to. The backend pushes to a stable feature branch on the source
/// repo (`editor/session-<uuid>`) and ensures an open PR against the
/// source's configured branch — so all edits a user makes in one sitting
/// land on the same review-ready PR.
///
/// Backends are created lazily on first write to keep `editor-api` startup
/// cheap (no clone-per-session up front). The registry survives for the
/// process lifetime; restart loses the in-memory map and a fresh save will
/// re-clone, finding any prior session branch on the remote and continuing
/// from there.
#[derive(Default)]
pub struct SessionRegistry {
    /// Backends keyed by `(session_id, source_id)`.
    backends: RwLock<HashMap<SessionBackendKey, SharedBackend>>,
}

impl SessionRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Resolve the backend for a (session, source) write.
    ///
    /// - **Local sources**: returns the existing global backend. There is
    ///   no upstream to PR to and the dev loop expects writes to land on
    ///   the local filesystem immediately.
    /// - **GitHub sources**: returns (or lazily creates) a
    ///   [`SessionGitBackend`] for this session+source. Requires a token
    ///   configured for the source — without one we surface a 403 because
    ///   silently dropping edits is worse than a clear error.
    ///
    /// `corpus` is read-locked once to look up the source definition and
    /// (for the local case) hand back the global backend. The session
    /// backend itself does not borrow from `corpus`.
    pub async fn resolve_session_backend(
        &self,
        corpus: &CorpusState,
        session_id: &str,
        source_id: &str,
    ) -> std::result::Result<ResolvedSessionBackend, SessionResolveError> {
        let source = corpus
            .registry
            .get_source(source_id)
            .ok_or_else(|| SessionResolveError::SourceNotFound(source_id.to_string()))?;

        match &source.source_type {
            SourceType::Local { .. } => corpus
                .backends
                .get(source_id)
                .map(|entry| ResolvedSessionBackend {
                    backend: entry.backend.clone(),
                    uses_session_pr: false,
                })
                .ok_or_else(|| SessionResolveError::SourceNotFound(source_id.to_string())),
            SourceType::GitHub { github } => {
                let key = (session_id.to_string(), source_id.to_string());

                // Fast path: backend already exists for this session+source.
                if let Some(existing) = self.backends.read().await.get(&key) {
                    return Ok(ResolvedSessionBackend {
                        backend: existing.clone(),
                        uses_session_pr: true,
                    });
                }

                // Slow path: build the backend under a write lock. We
                // re-check inside the write lock to avoid two racing
                // creators producing two backends for the same key.
                let mut map = self.backends.write().await;
                if let Some(existing) = map.get(&key) {
                    return Ok(ResolvedSessionBackend {
                        backend: existing.clone(),
                        uses_session_pr: true,
                    });
                }

                let token = resolve_token_for_source(
                    source_id,
                    source.auth_ref.as_deref(),
                    corpus.auth_file.as_deref(),
                )
                .map_err(|e| SessionResolveError::Other(e.to_string()))?
                .ok_or_else(|| SessionResolveError::NoToken(source_id.to_string()))?;

                // Per-(session, source) clone path: keeps each session
                // isolated so a checkout for session A doesn't disturb
                // session B's working tree mid-persist. Trade disk for
                // simplicity — cleanup on session expiry is a follow-up.
                let host_id = std::env::var("HOSTNAME").unwrap_or_else(|_| "local".to_string());
                let repo_path = std::env::temp_dir()
                    .join("regelrecht-editor-sessions")
                    .join(host_id)
                    .join(source_id)
                    .join(session_id);

                let repo_url = format!("https://github.com/{}/{}.git", github.owner, github.repo);
                let mut config = CorpusConfig::new(&repo_url, &repo_path);
                config.branch = github.effective_ref().to_string();
                config = config.with_token(&token);
                let client = CorpusClient::new(config);

                let session_branch = format!("editor/session-{session_id}");
                let mut backend: Box<dyn RepoBackend> = Box::new(
                    SessionGitBackend::new(
                        client,
                        github.path.clone(),
                        session_branch,
                        github.effective_ref().to_string(),
                        github.owner.clone(),
                        github.repo.clone(),
                        token,
                    )
                    .map_err(|e| SessionResolveError::Other(e.to_string()))?,
                );

                // Clone the source on first use of this (session, source).
                // ensure_ready may take a second or two; the write lock
                // means concurrent saves on the SAME (session, source)
                // serialise here — that's fine, those callers literally
                // need this backend.
                backend
                    .ensure_ready()
                    .await
                    .map_err(|e| SessionResolveError::Other(e.to_string()))?;

                let arc = Arc::new(Mutex::new(backend));
                map.insert(key, arc.clone());

                Ok(ResolvedSessionBackend {
                    backend: arc,
                    uses_session_pr: true,
                })
            }
        }
    }
}

/// Errors from [`SessionRegistry::resolve_session_backend`]. The
/// editor-api maps these to HTTP statuses (404 / 403 / 500) at the
/// handler boundary — keeps the registry independent of axum types.
#[derive(Debug)]
pub enum SessionResolveError {
    /// No source registered with that id.
    SourceNotFound(String),
    /// A GitHub source has no token configured. Surfaced to the user as
    /// 403 with a clear "source X is not configured for write-back"
    /// message; we deliberately do not silently fall back to a local-only
    /// commit because the user would think their edit reached upstream.
    NoToken(String),
    /// Anything else (clone failure, bad config, IO error). Surfaced as
    /// 500 with a generic message; the underlying error is logged.
    Other(String),
}

impl std::fmt::Display for SessionResolveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SourceNotFound(id) => write!(f, "source '{}' not found in registry", id),
            Self::NoToken(id) => write!(
                f,
                "source '{}' is not configured for write-back (no auth token)",
                id
            ),
            Self::Other(msg) => write!(f, "{}", msg),
        }
    }
}

impl std::error::Error for SessionResolveError {}
