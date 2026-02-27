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

    tokio::spawn(async {
        let listener = match tokio::net::TcpListener::bind("0.0.0.0:8000").await {
            Ok(l) => {
                tracing::info!("health endpoint listening on 0.0.0.0:8000");
                l
            }
            Err(e) => {
                tracing::error!(error = %e, "failed to bind health endpoint on port 8000");
                return;
            }
        };
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

    if let Err(e) = run_harvest_worker(config).await {
        tracing::error!(error = %e, "harvest worker exited with error");
        std::process::exit(1);
    }
}
