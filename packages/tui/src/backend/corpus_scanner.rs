use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Debug, Clone)]
pub struct CorpusNode {
    pub path: PathBuf,
    pub name: String,
    pub depth: usize,
    pub is_dir: bool,
}

#[derive(Debug, Clone, Default)]
pub struct LawMetadata {
    pub id: Option<String>,
    pub schema: Option<String>,
    pub regulatory_layer: Option<String>,
    pub publication_date: Option<String>,
    pub valid_from: Option<String>,
    pub bwb_id: Option<String>,
    pub name: Option<String>,
    pub article_count: usize,
}

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
pub fn extract_metadata(content: &str) -> LawMetadata {
    let mut meta = LawMetadata::default();

    for line in content.lines() {
        let trimmed = line.trim();

        if let Some(val) = trimmed.strip_prefix("$id:") {
            meta.id = Some(val.trim().trim_matches('\'').trim_matches('"').to_string());
        } else if let Some(val) = trimmed.strip_prefix("$schema:") {
            meta.schema = Some(val.trim().to_string());
        } else if let Some(val) = trimmed.strip_prefix("regulatory_layer:") {
            meta.regulatory_layer =
                Some(val.trim().trim_matches('\'').trim_matches('"').to_string());
        } else if let Some(val) = trimmed.strip_prefix("publication_date:") {
            meta.publication_date =
                Some(val.trim().trim_matches('\'').trim_matches('"').to_string());
        } else if let Some(val) = trimmed.strip_prefix("valid_from:") {
            meta.valid_from = Some(val.trim().trim_matches('\'').trim_matches('"').to_string());
        } else if let Some(val) = trimmed.strip_prefix("bwb_id:") {
            meta.bwb_id = Some(val.trim().trim_matches('\'').trim_matches('"').to_string());
        } else if let Some(val) = trimmed.strip_prefix("name:") {
            meta.name = Some(val.trim().trim_matches('\'').trim_matches('"').to_string());
        } else if trimmed.starts_with("- number:") {
            meta.article_count += 1;
        }
    }

    meta
}

/// Find the corpus regulation directory by checking common locations.
fn find_regulation_dir(project_root: &Path) -> Option<PathBuf> {
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
#[allow(dead_code)]
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
