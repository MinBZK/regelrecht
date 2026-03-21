//! Shared helpers for integration tests.

use std::path::PathBuf;

/// Get the regulation base path from `REGULATION_PATH` env var or default relative path.
pub fn regulation_base_path() -> PathBuf {
    std::env::var("REGULATION_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join("..")
                .join("corpus")
                .join("regulation")
        })
}
