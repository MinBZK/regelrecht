use tracing_subscriber::EnvFilter;

use regelrecht_pipeline::config::WorkerConfig;
use regelrecht_pipeline::db;
use regelrecht_pipeline::worker::run_enrich_worker;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let config = match WorkerConfig::from_env() {
        Ok(config) => config,
        Err(e) => {
            tracing::error!(error = %e, "failed to load configuration");
            std::process::exit(1);
        }
    };

    // Create a pool for the health check endpoint
    let health_pool = match db::create_pool(&config.pipeline_config()).await {
        Ok(pool) => pool,
        Err(e) => {
            tracing::error!(error = %e, "failed to create DB pool for health check");
            std::process::exit(1);
        }
    };

    let health_port = std::env::var("HEALTH_PORT")
        .ok()
        .and_then(|v| v.parse::<u16>().ok())
        .unwrap_or(8000);

    let bind_addr = format!("0.0.0.0:{health_port}");
    let listener = match tokio::net::TcpListener::bind(&bind_addr).await {
        Ok(l) => {
            tracing::info!("health endpoint listening on {bind_addr}");
            l
        }
        Err(e) => {
            tracing::error!(error = %e, "failed to bind health endpoint on {bind_addr}");
            std::process::exit(1);
        }
    };

    tokio::spawn(async move {
        loop {
            if let Ok((mut stream, _)) = listener.accept().await {
                use tokio::io::AsyncWriteExt;
                // Check database connectivity before reporting healthy
                let response =
                    match sqlx::query_scalar::<_, i32>("SELECT 1").fetch_one(&health_pool).await {
                        Ok(_) => {
                            b"HTTP/1.1 200 OK\r\nConnection: close\r\nContent-Length: 2\r\n\r\nOK"
                                as &[u8]
                        }
                        Err(_) => {
                            b"HTTP/1.1 503 Service Unavailable\r\nConnection: close\r\nContent-Length: 13\r\n\r\nDB unreachable"
                        }
                    };
                let _ = stream.write_all(response).await;
            }
        }
    });

    if let Err(e) = run_enrich_worker(config).await {
        tracing::error!(error = %e, "enrich worker exited with error");
        std::process::exit(1);
    }
}
