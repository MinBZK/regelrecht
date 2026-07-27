//! Shared model-building: turns the workspace into a [`Model`]. Both the
//! `generate` subcommand (writes the model to a file) and the `serve`
//! subcommand (regenerates it on-demand for the explorer) go through here, so
//! the extraction logic lives in exactly one place.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use walkdir::WalkDir;

use crate::crate_graph;
use crate::model::Model;
use crate::syn_pass;

/// Which crates get the deep source-level pass. The default is every crate:
/// the architecture explorer renders the deep structure (modules/types/methods)
/// for the whole workspace. `--deep a,b` narrows it to a subset (e.g. for a
/// quick run); `--deep-all` restores the default explicitly.
pub enum DeepScope {
    /// An explicit `--deep` list — only these crates get the deep pass.
    Only(Vec<String>),
    /// Every workspace crate (the default, also selectable with `--deep-all`).
    All,
}

/// Runs the crate-graph pass plus the (optional) deep source pass and returns
/// the assembled [`Model`] together with the discovered repo root.
pub fn build_model(
    manifest_path: Option<&Path>,
    deep: &DeepScope,
) -> Result<(Model, PathBuf), Box<dyn std::error::Error>> {
    let graph = crate_graph::load(manifest_path)?;

    let mut nodes = graph.nodes;
    let mut edges = graph.edges;
    for krate in &graph.crates {
        let deep_this = match deep {
            DeepScope::All => true,
            DeepScope::Only(list) => list.iter().any(|s| s == &krate.short),
        };
        if deep_this {
            syn_pass::extract_crate(&graph.repo_root, krate, &mut nodes, &mut edges);
        }
    }

    Ok((Model::new(nodes, edges), graph.repo_root))
}

/// Discovers the repo root cheaply (a `cargo metadata` run, no deep pass), so
/// `serve` can locate the source tree and the UI assets before it ever builds
/// the full model.
pub fn repo_root(manifest_path: Option<&Path>) -> Result<PathBuf, Box<dyn std::error::Error>> {
    Ok(crate_graph::load(manifest_path)?.repo_root)
}

/// Newest modification time across the workspace's Rust sources
/// (`packages/**/src/**/*.rs`). The `serve` cache keys on this: an unchanged
/// tree returns the same value and is served from cache, while any edit to a
/// source file bumps it and triggers a regeneration on the next request.
///
/// Returns `None` when no source file is found (e.g. a stripped checkout); the
/// caller treats that as "always rebuild".
pub fn latest_src_mtime(repo_root: &Path) -> Option<SystemTime> {
    let packages = repo_root.join("packages");
    let mut latest: Option<SystemTime> = None;
    // Prune build/vendor dirs up front: `target/` alone holds tens of thousands
    // of files, and walking it would make the cache check cost more than it
    // saves. None of them hold workspace `src/**/*.rs` sources.
    let is_pruned = |e: &walkdir::DirEntry| {
        matches!(
            e.file_name().to_str(),
            Some("target" | "node_modules" | ".git" | "dist")
        )
    };
    for entry in WalkDir::new(&packages)
        .into_iter()
        .filter_entry(|e| !e.file_type().is_dir() || !is_pruned(e))
        .flatten()
    {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        // Only files that live under a `src/` directory — mirrors the
        // `packages/**/src/**/*.rs` glob and skips `tests/`, `build.rs`, etc.
        if !path.components().any(|c| c.as_os_str() == "src") {
            continue;
        }
        // `walkdir::Error` and `std::io::Error` are distinct types, so the two
        // fallible steps can't share one `?`/`and_then`; take them separately.
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        let Ok(modified) = meta.modified() else {
            continue;
        };
        latest = Some(match latest {
            Some(current) if current >= modified => current,
            _ => modified,
        });
    }
    latest
}
