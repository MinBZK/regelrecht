//! The workspace, as cargo describes it: its crates and where the index goes.

use std::path::Path;

use crate::index::Paths;
use crate::model::CrateInput;

/// This tool is not part of what it describes.
const SELF_PACKAGE: &str = "regelrecht-code-guide";

pub struct Workspace {
    pub paths: Paths,
    pub crates: Vec<CrateInput>,
}

/// Reads the workspace with `cargo metadata` (from `packages/`, or the given
/// manifest).
pub fn load(manifest_path: Option<&Path>) -> Result<Workspace, String> {
    let mut cmd = cargo_metadata::MetadataCommand::new();
    cmd.no_deps();
    if let Some(p) = manifest_path {
        cmd.manifest_path(p);
    }
    let meta = cmd.exec().map_err(|e| format!("cargo metadata: {e}"))?;
    let root = meta.workspace_root.clone().into_std_path_buf();
    let members: std::collections::HashSet<_> = meta.workspace_members.iter().collect();
    let mut crates: Vec<CrateInput> = meta
        .packages
        .iter()
        .filter(|p| members.contains(&p.id) && p.name.as_str() != SELF_PACKAGE)
        .filter_map(|p| {
            let dir = p
                .manifest_path
                .parent()?
                .strip_prefix(&meta.workspace_root)
                .ok()?;
            Some(CrateInput {
                package: p.name.to_string(),
                name: p.name.trim_start_matches("regelrecht-").to_string(),
                dir: dir.as_str().replace('\\', "/"),
                description: p.description.clone(),
            })
        })
        .collect();
    crates.sort_by(|a, b| a.name.cmp(&b.name));
    let target = meta.target_directory.clone().into_std_path_buf();
    Ok(Workspace {
        paths: Paths::new(root, &target),
        crates,
    })
}
