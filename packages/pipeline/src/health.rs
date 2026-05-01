//! Health endpoint helper shared by worker binaries.
//!
//! RIG (Quattro/rijksapps) probes a tiny HTTP server inside each worker pod;
//! when the bind fails the pod restart-loops, so binding is treated as fatal
//! at startup. With `pool` provided the endpoint also runs a `SELECT 1`
//! before reporting OK so a DB outage trips the liveness probe.

use std::io;

use sqlx::PgPool;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

const OK_RESPONSE: &[u8] = b"HTTP/1.1 200 OK\r\nConnection: close\r\nContent-Length: 2\r\n\r\nOK";
const DB_UNAVAILABLE_RESPONSE: &[u8] =
    b"HTTP/1.1 503 Service Unavailable\r\nConnection: close\r\nContent-Length: 14\r\n\r\nDB unreachable";

/// Resolve the health endpoint port from `HEALTH_PORT`, falling back to 8000.
pub fn health_port() -> u16 {
    std::env::var("HEALTH_PORT")
        .ok()
        .and_then(|v| v.parse::<u16>().ok())
        .unwrap_or(8000)
}

/// Bind the health endpoint and spawn its accept loop. The bind is performed
/// before returning so the caller can `std::process::exit(1)` on failure
/// before any heavyweight worker setup.
///
/// If `pool` is supplied, each request runs `SELECT 1` and replies 503 on DB
/// failure; otherwise the endpoint always replies 200 OK.
pub async fn spawn_health_server(pool: Option<PgPool>) -> io::Result<JoinHandle<()>> {
    let port = health_port();
    let bind_addr = format!("0.0.0.0:{port}");
    let listener = TcpListener::bind(&bind_addr).await?;
    tracing::info!("health endpoint listening on {bind_addr}");

    Ok(tokio::spawn(async move {
        loop {
            let Ok((mut stream, _)) = listener.accept().await else {
                continue;
            };
            let response = match &pool {
                Some(p) => match sqlx::query_scalar::<_, i32>("SELECT 1").fetch_one(p).await {
                    Ok(_) => OK_RESPONSE,
                    Err(_) => DB_UNAVAILABLE_RESPONSE,
                },
                None => OK_RESPONSE,
            };
            let _ = stream.write_all(response).await;
        }
    }))
}
