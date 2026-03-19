use std::collections::HashMap;
use std::path::Path;

use walkdir::WalkDir;

use crate::error::{CorpusError, Result};
use crate::models::{Source, SourceType};

/// A loaded law with its source provenance.
#[derive(Debug, Clone)]
pub struct LoadedLaw {
    /// The law's `$id` field.
    pub law_id: String,
    /// The raw YAML content.
    pub yaml_content: String,
    /// Path to the source file.
    pub file_path: String,
    /// ID of the source that provided this law.
    pub source_id: String,
    /// Name of the source that provided this law.
    pub source_name: String,
    /// Priority of the source (lower = higher priority).
    pub source_priority: u32,
}

/// Aggregates laws from multiple sources with priority-based conflict resolution.
///
/// When multiple sources provide a law with the same `$id`, the source with the
/// lowest priority value wins. Equal priority with the same `$id` is an error.
#[derive(Debug)]
pub struct SourceMap {
    /// Laws indexed by `$id`, with provenance metadata.
    laws: HashMap<String, LoadedLaw>,
    /// Conflicts that were resolved (for reporting).
    resolved_conflicts: Vec<ConflictResolution>,
}

/// Record of a conflict that was resolved by priority.
#[derive(Debug, Clone)]
pub struct ConflictResolution {
    pub law_id: String,
    pub winner_source_id: String,
    pub winner_priority: u32,
    pub loser_source_id: String,
    pub loser_priority: u32,
}

impl SourceMap {
    /// Create an empty source map.
    pub fn new() -> Self {
        Self {
            laws: HashMap::new(),
            resolved_conflicts: Vec::new(),
        }
    }

    /// Load laws from a single source directory.
    ///
    /// Scans the directory for `.yaml` files, extracts the `$id` field,
    /// and adds them to the map with conflict resolution.
    pub fn load_source(&mut self, source: &Source) -> Result<usize> {
        let path = match &source.source_type {
            SourceType::Local { local } => &local.path,
            SourceType::GitHub { .. } => {
                return Err(CorpusError::Config(
                    "GitHub sources must be fetched before loading into SourceMap".to_string(),
                ));
            }
        };

        self.load_from_directory(path, source)
    }

    /// Load all YAML files from a directory into the source map.
    pub fn load_from_directory(&mut self, dir: &Path, source: &Source) -> Result<usize> {
        if !dir.exists() {
            return Ok(0);
        }

        let mut count = 0;

        for entry in WalkDir::new(dir)
            .follow_links(true)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if !path.is_file() || path.extension().is_none_or(|ext| ext != "yaml") {
                continue;
            }

            let content = std::fs::read_to_string(path).map_err(|e| {
                CorpusError::Config(format!(
                    "Failed to read {} from source '{}': {}",
                    path.display(),
                    source.id,
                    e
                ))
            })?;

            let law_id = match extract_law_id(&content) {
                Some(id) => id,
                None => continue, // Skip files without $id
            };

            let loaded = LoadedLaw {
                law_id: law_id.clone(),
                yaml_content: content,
                file_path: path.display().to_string(),
                source_id: source.id.clone(),
                source_name: source.name.clone(),
                source_priority: source.priority,
            };

            self.insert(loaded)?;
            count += 1;
        }

        Ok(count)
    }

    /// Insert a law into the map, resolving conflicts by priority.
    fn insert(&mut self, law: LoadedLaw) -> Result<()> {
        let law_id = law.law_id.clone();

        if let Some(existing) = self.laws.get(&law_id) {
            if existing.source_priority == law.source_priority {
                return Err(CorpusError::Config(format!(
                    "Conflict: law '{}' provided by both '{}' and '{}' with equal priority {}",
                    law_id, existing.source_id, law.source_id, law.source_priority
                )));
            }

            if law.source_priority < existing.source_priority {
                // New law wins (lower priority value = higher priority)
                self.resolved_conflicts.push(ConflictResolution {
                    law_id: law_id.clone(),
                    winner_source_id: law.source_id.clone(),
                    winner_priority: law.source_priority,
                    loser_source_id: existing.source_id.clone(),
                    loser_priority: existing.source_priority,
                });
                self.laws.insert(law_id, law);
            } else {
                // Existing law wins
                self.resolved_conflicts.push(ConflictResolution {
                    law_id: law_id.clone(),
                    winner_source_id: existing.source_id.clone(),
                    winner_priority: existing.source_priority,
                    loser_source_id: law.source_id.clone(),
                    loser_priority: law.source_priority,
                });
            }
        } else {
            self.laws.insert(law_id, law);
        }

        Ok(())
    }

    /// Get all loaded laws.
    pub fn laws(&self) -> impl Iterator<Item = &LoadedLaw> {
        self.laws.values()
    }

    /// Get a specific law by ID.
    pub fn get_law(&self, law_id: &str) -> Option<&LoadedLaw> {
        self.laws.get(law_id)
    }

    /// Get the number of loaded laws.
    pub fn len(&self) -> usize {
        self.laws.len()
    }

    /// Check if the source map is empty.
    pub fn is_empty(&self) -> bool {
        self.laws.is_empty()
    }

    /// Get all conflict resolutions that occurred during loading.
    pub fn resolved_conflicts(&self) -> &[ConflictResolution] {
        &self.resolved_conflicts
    }
}

impl Default for SourceMap {
    fn default() -> Self {
        Self::new()
    }
}

/// Extract the `$id` field from a YAML string.
///
/// Uses a simple line-based approach to avoid full YAML parsing overhead.
fn extract_law_id(yaml: &str) -> Option<String> {
    for line in yaml.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("$id:") {
            let value = rest.trim().trim_matches('"').trim_matches('\'');
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::models::{LocalSource, SourceType};
    use std::fs;
    use tempfile::TempDir;

    fn make_source(id: &str, name: &str, path: &Path, priority: u32) -> Source {
        Source {
            id: id.to_string(),
            name: name.to_string(),
            source_type: SourceType::Local {
                local: LocalSource {
                    path: path.to_path_buf(),
                },
            },
            scopes: vec![],
            priority,
        }
    }

    fn write_yaml(dir: &Path, subpath: &str, id: &str) {
        let path = dir.join(subpath);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(
            &path,
            format!(
                "$id: {id}\nregulatory_layer: WET\npublication_date: '2025-01-01'\narticles: []\n"
            ),
        )
        .unwrap();
    }

    #[test]
    fn test_extract_law_id() {
        assert_eq!(
            extract_law_id("$id: my_law\nfoo: bar"),
            Some("my_law".to_string())
        );
        assert_eq!(
            extract_law_id("$id: \"quoted_id\"\nfoo: bar"),
            Some("quoted_id".to_string())
        );
        assert_eq!(extract_law_id("foo: bar\nbaz: qux"), None);
    }

    #[test]
    fn test_load_single_source() {
        let dir = TempDir::new().unwrap();
        write_yaml(dir.path(), "wet/test_wet/2025-01-01.yaml", "test_wet");

        let source = make_source("central", "Central", dir.path(), 1);
        let mut map = SourceMap::new();
        let count = map.load_source(&source).unwrap();

        assert_eq!(count, 1);
        assert_eq!(map.len(), 1);

        let law = map.get_law("test_wet").unwrap();
        assert_eq!(law.source_id, "central");
        assert_eq!(law.source_priority, 1);
    }

    #[test]
    fn test_multi_source_no_overlap() {
        let dir_a = TempDir::new().unwrap();
        let dir_b = TempDir::new().unwrap();

        write_yaml(dir_a.path(), "wet/law_a/2025.yaml", "law_a");
        write_yaml(dir_b.path(), "wet/law_b/2025.yaml", "law_b");

        let source_a = make_source("central", "Central", dir_a.path(), 1);
        let source_b = make_source("gemeente", "Gemeente", dir_b.path(), 10);

        let mut map = SourceMap::new();
        map.load_source(&source_a).unwrap();
        map.load_source(&source_b).unwrap();

        assert_eq!(map.len(), 2);
        assert_eq!(map.get_law("law_a").unwrap().source_id, "central");
        assert_eq!(map.get_law("law_b").unwrap().source_id, "gemeente");
    }

    #[test]
    fn test_priority_conflict_lower_wins() {
        let dir_a = TempDir::new().unwrap();
        let dir_b = TempDir::new().unwrap();

        write_yaml(dir_a.path(), "wet/shared/2025.yaml", "shared_law");
        write_yaml(dir_b.path(), "wet/shared/2025.yaml", "shared_law");

        let source_a = make_source("central", "Central", dir_a.path(), 1);
        let source_b = make_source("overlap", "Overlap", dir_b.path(), 10);

        let mut map = SourceMap::new();
        map.load_source(&source_a).unwrap();
        map.load_source(&source_b).unwrap();

        assert_eq!(map.len(), 1);
        let law = map.get_law("shared_law").unwrap();
        assert_eq!(law.source_id, "central"); // Priority 1 wins over 10

        assert_eq!(map.resolved_conflicts().len(), 1);
        assert_eq!(map.resolved_conflicts()[0].winner_source_id, "central");
    }

    #[test]
    fn test_equal_priority_conflict_is_error() {
        let dir_a = TempDir::new().unwrap();
        let dir_b = TempDir::new().unwrap();

        write_yaml(dir_a.path(), "wet/dup/2025.yaml", "dup_law");
        write_yaml(dir_b.path(), "wet/dup/2025.yaml", "dup_law");

        let source_a = make_source("source-a", "Source A", dir_a.path(), 5);
        let source_b = make_source("source-b", "Source B", dir_b.path(), 5);

        let mut map = SourceMap::new();
        map.load_source(&source_a).unwrap();
        let result = map.load_source(&source_b);

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("dup_law"));
        assert!(err.contains("equal priority"));
    }

    #[test]
    fn test_empty_directory() {
        let dir = TempDir::new().unwrap();
        let source = make_source("empty", "Empty", dir.path(), 1);

        let mut map = SourceMap::new();
        let count = map.load_source(&source).unwrap();

        assert_eq!(count, 0);
        assert!(map.is_empty());
    }

    #[test]
    fn test_nonexistent_directory() {
        let source = make_source("missing", "Missing", Path::new("/nonexistent"), 1);

        let mut map = SourceMap::new();
        let count = map.load_source(&source).unwrap();

        assert_eq!(count, 0);
    }

    #[test]
    fn test_reverse_load_order_still_priority_wins() {
        // Load high-priority source second — it should still win
        let dir_a = TempDir::new().unwrap();
        let dir_b = TempDir::new().unwrap();

        write_yaml(dir_a.path(), "wet/law/2025.yaml", "contested_law");
        write_yaml(dir_b.path(), "wet/law/2025.yaml", "contested_law");

        let source_low = make_source("low", "Low Priority", dir_a.path(), 100);
        let source_high = make_source("high", "High Priority", dir_b.path(), 1);

        let mut map = SourceMap::new();
        map.load_source(&source_low).unwrap();
        map.load_source(&source_high).unwrap();

        assert_eq!(map.get_law("contested_law").unwrap().source_id, "high");
    }
}
