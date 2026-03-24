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

    let corpus_state = init_corpus();

    let app_state = AppState {
        corpus: Arc::new(RwLock::new(corpus_state)),
    };

    let static_dir = env::var("STATIC_DIR").unwrap_or_else(|_| "static".to_string());
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

/// Initialize the corpus registry and load local sources only.
///
/// GitHub sources are skipped — the editor serves only the laws bundled
/// in the Docker image. This avoids burning GitHub API rate limits on
/// thousands of files that aren't shown in the UI.
fn init_corpus() -> CorpusState {
    let manifest_str =
        env::var("CORPUS_REGISTRY_PATH").unwrap_or_else(|_| "corpus-registry.yaml".to_string());
    let local_str = env::var("CORPUS_REGISTRY_LOCAL_PATH")
        .unwrap_or_else(|_| "corpus-registry.local.yaml".to_string());
    let manifest_path = PathBuf::from(&manifest_str);
    let local_path = PathBuf::from(&local_str);

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

    let source_map = match registry.load_local_sources() {
        Ok(map) => {
            tracing::info!(laws = map.len(), "loaded corpus laws");
            map
        }
        Err(e) => {
            tracing::warn!(error = %e, "failed to load local sources");
            regelrecht_corpus::SourceMap::new()
        }
    };

    CorpusState {
        registry,
        source_map,
    }
}

fn empty_registry() -> regelrecht_corpus::CorpusRegistry {
    regelrecht_corpus::CorpusRegistry::from_yaml("schema_version: '1.0'\nsources: []\n")
        .unwrap_or_else(|_| {
            // This YAML is hardcoded and always valid
            unreachable!()
        })
}
