use std::collections::HashMap;
use std::path::Path;

use serde::Deserialize;
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
    /// Path to the source file. For local sources this is the absolute path
    /// on disk; for fetched (e.g. GitHub) sources this is the path inside
    /// the upstream repository.
    pub file_path: String,
    /// Path to the source file *relative to the source root* — i.e. the
    /// portion of [`file_path`](Self::file_path) below the source's own
    /// root directory. This is what backends use to address the file via
    /// [`RepoBackend::write_file`](crate::backend::RepoBackend::write_file)
    /// and friends, and avoids the structural-depth heuristic that breaks
    /// when a source root is configured at an unusual location.
    pub relative_path: String,
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

            // Compute the path relative to the source root so that writes
            // can address the file via the backend without re-deriving the
            // structural location from a depth heuristic.
            let relative_path = path
                .strip_prefix(dir)
                .map(|p| p.display().to_string())
                .unwrap_or_else(|_| path.display().to_string());

            let loaded = LoadedLaw {
                law_id: law_id.clone(),
                name,
                yaml_content: content,
                file_path: path.display().to_string(),
                relative_path,
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
    ///
    /// `file_path` is the path inside the upstream repository (e.g.
    /// `regulation/nl/wet/my_law/2025.yaml`). `source_subpath`, when set,
    /// is the in-repo prefix that the source is rooted at — it is stripped
    /// to compute the source-root-relative path stored on `LoadedLaw`.
    pub fn load_fetched_file(
        &mut self,
        content: &str,
        file_path: &str,
        source_subpath: Option<&str>,
        source_id: &str,
        source_name: &str,
        source_priority: u32,
    ) -> Result<bool> {
        let law_id = match extract_law_id(content) {
            Some(id) => id,
            None => return Ok(false),
        };

        let name = extract_law_name(content);

        // Strip the source's in-repo subpath so the stored relative path is
        // relative to the source root, matching the on-disk layout the
        // backend writes to.
        let relative_path = match source_subpath {
            Some(sub) if !sub.is_empty() => {
                let trimmed = sub.trim_end_matches('/');
                file_path
                    .strip_prefix(&format!("{trimmed}/"))
                    .unwrap_or(file_path)
                    .to_string()
            }
            _ => file_path.to_string(),
        };

        let loaded = LoadedLaw {
            law_id: law_id.clone(),
            name,
            yaml_content: content.to_string(),
            file_path: file_path.to_string(),
            relative_path,
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

    /// Update the cached YAML content for an existing law. Used after the
    /// editor persists an edit through [`crate::backend::RepoBackend`], so
    /// subsequent GETs (and dependency walks) see the new text without
    /// waiting for a full corpus reload. Returns `true` if the law was
    /// present and updated, `false` otherwise.
    ///
    /// Only `yaml_content` (and the optional human-readable `name`, which is
    /// derived from the YAML) is updated — `$id`, `file_path`, source
    /// provenance, and priority are stable across an edit and are left
    /// untouched. If the caller writes a new file under a different `$id`
    /// that's a different operation (unsupported via this hook).
    pub fn update_yaml_content(&mut self, law_id: &str, new_content: String) -> bool {
        let Some(law) = self.laws.get_mut(law_id) else {
            return false;
        };
        law.name = extract_law_name(&new_content);
        law.yaml_content = new_content;
        true
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
pub(crate) fn today_str() -> String {
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
pub(crate) fn pick_best_version(existing: Option<&str>, new: Option<&str>, today: &str) -> bool {
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

/// Verify that a string parses as well-formed YAML without enforcing a
/// particular schema. Used by write handlers that want to reject garbage
/// input before persisting to the corpus backend, without committing to
/// the corpus library's full law-schema validation (which belongs in a
/// separate layer).
pub fn validate_yaml_syntax(content: &str) -> Result<()> {
    serde_yaml_ng::from_str::<serde_yaml_ng::Value>(content)?;
    Ok(())
}

/// Extract the top-level `$id` field from a YAML string.
///
/// Uses a simple line-based approach to avoid full YAML parsing overhead.
/// Only matches `$id:` at the start of a line (no leading whitespace) to
/// avoid matching nested `$id:` fields.
pub fn extract_law_id(yaml: &str) -> Option<String> {
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

/// Extract the raw `name:` value including `#` references.
fn extract_raw_name(yaml: &str) -> Option<String> {
    for line in yaml.lines() {
        if let Some(rest) = line.strip_prefix("name:") {
            let value = rest.trim().trim_matches('"').trim_matches('\'');
            if !value.is_empty() {
                return Some(value.to_string());
            }
            return None;
        }
    }
    None
}

// --- Minimal deserialization types for display-name and output resolution ---

#[derive(Deserialize, Default)]
struct LawDoc {
    #[serde(default)]
    articles: Vec<LawArticle>,
}

#[derive(Deserialize, Default)]
struct LawArticle {
    #[serde(default)]
    number: Option<String>,
    #[serde(default)]
    machine_readable: Option<LawMr>,
}

#[derive(Deserialize, Default)]
struct LawMr {
    #[serde(default)]
    execution: Option<LawExec>,
}

#[derive(Deserialize, Default)]
struct LawExec {
    #[serde(default)]
    actions: Vec<LawAction>,
    #[serde(default)]
    output: Vec<LawOutput>,
}

#[derive(Deserialize, Default)]
struct LawAction {
    #[serde(default)]
    output: Option<String>,
    #[serde(default)]
    value: Option<serde_yaml_ng::Value>,
}

/// An output entry from `execution.output`.
#[derive(Deserialize, Default)]
struct LawOutput {
    #[serde(default)]
    name: String,
    #[serde(default, rename = "type")]
    output_type: Option<String>,
}

/// Resolve a law's human-readable display name.
///
/// If the YAML has a literal `name:` field (e.g. `name: Kieswet`), returns
/// that. If the name is an output reference (e.g. `name: '#wet_naam'`),
/// parses the YAML to find the action whose output matches the reference
/// and returns its scalar value. Returns `None` when no name can be resolved.
pub fn resolve_display_name(yaml: &str) -> Option<String> {
    // Fast path: literal name
    if let Some(name) = extract_law_name(yaml) {
        return Some(name);
    }

    // Check for # reference
    let raw = extract_raw_name(yaml)?;
    let reference = raw.strip_prefix('#')?;

    // Parse YAML to find the action that resolves this reference
    let doc: LawDoc = serde_yaml_ng::from_str(yaml).ok()?;
    for article in &doc.articles {
        let Some(mr) = &article.machine_readable else {
            continue;
        };
        let Some(exec) = &mr.execution else {
            continue;
        };
        for action in &exec.actions {
            if action.output.as_deref() == Some(reference) {
                if let Some(serde_yaml_ng::Value::String(s)) = &action.value {
                    return Some(s.clone());
                }
            }
        }
    }

    None
}

/// Collect all outputs declared across all articles in a law.
///
/// Returns `(name, type, article_number)` tuples, deduplicated by name
/// (first occurrence wins).
pub fn collect_law_outputs(yaml: &str) -> Vec<(String, String, String)> {
    let doc: LawDoc = match serde_yaml_ng::from_str(yaml) {
        Ok(d) => d,
        Err(_) => return Vec::new(),
    };

    let mut seen = HashMap::new();
    let mut results = Vec::new();

    for article in &doc.articles {
        let article_number = article.number.as_deref().unwrap_or("").to_string();
        let Some(mr) = &article.machine_readable else {
            continue;
        };
        let Some(exec) = &mr.execution else {
            continue;
        };
        for output in &exec.output {
            if !output.name.is_empty() && !seen.contains_key(&output.name) {
                seen.insert(output.name.clone(), ());
                results.push((
                    output.name.clone(),
                    output
                        .output_type
                        .clone()
                        .unwrap_or_else(|| "string".to_string()),
                    article_number.clone(),
                ));
            }
        }
    }

    results.sort_by(|a, b| a.0.cmp(&b.0));
    results
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
    fn test_update_yaml_content_updates_existing_law() {
        let dir = TempDir::new().unwrap();
        write_yaml(dir.path(), "wet/test_wet/2025-01-01.yaml", "test_wet");

        let source = make_source("central", "Central", dir.path(), 1);
        let mut map = SourceMap::new();
        map.load_source(&source).unwrap();

        let original = map.get_law("test_wet").unwrap().clone();

        let new_content =
            "$id: test_wet\nname: Updated Name\nregulatory_layer: WET\narticles: []\n".to_string();
        let updated = map.update_yaml_content("test_wet", new_content.clone());

        assert!(updated, "update should report success for existing law");
        let law = map.get_law("test_wet").unwrap();
        assert_eq!(law.yaml_content, new_content);
        assert_eq!(law.name.as_deref(), Some("Updated Name"));
        // All provenance fields are preserved — update_yaml_content only
        // touches content + name. `relative_path` is load-bearing because
        // the editor-api write handler targets it; a regression that
        // cleared it would silently send reads and writes to different
        // files, so we assert it explicitly alongside file_path/source_*.
        assert_eq!(law.file_path, original.file_path);
        assert_eq!(law.relative_path, original.relative_path);
        assert_eq!(law.source_id, original.source_id);
        assert_eq!(law.source_name, original.source_name);
        assert_eq!(law.source_priority, original.source_priority);
    }

    #[test]
    fn test_update_yaml_content_recomputes_name_to_none() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("wet/test_wet/2025-01-01.yaml");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(
            &path,
            "$id: test_wet\nname: Original\nregulatory_layer: WET\narticles: []\n",
        )
        .unwrap();

        let source = make_source("central", "Central", dir.path(), 1);
        let mut map = SourceMap::new();
        map.load_source(&source).unwrap();
        assert_eq!(
            map.get_law("test_wet").unwrap().name.as_deref(),
            Some("Original")
        );

        // Remove the `name:` field — name should recompute to None.
        let new_content = "$id: test_wet\nregulatory_layer: WET\narticles: []\n".to_string();
        let updated = map.update_yaml_content("test_wet", new_content);
        assert!(updated);
        assert_eq!(map.get_law("test_wet").unwrap().name, None);
    }

    #[test]
    fn test_update_yaml_content_missing_law_returns_false() {
        let mut map = SourceMap::new();
        let updated = map.update_yaml_content("nonexistent_law", "$id: foo\n".to_string());
        assert!(!updated, "update should report failure for missing law");
        assert_eq!(map.len(), 0, "missing law should not be inserted");
    }

    #[test]
    fn test_validate_yaml_syntax_accepts_well_formed() {
        assert!(validate_yaml_syntax("$id: foo\nname: bar\narticles: []\n").is_ok());
        assert!(validate_yaml_syntax("---\nfoo: 1\n").is_ok());
        assert!(validate_yaml_syntax("").is_ok()); // empty doc is valid
    }

    #[test]
    fn test_validate_yaml_syntax_rejects_garbage() {
        // Unclosed quote.
        assert!(validate_yaml_syntax("name: \"unterminated\nfoo: bar\n").is_err());
        // Tab indentation inside a block mapping.
        assert!(validate_yaml_syntax("name:\n\tfoo: bar\n").is_err());
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

    #[test]
    fn test_resolve_display_name_literal() {
        let yaml = "$id: kieswet\nname: Kieswet\narticles: []\n";
        assert_eq!(resolve_display_name(yaml), Some("Kieswet".to_string()));
    }

    #[test]
    fn test_resolve_display_name_output_reference() {
        let yaml = r#"$id: zorgtoeslagwet
name: '#wet_naam'
articles:
  - number: '8'
    machine_readable:
      execution:
        actions:
          - output: wet_naam
            value: Wet op de zorgtoeslag
"#;
        assert_eq!(
            resolve_display_name(yaml),
            Some("Wet op de zorgtoeslag".to_string())
        );
    }

    #[test]
    fn test_resolve_display_name_unresolvable_reference() {
        let yaml = "$id: test\nname: '#missing_output'\narticles: []\n";
        assert_eq!(resolve_display_name(yaml), None);
    }

    #[test]
    fn test_resolve_display_name_no_name_field() {
        let yaml = "$id: test\narticles: []\n";
        assert_eq!(resolve_display_name(yaml), None);
    }

    #[test]
    fn test_collect_law_outputs() {
        let yaml = r#"$id: brp
articles:
  - number: '2.7'
    machine_readable:
      execution:
        output:
          - name: leeftijd
            type: number
        actions: []
  - number: '2.8'
    machine_readable:
      execution:
        output:
          - name: heeft_partner
            type: boolean
        actions: []
"#;
        let outputs = collect_law_outputs(yaml);
        assert_eq!(outputs.len(), 2);
        assert_eq!(
            outputs[0],
            (
                "heeft_partner".to_string(),
                "boolean".to_string(),
                "2.8".to_string()
            )
        );
        assert_eq!(
            outputs[1],
            (
                "leeftijd".to_string(),
                "number".to_string(),
                "2.7".to_string()
            )
        );
    }

    #[test]
    fn test_collect_law_outputs_deduplicates() {
        let yaml = r#"$id: test
articles:
  - number: '1'
    machine_readable:
      execution:
        output:
          - name: foo
            type: string
        actions: []
  - number: '2'
    machine_readable:
      execution:
        output:
          - name: foo
            type: number
        actions: []
"#;
        let outputs = collect_law_outputs(yaml);
        assert_eq!(outputs.len(), 1);
        // First occurrence wins
        assert_eq!(outputs[0].1, "string");
    }

    #[test]
    fn test_collect_law_outputs_empty() {
        let yaml = "$id: test\narticles: []\n";
        let outputs = collect_law_outputs(yaml);
        assert!(outputs.is_empty());
    }
}
