use std::env;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::State;
use axum::http::StatusCode;
use axum::middleware as axum_middleware;
use axum::routing::{delete, get, post};
use axum::Router;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;
use tower_sessions::cookie::SameSite;
use tower_sessions::{ExpiredDeletion, Expiry, SessionManagerLayer};
use tower_sessions_sqlx_store::PostgresStore;
use tracing_subscriber::EnvFilter;

mod auth;
mod config;
mod corpus_handlers;
mod handlers;
mod metrics;
mod middleware;
mod models;
mod oidc;
mod state;

use config::AppConfig;
use state::AppState;

const ACQUIRE_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_RETRIES: u32 = 10;
const RETRY_INTERVAL: Duration = Duration::from_secs(3);

async fn health(State(state): State<AppState>) -> Result<&'static str, StatusCode> {
    sqlx::query_scalar::<_, i32>("SELECT 1")
        .fetch_one(&state.pool)
        .await
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
    Ok("OK")
}

fn main() {
    // Map CORPUS_GIT_TOKEN → CORPUS_AUTH_CENTRAL_TOKEN before spawning any threads.
    // This avoids `unsafe { env::set_var }` inside the async runtime.
    if env::var("CORPUS_AUTH_CENTRAL_TOKEN").is_err() {
        if let Ok(token) = env::var("CORPUS_GIT_TOKEN") {
            // SAFETY: no other threads exist yet — the tokio runtime hasn't started.
            unsafe { env::set_var("CORPUS_AUTH_CENTRAL_TOKEN", &token) };
        }
    }

    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("failed to build tokio runtime: {e}");
            std::process::exit(1);
        }
    };
    runtime.block_on(async_main());
}

async fn async_main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let app_config = AppConfig::from_env();

    let database_url = match env::var("DATABASE_URL").or_else(|_| env::var("DATABASE_SERVER_FULL"))
    {
        Ok(url) => url,
        Err(_) => {
            tracing::error!("DATABASE_URL or DATABASE_SERVER_FULL environment variable is not set");
            std::process::exit(1);
        }
    };

    tracing::info!("connecting to database...");

    let mut pool = None;
    for attempt in 1..=MAX_RETRIES {
        match PgPoolOptions::new()
            .max_connections(5)
            .acquire_timeout(ACQUIRE_TIMEOUT)
            .connect(&database_url)
            .await
        {
            Ok(p) => {
                pool = Some(p);
                break;
            }
            Err(e) => {
                tracing::warn!(attempt, error = %e, "failed to connect, retrying...");
                if attempt == MAX_RETRIES {
                    tracing::error!("exhausted all {MAX_RETRIES} connection attempts");
                    std::process::exit(1);
                }
                tokio::time::sleep(RETRY_INTERVAL).await;
            }
        }
    }
    let pool: PgPool = pool.unwrap_or_else(|| {
        tracing::error!("unreachable: no pool after retry loop");
        std::process::exit(1);
    });

    tracing::info!("connected to database");

    if let Err(e) = regelrecht_pipeline::ensure_schema(&pool).await {
        tracing::error!(error = %e, "database migration failed");
        std::process::exit(1);
    }

    let (oidc_client, end_session_url) = if let Some(ref oidc_config) = app_config.oidc {
        match oidc::discover_client(oidc_config).await {
            Ok(result) => (Some(Arc::new(result.client)), result.end_session_url),
            Err(e) => {
                tracing::error!(error = %e, "OIDC discovery failed");
                std::process::exit(1);
            }
        }
    } else {
        (None, None)
    };

    let session_store = PostgresStore::new(pool.clone());
    if let Err(e) = session_store.migrate().await {
        tracing::error!(error = %e, "failed to create session table");
        std::process::exit(1);
    }
    tracing::info!("session store ready (PostgreSQL-backed)");

    let _deletion_task = tokio::task::spawn(
        session_store
            .clone()
            .continuously_delete_expired(tokio::time::Duration::from_secs(60)),
    );

    // Initialize corpus registry (with async GitHub source loading)
    let corpus_state = init_corpus().await;

    let app_state = AppState {
        pool,
        oidc_client,
        end_session_url,
        config: Arc::new(app_config),
        metrics_cache: Arc::new(metrics::new_cache()),
        corpus: Arc::new(tokio::sync::RwLock::new(corpus_state)),
    };

    let session_layer = SessionManagerLayer::new(session_store)
        .with_expiry(Expiry::OnInactivity(time::Duration::hours(8)))
        .with_same_site(SameSite::Lax)
        .with_http_only(true)
        .with_secure(app_state.config.is_auth_enabled());

    // CORS configuration for public corpus endpoints
    let cors_layer = build_cors_layer();

    // Protected API routes (behind auth)
    let protected_routes = Router::new()
        .route("/api/law_entries", get(handlers::list_law_entries))
        .route("/api/jobs", get(handlers::list_jobs))
        .route("/api/jobs/{job_id}", get(handlers::get_job))
        .route("/api/harvest-jobs", post(handlers::create_harvest_job))
        .route("/api/enrich-jobs", post(handlers::create_enrich_jobs))
        .route("/api/jobs", delete(handlers::delete_all_jobs))
        .route(
            "/api/sources/{source_id}/sync",
            post(corpus_handlers::sync_source),
        )
        .route_layer(axum_middleware::from_fn_with_state(
            app_state.clone(),
            middleware::require_auth,
        ));

    // Public corpus read endpoints (with CORS for cross-origin editor access)
    let corpus_routes = Router::new()
        .route("/api/sources", get(corpus_handlers::list_sources))
        .route("/api/corpus/laws", get(corpus_handlers::list_corpus_laws))
        .route(
            "/api/corpus/laws/{law_id}",
            get(corpus_handlers::get_corpus_law),
        )
        .layer(cors_layer);

    let auth_routes = Router::new()
        .route("/auth/login", get(auth::login))
        .route("/auth/callback", get(auth::callback))
        .route("/auth/logout", get(auth::logout))
        .route("/auth/status", get(auth::status));

    let app = Router::new()
        .route("/health", get(health))
        .route("/metrics", get(metrics::metrics_handler))
        .route("/api/info", get(handlers::platform_info))
        .merge(auth_routes)
        .merge(protected_routes)
        .merge(corpus_routes)
        .with_state(app_state)
        .layer(session_layer)
        .layer(axum_middleware::from_fn(middleware::security_headers))
        .layer(TraceLayer::new_for_http())
        .fallback_service(ServeDir::new(
            env::var("STATIC_DIR").unwrap_or_else(|_| "static".to_string()),
        ));

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

/// Build a CORS layer for public corpus endpoints.
///
/// Allowed origins are configured via `CORS_ALLOWED_ORIGINS` (comma-separated).
/// If not set, defaults to permissive CORS to allow any editor deployment.
fn build_cors_layer() -> CorsLayer {
    use axum::http::Method;

    let layer = CorsLayer::new().allow_methods([Method::GET]);

    match env::var("CORS_ALLOWED_ORIGINS") {
        Ok(origins) if !origins.is_empty() => {
            let parsed: Vec<_> = origins
                .split(',')
                .filter_map(|o| o.trim().parse().ok())
                .collect();
            tracing::info!(origins = ?parsed, "CORS: configured allowed origins");
            layer.allow_origin(parsed)
        }
        _ => {
            tracing::info!("CORS: no CORS_ALLOWED_ORIGINS set, allowing any origin");
            layer.allow_origin(tower_http::cors::Any)
        }
    }
}

/// Initialize the corpus registry and load all sources (local + GitHub).
///
/// Registry file paths can be configured via environment variables:
/// - `CORPUS_REGISTRY_PATH` (default: `corpus-registry.yaml`)
/// - `CORPUS_REGISTRY_LOCAL_PATH` (default: `corpus-registry.local.yaml`)
/// - `CORPUS_AUTH_PATH` (optional, path to `corpus-auth.yaml`)
///
/// For GitHub sources, token resolution uses `CORPUS_AUTH_{SOURCE_ID}_TOKEN` env vars.
/// The `CORPUS_GIT_TOKEN` → `CORPUS_AUTH_CENTRAL_TOKEN` mapping is handled in `main()`
/// before the tokio runtime starts.
async fn init_corpus() -> state::CorpusState {
    let manifest_str =
        env::var("CORPUS_REGISTRY_PATH").unwrap_or_else(|_| "corpus-registry.yaml".to_string());
    let local_str = env::var("CORPUS_REGISTRY_LOCAL_PATH")
        .unwrap_or_else(|_| "corpus-registry.local.yaml".to_string());
    let manifest_path = std::path::PathBuf::from(&manifest_str);
    let local_path = std::path::PathBuf::from(&local_str);

    let registry = if manifest_path.exists() {
        match regelrecht_corpus::CorpusRegistry::load(&manifest_path, Some(&local_path)) {
            Ok(r) => {
                tracing::info!(sources = r.sources().len(), "Loaded corpus registry");
                r
            }
            Err(e) => {
                tracing::warn!(error = %e, "Failed to load corpus registry, using empty");
                regelrecht_corpus::CorpusRegistry::from_yaml("schema_version: '1.0'\nsources: []\n")
                    .unwrap_or_else(|_| {
                        // This YAML is hardcoded and always valid
                        unreachable!()
                    })
            }
        }
    } else {
        tracing::info!("No corpus-registry.yaml found, corpus endpoints will return empty results");
        regelrecht_corpus::CorpusRegistry::from_yaml("schema_version: '1.0'\nsources: []\n")
            .unwrap_or_else(|_| unreachable!())
    };

    let auth_file = env::var("CORPUS_AUTH_PATH")
        .ok()
        .map(std::path::PathBuf::from);

    // Try async load (local + GitHub); fall back to local-only on failure
    let source_map = match registry.load_all_sources_async(auth_file.as_deref()).await {
        Ok(map) => {
            tracing::info!(
                laws = map.len(),
                "Loaded all corpus sources (local + GitHub)"
            );
            map
        }
        Err(e) => {
            tracing::warn!(error = %e, "Failed to load all sources, falling back to local-only");
            match registry.load_local_sources() {
                Ok(map) => {
                    tracing::info!(laws = map.len(), "Loaded local corpus sources (fallback)");
                    map
                }
                Err(e2) => {
                    tracing::warn!(error = %e2, "Failed to load local corpus sources");
                    regelrecht_corpus::SourceMap::new()
                }
            }
        }
    };

    state::CorpusState {
        registry,
        source_map,
    }
}
