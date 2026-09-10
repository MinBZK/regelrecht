//! Regulation loader for BDD tests
//!
//! Loads all YAML regulation files from every jurisdiction directory under
//! `corpus/regulation/` (e.g. `nl/`, `lu/`, `eu/`).

use crate::common::regulation_base_path;
use regelrecht_engine::{EngineError, LawExecutionService};
use walkdir::WalkDir;

/// Load all regulation YAML files into the service.
///
/// Scans each jurisdiction subdirectory of `corpus/regulation/` (or
/// `REGULATION_PATH`) — historically only `nl/`, now also `lu/`, `eu/`, etc. —
/// and loads all `.yaml` files found.
pub fn load_all_regulations(service: &mut LawExecutionService) -> Result<usize, EngineError> {
    let regulation_root = regulation_base_path();

    if !regulation_root.exists() {
        return Err(EngineError::LoadError(format!(
            "Regulation directory not found: {}",
            regulation_root.display()
        )));
    }

    let mut roots = Vec::new();
    let entries = std::fs::read_dir(&regulation_root).map_err(|e| {
        EngineError::LoadError(format!(
            "Failed to read {}: {}",
            regulation_root.display(),
            e
        ))
    })?;
    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.is_dir() {
            roots.push(path);
        }
    }
    // Fallback: REGULATION_PATH may already point at a single-jurisdiction tree
    // (e.g. a checkout that only has wet/… at the top level).
    if roots.is_empty() {
        roots.push(regulation_root);
    }

    let mut count = 0;

    for regulation_dir in roots {
        for entry in WalkDir::new(&regulation_dir)
            .follow_links(true)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();

            // Only process YAML files
            if path.is_file() && path.extension().is_some_and(|ext| ext == "yaml") {
                let content = std::fs::read_to_string(path).map_err(|e| {
                    EngineError::LoadError(format!("Failed to read {}: {}", path.display(), e))
                })?;

                match service.load_law(&content) {
                    Ok(law_id) => {
                        tracing::debug!(law_id = %law_id, path = %path.display(), "Loaded law");
                        count += 1;
                    }
                    Err(e) => {
                        tracing::warn!(
                            path = %path.display(),
                            error = %e,
                            "Failed to load law file (skipping)"
                        );
                        // Continue loading other files even if one fails
                    }
                }
            }
        }
    }

    tracing::info!(count = count, "Loaded regulations");
    Ok(count)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use super::load_all_regulations;
    use regelrecht_engine::LawExecutionService;

    #[test]
    fn test_load_all_regulations() {
        let mut service = LawExecutionService::new();
        let count = load_all_regulations(&mut service).expect("Failed to load regulations");
        assert!(count > 0, "Expected to load at least one regulation");
    }

    #[test]
    fn test_specific_laws_loaded() {
        let mut service = LawExecutionService::new();
        load_all_regulations(&mut service).expect("Failed to load regulations");

        // Check that key laws are loaded
        assert!(
            service.has_law("participatiewet"),
            "participatiewet should be loaded"
        );
        assert!(
            service.has_law("burgerlijk_wetboek_boek_5"),
            "BW5 should be loaded"
        );
    }
}
