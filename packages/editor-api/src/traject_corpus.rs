//! Per-traject corpus state.
//!
//! Each traject owns a federated corpus config in the database
//! (`traject_corpus_sources`) that mirrors the shape of
//! `corpus-registry.yaml`. When the active traject changes, the editor
//! routes reads and writes through that traject's [`TrajectCorpus`] instead
//! of the globally configured [`crate::state::CorpusState`].
//!
//! Construction is lazy: the cache holds a slot per traject, and the
//! first request that needs the traject pays the clone cost. The slot's
//! build lock means concurrent first-touches on the same traject share
//! one clone; first-touches on *different* trajects never block each
//! other.
//!
//! The cached index snapshot expires after [`TRAJECT_INDEX_TTL`]: the
//! first request past the TTL re-enumerates the sources and swaps in a
//! fresh [`SourceMap`] (new laws merged upstream, re-harvests, saves on
//! another replica become visible without a process restart), while the
//! backends, the post-save overlay and the changed-laws cache carry over
//! so in-flight saves and read-your-writes semantics are unaffected.

use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use regelrecht_corpus::backend::create_backend;
use regelrecht_corpus::models::{GitHubSource, LocalSource, Source, SourceType};
use regelrecht_corpus::source_map::collect_law_implements;
use regelrecht_corpus::{CorpusRegistry, SourceMap};
use sqlx::PgPool;
use tokio::sync::{Mutex, RwLock};
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
    /// Source id of the writable-own backend in this traject. Used by
    /// handlers that have no per-law context (e.g. the documents CRUD
    /// endpoints) to address the traject's own branch directly without
    /// having to reverse-engineer it out of `write_target_for_source`.
    pub writable_own_source_id: String,
    /// Read-your-writes overlay for law YAML content: only bodies that
    /// went through a successful `save_law` (see [`record_save`]). After a
    /// save we mirror the persisted body here so subsequent reads in the
    /// same traject (any session, any user) see the new content without
    /// forcing a full source_map rebuild against GitHub. The overlay is
    /// content-only — `LoadedLaw` metadata (source_id/relative_path)
    /// doesn't change with a content edit, so backend resolution keeps
    /// using `corpus.source_map`.
    ///
    /// Shared (via the `Arc`) across TTL index refreshes: a refreshed
    /// snapshot must never resurrect pre-save content, and an in-flight
    /// save that calls `record_save` on the pre-refresh instance must be
    /// visible on the post-refresh one. It is only dropped when the whole
    /// traject cache entry is invalidated (source config change).
    ///
    /// Unbounded growth is intentional: in practice the size is bounded
    /// by the number of distinct laws edited in this traject, with a
    /// memory budget of roughly N laws × YAML size (KBs). If a bulk-edit
    /// flow is ever added that touches many laws per traject, revisit
    /// with an LRU cap. (Lazily-fetched read bodies live in the bounded
    /// `body_cache` below, not here.)
    overlay: Arc<RwLock<HashMap<String, String>>>,
    /// Cache of law bodies fetched lazily on first read (see `law_yaml`),
    /// so a re-read of an unloaded law (read-only article view, reload,
    /// another tab) doesn't spend another Contents API call.
    ///
    /// Unlike `overlay` this is per-snapshot: a TTL index refresh starts
    /// with an empty cache, so upstream content changes become visible
    /// within [`TRAJECT_INDEX_TTL`] instead of being served stale until
    /// process restart. Bounded at [`BODY_CACHE_MAX_ENTRIES`] with FIFO
    /// eviction so a crawl across a large federated corpus can't grow it
    /// without limit.
    body_cache: RwLock<BoundedBodyCache>,
    /// Per-snapshot "law → laws it `implements`" index, built on demand by
    /// [`implementors_of`] and discarded with the snapshot (a TTL refresh
    /// or invalidation starts from `None`). Building it is the one
    /// O(corpus) body scan; afterwards every implementors lookup is an
    /// in-memory reverse scan over the parsed lists.
    implements_index: RwLock<Option<Arc<ImplementsIndex>>>,
    /// Single-flight gate for building `implements_index`. Held across
    /// the (potentially long) body-fetching scan WITHOUT holding
    /// `implements_index` itself, so `record_save` — which runs while the
    /// writable-own backend mutex is held — can update the index without
    /// any lock cycle against the scan's backend fetches.
    implements_build_lock: Mutex<()>,
    /// Short-lived cache of the "edited in this traject" diff
    /// ([`changed_law_ids`]). Each `changed-laws` request would otherwise
    /// fire a GitHub Compare API call, and the library sidebar re-requests
    /// it on every load / traject switch — on a shared, rate-limited token
    /// that adds up fast. The cache collapses a burst of library loads into
    /// one Compare call per [`CHANGED_LAWS_TTL`] window. `save_law`
    /// invalidates it so the saving user sees their own edit appear
    /// immediately (read-your-writes, like `overlay`); other members /
    /// replicas converge within the TTL. A stale entry is also served as a
    /// fallback when a fresh Compare call fails (e.g. token throttled), so
    /// the section degrades to slightly-stale rather than vanishing.
    /// Shared (via the `Arc`) across TTL index refreshes, like `overlay`:
    /// a refresh must not undo `save_law`'s invalidation of this cache.
    changed_cache: Arc<RwLock<Option<ChangedLawsCache>>>,
}

/// Cached result of [`TrajectCorpus::changed_law_ids`] with the instant it
/// was computed, for TTL expiry.
struct ChangedLawsCache {
    computed_at: Instant,
    law_ids: Vec<String>,
}

/// How long a cached changed-laws diff is served before a fresh GitHub
/// Compare call is made. Short enough that another member's save shows up
/// promptly in the sidebar, long enough to collapse a burst of library
/// loads (mount + tab switches + retries) into a single Compare call.
const CHANGED_LAWS_TTL: Duration = Duration::from_secs(60);

/// How long a traject's cached index snapshot (its [`SourceMap`] plus the
/// derived body / implements caches) is served before the first request
/// past the deadline re-enumerates the sources. Same convergence target
/// as [`CHANGED_LAWS_TTL`]: upstream changes (new laws merged, re-harvests,
/// saves on another replica) show up within a minute instead of requiring
/// a process restart, while a burst of library loads still hits the
/// snapshot. The refresh keeps backends, the post-save overlay and the
/// changed-laws cache (see [`TrajectCorpusCache::get_or_build`]).
const TRAJECT_INDEX_TTL: Duration = Duration::from_secs(60);

/// Upper bound on lazily-fetched law bodies cached per traject snapshot
/// (see [`TrajectCorpus::body_cache`]). At a typical body size of a few
/// tens of KB this caps the cache at low tens of MB per traject — enough
/// to cover an implements-index scan over a mid-sized federated corpus
/// without refetching, while a full-corpus crawl merely evicts FIFO
/// instead of growing without bound.
const BODY_CACHE_MAX_ENTRIES: usize = 1024;

/// Lazily-fetched law bodies with a FIFO size cap. FIFO (not LRU) keeps
/// reads lock-cheap: a cache hit only needs the outer `RwLock`'s read
/// guard, no per-hit reordering under a write lock. Eviction order only
/// matters once a traject's read set exceeds [`BODY_CACHE_MAX_ENTRIES`],
/// which is already crawl territory.
#[derive(Default)]
struct BoundedBodyCache {
    map: HashMap<String, String>,
    /// Insertion order of the keys in `map`, oldest first.
    order: VecDeque<String>,
}

impl BoundedBodyCache {
    fn get(&self, law_id: &str) -> Option<&String> {
        self.map.get(law_id)
    }

    fn insert(&mut self, law_id: String, body: String) {
        if !self.map.contains_key(&law_id) {
            while self.map.len() >= BODY_CACHE_MAX_ENTRIES {
                let Some(oldest) = self.order.pop_front() else {
                    break;
                };
                self.map.remove(&oldest);
            }
            self.order.push_back(law_id.clone());
        }
        self.map.insert(law_id, body);
    }
}

/// Per-snapshot forward index: each law's parsed `implements` list, plus
/// the laws whose body couldn't be fetched during the scan (throttling,
/// token expiry, …) and therefore couldn't be checked.
///
/// `Clone` exists for `Arc::make_mut` in [`TrajectCorpus::record_save`]:
/// a post-save entry update copy-on-writes when a lookup still holds the
/// previous `Arc`.
#[derive(Clone)]
struct ImplementsIndex {
    /// `law_id` → `$id`s of the higher laws it declares it implements.
    /// Laws with an empty list are omitted.
    implements_by_law: HashMap<String, Vec<String>>,
    /// Laws skipped because their body fetch failed. Kept so lookups can
    /// report partiality instead of silently passing off an incomplete
    /// scan as "no implementors". Self-heals at the next snapshot
    /// (TTL refresh / invalidation), which rebuilds the index.
    failed_law_ids: Vec<String>,
}

/// Result of [`TrajectCorpus::implementors_of`]: the implementing law ids
/// plus the ids that could not be checked because their body fetch failed
/// when the index was built.
pub struct ImplementorsResult {
    pub implementors: Vec<String>,
    pub skipped_law_ids: Vec<String>,
}

impl TrajectCorpus {
    /// Resolve the YAML content for a law in this traject, preferring the
    /// post-save overlay over the source_map snapshot built at traject
    /// activation time.
    ///
    /// The source_map is a lightweight **index** — for GitHub-backed sources
    /// its entries carry only metadata (no body). When a metadata-only law is
    /// read for the first time, its body is fetched lazily from the law's own
    /// source backend (one Contents API call), rather than every law's content
    /// being fetched up front at traject-activation time.
    ///
    /// `Ok(None)` is a genuine miss (the law isn't in this traject's corpus, or
    /// its source's backend never initialised). A lazy-fetch failure (GitHub
    /// throttling, token expiry, a network blip) is returned as `Err` so the
    /// caller can answer "failed to load" instead of masking a transient error
    /// as a 404 "not found".
    pub async fn law_yaml(
        &self,
        law_id: &str,
    ) -> Result<Option<String>, regelrecht_corpus::error::CorpusError> {
        if let Some(text) = self.overlay.read().await.get(law_id) {
            return Ok(Some(text.clone()));
        }
        if let Some(text) = self.body_cache.read().await.get(law_id) {
            return Ok(Some(text.clone()));
        }

        // Pull the bits we need out of the index entry, then drop the borrow
        // before the await below.
        let (source_id, relative_path) = {
            let Some(law) = self.corpus.source_map.get_law(law_id) else {
                return Ok(None);
            };
            if law.is_loaded() {
                return Ok(Some(law.yaml_content.clone()));
            }
            (law.source_id.clone(), law.relative_path.clone())
        };

        // The source is indexed but its backend was skipped at build time
        // (already logged then) — the law isn't readable. Treat as a miss.
        let Some(entry) = self.corpus.backends.get(&source_id) else {
            return Ok(None);
        };

        let content = {
            let backend = entry.backend.lock().await;
            // `?` propagates a read error rather than collapsing it to None.
            backend
                .read_file(std::path::Path::new(&relative_path))
                .await?
        };
        let Some(content) = content else {
            return Ok(None);
        };

        // Cache the lazily-fetched body so re-reads of this unloaded law don't
        // each spend another Contents API call (read-only views, reloads, a
        // second tab). Per-snapshot: discarded on a TTL index refresh so
        // upstream content changes converge, and bounded so a corpus-wide
        // crawl can't grow it without limit. Also makes a genuinely-empty
        // body a one-shot fetch rather than re-fetching every call (the
        // empty-`yaml_content` "unloaded" sentinel can't tell them apart,
        // but this cache short-circuits before that check).
        self.body_cache
            .write()
            .await
            .insert(law_id.to_string(), content.clone());
        Ok(Some(content))
    }

    /// Mirror a freshly-saved law's content into the read-your-writes
    /// overlay. Called by `save_law` after a successful `backend.persist`,
    /// so the next GET on the same law (or a refresh) sees the new body.
    ///
    /// Also keeps the per-snapshot implements index coherent: a save can
    /// add or drop `implements` declarations, so when the index has been
    /// built its entry for this law is replaced with the new body's list.
    /// (When a save races an in-flight index build the scan may have read
    /// the pre-save body after this update ran; the entry is then stale
    /// until the next snapshot — bounded by [`TRAJECT_INDEX_TTL`].)
    pub async fn record_save(&self, law_id: String, body: String) {
        let implements = collect_law_implements(&body);
        self.overlay.write().await.insert(law_id.clone(), body);
        if let Some(index) = self.implements_index.write().await.as_mut() {
            let index = Arc::make_mut(index);
            // The law's body is now known, so it can no longer count as
            // "skipped due to fetch failure" from an earlier scan.
            index.failed_law_ids.retain(|id| id != &law_id);
            if implements.is_empty() {
                index.implements_by_law.remove(&law_id);
            } else {
                index.implements_by_law.insert(law_id, implements);
            }
        }
    }

    /// Law ids whose articles declare `implements` for `law_id` (the IoC
    /// reverse link), resolved against this snapshot's federated corpus.
    ///
    /// The first call builds the per-snapshot [`ImplementsIndex`] — the
    /// one O(corpus) scan that lazily fetches the body of every
    /// metadata-only law — and every later call (any target law) is an
    /// in-memory reverse lookup. Bodies fetched by the scan land in
    /// `body_cache`, so opening one of the scanned laws afterwards is
    /// also free. Laws whose body fetch failed are reported in
    /// [`ImplementorsResult::skipped_law_ids`] instead of being silently
    /// indistinguishable from "doesn't implement anything"; the failed
    /// set is retried when the snapshot rolls over (TTL refresh).
    pub async fn implementors_of(&self, law_id: &str) -> ImplementorsResult {
        let index = self.implements_index_get_or_build().await;
        let mut implementors: Vec<String> = index
            .implements_by_law
            .iter()
            .filter(|(id, implemented)| {
                id.as_str() != law_id && implemented.iter().any(|i| i == law_id)
            })
            .map(|(id, _)| id.clone())
            .collect();
        implementors.sort();
        ImplementorsResult {
            implementors,
            skipped_law_ids: index.failed_law_ids.clone(),
        }
    }

    /// Get the snapshot's implements index, building it on first use.
    ///
    /// Single-flighted on `implements_build_lock`; the (long, fetching)
    /// build never holds the `implements_index` RwLock itself, so a
    /// concurrent `record_save` — which runs under the writable-own
    /// backend mutex that the scan may also need — can always complete.
    async fn implements_index_get_or_build(&self) -> Arc<ImplementsIndex> {
        if let Some(index) = self.implements_index.read().await.as_ref() {
            return index.clone();
        }
        let _build = self.implements_build_lock.lock().await;
        // Re-check: another task may have built it while we waited.
        if let Some(index) = self.implements_index.read().await.as_ref() {
            return index.clone();
        }

        let mut implements_by_law = HashMap::new();
        let mut failed_law_ids = Vec::new();
        // Collect ids first so the source_map borrow doesn't live across
        // the awaits below.
        let law_ids: Vec<String> = self
            .corpus
            .source_map
            .laws()
            .map(|law| law.law_id.clone())
            .collect();
        for law_id in law_ids {
            // Resolve through `law_yaml`, NOT `LoadedLaw::yaml_content`:
            // federated laws are metadata-only until first read. The
            // overlay/body_cache make repeat scans cheap.
            match self.law_yaml(&law_id).await {
                Ok(Some(yaml)) => {
                    let implements = collect_law_implements(&yaml);
                    if !implements.is_empty() {
                        implements_by_law.insert(law_id, implements);
                    }
                }
                // A genuine miss (no backend / file vanished) has nothing
                // to index and nothing to retry.
                Ok(None) => {}
                Err(e) => {
                    tracing::debug!(law_id = %law_id, error = %e, "implements scan: body fetch failed");
                    failed_law_ids.push(law_id);
                }
            }
        }
        if !failed_law_ids.is_empty() {
            tracing::warn!(
                failed = failed_law_ids.len(),
                indexed = implements_by_law.len(),
                "implements index built with fetch failures; implementor lists may be incomplete until the next snapshot"
            );
        }
        failed_law_ids.sort();

        let index = Arc::new(ImplementsIndex {
            implements_by_law,
            failed_law_ids,
        });
        *self.implements_index.write().await = Some(index.clone());
        index
    }

    /// Law ids that have been edited in this traject: the diff between the
    /// writable-own source's traject branch and its base branch, mapped
    /// back to law ids via the source map.
    ///
    /// This is the durable source of truth for "what changed in this
    /// traject" — every save commits to the branch, so the branch-vs-base
    /// diff survives process restarts and is shared across all members
    /// (unlike the in-memory `overlay`, which is only a read-your-writes
    /// cache). Returns an empty list when nothing has been saved yet (the
    /// traject branch doesn't exist → the Compare API 404s → empty) or the
    /// writable-own backend isn't initialised.
    ///
    /// Cached for [`CHANGED_LAWS_TTL`] (see `changed_cache`) so the library
    /// sidebar's repeated loads don't each spend a GitHub Compare call. On a
    /// fresh-compute failure (e.g. token throttled) a stale cached value, if
    /// any, is served rather than propagating the error — the sidebar keeps
    /// showing the last-known edit set instead of dropping the section.
    pub async fn changed_law_ids(
        &self,
    ) -> Result<Vec<String>, regelrecht_corpus::error::CorpusError> {
        // Fast path: a fresh cache entry serves without any GitHub call.
        if let Some(cached) = self.changed_cache.read().await.as_ref() {
            if cached.computed_at.elapsed() < CHANGED_LAWS_TTL {
                return Ok(cached.law_ids.clone());
            }
        }

        match self.compute_changed_law_ids().await {
            Ok(ids) => {
                *self.changed_cache.write().await = Some(ChangedLawsCache {
                    computed_at: Instant::now(),
                    law_ids: ids.clone(),
                });
                Ok(ids)
            }
            Err(e) => {
                // Serve a stale entry (if we have one) rather than dropping
                // the section when GitHub is momentarily unavailable /
                // rate-limited. Only propagate when there's nothing cached.
                if let Some(cached) = self.changed_cache.read().await.as_ref() {
                    tracing::warn!(
                        error = %e,
                        "changed-laws compute failed; serving stale cached value"
                    );
                    return Ok(cached.law_ids.clone());
                }
                Err(e)
            }
        }
    }

    /// Drop the cached changed-laws diff so the next [`changed_law_ids`]
    /// call recomputes against GitHub. Called by `save_law` so the saving
    /// user's new edit shows up in the sidebar immediately instead of after
    /// the TTL — the changed-laws analogue of [`record_save`].
    pub async fn invalidate_changed_cache(&self) {
        *self.changed_cache.write().await = None;
    }

    /// Uncached computation behind [`changed_law_ids`]: ask the writable-own
    /// backend for its branch-vs-base diff and map the changed paths to law
    /// ids via the source map.
    async fn compute_changed_law_ids(
        &self,
    ) -> Result<Vec<String>, regelrecht_corpus::error::CorpusError> {
        let Some(entry) = self.corpus.backends.get(&self.writable_own_source_id) else {
            return Ok(Vec::new());
        };
        let changed = {
            let backend = entry.backend.lock().await;
            backend.changed_files().await?
        };
        if changed.is_empty() {
            return Ok(Vec::new());
        }
        // Match changed source-relative paths against loaded laws. Normalise
        // separators so a relative_path computed on a non-Unix host still
        // matches the forward-slash paths GitHub returns.
        let changed: std::collections::HashSet<String> =
            changed.into_iter().map(|p| p.replace('\\', "/")).collect();
        let mut ids: Vec<String> = self
            .corpus
            .source_map
            .laws()
            .filter(|law| changed.contains(&law.relative_path.replace('\\', "/")))
            .map(|law| law.law_id.clone())
            .collect();
        ids.sort();
        ids.dedup();
        Ok(ids)
    }
}

/// A built traject corpus plus the instant its index snapshot was
/// (re)built, for TTL expiry.
struct CachedCorpus {
    corpus: Arc<TrajectCorpus>,
    built_at: Instant,
}

impl CachedCorpus {
    fn is_fresh(&self, ttl: Duration) -> bool {
        self.built_at.elapsed() < ttl
    }
}

/// Per-traject cache slot. `state` holds the current snapshot (None until
/// first build); `build_lock` single-flights both the initial build and
/// TTL refreshes so concurrent first-touches share one clone and a
/// refresh herd collapses to one source enumeration.
#[derive(Default)]
struct TrajectSlot {
    state: RwLock<Option<CachedCorpus>>,
    build_lock: Mutex<()>,
}

/// Lazy registry of per-traject corpora, mirroring the
/// `CorpusState`-per-traject design. Concurrent first-touches on the same
/// traject share one build; first-touches on *different* trajects never
/// block each other. A built snapshot is served for `index_ttl` and then
/// refreshed in place (see [`Self::get_or_build`]).
pub struct TrajectCorpusCache {
    cells: RwLock<HashMap<Uuid, Arc<TrajectSlot>>>,
    /// How long an index snapshot is served before it is refreshed.
    /// [`TRAJECT_INDEX_TTL`] in production; tests inject shorter values
    /// via [`Self::with_index_ttl`].
    index_ttl: Duration,
}

impl Default for TrajectCorpusCache {
    fn default() -> Self {
        Self::new()
    }
}

impl TrajectCorpusCache {
    pub fn new() -> Self {
        Self::with_index_ttl(TRAJECT_INDEX_TTL)
    }

    /// Cache with a caller-chosen snapshot TTL — the injection point for
    /// tests that need to force (or rule out) a refresh without waiting
    /// out the production TTL.
    pub fn with_index_ttl(index_ttl: Duration) -> Self {
        Self {
            cells: RwLock::new(HashMap::new()),
            index_ttl,
        }
    }

    /// Get-or-build the corpus state for a traject.
    ///
    /// On a cache miss the slow path queries `traject_corpus_sources`,
    /// instantiates a backend per source (cloning when needed via
    /// `ensure_ready`), and stitches them into a [`CorpusState`].
    ///
    /// When the cached snapshot is older than the index TTL, the first
    /// request past the deadline re-enumerates the sources and swaps in a
    /// refreshed [`TrajectCorpus`] (see [`refresh_traject_corpus`] for
    /// what carries over); concurrent requests keep being served the
    /// stale snapshot while one refresh is in flight, and a failed
    /// refresh extends the stale snapshot for another TTL window instead
    /// of erroring reads (same degrade-to-stale stance as
    /// [`TrajectCorpus::changed_law_ids`]).
    pub async fn get_or_build(
        &self,
        pool: &PgPool,
        traject_id: Uuid,
        auth_file: Option<PathBuf>,
    ) -> Result<Arc<TrajectCorpus>, TrajectCorpusError> {
        let slot = {
            let mut map = self.cells.write().await;
            map.entry(traject_id)
                .or_insert_with(|| Arc::new(TrajectSlot::default()))
                .clone()
        };

        // Fast path: a fresh snapshot serves without touching the build
        // lock. A stale-but-present snapshot is remembered so it can be
        // served when another task is already refreshing.
        let stale = {
            let state = slot.state.read().await;
            match state.as_ref() {
                Some(cached) if cached.is_fresh(self.index_ttl) => {
                    return Ok(cached.corpus.clone())
                }
                Some(cached) => Some(cached.corpus.clone()),
                None => None,
            }
        };

        let _build = match &stale {
            // Nothing cached: every caller must wait for the one build.
            None => slot.build_lock.lock().await,
            // Stale: only one task refreshes; the rest serve stale rather
            // than queueing up behind a network round-trip.
            Some(stale_corpus) => match slot.build_lock.try_lock() {
                Ok(guard) => guard,
                Err(_) => return Ok(stale_corpus.clone()),
            },
        };

        // Re-check under the build lock: the previous holder may have
        // built/refreshed while we waited.
        let stale = {
            let state = slot.state.read().await;
            match state.as_ref() {
                Some(cached) if cached.is_fresh(self.index_ttl) => {
                    return Ok(cached.corpus.clone())
                }
                Some(cached) => Some(cached.corpus.clone()),
                None => None,
            }
        };

        let corpus = match stale {
            None => build_traject_corpus(pool, traject_id, auth_file.as_deref()).await?,
            Some(old) => match refresh_traject_corpus(&old, traject_id).await {
                Ok(refreshed) => refreshed,
                Err(e) => {
                    // Serve (and re-arm) the stale snapshot rather than
                    // failing reads on a transient enumeration error; the
                    // re-armed `built_at` stops every subsequent request
                    // from re-attempting against a throttled upstream.
                    tracing::warn!(
                        traject = %traject_id,
                        error = %e,
                        "traject index refresh failed; serving stale snapshot for another TTL"
                    );
                    *slot.state.write().await = Some(CachedCorpus {
                        corpus: old.clone(),
                        built_at: Instant::now(),
                    });
                    return Ok(old);
                }
            },
        };

        *slot.state.write().await = Some(CachedCorpus {
            corpus: corpus.clone(),
            built_at: Instant::now(),
        });
        Ok(corpus)
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
    let writable_own_source_id = rows
        .iter()
        .find(|r| r.is_writable_own)
        .map(|r| r.source_id.clone())
        .ok_or(TrajectCorpusError::NoWritableOwn)?;

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

    // Build a lightweight INDEX of every law across the traject's sources —
    // the private writable-own repo and the seeded central corpus — so the
    // bibliotheek search can surface any law without fetching every law's body
    // up front (which meant N per-file GitHub Contents API calls per traject
    // build — slow and rate-limited). Bodies are fetched lazily on first read
    // (see `TrajectCorpus::law_yaml`). Per-source index failures are tolerated
    // inside `index_all_sources_async` (a bad seed source is skipped, not
    // fatal). A failure to enumerate the writable-own source is the one we care
    // about most — its laws are the point of the traject — so it's logged at
    // error level for operators. The traject still opens, though: it degrades
    // (those laws missing from search until the next rebuild) rather than
    // failing entirely on what is usually a transient GitHub hiccup. The hard
    // failure path is the writable-own *backend* init above, which returns
    // `Err` so a broken write target never opens silently.
    let source_map = match registry.index_all_sources_async(auth_file).await {
        Ok((map, failed)) => {
            if failed.iter().any(|id| id == &writable_own_source_id) {
                tracing::error!(
                    traject = %traject_id,
                    source_id = %writable_own_source_id,
                    "traject writable-own source failed to load — the traject's own laws \
                     will be missing from the bibliotheek until the corpus is rebuilt"
                );
            }
            map
        }
        Err(e) => {
            tracing::warn!(
                traject = %traject_id,
                error = %e,
                "traject corpus load failed, falling back to local-only"
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
        writable_own_source_id,
        overlay: Arc::new(RwLock::new(HashMap::new())),
        body_cache: RwLock::new(BoundedBodyCache::default()),
        implements_index: RwLock::new(None),
        implements_build_lock: Mutex::new(()),
        changed_cache: Arc::new(RwLock::new(None)),
    }))
}

/// Build a TTL-refreshed [`TrajectCorpus`] from an existing one: a fresh
/// index snapshot over the *same* sources and backends.
///
/// Carried over from `old` (so a refresh can never break in-flight save
/// semantics):
/// - the **backends map** — the same `Arc<Mutex<…>>` instances, so a save
///   holding a backend mutex across the refresh keeps excluding writers
///   and the writable-own → seed lock-ordering invariant is unaffected;
/// - the **post-save `overlay`** (shared `Arc`) — refreshed reads keep
///   seeing saved bodies (never resurrect pre-save content), and a save
///   that lands on the pre-refresh instance is visible post-refresh;
/// - the **changed-laws cache** (shared `Arc`) — same reasoning for
///   `save_law`'s invalidation;
/// - the **write routing** (`write_target_for_source`,
///   `writable_own_source_id`) and the registry/auth config, which only
///   change through traject create/delete → [`TrajectCorpusCache::invalidate`].
///
/// Fresh in the new instance:
/// - the **`source_map` index snapshot** — the point of the refresh;
/// - the **`body_cache`** — so upstream body changes become visible;
/// - the **implements index** — rebuilt on demand against the new snapshot.
///
/// Any source failing to enumerate fails the whole refresh: the caller
/// then serves the previous (complete) snapshot for another TTL, which
/// strictly beats swapping in a snapshot with thousands of laws missing.
async fn refresh_traject_corpus(
    old: &Arc<TrajectCorpus>,
    traject_id: Uuid,
) -> Result<Arc<TrajectCorpus>, TrajectCorpusError> {
    let registry = old.corpus.registry.clone();
    let auth_file = old.corpus.auth_file.clone();
    let (source_map, failed) = registry
        .index_all_sources_async(auth_file.as_deref())
        .await?;
    if !failed.is_empty() {
        return Err(TrajectCorpusError::Corpus(
            regelrecht_corpus::error::CorpusError::Config(format!(
                "index refresh for traject {traject_id} failed to enumerate sources: {failed:?}"
            )),
        ));
    }

    Ok(Arc::new(TrajectCorpus {
        corpus: CorpusState {
            registry,
            source_map,
            backends: old.corpus.backends.clone(),
            auth_file,
        },
        write_target_for_source: old.write_target_for_source.clone(),
        writable_own_source_id: old.writable_own_source_id.clone(),
        overlay: old.overlay.clone(),
        body_cache: RwLock::new(BoundedBodyCache::default()),
        implements_index: RwLock::new(None),
        implements_build_lock: Mutex::new(()),
        changed_cache: old.changed_cache.clone(),
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

#[cfg(test)]
mod tests {
    //! Unit tests for the snapshot caches: the bounded lazy-body cache,
    //! the TTL freshness rule, the per-snapshot implements index (incl.
    //! the fetch-failure path) and the carry-over semantics of a TTL
    //! index refresh. The DB-backed `get_or_build` flow is covered by
    //! the `traject_reads_test` integration tests.
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    use std::path::Path;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use async_trait::async_trait;
    use regelrecht_corpus::backend::{
        FileEntry, PersistOutcome, RepoBackend, WriteContext as CorpusWriteContext,
    };
    use regelrecht_corpus::error::{CorpusError, Result as CorpusResult};

    /// In-memory backend stub: serves a fixed file set, optionally
    /// failing every read (the throttled-fetch path), and counts reads
    /// so tests can assert a cache hit skipped the backend.
    struct StubBackend {
        files: HashMap<String, String>,
        fail_reads: bool,
        reads: Arc<AtomicUsize>,
    }

    impl StubBackend {
        fn with_files(files: &[(&str, &str)]) -> Self {
            Self {
                files: files
                    .iter()
                    .map(|(p, c)| (p.to_string(), c.to_string()))
                    .collect(),
                fail_reads: false,
                reads: Arc::new(AtomicUsize::new(0)),
            }
        }

        fn failing() -> Self {
            Self {
                files: HashMap::new(),
                fail_reads: true,
                reads: Arc::new(AtomicUsize::new(0)),
            }
        }
    }

    #[async_trait]
    impl RepoBackend for StubBackend {
        async fn read_file(&self, path: &Path) -> CorpusResult<Option<String>> {
            self.reads.fetch_add(1, Ordering::SeqCst);
            if self.fail_reads {
                return Err(CorpusError::Git("simulated throttle".to_string()));
            }
            Ok(self.files.get(path.to_str().unwrap()).cloned())
        }
        async fn write_file(&self, _: &Path, _: &str) -> CorpusResult<()> {
            Ok(())
        }
        async fn delete_file(&self, _: &Path) -> CorpusResult<()> {
            Ok(())
        }
        async fn list_files(&self, _: &Path, _: Option<&str>) -> CorpusResult<Vec<FileEntry>> {
            Ok(Vec::new())
        }
        async fn persist(&self, _: &CorpusWriteContext) -> CorpusResult<PersistOutcome> {
            Ok(PersistOutcome::default())
        }
        async fn ensure_ready(&mut self) -> CorpusResult<()> {
            Ok(())
        }
        fn is_writable(&self) -> bool {
            true
        }
    }

    fn backend_entry(backend: StubBackend) -> BackendEntry {
        BackendEntry {
            backend: Arc::new(Mutex::new(Box::new(backend) as Box<dyn RepoBackend>)),
            writable: true,
        }
    }

    /// Bare `TrajectCorpus` over the given index + backends; no DB, no
    /// network.
    fn test_corpus(
        source_map: SourceMap,
        backends: HashMap<String, BackendEntry>,
    ) -> TrajectCorpus {
        TrajectCorpus {
            corpus: CorpusState {
                registry: CorpusRegistry::empty(),
                source_map,
                backends,
                auth_file: None,
            },
            write_target_for_source: HashMap::new(),
            writable_own_source_id: "own".to_string(),
            overlay: Arc::new(RwLock::new(HashMap::new())),
            body_cache: RwLock::new(BoundedBodyCache::default()),
            implements_index: RwLock::new(None),
            implements_build_lock: Mutex::new(()),
            changed_cache: Arc::new(RwLock::new(None)),
        }
    }

    fn metadata_entry(map: &mut SourceMap, law_id: &str, source_id: &str) {
        map.load_metadata_entry(
            law_id,
            &format!("wet/{law_id}/2025-01-01.yaml"),
            None,
            source_id,
            source_id,
            // Distinct priorities per source: equal-priority conflicts
            // across sources are a hard SourceMap error.
            if source_id == "own" { 0 } else { 1 },
        )
        .unwrap();
    }

    fn law_body(law_id: &str, implements: Option<&str>) -> String {
        match implements {
            Some(higher) => format!(
                "$id: {law_id}\narticles:\n  - number: '1'\n    machine_readable:\n      implements:\n        - law: {higher}\n"
            ),
            None => format!("$id: {law_id}\narticles: []\n"),
        }
    }

    // ---- BoundedBodyCache ----

    #[test]
    fn body_cache_evicts_oldest_at_cap() {
        let mut cache = BoundedBodyCache::default();
        for i in 0..BODY_CACHE_MAX_ENTRIES + 10 {
            cache.insert(format!("law_{i}"), "body".to_string());
        }
        assert_eq!(cache.map.len(), BODY_CACHE_MAX_ENTRIES);
        // The 10 oldest entries were evicted FIFO; the newest survive.
        assert!(cache.get("law_0").is_none());
        assert!(cache.get("law_9").is_none());
        assert!(cache.get("law_10").is_some());
        assert!(cache
            .get(&format!("law_{}", BODY_CACHE_MAX_ENTRIES + 9))
            .is_some());
    }

    #[test]
    fn body_cache_overwrite_does_not_grow_or_evict() {
        let mut cache = BoundedBodyCache::default();
        cache.insert("a".to_string(), "v1".to_string());
        cache.insert("b".to_string(), "v1".to_string());
        cache.insert("a".to_string(), "v2".to_string());
        assert_eq!(cache.map.len(), 2);
        assert_eq!(cache.order.len(), 2);
        assert_eq!(cache.get("a").map(String::as_str), Some("v2"));
    }

    // ---- TTL freshness ----

    #[test]
    fn cached_corpus_freshness_respects_ttl() {
        let cached = CachedCorpus {
            corpus: Arc::new(test_corpus(SourceMap::new(), HashMap::new())),
            built_at: Instant::now(),
        };
        // A just-built snapshot is fresh under the production TTL…
        assert!(cached.is_fresh(TRAJECT_INDEX_TTL));
        // …and stale under a zero TTL (the injection tests use).
        assert!(!cached.is_fresh(Duration::ZERO));
    }

    // ---- law_yaml / body cache interplay ----

    #[tokio::test]
    async fn law_yaml_caches_lazy_fetch_and_prefers_overlay() {
        let mut map = SourceMap::new();
        metadata_entry(&mut map, "wet_a", "seed");
        let stub = StubBackend::with_files(&[("wet/wet_a/2025-01-01.yaml", "$id: wet_a\n")]);
        let reads = stub.reads.clone();
        let corpus = test_corpus(
            map,
            HashMap::from([("seed".to_string(), backend_entry(stub))]),
        );

        // First read fetches and caches…
        assert_eq!(
            corpus.law_yaml("wet_a").await.unwrap().as_deref(),
            Some("$id: wet_a\n")
        );
        assert_eq!(reads.load(Ordering::SeqCst), 1);
        // …second read is a body-cache hit, no backend round-trip.
        assert_eq!(
            corpus.law_yaml("wet_a").await.unwrap().as_deref(),
            Some("$id: wet_a\n")
        );
        assert_eq!(reads.load(Ordering::SeqCst), 1);

        // A save wins over the cached body (read-your-writes).
        corpus
            .record_save("wet_a".to_string(), "$id: wet_a\nname: saved\n".to_string())
            .await;
        assert_eq!(
            corpus.law_yaml("wet_a").await.unwrap().as_deref(),
            Some("$id: wet_a\nname: saved\n")
        );
    }

    // ---- implements index ----

    #[tokio::test]
    async fn implementors_of_builds_index_once_and_reverse_looks_up() {
        let mut map = SourceMap::new();
        metadata_entry(&mut map, "regeling_a", "seed");
        metadata_entry(&mut map, "wet_hoger", "seed");
        let stub = StubBackend::with_files(&[
            (
                "wet/regeling_a/2025-01-01.yaml",
                &law_body("regeling_a", Some("wet_hoger")),
            ),
            (
                "wet/wet_hoger/2025-01-01.yaml",
                &law_body("wet_hoger", None),
            ),
        ]);
        let reads = stub.reads.clone();
        let corpus = test_corpus(
            map,
            HashMap::from([("seed".to_string(), backend_entry(stub))]),
        );

        let result = corpus.implementors_of("wet_hoger").await;
        assert_eq!(result.implementors, vec!["regeling_a".to_string()]);
        assert!(result.skipped_law_ids.is_empty());
        let reads_after_build = reads.load(Ordering::SeqCst);
        assert_eq!(reads_after_build, 2, "index build fetches each body once");

        // A second lookup — different target — reuses the index without
        // re-fetching anything.
        let result = corpus.implementors_of("regeling_a").await;
        assert!(result.implementors.is_empty());
        assert_eq!(reads.load(Ordering::SeqCst), reads_after_build);
    }

    #[tokio::test]
    async fn implementors_of_reports_fetch_failures_as_skipped() {
        let mut map = SourceMap::new();
        metadata_entry(&mut map, "regeling_a", "seed");
        metadata_entry(&mut map, "wet_kapot", "broken");
        let stub = StubBackend::with_files(&[(
            "wet/regeling_a/2025-01-01.yaml",
            &law_body("regeling_a", Some("wet_hoger")),
        )]);
        let corpus = test_corpus(
            map,
            HashMap::from([
                ("seed".to_string(), backend_entry(stub)),
                ("broken".to_string(), backend_entry(StubBackend::failing())),
            ]),
        );

        let result = corpus.implementors_of("wet_hoger").await;
        // The throttled law is reported as skipped — distinguishable from
        // "checked and implements nothing" — while the healthy law is
        // still found.
        assert_eq!(result.implementors, vec!["regeling_a".to_string()]);
        assert_eq!(result.skipped_law_ids, vec!["wet_kapot".to_string()]);
    }

    #[tokio::test]
    async fn record_save_updates_built_implements_index_in_place() {
        let mut map = SourceMap::new();
        metadata_entry(&mut map, "regeling_a", "seed");
        let stub = StubBackend::with_files(&[(
            "wet/regeling_a/2025-01-01.yaml",
            &law_body("regeling_a", None),
        )]);
        let corpus = test_corpus(
            map,
            HashMap::from([("seed".to_string(), backend_entry(stub))]),
        );

        // Build the index against the pre-save body: no implementors.
        assert!(corpus
            .implementors_of("wet_hoger")
            .await
            .implementors
            .is_empty());

        // The save adds an `implements` declaration; the lookup must see
        // it immediately — no snapshot rollover needed.
        corpus
            .record_save(
                "regeling_a".to_string(),
                law_body("regeling_a", Some("wet_hoger")),
            )
            .await;
        assert_eq!(
            corpus.implementors_of("wet_hoger").await.implementors,
            vec!["regeling_a".to_string()]
        );

        // And the reverse: a save dropping the declaration removes it.
        corpus
            .record_save("regeling_a".to_string(), law_body("regeling_a", None))
            .await;
        assert!(corpus
            .implementors_of("wet_hoger")
            .await
            .implementors
            .is_empty());
    }

    // ---- TTL index refresh ----

    /// Build a real local-source `TrajectCorpus` over `dir`, the manual
    /// equivalent of `build_traject_corpus` without the DB round-trip.
    async fn local_corpus(dir: &Path) -> Arc<TrajectCorpus> {
        let source = Source {
            id: "own".to_string(),
            name: "Own".to_string(),
            source_type: SourceType::Local {
                local: LocalSource {
                    path: dir.to_path_buf(),
                },
            },
            scopes: vec![],
            priority: 0,
            auth_ref: None,
        };
        let registry = CorpusRegistry::from_sources(vec![source.clone()]);
        let source_map = registry.load_local_sources().unwrap();
        let mut backend = create_backend(&source, None).unwrap();
        backend.ensure_ready().await.unwrap();
        let writable = backend.is_writable();
        let backends = HashMap::from([(
            "own".to_string(),
            BackendEntry {
                backend: Arc::new(Mutex::new(backend)),
                writable,
            },
        )]);
        Arc::new(TrajectCorpus {
            corpus: CorpusState {
                registry,
                source_map,
                backends,
                auth_file: None,
            },
            write_target_for_source: HashMap::new(),
            writable_own_source_id: "own".to_string(),
            overlay: Arc::new(RwLock::new(HashMap::new())),
            body_cache: RwLock::new(BoundedBodyCache::default()),
            implements_index: RwLock::new(None),
            implements_build_lock: Mutex::new(()),
            changed_cache: Arc::new(RwLock::new(None)),
        })
    }

    fn write_law_file(dir: &Path, law_id: &str, body: &str) {
        let law_dir = dir.join("wet").join(law_id);
        std::fs::create_dir_all(&law_dir).unwrap();
        std::fs::write(law_dir.join("2025-01-01.yaml"), body).unwrap();
    }

    #[tokio::test]
    async fn refresh_swaps_index_but_carries_backends_overlay_and_changed_cache() {
        let dir = tempfile::tempdir().unwrap();
        write_law_file(dir.path(), "wet_a", "$id: wet_a\nname: v1\n");
        let old = local_corpus(dir.path()).await;

        // A save lands in the overlay before the refresh…
        old.record_save("wet_a".to_string(), "$id: wet_a\nname: saved\n".to_string())
            .await;
        // …and upstream gains a brand-new law the old snapshot misses.
        write_law_file(dir.path(), "wet_b", "$id: wet_b\nname: nieuw\n");
        assert!(old.corpus.source_map.get_law("wet_b").is_none());

        let refreshed = refresh_traject_corpus(&old, Uuid::new_v4())
            .await
            .expect("local refresh must succeed");

        // Fresh snapshot: the new upstream law is now indexed.
        assert!(refreshed.corpus.source_map.get_law("wet_b").is_some());
        // The backends are the *same* mutexes — an in-flight save keeps
        // excluding writers across the swap.
        assert!(Arc::ptr_eq(
            &old.corpus.backends["own"].backend,
            &refreshed.corpus.backends["own"].backend
        ));
        // The overlay carried over: the refresh must never resurrect
        // pre-save content.
        assert_eq!(
            refreshed.law_yaml("wet_a").await.unwrap().as_deref(),
            Some("$id: wet_a\nname: saved\n")
        );
        // A save recorded on the OLD instance after the swap is visible
        // through the refreshed one (shared overlay, not a copy).
        old.record_save(
            "wet_a".to_string(),
            "$id: wet_a\nname: saved2\n".to_string(),
        )
        .await;
        assert_eq!(
            refreshed.law_yaml("wet_a").await.unwrap().as_deref(),
            Some("$id: wet_a\nname: saved2\n")
        );
        // The changed-laws cache is shared for the same reason.
        assert!(Arc::ptr_eq(&old.changed_cache, &refreshed.changed_cache));
    }

    #[tokio::test]
    async fn refresh_resets_the_implements_index_with_the_snapshot() {
        let dir = tempfile::tempdir().unwrap();
        write_law_file(dir.path(), "wet_hoger", &law_body("wet_hoger", None));
        let old = local_corpus(dir.path()).await;
        assert!(old
            .implementors_of("wet_hoger")
            .await
            .implementors
            .is_empty());

        // A new implementing regulation lands upstream (e.g. merged on
        // the central corpus / saved on another replica).
        write_law_file(
            dir.path(),
            "regeling_a",
            &law_body("regeling_a", Some("wet_hoger")),
        );
        let refreshed = refresh_traject_corpus(&old, Uuid::new_v4())
            .await
            .expect("local refresh must succeed");

        // The refreshed snapshot rebuilds its own index and finds it; the
        // old instance's cached index is untouched (per-snapshot).
        assert_eq!(
            refreshed.implementors_of("wet_hoger").await.implementors,
            vec!["regeling_a".to_string()]
        );
        assert!(old
            .implementors_of("wet_hoger")
            .await
            .implementors
            .is_empty());
    }
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
