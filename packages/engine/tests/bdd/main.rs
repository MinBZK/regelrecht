//! BDD Test Runner for RegelRecht Engine
//!
//! Runs Cucumber/Gherkin tests using the same feature files as the Python implementation.
//!
//! # Usage
//!
//! ```bash
//! cargo test --test bdd -- --nocapture
//! ```
//!
//! Or via just:
//!
//! ```bash
//! just rust-bdd
//! ```

// Allow panic/expect in test code - these are appropriate for test setup
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

mod helpers;
mod steps;
mod world;

use cucumber::World;
use std::path::Path;

#[tokio::main]
async fn main() {
    // Initialize tracing subscriber (respects RUST_LOG env var)
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_test_writer()
        .init();

    // Find the features directory relative to the package
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let features_dir = Path::new(manifest_dir)
        .parent() // packages/
        .and_then(|p| p.parent()) // project root
        .map(|p| p.join("features"))
        .expect("Could not find features directory");

    if !features_dir.exists() {
        panic!("Features directory not found: {}", features_dir.display());
    }

    // Run cucumber with all features
    // run_and_exit() exits with non-zero code on test failures (unlike run())
    world::RegelrechtWorld::cucumber()
        .max_concurrent_scenarios(1) // Run scenarios sequentially for predictable state
        .with_default_cli()
        .run_and_exit(features_dir)
        .await;
}
