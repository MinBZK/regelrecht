use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use regelrecht_auth::{ConfiguredClient, OidcAppState, OidcConfig};
use regelrecht_corpus::auth::resolve_token_for_source;
use regelrecht_corpus::backend::{RepoBackend, SessionGitBackend};
use regelrecht_corpus::config::CorpusConfig;
use regelrecht_corpus::models::{Source, SourceType};
use regelrecht_corpus::{CorpusClient, SourceMap};
use sqlx::PgPool;
use tokio::sync::{Mutex, OnceCell, RwLock};

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

    /// Snapshot the corpus state needed by
    /// [`SessionRegistry::resolve_session_backend`] so the caller can drop
    /// the corpus read guard before the (potentially slow) session backend
    /// init runs. The init future allocates a per-(session, source)
    /// `SessionGitBackend` and runs `ensure_ready` — which on a cold cache
    /// performs a full `git clone` — and we must not hold the corpus
    /// `RwLock` read guard across that or a concurrent `POST
    /// /api/corpus/reload` write-locker is starved for the duration.
    ///
    /// Returns `None` when no source with `source_id` is registered. The
    /// caller maps that to a 404 at the HTTP boundary.
    pub fn snapshot_source_for_session(&self, source_id: &str) -> Option<SessionSourceSnapshot> {
        let source = self.registry.get_source(source_id)?.clone();
        let local_backend = self.backends.get(source_id).map(|e| e.backend.clone());
        Some(SessionSourceSnapshot {
            source,
            local_backend,
            auth_file: self.auth_file.clone(),
        })
    }
}

/// Owned snapshot of the corpus state that
/// [`SessionRegistry::resolve_session_backend`] needs to decide how to
/// route a (session, source) write. Built via
/// [`CorpusState::snapshot_source_for_session`] while holding the corpus
/// read guard; the guard can be dropped before the resolver runs so the
/// slow-path `git clone` does not block concurrent corpus-reload
/// writers.
pub struct SessionSourceSnapshot {
    pub source: Source,
    /// Currently-registered global backend for this source, if any. Only
    /// used when `source.source_type` is `Local` — GitHub sources route
    /// to a freshly created per-session backend.
    pub local_backend: Option<Arc<Mutex<Box<dyn RepoBackend>>>>,
    /// Path to corpus-auth.yaml; cloned out of the corpus guard so the
    /// resolver can read it without holding the lock.
    pub auth_file: Option<PathBuf>,
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
///
/// Each map entry is an [`Arc<OnceCell<SharedBackend>>`]. The registry
/// write lock is held only long enough to insert the placeholder cell;
/// the actual clone runs against the per-key cell so two callers on
/// **different** keys never block each other on the slow path. Concurrent
/// callers on the **same** key serialise inside the cell's `get_or_try_init`
/// — which is the desired behaviour for that case.
/// Hard cap on the number of `(session, source)` backends kept in the
/// registry simultaneously. Each entry holds a full on-disk git clone,
/// so unbounded growth is a disk-exhaustion vector — a misbehaving (or
/// hostile authenticated) client that rotates `X-Editor-Session` values
/// would otherwise be able to fill `/tmp` until the pod fails. A proper
/// LRU reaper that prunes idle sessions is tracked separately; this cap
/// is the minimum guard so the registry refuses new keys with `503`
/// rather than crashing the process.
pub const SESSION_REGISTRY_MAX_ENTRIES: usize = 5_000;

#[derive(Default)]
pub struct SessionRegistry {
    /// Backends keyed by `(session_id, source_id)`. The cells are
    /// initialised lazily by the first caller for each key.
    backends: RwLock<HashMap<SessionBackendKey, Arc<OnceCell<SharedBackend>>>>,
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
    /// `snapshot` is an owned slice of the corpus state captured via
    /// [`CorpusState::snapshot_source_for_session`]. Callers MUST drop the
    /// `state.corpus` read guard before invoking this method — the
    /// GitHub-source slow path runs `git clone` inside the cell, and
    /// holding the corpus `RwLock` read guard across that would starve
    /// concurrent `POST /api/corpus/reload` write-lockers.
    pub async fn resolve_session_backend(
        &self,
        snapshot: &SessionSourceSnapshot,
        session_id: &str,
        source_id: &str,
    ) -> std::result::Result<ResolvedSessionBackend, SessionResolveError> {
        match &snapshot.source.source_type {
            SourceType::Local { .. } => snapshot
                .local_backend
                .as_ref()
                .map(|backend| ResolvedSessionBackend {
                    backend: backend.clone(),
                    uses_session_pr: false,
                })
                .ok_or_else(|| SessionResolveError::SourceNotFound(source_id.to_string())),
            SourceType::GitHub { github } => {
                let key = (session_id.to_string(), source_id.to_string());

                // Fast path: backend already exists and is initialised for
                // this session+source.
                if let Some(cell) = self.backends.read().await.get(&key) {
                    if let Some(existing) = cell.get() {
                        return Ok(ResolvedSessionBackend {
                            backend: existing.clone(),
                            uses_session_pr: true,
                        });
                    }
                }

                // Get or insert a per-key OnceCell under the write lock,
                // then release the lock immediately. The slow `ensure_ready`
                // (which does a full `git clone`) runs against the cell
                // outside the registry lock, so callers writing to a
                // **different** (session, source) key are never blocked
                // by a clone in flight elsewhere.
                let cell = {
                    let mut map = self.backends.write().await;
                    // Refuse new keys once the cap is reached. Existing keys
                    // (already in the map) still resolve so an in-flight
                    // session is not killed mid-edit; only NEW (session,
                    // source) pairs hit the 503.
                    if !map.contains_key(&key) && map.len() >= SESSION_REGISTRY_MAX_ENTRIES {
                        return Err(SessionResolveError::CapacityExceeded);
                    }
                    map.entry(key)
                        .or_insert_with(|| Arc::new(OnceCell::new()))
                        .clone()
                };

                // `get_or_try_init` runs the init future exactly once per
                // cell; concurrent callers on the SAME key serialise here,
                // which is what we want — they all need the same backend.
                let backend = cell
                    .get_or_try_init(|| async {
                        let token = resolve_token_for_source(
                            source_id,
                            snapshot.source.auth_ref.as_deref(),
                            snapshot.auth_file.as_deref(),
                        )
                        .map_err(|e| SessionResolveError::Other(e.to_string()))?
                        .ok_or_else(|| SessionResolveError::NoToken(source_id.to_string()))?;

                        // Per-(session, source) clone path: keeps each session
                        // isolated so a checkout for session A doesn't disturb
                        // session B's working tree mid-persist. Trade disk for
                        // simplicity — cleanup on session expiry is a follow-up.
                        let host_id =
                            std::env::var("HOSTNAME").unwrap_or_else(|_| "local".to_string());
                        let repo_path = std::env::temp_dir()
                            .join("regelrecht-editor-sessions")
                            .join(host_id)
                            .join(source_id)
                            .join(session_id);

                        let repo_url =
                            format!("https://github.com/{}/{}.git", github.owner, github.repo);
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
                        // ensure_ready may take a second or two; runs **without**
                        // the registry write lock held so other keys are
                        // unaffected.
                        backend
                            .ensure_ready()
                            .await
                            .map_err(|e| SessionResolveError::Other(e.to_string()))?;

                        Ok::<SharedBackend, SessionResolveError>(Arc::new(Mutex::new(backend)))
                    })
                    .await?
                    .clone();

                Ok(ResolvedSessionBackend {
                    backend,
                    uses_session_pr: true,
                })
            }
        }
    }
}

/// Errors from [`SessionRegistry::resolve_session_backend`]. The
/// editor-api maps these to HTTP statuses (404 / 403 / 503 / 500) at the
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
    /// The registry has hit its hard cap on concurrent (session, source)
    /// backends. Each backend holds an on-disk git clone, so unbounded
    /// growth is a disk-exhaustion vector; this guards against rotation
    /// attacks or honest leaks until a proper LRU reaper lands. Surfaced
    /// as 503 to tell the caller "try again later".
    CapacityExceeded,
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
            Self::CapacityExceeded => write!(
                f,
                "session registry at capacity ({} entries)",
                SESSION_REGISTRY_MAX_ENTRIES
            ),
            Self::Other(msg) => write!(f, "{}", msg),
        }
    }
}

impl std::error::Error for SessionResolveError {}

#[cfg(test)]
mod tests {
    //! SessionRegistry resolution tests. The GitHub-source happy path
    //! (the actual `git clone` + PR open) is covered by
    //! `SessionGitBackend` integration tests in `regelrecht-corpus`; here
    //! we lock down the routing decisions (local pass-through, source-
    //! not-found, no-token mapping).
    use super::*;

    fn registry_yaml_for(source_yaml: &str) -> String {
        format!("schema_version: '1'\nsources:\n{source_yaml}")
    }

    fn corpus_state_from_yaml(yaml: &str) -> CorpusState {
        let registry = regelrecht_corpus::CorpusRegistry::from_yaml(yaml).unwrap();
        CorpusState {
            registry,
            source_map: SourceMap::new(),
            backends: HashMap::new(),
            auth_file: None,
        }
    }

    #[tokio::test]
    async fn resolve_unknown_source_returns_source_not_found() {
        let yaml = registry_yaml_for(
            r"  - id: known
    name: Known
    type: local
    local:
      path: /tmp/does-not-matter
    priority: 1
",
        );
        let corpus = corpus_state_from_yaml(&yaml);
        // Unknown source has no snapshot — the handler maps that to 404
        // upstream of the resolver. The resolver itself is only invoked
        // for known sources.
        assert!(corpus.snapshot_source_for_session("unknown").is_none());
    }

    #[tokio::test]
    async fn resolve_local_source_without_registered_backend_is_source_not_found() {
        // Source is declared in the registry but no backend was registered
        // for it. The handler maps SourceNotFound → 404, which is correct
        // because the source is effectively unusable for writes.
        let yaml = registry_yaml_for(
            r"  - id: local-src
    name: Local
    type: local
    local:
      path: /tmp/does-not-matter
    priority: 1
",
        );
        let corpus = corpus_state_from_yaml(&yaml);
        let snapshot = corpus
            .snapshot_source_for_session("local-src")
            .expect("source is registered");
        let reg = SessionRegistry::new();
        let result = reg
            .resolve_session_backend(&snapshot, "sess-1", "local-src")
            .await;
        match result {
            Err(SessionResolveError::SourceNotFound(_)) => {}
            Err(other) => panic!("expected SourceNotFound, got {other:?}"),
            Ok(_) => panic!("expected SourceNotFound, got Ok"),
        }
    }

    #[tokio::test]
    async fn resolve_github_source_without_token_is_no_token() {
        // No auth_ref and no auth_file → resolve_token_for_source returns
        // Ok(None), which the registry must surface as NoToken so the
        // handler can map it to 403 (not silently fall back).
        let yaml = registry_yaml_for(
            r"  - id: gh-src
    name: GH
    type: github
    github:
      owner: minbzk
      repo: test
      branch: main
    priority: 1
",
        );
        let corpus = corpus_state_from_yaml(&yaml);
        let snapshot = corpus
            .snapshot_source_for_session("gh-src")
            .expect("source is registered");
        let reg = SessionRegistry::new();
        let result = reg
            .resolve_session_backend(&snapshot, "sess-1", "gh-src")
            .await;
        match result {
            Err(SessionResolveError::NoToken(id)) => assert_eq!(id, "gh-src"),
            Err(other) => panic!("expected NoToken, got {other:?}"),
            Ok(_) => panic!("expected NoToken, got Ok"),
        }
    }
}
