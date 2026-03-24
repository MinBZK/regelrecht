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
    /// The law's `name` field (human-readable title), if present.
    pub name: Option<String>,
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
            .filter_map(|e| match e {
                Ok(entry) => Some(entry),
                Err(err) => {
                    tracing::warn!(
                        path = ?err.path(),
                        error = %err,
                        "Failed to read directory entry, skipping"
                    );
                    None
                }
            })
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

            let name = extract_law_name(&content);

            let loaded = LoadedLaw {
                law_id: law_id.clone(),
                name,
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
    ///
    /// When two entries share the same `$id` and priority:
    /// - **Same source**: multiple versions of one law. Keep the version whose
    ///   `valid_from` date (from the filename) is closest to today without
    ///   exceeding it. If no version is currently valid, keep the latest.
    /// - **Different sources**: this is still a hard error (ambiguous ownership).
    fn insert(&mut self, law: LoadedLaw) -> Result<()> {
        let law_id = law.law_id.clone();

        if let Some(existing) = self.laws.get(&law_id) {
            if existing.source_priority == law.source_priority {
                // Same source with multiple versions → pick best version
                if existing.source_id == law.source_id {
                    let existing_date = extract_date_from_path(&existing.file_path);
                    let new_date = extract_date_from_path(&law.file_path);
                    let today = today_str();

                    let new_wins =
                        pick_best_version(existing_date.as_deref(), new_date.as_deref(), &today);

                    if new_wins {
                        tracing::debug!(
                            law_id = %law_id,
                            kept = %law.file_path,
                            dropped = %existing.file_path,
                            "same-source version conflict resolved"
                        );
                        self.laws.insert(law_id, law);
                    } else {
                        tracing::debug!(
                            law_id = %law_id,
                            kept = %existing.file_path,
                            dropped = %law.file_path,
                            "same-source version conflict resolved"
                        );
                    }
                    return Ok(());
                }

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

    /// Load a single fetched file (from GitHub or other remote) into the map.
    pub fn load_fetched_file(
        &mut self,
        content: &str,
        file_path: &str,
        source_id: &str,
        source_name: &str,
        source_priority: u32,
    ) -> Result<bool> {
        let law_id = match extract_law_id(content) {
            Some(id) => id,
            None => return Ok(false),
        };

        let name = extract_law_name(content);

        let loaded = LoadedLaw {
            law_id: law_id.clone(),
            name,
            yaml_content: content.to_string(),
            file_path: file_path.to_string(),
            source_id: source_id.to_string(),
            source_name: source_name.to_string(),
            source_priority,
        };

        self.insert(loaded)?;
        Ok(true)
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

/// Extract a YYYY-MM-DD date from the filename component of a path.
///
/// Matches the convention `…/law_id/2025-01-01.yaml`.
fn extract_date_from_path(path: &str) -> Option<String> {
    let filename = path.rsplit('/').next().unwrap_or(path);
    let stem = filename.strip_suffix(".yaml")?;
    // Validate YYYY-MM-DD pattern
    if stem.len() == 10
        && stem.as_bytes()[4] == b'-'
        && stem.as_bytes()[7] == b'-'
        && stem.bytes().filter(|b| b.is_ascii_digit()).count() == 8
    {
        Some(stem.to_string())
    } else {
        None
    }
}

/// Return today's date as "YYYY-MM-DD".
fn today_str() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    // 86400 seconds per day, epoch is 1970-01-01
    let days = now / 86400;
    // Simple conversion: count years/months/days from epoch
    let (y, m, d) = days_to_ymd(days);
    format!("{y:04}-{m:02}-{d:02}")
}

/// Convert days since Unix epoch to (year, month, day).
fn days_to_ymd(mut days: u64) -> (u64, u64, u64) {
    // Algorithm from https://howardhinnant.github.io/date_algorithms.html
    days += 719_468;
    let era = days / 146_097;
    let doe = days - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

/// Decide whether `new_date` should replace `existing_date`.
///
/// Rules:
/// 1. Currently valid (date <= today) beats future-only.
/// 2. Among currently valid dates, the latest wins (most up-to-date).
/// 3. Among future dates, the latest wins.
fn pick_best_version(existing: Option<&str>, new: Option<&str>, today: &str) -> bool {
    match (existing, new) {
        (None, Some(_)) => true,
        (Some(_), None) => false,
        (None, None) => false,
        (Some(e), Some(n)) => {
            let e_valid = e <= today;
            let n_valid = n <= today;
            match (e_valid, n_valid) {
                // Both valid or both future → latest date wins
                _ if e_valid == n_valid => n > e,
                // Only new is valid now → new wins
                (false, true) => true,
                // Only existing is valid now → existing stays
                (true, false) => false,
                _ => unreachable!(),
            }
        }
    }
}

/// Extract the top-level `$id` field from a YAML string.
///
/// Uses a simple line-based approach to avoid full YAML parsing overhead.
/// Only matches `$id:` at the start of a line (no leading whitespace) to
/// avoid matching nested `$id:` fields.
fn extract_law_id(yaml: &str) -> Option<String> {
    for line in yaml.lines() {
        if let Some(rest) = line.strip_prefix("$id:") {
            let value = rest.trim().trim_matches('"').trim_matches('\'');
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

/// Extract the top-level `name` field from a YAML string.
///
/// Skips names starting with `#` (output references resolved at runtime).
fn extract_law_name(yaml: &str) -> Option<String> {
    for line in yaml.lines() {
        if let Some(rest) = line.strip_prefix("name:") {
            let value = rest.trim().trim_matches('"').trim_matches('\'');
            if !value.is_empty() && !value.starts_with('#') {
                return Some(value.to_string());
            }
            return None;
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
            auth_ref: None,
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
    fn test_extract_law_id_ignores_indented() {
        // Nested $id: should not be matched
        let yaml = "name: test\narticles:\n  - $id: nested_id\n";
        assert_eq!(extract_law_id(yaml), None);

        // But top-level $id: should still work
        let yaml = "$id: top_level\narticles:\n  - $id: nested_id\n";
        assert_eq!(extract_law_id(yaml), Some("top_level".to_string()));
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
    fn test_equal_priority_different_sources_is_error() {
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
    fn test_same_source_multiple_versions_keeps_latest_valid() {
        let dir = TempDir::new().unwrap();

        // Two versions of the same law, both currently valid (dates in the past)
        write_yaml(dir.path(), "wet/my_law/2024-01-01.yaml", "my_law");
        write_yaml(dir.path(), "wet/my_law/2025-01-01.yaml", "my_law");

        let source = make_source("local", "Local", dir.path(), 1);
        let mut map = SourceMap::new();
        let count = map.load_source(&source).unwrap();

        // Both files are loaded but only one law in the map
        assert_eq!(count, 2);
        assert_eq!(map.len(), 1);

        let law = map.get_law("my_law").unwrap();
        // 2025 version should win (latest valid)
        assert!(law.file_path.contains("2025-01-01"));
    }

    #[test]
    fn test_same_source_valid_beats_future() {
        let dir = TempDir::new().unwrap();

        // One valid now, one far in the future
        write_yaml(dir.path(), "wet/my_law/2024-01-01.yaml", "my_law");
        write_yaml(dir.path(), "wet/my_law/2099-01-01.yaml", "my_law");

        let source = make_source("local", "Local", dir.path(), 1);
        let mut map = SourceMap::new();
        map.load_source(&source).unwrap();

        let law = map.get_law("my_law").unwrap();
        // 2024 is currently valid, 2099 is future → 2024 wins
        assert!(law.file_path.contains("2024-01-01"));
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
