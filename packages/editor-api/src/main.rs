use std::collections::HashSet;
use std::env;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use axum::middleware as axum_middleware;
use axum::routing::get;
use axum::Router;
use tokio::sync::RwLock;
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;

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

    let static_dir = env::var("STATIC_DIR").unwrap_or_else(|_| "static".to_string());
    let corpus_state = init_corpus(&static_dir).await;

    let app_state = AppState {
        corpus: Arc::new(RwLock::new(corpus_state)),
    };

    let index_file = PathBuf::from(&static_dir).join("index.html");

    let api_routes = Router::new()
        .route("/api/sources", get(corpus_handlers::list_sources))
        .route("/api/corpus/laws", get(corpus_handlers::list_corpus_laws))
        .route(
            "/api/corpus/laws/{law_id}",
            get(corpus_handlers::get_corpus_law),
        );

    let app = Router::new()
        .route("/health", get(|| async { "OK" }))
        .merge(api_routes)
        .with_state(app_state)
        .layer(axum_middleware::from_fn(middleware::security_headers))
        .layer(TraceLayer::new_for_http())
        .fallback_service(ServeDir::new(&static_dir).not_found_service(ServeFile::new(index_file)));

    let addr = SocketAddr::from(([0, 0, 0, 0], 8000));
    tracing::info!("listening on {addr}");

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .unwrap_or_else(|e| {
            tracing::error!(error = %e, "failed to bind on {addr}");
            std::process::exit(1);
        });

    if let Err(e) = axum::serve(listener, app).await {
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

    CorpusState {
        registry,
        source_map,
    }
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
