//! Per-traject corpus state.
//!
//! Each traject owns a federated corpus config in the database
//! (`traject_corpus_sources`) that mirrors the shape of
//! `corpus-registry.yaml`. When the active traject changes, the editor
//! routes reads and writes through that traject's [`TrajectCorpus`] instead
//! of the globally configured [`crate::state::CorpusState`].
//!
//! Construction is lazy: the cache holds a [`OnceCell`] per traject, and
//! the first request that needs the traject pays the clone cost. The cell
//! pattern means concurrent first-touches on the same traject share one
//! clone; first-touches on *different* trajects never block each other.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use regelrecht_corpus::backend::create_backend;
use regelrecht_corpus::models::{GitHubSource, LocalSource, Source, SourceType};
use regelrecht_corpus::{CorpusRegistry, SourceMap};
use sqlx::PgPool;
use tokio::sync::{Mutex, OnceCell, RwLock};
use uuid::Uuid;

use crate::state::{BackendEntry, CorpusState};

/// Resolved corpus state for a single traject, plus per-source write
/// routing.
pub struct TrajectCorpus {
    pub corpus: CorpusState,
    /// Maps the `source_id` a law was loaded from to the `source_id`
    /// whose backend should receive the write. When the read source is
    /// itself writable (local source, or a GitHub source that doesn't
    /// need a traject-specific branch override), there's no entry and
    /// the caller falls back to the read `source_id` directly.
    ///
    /// Today this map carries one entry: the writable-own's `auth_ref`
    /// (which points at the seed source it shadows, e.g. `minbzk-central`)
    /// mapped to the writable-own's own `source_id`. That gives "save
    /// the law back where it came from" for laws read from the seed,
    /// routed through the traject's own branch on the same upstream
    /// repo.
    pub write_target_for_source: HashMap<String, String>,
    /// Read-your-writes overlay for law YAML content. After a successful
    /// `save_law` we mirror the persisted body here so subsequent reads
    /// in the same traject (any session, any user) see the new content
    /// without forcing a full source_map rebuild against GitHub. The
    /// overlay is content-only — `LoadedLaw` metadata
    /// (source_id/relative_path) doesn't change with a content edit, so
    /// backend resolution keeps using `corpus.source_map`.
    ///
    /// Unbounded growth is intentional: in practice the size is bounded
    /// by the number of distinct laws edited in this traject, with a
    /// memory budget of roughly N laws × YAML size (KBs). The overlay
    /// is cleared when the `TrajectCorpus` cache entry is invalidated
    /// (e.g. on source config change). If a bulk-edit flow is ever
    /// added that touches many laws per traject, revisit with an LRU
    /// cap.
    overlay: RwLock<HashMap<String, String>>,
    /// Git branch the traject's writable-own source pushes to
    /// (`traject/{slug}-{8hex}`). `None` for trajects whose writable source is
    /// local (no branch). Used by the suggest-on-save flow to tell the pipeline
    /// which branch to read the saved law from and write the sidecar to.
    writable_branch: Option<String>,
}

impl TrajectCorpus {
    /// Resolve the YAML content for a law in this traject, preferring the
    /// post-save overlay over the source_map snapshot built at traject
    /// activation time.
    pub async fn law_yaml(&self, law_id: &str) -> Option<String> {
        if let Some(text) = self.overlay.read().await.get(law_id) {
            return Some(text.clone());
        }
        self.corpus
            .source_map
            .get_law(law_id)
            .map(|l| l.yaml_content.clone())
    }

    /// Mirror a freshly-saved law's content into the read-your-writes
    /// overlay. Called by `save_law` after a successful `backend.persist`,
    /// so the next GET on the same law (or a refresh) sees the new body.
    pub async fn record_save(&self, law_id: String, body: String) {
        self.overlay.write().await.insert(law_id, body);
    }

    /// The git branch the traject's saves land on, if the writable source is a
    /// GitHub branch. `None` for local writable sources.
    pub fn writable_branch(&self) -> Option<&str> {
        self.writable_branch.as_deref()
    }
}

/// Lazy registry of per-traject corpora, mirroring the
/// `CorpusState`-per-traject design. Each cell is initialised exactly
/// once; concurrent first-touches on the same traject share the clone.
#[derive(Default)]
pub struct TrajectCorpusCache {
    cells: RwLock<HashMap<Uuid, Arc<OnceCell<Arc<TrajectCorpus>>>>>,
}

impl TrajectCorpusCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Get-or-build the corpus state for a traject.
    ///
    /// On a cache miss the slow path queries `traject_corpus_sources`,
    /// instantiates a backend per source (cloning when needed via
    /// `ensure_ready`), and stitches them into a [`CorpusState`].
    pub async fn get_or_build(
        &self,
        pool: &PgPool,
        traject_id: Uuid,
        auth_file: Option<PathBuf>,
        favorites: &std::collections::HashSet<String>,
    ) -> Result<Arc<TrajectCorpus>, TrajectCorpusError> {
        let cell = {
            let mut map = self.cells.write().await;
            map.entry(traject_id)
                .or_insert_with(|| Arc::new(OnceCell::new()))
                .clone()
        };

        let built = cell
            .get_or_try_init(|| async {
                build_traject_corpus(pool, traject_id, auth_file.as_deref(), favorites).await
            })
            .await?;
        Ok(built.clone())
    }

    /// Drop the cached entry for a traject so the next request rebuilds.
    /// Used after the traject's sources change so stale clones aren't
    /// served further.
    pub async fn invalidate(&self, traject_id: Uuid) {
        self.cells.write().await.remove(&traject_id);
    }
}

#[derive(Debug)]
pub enum TrajectCorpusError {
    NotFound,
    NoWritableOwn,
    Db(sqlx::Error),
    Corpus(regelrecht_corpus::error::CorpusError),
}

impl std::fmt::Display for TrajectCorpusError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound => write!(f, "traject not found or has no sources"),
            Self::NoWritableOwn => write!(f, "traject has no writable-own source configured"),
            Self::Db(e) => write!(f, "database error: {e}"),
            Self::Corpus(e) => write!(f, "corpus error: {e}"),
        }
    }
}

impl std::error::Error for TrajectCorpusError {}

impl From<sqlx::Error> for TrajectCorpusError {
    fn from(e: sqlx::Error) -> Self {
        Self::Db(e)
    }
}

impl From<regelrecht_corpus::error::CorpusError> for TrajectCorpusError {
    fn from(e: regelrecht_corpus::error::CorpusError) -> Self {
        Self::Corpus(e)
    }
}

/// Build a fresh [`TrajectCorpus`]: load sources from DB, clone backends,
/// produce a [`SourceMap`].
async fn build_traject_corpus(
    pool: &PgPool,
    traject_id: Uuid,
    auth_file: Option<&std::path::Path>,
    favorites: &std::collections::HashSet<String>,
) -> Result<Arc<TrajectCorpus>, TrajectCorpusError> {
    let rows = sqlx::query_as::<_, TrajectSourceRow>(
        "SELECT source_id, name, source_type::text AS source_type,
                gh_owner, gh_repo, gh_branch, gh_base_branch, gh_path, gh_ref,
                local_path, priority, auth_ref, scopes, is_writable_own
         FROM traject_corpus_sources
         WHERE traject_id = $1
         ORDER BY priority",
    )
    .bind(traject_id)
    .fetch_all(pool)
    .await?;

    if rows.is_empty() {
        return Err(TrajectCorpusError::NotFound);
    }

    // Confirm the traject has a writable_own source — without one we
    // can't route saves on laws read from the seed sources. The actual
    // routing happens via `write_target_for_source` below; the local
    // here is just the guard against a misconfigured traject.
    let writable_own_row = rows
        .iter()
        .find(|r| r.is_writable_own)
        .ok_or(TrajectCorpusError::NoWritableOwn)?;
    let writable_own_source_id = writable_own_row.source_id.clone();
    // The git branch the traject's saves land on (`traject/{slug}-{8hex}`).
    // Captured here so the suggest-on-save flow can tell the pipeline which
    // branch to read the saved law from and commit the suggestion sidecar to.
    let writable_branch = writable_own_row.gh_branch.clone();

    // Build the read-source → write-target-source map. Every non-
    // writable_own source (local seed, GitHub seed, …) is routed to the
    // writable_own's backend so a save on any read-only seed-loaded law
    // lands on the traject's branch. The earlier `auth_ref`-only
    // mapping only fired when the writable_own's auth_ref matched a
    // seed's source_id verbatim, which broke for preview/local-stack
    // deploys where the seed is `local` but auth_ref still resolves a
    // GitHub token — saves on those laws then silently fell back to
    // the local backend and never reached the traject branch.
    let mut write_target_for_source: HashMap<String, String> = HashMap::new();
    for row in &rows {
        if !row.is_writable_own {
            write_target_for_source.insert(row.source_id.clone(), writable_own_source_id.clone());
        }
    }

    let sources: Vec<Source> = rows.iter().map(|r| r.to_source()).collect();
    let registry = CorpusRegistry::from_sources(sources.clone());

    // Build a backend per source, scoped to a traject-specific clone path.
    let mut backends: HashMap<String, BackendEntry> = HashMap::new();
    for (row, source) in rows.iter().zip(sources.iter()) {
        // For the writable-own source we resolve strictly (no legacy
        // `CORPUS_GIT_TOKEN` fallback). The `auth_ref` on this row was
        // derived from the create-request's repo coords, so a missing
        // per-repo token MUST fail closed — not transparently ship the
        // central token to a user-chosen GitHub repo on the next push.
        // Seeded (non-writable) sources keep the legacy fallback so
        // pre-existing deployments that rely on a single global PAT for
        // read-only mirrors keep working.
        let token_result = if row.is_writable_own {
            let key = source.auth_ref.as_deref().unwrap_or(&source.id);
            regelrecht_corpus::auth::resolve_token_strict(key, auth_file)
        } else {
            regelrecht_corpus::auth::resolve_token_for_source(
                &source.id,
                source.auth_ref.as_deref(),
                auth_file,
            )
        };
        let token = token_result.unwrap_or_else(|e| {
            tracing::warn!(
                traject = %traject_id,
                source_id = %source.id,
                error = %e,
                "failed to resolve auth token for traject source"
            );
            None
        });
        // Diagnostic: token=None on the writable-own source means git
        // push will hit "could not read Username" later. Surface it now
        // with both the source_id and the resolved auth_ref so an
        // operator can see whether the row carries the expected ref and
        // whether the env var matches.
        if token.is_none() && source.id == writable_own_source_id {
            let expected_env = regelrecht_corpus::auth::token_env_name(
                source.auth_ref.as_deref().unwrap_or(&source.id),
            );
            tracing::error!(
                traject = %traject_id,
                source_id = %source.id,
                auth_ref = ?source.auth_ref,
                auth_file = ?auth_file,
                expected_env = %expected_env,
                "traject writable-own source resolved NO token — push will fail"
            );
        }

        // GitHub sources go through the in-memory Contents-API backend
        // (one isolated `GitHubApiBackend` per traject — no clone, no
        // working tree on disk). Local sources keep their configured
        // path — they're already isolated by definition.
        let backend_result = match &source.source_type {
            SourceType::GitHub { github } => build_traject_github_backend(
                traject_id,
                source,
                github,
                row.gh_base_branch.as_deref(),
                token.as_deref(),
            ),
            SourceType::Local { .. } => create_backend(source, token.as_deref()),
        };

        let is_writable_own = source.id == writable_own_source_id;
        match backend_result {
            Ok(mut backend) => {
                if let Err(e) = backend.ensure_ready().await {
                    if is_writable_own {
                        // The traject's whole purpose is to push edits to
                        // this branch; falling through with a missing
                        // writable backend would make every save 503 with
                        // no signal that the underlying clone failed.
                        tracing::error!(
                            traject = %traject_id,
                            source_id = %source.id,
                            error = %e,
                            "traject writable-own backend init failed"
                        );
                        return Err(TrajectCorpusError::Corpus(e));
                    }
                    tracing::warn!(
                        traject = %traject_id,
                        source_id = %source.id,
                        error = %e,
                        "traject backend init failed, skipping"
                    );
                    continue;
                }
                let writable = backend.is_writable();
                backends.insert(
                    source.id.clone(),
                    BackendEntry {
                        backend: Arc::new(Mutex::new(backend)),
                        writable,
                    },
                );
            }
            Err(e) => {
                if is_writable_own {
                    tracing::error!(
                        traject = %traject_id,
                        source_id = %source.id,
                        error = %e,
                        "failed to create traject writable-own backend"
                    );
                    return Err(TrajectCorpusError::Corpus(e));
                }
                tracing::warn!(
                    traject = %traject_id,
                    source_id = %source.id,
                    error = %e,
                    "failed to create traject backend"
                );
            }
        }
    }

    // Load laws from the same favorites set the global corpus uses, so the
    // traject sees the same law set (minus any laws the traject's sources
    // don't contain).
    let source_map = match registry.load_favorites_async(favorites, auth_file).await {
        Ok(map) => map,
        Err(e) => {
            tracing::warn!(
                traject = %traject_id,
                error = %e,
                "traject favorites load failed, falling back to local-only"
            );
            registry
                .load_local_sources()
                .unwrap_or_else(|_| SourceMap::new())
        }
    };

    Ok(Arc::new(TrajectCorpus {
        corpus: CorpusState {
            registry,
            source_map,
            backends,
            auth_file: auth_file.map(|p| p.to_path_buf()),
        },
        write_target_for_source,
        overlay: RwLock::new(HashMap::new()),
        writable_branch,
    }))
}

/// Build a [`GitHubApiBackend`] for a traject source — no clone, no
/// `/tmp` working tree. Reads, writes, branch-creation all go through
/// the GitHub REST API. The branch on the writable_own source is
/// created lazily (in `ensure_ready`) from `base_branch` if it doesn't
/// yet exist on the remote — preserving the "first save creates the
/// branch" behaviour the old `GitBackend` clone path had.
fn build_traject_github_backend(
    _traject_id: Uuid,
    _source: &Source,
    github: &GitHubSource,
    base_branch: Option<&str>,
    token: Option<&str>,
) -> Result<Box<dyn regelrecht_corpus::backend::RepoBackend>, regelrecht_corpus::error::CorpusError>
{
    use regelrecht_corpus::github_api_backend::GitHubApiBackend;

    // `traject_id` and `source.id` used to namespace the on-disk clone
    // path; with the API backend the branch + URL already disambiguate,
    // so the parameters are kept on the signature for call-site
    // symmetry only.
    let backend = GitHubApiBackend::new(
        github,
        Some(base_branch.unwrap_or("main").to_string()),
        token.map(|t| t.to_string()),
    )?;
    Ok(Box::new(backend))
}

/// DB row mirror for `traject_corpus_sources`. `gh_base_branch` is kept
/// outside [`Source`] because it's traject-flow-specific (clone-then-
/// branch-from) and the global [`Source`] type doesn't carry it.
#[derive(sqlx::FromRow, Debug, Clone)]
struct TrajectSourceRow {
    source_id: String,
    name: String,
    source_type: String,
    gh_owner: Option<String>,
    gh_repo: Option<String>,
    gh_branch: Option<String>,
    gh_base_branch: Option<String>,
    gh_path: Option<String>,
    gh_ref: Option<String>,
    local_path: Option<String>,
    priority: i32,
    auth_ref: Option<String>,
    scopes: serde_json::Value,
    is_writable_own: bool,
}

impl TrajectSourceRow {
    fn to_source(&self) -> Source {
        let source_type = match self.source_type.as_str() {
            "github" => SourceType::GitHub {
                github: GitHubSource {
                    owner: self.gh_owner.clone().unwrap_or_default(),
                    repo: self.gh_repo.clone().unwrap_or_default(),
                    branch: self.gh_branch.clone().unwrap_or_default(),
                    path: self.gh_path.clone(),
                    git_ref: self.gh_ref.clone(),
                },
            },
            _ => SourceType::Local {
                local: LocalSource {
                    path: PathBuf::from(self.local_path.clone().unwrap_or_default()),
                },
            },
        };

        let scopes = serde_json::from_value(self.scopes.clone()).unwrap_or_else(|e| {
            tracing::warn!(
                source_id = %self.source_id,
                error = %e,
                "traject_corpus_sources.scopes failed to deserialise, falling back to empty list"
            );
            Default::default()
        });

        Source {
            id: self.source_id.clone(),
            name: self.name.clone(),
            source_type,
            scopes,
            priority: self.priority.max(0) as u32,
            auth_ref: self.auth_ref.clone(),
        }
    }
}
