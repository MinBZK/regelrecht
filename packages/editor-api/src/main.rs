use std::collections::{HashMap, HashSet};
use std::env;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use axum::middleware as axum_middleware;
use axum::routing::get;
use axum::Router;
use tokio::sync::{Mutex, RwLock};
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;
use tower_sessions::ExpiredDeletion;
use tower_sessions::Expiry;
use tower_sessions::SessionManagerLayer;
use tower_sessions_memory_store::MemoryStore;
use tower_sessions_sqlx_store::PostgresStore;
use tracing_subscriber::EnvFilter;

mod config;
mod corpus_handlers;
mod favorites;
mod feature_flags;
mod harvest_proxy;
mod middleware;
mod state;
mod user_settings;

use state::{AppState, CorpusState};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let app_config = config::AppConfig::from_env();

    // --- OIDC discovery (conditional) ---
    let (oidc_client, end_session_url) = if let Some(ref oidc_config) = app_config.oidc {
        match regelrecht_auth::discover_client(oidc_config).await {
            Ok(result) => (Some(Arc::new(result.client)), result.end_session_url),
            Err(e) => {
                tracing::error!(error = %e, "OIDC discovery failed");
                std::process::exit(1);
            }
        }
    } else {
        (None, None)
    };

    // --- HTTP client for OIDC token exchange ---
    let http_client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .unwrap_or_else(|e| {
            tracing::error!(error = %e, "failed to build HTTP client");
            std::process::exit(1);
        });

    // --- Corpus init ---
    let static_dir = env::var("STATIC_DIR").unwrap_or_else(|_| "static".to_string());
    let corpus_state = init_corpus(&static_dir).await;

    let hostname = env::var("HOSTNAME").ok();
    let pipeline_api_url =
        resolve_pipeline_api_url(hostname.as_deref(), env::var("PIPELINE_API_URL").ok());
    let hostname_log = hostname.as_deref().unwrap_or("<none>");
    match &pipeline_api_url {
        Some(url) => {
            tracing::info!(url = %url, hostname = %hostname_log, "pipeline-api proxy target")
        }
        None => tracing::info!(
            hostname = %hostname_log,
            "no pipeline-api URL configured, harvest proxy disabled"
        ),
    }

    let mut app_state = AppState {
        corpus: Arc::new(RwLock::new(corpus_state)),
        oidc_client,
        end_session_url,
        config: Arc::new(app_config),
        http_client,
        pool: None, // set below when auth is enabled
        pipeline_api_url,
        reload_lock: Arc::new(tokio::sync::Mutex::new(())),
    };

    let index_file = PathBuf::from(&static_dir).join("index.html");

    // --- Routes ---
    let auth_routes = regelrecht_auth::auth_routes::<AppState>();

    // Public API routes — accessible without authentication
    let public_api_routes = Router::new()
        .route("/api/sources", get(corpus_handlers::list_sources))
        .route("/api/corpus/laws", get(corpus_handlers::list_corpus_laws))
        .route(
            "/api/corpus/laws/{law_id}",
            get(corpus_handlers::get_corpus_law),
        )
        .route(
            "/api/corpus/laws/{law_id}/outputs",
            get(corpus_handlers::list_law_outputs),
        )
        .route(
            "/api/corpus/laws/{law_id}/scenarios",
            get(corpus_handlers::list_scenarios),
        )
        .route(
            "/api/corpus/laws/{law_id}/scenarios/{filename}",
            get(corpus_handlers::get_scenario),
        )
        .route("/api/feature-flags", get(feature_flags::list_feature_flags))
        // Harvest status — forwarded to pipeline-api. Read-only DB lookup,
        // safe to expose unauthenticated. (The search endpoint lives behind
        // auth because it triggers outbound requests to zoekservice.overheid.nl
        // and would otherwise be an amplification vector.)
        .route("/api/harvest/status", get(harvest_proxy::proxy_harvest));

    // Protected API routes — require authentication when OIDC is enabled.
    // Write endpoints (PUT/DELETE) for scenarios live here so they cannot be
    // invoked anonymously when a deployment has a git push token configured.
    //
    // The 1 MiB body cap is generous for a single Gherkin scenario file
    // (real-world scenarios are a few KiB) and prevents a caller from
    // streaming an arbitrarily large body to disk — important when OIDC
    // is disabled in local dev and the endpoint is reachable without auth.
    const MAX_SCENARIO_BODY: usize = 1024 * 1024;
    // Law YAMLs are larger than scenarios — zorgtoeslag's ~25 KiB is typical
    // but federated regulations can reach a few hundred KiB. A 5 MiB cap
    // gives ample headroom while still rejecting pathological bodies.
    const MAX_LAW_BODY: usize = 5 * 1024 * 1024;
    let protected_api_routes = Router::new()
        .route(
            "/api/corpus/laws/{law_id}/scenarios/{filename}",
            axum::routing::put(corpus_handlers::save_scenario)
                .delete(corpus_handlers::delete_scenario)
                .layer(axum::extract::DefaultBodyLimit::max(MAX_SCENARIO_BODY)),
        )
        .route(
            "/api/corpus/laws/{law_id}",
            axum::routing::put(corpus_handlers::save_law)
                .layer(axum::extract::DefaultBodyLimit::max(MAX_LAW_BODY)),
        )
        .route("/api/favorites", get(favorites::list))
        .route(
            "/api/favorites/{law_id}",
            axum::routing::put(favorites::add).delete(favorites::remove),
        )
        // Harvest proxy — write operations behind auth. Search is also
        // behind auth because it makes outbound requests to the SRU API.
        .route("/api/harvest/search", get(harvest_proxy::proxy_harvest))
        .route(
            "/api/harvest",
            axum::routing::post(harvest_proxy::proxy_harvest),
        )
        .route(
            "/api/harvest/batch",
            axum::routing::post(harvest_proxy::proxy_harvest),
        )
        .route(
            "/api/corpus/reload",
            axum::routing::post(corpus_handlers::reload_corpus),
        )
        .route(
            "/api/feature-flags/{key}",
            axum::routing::put(feature_flags::update_feature_flag),
        )
        .route("/api/user/settings", get(user_settings::list))
        .route(
            "/api/user/settings/{key}",
            axum::routing::put(user_settings::set),
        )
        .route_layer(axum_middleware::from_fn_with_state(
            app_state.clone(),
            middleware::require_session_auth::<AppState>,
        ));

    // --- Build app with session layer ---
    // SessionManagerLayer is generic over the store type, so we build the
    // router in two branches depending on whether auth is enabled.
    if app_state.config.is_auth_enabled() {
        let database_url = env::var("DATABASE_URL")
            .or_else(|_| env::var("DATABASE_SERVER_FULL"))
            .unwrap_or_else(|_| {
                tracing::error!(
                    "DATABASE_URL is required when OIDC is enabled (for session storage)"
                );
                std::process::exit(1);
            });

        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(5)
            .connect(&database_url)
            .await
            .unwrap_or_else(|e| {
                tracing::error!(error = %e, "failed to connect to database");
                std::process::exit(1);
            });

        // The editor shares a database with the pipeline; ensure_schema runs
        // all pipeline migrations (including 0008_user_favorites). If the
        // services are ever split to separate databases this should be replaced
        // with an editor-specific migration runner.
        if let Err(e) = regelrecht_pipeline::ensure_schema(&pool).await {
            tracing::error!(error = %e, "database migration failed");
            std::process::exit(1);
        }

        app_state.pool = Some(pool.clone());

        let session_store = PostgresStore::new(pool);
        if let Err(e) = session_store.migrate().await {
            tracing::error!(error = %e, "failed to create session table");
            std::process::exit(1);
        }
        tracing::info!("session store ready (PostgreSQL-backed)");

        let deletion_handle = tokio::task::spawn(
            session_store
                .clone()
                .continuously_delete_expired(tokio::time::Duration::from_secs(60)),
        );

        let session_layer = SessionManagerLayer::new(session_store)
            .with_expiry(Expiry::OnInactivity(time::Duration::hours(8)))
            .with_same_site(tower_sessions::cookie::SameSite::Lax)
            .with_http_only(true)
            .with_secure(true);

        let app = Router::new()
            .route("/health", get(|| async { "OK" }))
            .merge(auth_routes)
            .merge(public_api_routes)
            .merge(protected_api_routes)
            .with_state(app_state)
            .layer(session_layer)
            .layer(axum_middleware::from_fn(middleware::security_headers))
            .layer(TraceLayer::new_for_http())
            .fallback_service(
                ServeDir::new(&static_dir).not_found_service(ServeFile::new(&index_file)),
            );

        serve(app, Some(deletion_handle)).await;
    } else {
        // No .with_secure(true) — this branch only runs when OIDC is disabled
        // (local development over plain HTTP). In production OIDC is always
        // enabled and the auth-enabled branch above sets secure cookies.
        let session_layer = SessionManagerLayer::new(MemoryStore::default())
            .with_same_site(tower_sessions::cookie::SameSite::Lax)
            .with_http_only(true);

        let app = Router::new()
            .route("/health", get(|| async { "OK" }))
            .merge(auth_routes)
            .merge(public_api_routes)
            .merge(protected_api_routes)
            .with_state(app_state)
            .layer(session_layer)
            .layer(axum_middleware::from_fn(middleware::security_headers))
            .layer(TraceLayer::new_for_http())
            .fallback_service(
                ServeDir::new(&static_dir).not_found_service(ServeFile::new(index_file)),
            );

        serve(app, None).await;
    }
}

async fn serve(
    app: Router,
    deletion_handle: Option<
        tokio::task::JoinHandle<Result<(), tower_sessions::session_store::Error>>,
    >,
) {
    let addr = SocketAddr::from(([0, 0, 0, 0], 8000));
    tracing::info!("listening on {addr}");

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .unwrap_or_else(|e| {
            tracing::error!(error = %e, "failed to bind on {addr}");
            std::process::exit(1);
        });

    if let Some(deletion_handle) = deletion_handle {
        tokio::select! {
            result = axum::serve(listener, app) => {
                if let Err(e) = result {
                    tracing::error!(error = %e, "server error");
                    std::process::exit(1);
                }
            }
            result = deletion_handle => {
                match result {
                    Ok(Ok(())) => tracing::error!("session deletion task exited unexpectedly"),
                    Ok(Err(e)) => tracing::error!(error = %e, "session deletion task failed"),
                    Err(e) => tracing::error!(error = %e, "session deletion task panicked"),
                }
                std::process::exit(1);
            }
        }
    } else if let Err(e) = axum::serve(listener, app).await {
        tracing::error!(error = %e, "server error");
        std::process::exit(1);
    }
}

/// Initialize the corpus: load local sources, then fetch only the
/// favorites that are missing from GitHub sources.
async fn init_corpus(static_dir: &str) -> CorpusState {
    let manifest_str =
        env::var("CORPUS_REGISTRY_PATH").unwrap_or_else(|_| "corpus-registry.yaml".to_string());
    let local_str = env::var("CORPUS_REGISTRY_LOCAL_PATH")
        .unwrap_or_else(|_| "corpus-registry.local.yaml".to_string());
    let auth_str = env::var("CORPUS_AUTH_FILE").unwrap_or_else(|_| "corpus-auth.yaml".to_string());
    let manifest_path = PathBuf::from(&manifest_str);
    let local_path = PathBuf::from(&local_str);
    let auth_path = PathBuf::from(&auth_str);

    let registry = if manifest_path.exists() {
        match regelrecht_corpus::CorpusRegistry::load(&manifest_path, Some(&local_path)) {
            Ok(r) => {
                tracing::info!(sources = r.sources().len(), "loaded corpus registry");
                r
            }
            Err(e) => {
                tracing::warn!(error = %e, "failed to load corpus registry, using empty");
                empty_registry()
            }
        }
    } else {
        tracing::info!("no corpus-registry.yaml found, corpus endpoints will return empty results");
        empty_registry()
    };

    let favorites = load_favorites(static_dir);
    let auth_file = if auth_path.exists() {
        Some(auth_path.as_path())
    } else {
        None
    };

    let source_map = match registry.load_favorites_async(&favorites, auth_file).await {
        Ok(map) => {
            tracing::info!(laws = map.len(), "loaded corpus laws");
            map
        }
        Err(e) => {
            tracing::warn!(error = %e, "failed to load favorites from GitHub, falling back to local-only");
            match registry.load_local_sources() {
                Ok(map) => {
                    tracing::info!(laws = map.len(), "loaded corpus laws (local-only fallback)");
                    map
                }
                Err(e2) => {
                    tracing::warn!(error = %e2, "failed to load local sources");
                    regelrecht_corpus::SourceMap::new()
                }
            }
        }
    };

    let backends = init_backends(&registry, auth_file).await;

    CorpusState {
        registry,
        source_map,
        backends,
        auth_file: auth_file.map(|p| p.to_path_buf()),
    }
}

/// Create and initialize backends for each registered source.
///
/// All successfully-initialised backends are registered, including read-only
/// ones (e.g. a local source on a read-only container filesystem). Reads
/// route through the same backends as writes so the editor never has a
/// read/write path mismatch — see [`crate::state::BackendEntry::writable`].
async fn init_backends(
    registry: &regelrecht_corpus::CorpusRegistry,
    auth_file: Option<&std::path::Path>,
) -> HashMap<String, crate::state::BackendEntry> {
    let mut backends = HashMap::new();

    for source in registry.sources() {
        let token = regelrecht_corpus::auth::resolve_token_for_source(
            &source.id,
            source.auth_ref.as_deref(),
            auth_file,
        )
        .unwrap_or_else(|e| {
            tracing::warn!(source_id = %source.id, error = %e, "failed to resolve auth token");
            None
        });

        // When a push token is present, the backend will push commits to the
        // remote repo. This requires authentication on the write endpoints —
        // do NOT enable push tokens without adding auth middleware first.
        match regelrecht_corpus::backend::create_backend(source, token.as_deref()) {
            Ok(mut backend) => {
                if let Err(e) = backend.ensure_ready().await {
                    tracing::warn!(
                        source_id = %source.id,
                        error = %e,
                        "backend init failed, skipping registration"
                    );
                    continue;
                }
                let writable = backend.is_writable();
                tracing::info!(
                    source_id = %source.id,
                    writable,
                    "backend ready"
                );
                backends.insert(
                    source.id.clone(),
                    crate::state::BackendEntry {
                        backend: Arc::new(Mutex::new(backend)),
                        writable,
                    },
                );
            }
            Err(e) => {
                tracing::warn!(
                    source_id = %source.id,
                    error = %e,
                    "failed to create backend"
                );
            }
        }
    }

    backends
}

/// Read favorites.json from the static directory.
fn load_favorites(static_dir: &str) -> HashSet<String> {
    let path = PathBuf::from(static_dir).join("favorites.json");
    match std::fs::read_to_string(&path) {
        Ok(content) => match serde_json::from_str::<Vec<String>>(&content) {
            Ok(ids) => {
                tracing::info!(count = ids.len(), "loaded favorites");
                ids.into_iter().collect()
            }
            Err(e) => {
                tracing::warn!(error = %e, "failed to parse favorites.json");
                HashSet::new()
            }
        },
        Err(_) => {
            tracing::info!("no favorites.json found");
            HashSet::new()
        }
    }
}

fn empty_registry() -> regelrecht_corpus::CorpusRegistry {
    regelrecht_corpus::CorpusRegistry::from_yaml("schema_version: '1.0'\nsources: []\n")
        .unwrap_or_else(|_| unreachable!())
}

/// Resolve the pipeline-api URL, preferring pod HOSTNAME over environment
/// override.
///
/// Priority: `HOSTNAME` → `PIPELINE_API_URL` → `None`.
///
/// HOSTNAME wins because ZAD's alias-based env injection is resolved at
/// component-creation time and gets stuck with stale values when the
/// pipelineapi component is renamed or its port changes. The pod hostname
/// is bound by Kubernetes and also cannot be inherited via ZAD's
/// `clone-from`, so it is the only reliable "which deployment am I?" signal
/// — matching the pattern `regelrecht_corpus::deployment_from_hostname`
/// uses for corpus branch resolution (see PR #574 for the corpus rationale).
///
/// `PIPELINE_API_URL` remains as an explicit override for local dev where
/// HOSTNAME doesn't match the `{deployment}-{component}-{rs}-{pod}` shape.
///
/// Edge case: a dev machine whose HOSTNAME happens to match that shape *and*
/// starts with `regelrecht` or `pr<N>` would silently derive a cluster-internal
/// URL that won't resolve locally. `deployment_from_hostname`'s whitelist is
/// the sole guard here; the old `KUBERNETES_SERVICE_HOST` gate is gone.
fn resolve_pipeline_api_url(hostname: Option<&str>, env_url: Option<String>) -> Option<String> {
    hostname
        .and_then(regelrecht_corpus::deployment_from_hostname)
        .map(|deployment| format!("http://{deployment}-pipelineapi:8000"))
        .or(env_url)
}

#[cfg(test)]
mod pipeline_api_url_tests {
    use super::resolve_pipeline_api_url;

    #[test]
    fn prod_pod_hostname_derives_regelrecht_pipelineapi() {
        let url = resolve_pipeline_api_url(Some("regelrecht-editor-abc-xyz"), None);
        assert_eq!(url.as_deref(), Some("http://regelrecht-pipelineapi:8000"));
    }

    #[test]
    fn pr_preview_pod_hostname_derives_pr_pipelineapi() {
        let url = resolve_pipeline_api_url(Some("pr123-editor-abc-xyz"), None);
        assert_eq!(url.as_deref(), Some("http://pr123-pipelineapi:8000"));
    }

    /// Regression test: even with a stale `PIPELINE_API_URL` shadowing the
    /// resolution (e.g. ZAD alias injection leftover pointing at an old
    /// `pipelineapi-pr552:8001`), HOSTNAME must still win.
    #[test]
    fn hostname_wins_over_stale_env_var() {
        let url = resolve_pipeline_api_url(
            Some("regelrecht-editor-abc-xyz"),
            Some("http://pipelineapi-pr552:8001".to_string()),
        );
        assert_eq!(url.as_deref(), Some("http://regelrecht-pipelineapi:8000"));
    }

    #[test]
    fn dev_hostname_falls_back_to_env_override() {
        let url = resolve_pipeline_api_url(
            Some("tim-laptop"),
            Some("http://localhost:8001".to_string()),
        );
        assert_eq!(url.as_deref(), Some("http://localhost:8001"));
    }

    /// A non-whitelisted pod-shaped hostname (≥3 hyphens but first segment is
    /// not `regelrecht` or `pr<N>`) must not be trusted as a deployment name —
    /// `deployment_from_hostname` returns `None` and we fall through to the
    /// env override. Documents the whitelist boundary explicitly.
    #[test]
    fn non_whitelisted_pod_hostname_falls_back_to_env() {
        let url = resolve_pipeline_api_url(
            Some("feature-editor-abc-xyz"),
            Some("http://localhost:8001".to_string()),
        );
        assert_eq!(url.as_deref(), Some("http://localhost:8001"));
    }

    #[test]
    fn no_hostname_and_no_env_returns_none() {
        assert!(resolve_pipeline_api_url(None, None).is_none());
    }

    #[test]
    fn no_hostname_uses_env_override() {
        let url = resolve_pipeline_api_url(None, Some("http://localhost:8001".to_string()));
        assert_eq!(url.as_deref(), Some("http://localhost:8001"));
    }
}
