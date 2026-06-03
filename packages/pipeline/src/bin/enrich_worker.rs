use regelrecht_pipeline::config::WorkerConfig;
use regelrecht_pipeline::db;
use regelrecht_pipeline::health::spawn_health_server;
use regelrecht_pipeline::worker::{run_enrich_worker, run_suggest_worker};

#[tokio::main]
async fn main() {
    regelrecht_shared::telemetry::init_subscriber("info");

    let config = match WorkerConfig::from_env() {
        Ok(config) => config,
        Err(e) => {
            tracing::error!(error = %e, "failed to load configuration");
            std::process::exit(1);
        }
    };

    let health_pool = match db::create_pool(&config.pipeline_config()).await {
        Ok(pool) => pool,
        Err(e) => {
            tracing::error!(error = %e, "failed to create DB pool for health check");
            std::process::exit(1);
        }
    };

    let health_handle = match spawn_health_server(Some(health_pool)).await {
        Ok(h) => h,
        Err(e) => {
            tracing::error!(error = %e, "failed to bind health endpoint");
            std::process::exit(1);
        }
    };

    // Run the editor-suggestion worker alongside enrich in the same binary, so
    // suggestions share this component's deployment, secrets, and baked-in
    // skills without a separate ZAD component.
    let suggest_config = config.clone();

    tokio::select! {
        result = run_enrich_worker(config) => {
            if let Err(e) = result {
                tracing::error!(error = %e, "enrich worker exited with error");
                std::process::exit(1);
            }
        }
        result = run_suggest_worker(suggest_config) => {
            if let Err(e) = result {
                tracing::error!(error = %e, "suggest worker exited with error");
                std::process::exit(1);
            }
        }
        result = health_handle => {
            match result {
                Ok(_) => tracing::error!("health endpoint task exited unexpectedly"),
                Err(e) => tracing::error!(error = %e, "health endpoint task panicked"),
            }
            std::process::exit(1);
        }
    }
}
