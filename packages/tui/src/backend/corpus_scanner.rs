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
///
/// The engine backend loads from the same tree, so both go through this one
/// list: a fifth candidate added to only one of them would show laws in the
/// corpus browser that the engine never loaded.
pub fn find_regulation_dir(project_root: &Path) -> Option<PathBuf> {
    let candidates = [
        project_root.join("corpus/regulation/nl"),
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
mod tests {
    use super::*;

    fn write(path: &Path, body: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, body).unwrap();
    }

    #[test]
    fn corpus_yaml_files_finds_both_yaml_spellings_under_the_first_candidate() {
        let root = tempfile::tempdir().unwrap();
        let nl = root.path().join("corpus/regulation/nl");
        write(&nl.join("wet/foo/2025-01-01.yaml"), "$id: foo\n");
        write(&nl.join("wet/bar/2025-01-01.yml"), "$id: bar\n");
        write(&nl.join("wet/foo/README.md"), "not a law\n");

        let found = corpus_yaml_files(root.path());

        assert_eq!(found.len(), 2, "found: {found:?}");
        assert!(found.iter().all(|p| p.starts_with(&nl)));
    }

    #[test]
    fn find_regulation_dir_prefers_the_nl_subtree_over_its_parent() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("corpus/regulation/nl")).unwrap();

        assert_eq!(
            find_regulation_dir(root.path()),
            Some(root.path().join("corpus/regulation/nl"))
        );
    }

    #[test]
    fn a_project_without_a_corpus_yields_no_files() {
        let root = tempfile::tempdir().unwrap();

        assert!(find_regulation_dir(root.path()).is_none());
        assert!(corpus_yaml_files(root.path()).is_empty());
    }
}
