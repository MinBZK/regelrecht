//! CLI entry point for the harvester.

use regelrecht_harvester::cli;
use tracing_subscriber::EnvFilter;

fn main() {
    // Initialize tracing with WARN level by default, respecting RUST_LOG
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn")),
        )
        .with_target(false)
        .init();

    if let Err(e) = cli::run() {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}
