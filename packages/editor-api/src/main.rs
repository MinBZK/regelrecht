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
mod middleware;
mod state;

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

    let app_state = AppState {
        corpus: Arc::new(RwLock::new(corpus_state)),
        oidc_client,
        end_session_url,
        config: Arc::new(app_config),
        http_client,
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
            "/api/corpus/laws/{law_id}/scenarios",
            get(corpus_handlers::list_scenarios),
        )
        .route(
            "/api/corpus/laws/{law_id}/scenarios/{filename}",
            get(corpus_handlers::get_scenario),
        );

    // Protected API routes — require authentication when OIDC is enabled.
    // Write endpoints (PUT/DELETE) for scenarios live here so they cannot be
    // invoked anonymously when a deployment has a git push token configured.
    let protected_api_routes = Router::new()
        .route(
            "/api/corpus/laws/{law_id}/scenarios/{filename}",
            axum::routing::put(corpus_handlers::save_scenario)
                .delete(corpus_handlers::delete_scenario),
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
    }
}

/// Create and initialize write backends for each registered source.
async fn init_backends(
    registry: &regelrecht_corpus::CorpusRegistry,
    auth_file: Option<&std::path::Path>,
) -> HashMap<String, Arc<Mutex<Box<dyn regelrecht_corpus::backend::RepoBackend>>>> {
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
                        "backend init failed, writes disabled for this source"
                    );
                    continue;
                }
                if !backend.is_writable() {
                    tracing::info!(
                        source_id = %source.id,
                        "backend is read-only, skipping"
                    );
                    continue;
                }
                tracing::info!(source_id = %source.id, "write backend ready");
                backends.insert(source.id.clone(), Arc::new(Mutex::new(backend)));
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
