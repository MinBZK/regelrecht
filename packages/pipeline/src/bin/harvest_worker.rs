use tracing_subscriber::EnvFilter;

use regelrecht_pipeline::config::WorkerConfig;
use regelrecht_pipeline::worker::run_harvest_worker;

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

    // Bind health endpoint before starting worker — failure is fatal because
    // RIG requires the liveprobe to be reachable; without it the pod restart-loops.
    let listener = match tokio::net::TcpListener::bind("0.0.0.0:8000").await {
        Ok(l) => {
            tracing::info!("health endpoint listening on 0.0.0.0:8000");
            l
        }
        Err(e) => {
            tracing::error!(error = %e, "failed to bind health endpoint on port 8000");
            std::process::exit(1);
        }
    };

    let health_handle = tokio::spawn(async move {
        loop {
            if let Ok((mut stream, _)) = listener.accept().await {
                use tokio::io::AsyncWriteExt;
                let _ = stream
                    .write_all(
                        b"HTTP/1.1 200 OK\r\nConnection: close\r\nContent-Length: 2\r\n\r\nOK",
                    )
                    .await;
            }
        }
    });

    tokio::select! {
        result = run_harvest_worker(config) => {
            if let Err(e) = result {
                tracing::error!(error = %e, "harvest worker exited with error");
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
