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

/// Resolved corpus state for a single traject, plus the id of the source
/// that receives writes.
pub struct TrajectCorpus {
    pub corpus: CorpusState,
    /// `source_id` of the row with `is_writable_own = TRUE` in
    /// `traject_corpus_sources`. Writes from save handlers route here.
    pub writable_own_source_id: String,
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

    let writable_own_source_id = rows
        .iter()
        .find(|r| r.is_writable_own)
        .map(|r| r.source_id.clone())
        .ok_or(TrajectCorpusError::NoWritableOwn)?;

    let sources: Vec<Source> = rows.iter().map(|r| r.to_source()).collect();
    let registry = CorpusRegistry::from_sources(sources.clone());

    // Build a backend per source, scoped to a traject-specific clone path.
    let mut backends: HashMap<String, BackendEntry> = HashMap::new();
    for (row, source) in rows.iter().zip(sources.iter()) {
        let token = regelrecht_corpus::auth::resolve_token_for_source(
            &source.id,
            source.auth_ref.as_deref(),
            auth_file,
        )
        .unwrap_or_else(|e| {
            tracing::warn!(
                traject = %traject_id,
                source_id = %source.id,
                error = %e,
                "failed to resolve auth token for traject source"
            );
            None
        });

        // For GitHub sources we override the clone path so each traject
        // gets its own working tree. Local sources keep their configured
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
        writable_own_source_id,
    }))
}

/// Build a [`GitBackend`] whose clone path lives under a traject-specific
/// directory, so two trajects writing to the same upstream repo never
/// share a working tree.
fn build_traject_github_backend(
    traject_id: Uuid,
    source: &Source,
    github: &GitHubSource,
    base_branch: Option<&str>,
    token: Option<&str>,
) -> Result<Box<dyn regelrecht_corpus::backend::RepoBackend>, regelrecht_corpus::error::CorpusError>
{
    use regelrecht_corpus::backend::GitBackend;
    use regelrecht_corpus::config::CorpusConfig;
    use regelrecht_corpus::CorpusClient;

    let host_id = std::env::var("HOSTNAME").unwrap_or_else(|_| "local".to_string());
    let repo_path = std::env::temp_dir()
        .join("regelrecht-editor-corpus")
        .join("trajects")
        .join(host_id)
        .join(traject_id.to_string())
        .join(&source.id);

    let repo_url = format!("https://github.com/{}/{}.git", github.owner, github.repo);
    let mut config = CorpusConfig::new(&repo_url, &repo_path);
    config.branch = github.effective_ref().to_string();
    // For traject writable branches we expect the branch to not yet exist
    // on the remote on first clone — `git_clone_and_create_branch` will
    // fall back to `base_branch` and push a new branch. Default to `main`
    // for trajects (overrides the global default of `development`); the
    // caller can override per-source when their writable repo's default
    // branch is something else.
    config.base_branch = Some(base_branch.unwrap_or("main").to_string());
    if let Some(t) = token {
        config = config.with_token(t);
    }
    let client = CorpusClient::new(config);
    Ok(Box::new(GitBackend::new(client, github.path.clone())))
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
