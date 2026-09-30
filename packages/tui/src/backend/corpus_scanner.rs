use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Debug, Clone)]
pub struct CorpusNode {
    pub path: PathBuf,
    pub name: String,
    pub depth: usize,
    pub is_dir: bool,
}

/// Law header metadata. Re-exported from the shared `regelrecht-law-model`
/// crate so the tolerant header parser is defined in exactly one place.
pub use regelrecht_law_model::LawHeader as LawMetadata;

/// Scan the corpus directory and return a flat list of nodes for tree rendering.
pub fn scan_corpus(corpus_root: &Path) -> Vec<CorpusNode> {
    let mut nodes = Vec::new();

    let regulation_dir = find_regulation_dir(corpus_root);
    let base = match regulation_dir {
        Some(ref d) => d.as_path(),
        None => return nodes,
    };

    let base_depth = base.components().count();

    for entry in WalkDir::new(base).sort_by_file_name().into_iter().flatten() {
        let path = entry.path().to_path_buf();
        let depth = path.components().count().saturating_sub(base_depth);

        // Skip the root itself
        if depth == 0 {
            continue;
        }

        let name = entry.file_name().to_string_lossy().to_string();

        nodes.push(CorpusNode {
            path,
            name,
            depth: depth.saturating_sub(1),
            is_dir: entry.file_type().is_dir(),
        });
    }

    nodes
}

/// Extract metadata from a YAML law file without full deserialization.
///
/// Delegates to the shared tolerant header parser in `regelrecht-law-model`.
pub fn extract_metadata(content: &str) -> LawMetadata {
    regelrecht_law_model::parse_law_header(content)
}

/// Find the corpus regulation directory by checking common locations.
fn find_regulation_dir(project_root: &Path) -> Option<PathBuf> {
    let candidates = [
        // The whole of corpus/regulation, not nl/ alone: EU regulations live
        // in eu/ beside it.
        project_root.join("corpus/regulation"),
        project_root.join("corpus/central/nl"),
        project_root.join("corpus/central"),
    ];

    for candidate in &candidates {
        if candidate.is_dir() {
            return Some(candidate.clone());
        }
    }
    None
}

/// Get all YAML file paths from the corpus.
pub fn corpus_yaml_files(project_root: &Path) -> Vec<PathBuf> {
    let regulation_dir = match find_regulation_dir(project_root) {
        Some(d) => d,
        None => return Vec::new(),
    };

    WalkDir::new(regulation_dir)
        .sort_by_file_name()
        .into_iter()
        .flatten()
        .filter(|e| {
            e.file_type().is_file()
                && e.path()
                    .extension()
                    .is_some_and(|ext| ext == "yaml" || ext == "yml")
        })
        .map(|e| e.path().to_path_buf())
        .collect()
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    fn repo_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .expect("repository root")
            .to_path_buf()
    }

    #[test]
    fn corpus_includes_eu_regulations() {
        let files = corpus_yaml_files(&repo_root());
        assert!(
            files
                .iter()
                .any(|p| p.ends_with("nl/wet/wet_op_de_zorgtoeslag/2025-01-01.yaml")),
            "a Dutch law under nl/ should be found"
        );
        assert!(
            files
                .iter()
                .any(|p| p.ends_with("eu/verordening/verordening_eu_2023_1230/2027-01-14.yaml")),
            "an EU regulation under eu/ should be found"
        );
    }
}
