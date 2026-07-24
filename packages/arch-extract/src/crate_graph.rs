//! Tier-1a extraction: the crate graph and internal path-dependencies, straight
//! from `cargo metadata`. This is the solid backbone of the model — the
//! well-known layer graph (`shared` → `law-model`/`auth` →
//! `engine`/`harvester`/`corpus` → `pipeline` → `admin`/`editor-api`/`tui`)
//! falls out of the normal (non-dev, non-build) path dependencies between
//! workspace members.

use cargo_metadata::{CargoOpt, DependencyKind, Metadata, MetadataCommand};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::model::{Edge, EdgeKind, Kind, Level, Node};

/// The tooling crate itself is not part of the product architecture, so it is
/// excluded from the model (keeping exactly the 10 product crates).
const SELF_CRATE: &str = "regelrecht-arch-extract";

/// A workspace member we will extract, with the bits later stages need.
pub struct CrateInfo {
    /// Short label used in node ids, e.g. `engine` (the `regelrecht-` prefix
    /// stripped). Matches the ids in the ticket (`crate:engine`).
    pub short: String,
    /// Stable node id, `crate:<short>`.
    pub node_id: String,
    /// Absolute crate directory (parent of Cargo.toml).
    pub dir: PathBuf,
}

pub struct CrateGraph {
    pub repo_root: PathBuf,
    pub crates: Vec<CrateInfo>,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
}

/// Strips the shared `regelrecht-` prefix to get the short label used in ids.
fn short_name(package_name: &str) -> String {
    package_name
        .strip_prefix("regelrecht-")
        .unwrap_or(package_name)
        .to_string()
}

/// Makes `p` relative to `root` for stable, machine-independent paths in the
/// model; falls back to the absolute path if it is not under the root.
pub fn rel(root: &Path, p: &Path) -> String {
    p.strip_prefix(root)
        .unwrap_or(p)
        .to_string_lossy()
        .replace('\\', "/")
}

/// Runs `cargo metadata` for the workspace whose manifest lives at
/// `<packages_manifest>` (or, when `None`, discovered from the current dir),
/// and builds the crate-level nodes and `depends-on` edges.
pub fn load(manifest_path: Option<&Path>) -> Result<CrateGraph, Box<dyn std::error::Error>> {
    let mut cmd = MetadataCommand::new();
    cmd.features(CargoOpt::AllFeatures);
    if let Some(mp) = manifest_path {
        cmd.manifest_path(mp);
    }
    let metadata: Metadata = cmd.exec()?;

    // `metadata.workspace_root` is `.../packages`; the repo root is its parent
    // so paths in the model read as `packages/engine/...`.
    let workspace_root = metadata.workspace_root.clone().into_std_path_buf();
    let repo_root = workspace_root
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or(workspace_root);

    // Workspace members only (exclude external deps and the tooling crate).
    let member_ids: std::collections::HashSet<_> =
        metadata.workspace_members.iter().cloned().collect();

    let mut crates: Vec<CrateInfo> = Vec::new();
    let mut by_name: BTreeMap<String, String> = BTreeMap::new(); // package name -> node id

    for pkg in &metadata.packages {
        if !member_ids.contains(&pkg.id) || pkg.name.as_str() == SELF_CRATE {
            continue;
        }
        let short = short_name(pkg.name.as_str());
        let node_id = format!("crate:{short}");
        let dir = pkg
            .manifest_path
            .parent()
            .map(|p| p.as_std_path().to_path_buf())
            .unwrap_or_else(|| repo_root.clone());
        by_name.insert(pkg.name.to_string(), node_id.clone());
        crates.push(CrateInfo {
            short,
            node_id,
            dir,
        });
    }
    crates.sort_by(|a, b| a.short.cmp(&b.short));

    let mut nodes: Vec<Node> = Vec::new();
    let mut edges: Vec<Edge> = Vec::new();

    for pkg in &metadata.packages {
        let Some(node_id) = by_name.get(pkg.name.as_str()) else {
            continue;
        };
        let dir = pkg
            .manifest_path
            .parent()
            .map(|p| p.as_std_path().to_path_buf())
            .unwrap_or_else(|| repo_root.clone());

        nodes.push(Node {
            id: node_id.clone(),
            level: Level::Container,
            kind: Kind::Crate,
            lang: "rust".to_string(),
            name: short_name(pkg.name.as_str()),
            path: rel(&repo_root, &dir),
            parent: None,
            doc: pkg.description.clone(),
        });

        // Internal normal dependencies become `depends-on` edges. Restricting
        // to `Normal` (skipping dev/build deps) keeps the graph the clean
        // production layer graph the acceptance criteria check against.
        for dep in &pkg.dependencies {
            if dep.kind != DependencyKind::Normal {
                continue;
            }
            if let Some(to) = by_name.get(dep.name.as_str()) {
                edges.push(Edge {
                    from: node_id.clone(),
                    to: to.clone(),
                    kind: EdgeKind::DependsOn,
                });
            }
        }
    }

    Ok(CrateGraph {
        repo_root,
        crates,
        nodes,
        edges,
    })
}
