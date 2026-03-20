use std::path::Path;

use crate::error::{CorpusError, Result};
use crate::models::{RegistryManifest, Source, SourceType};
use crate::source_map::SourceMap;

/// Corpus registry that manages source definitions.
///
/// Loads sources from `corpus-registry.yaml` and optionally merges
/// local overrides from `corpus-registry.local.yaml`.
#[derive(Debug)]
pub struct CorpusRegistry {
    sources: Vec<Source>,
}

impl CorpusRegistry {
    /// Create an empty registry (for tests that don't need corpus).
    pub fn empty() -> Self {
        Self {
            sources: Vec::new(),
        }
    }

    /// Load the registry from a manifest file, optionally merging a local override.
    ///
    /// The local override file replaces sources with the same `id` entirely.
    pub fn load(manifest_path: &Path, local_override_path: Option<&Path>) -> Result<Self> {
        let content = std::fs::read_to_string(manifest_path).map_err(|e| {
            CorpusError::Config(format!(
                "Failed to read registry manifest {}: {}",
                manifest_path.display(),
                e
            ))
        })?;

        let manifest: RegistryManifest = serde_yaml::from_str(&content).map_err(|e| {
            CorpusError::Config(format!(
                "Failed to parse registry manifest {}: {}",
                manifest_path.display(),
                e
            ))
        })?;

        let mut sources = manifest.sources;

        if let Some(local_path) = local_override_path {
            if local_path.exists() {
                let local_content = std::fs::read_to_string(local_path).map_err(|e| {
                    CorpusError::Config(format!(
                        "Failed to read local override {}: {}",
                        local_path.display(),
                        e
                    ))
                })?;

                let local_manifest: RegistryManifest = serde_yaml::from_str(&local_content)
                    .map_err(|e| {
                        CorpusError::Config(format!(
                            "Failed to parse local override {}: {}",
                            local_path.display(),
                            e
                        ))
                    })?;

                sources = merge_sources(sources, local_manifest.sources);
            }
        }

        // Sort by priority (lowest value = highest priority)
        sources.sort_by_key(|s| s.priority);

        Ok(Self { sources })
    }

    /// Load from a YAML string (useful for testing).
    pub fn from_yaml(yaml: &str) -> Result<Self> {
        let manifest: RegistryManifest = serde_yaml::from_str(yaml)
            .map_err(|e| CorpusError::Config(format!("Failed to parse registry YAML: {}", e)))?;

        let mut sources = manifest.sources;
        sources.sort_by_key(|s| s.priority);

        Ok(Self { sources })
    }

    /// Get all sources, ordered by priority (lowest value first).
    pub fn sources(&self) -> &[Source] {
        &self.sources
    }

    /// Get a source by ID.
    pub fn get_source(&self, id: &str) -> Option<&Source> {
        self.sources.iter().find(|s| s.id == id)
    }

    /// Load all local sources into a SourceMap.
    ///
    /// GitHub sources are skipped — use [`load_all_sources_async`] to include them.
    pub fn load_local_sources(&self) -> Result<SourceMap> {
        let mut map = SourceMap::new();
        for source in &self.sources {
            match &source.source_type {
                SourceType::Local { .. } => {
                    map.load_source(source)?;
                }
                SourceType::GitHub { .. } => {
                    tracing::debug!(
                        source_id = %source.id,
                        "Skipping GitHub source in sync load"
                    );
                }
            }
        }
        Ok(map)
    }

    /// Load all sources (local + GitHub) into a SourceMap.
    ///
    /// Fetches GitHub sources using the provided auth file for token lookup.
    ///
    /// **Note:** A fresh `GitHubFetcher` is created on each call, so the
    /// `NotModified` branch is currently unreachable (no prior ETag exists).
    /// If caching is added later, callers must ensure that `NotModified`
    /// sources are merged from a previous `SourceMap` instead of silently
    /// dropped.
    #[cfg(feature = "github")]
    pub async fn load_all_sources_async(&self, auth_file: Option<&Path>) -> Result<SourceMap> {
        let mut map = SourceMap::new();
        let mut fetcher = crate::github::GitHubFetcher::new()?;

        for source in &self.sources {
            match &source.source_type {
                SourceType::Local { .. } => {
                    map.load_source(source)?;
                }
                SourceType::GitHub { github } => {
                    let token = crate::auth::resolve_token(&source.id, auth_file)?;
                    match fetcher.fetch_source(github, token.as_deref()).await? {
                        crate::github::FetchResult::Fetched(files) => {
                            for file in &files {
                                map.load_fetched_file(
                                    &file.content,
                                    &file.path,
                                    &source.id,
                                    &source.name,
                                    source.priority,
                                )?;
                            }
                        }
                        crate::github::FetchResult::NotModified => {
                            tracing::debug!(
                                source_id = %source.id,
                                "GitHub source unchanged, skipping"
                            );
                        }
                    }
                }
            }
        }
        Ok(map)
    }
}

/// Merge base sources with local overrides.
///
/// Sources in `overrides` with the same `id` as a base source replace it entirely.
/// Sources in `overrides` with new `id`s are appended.
fn merge_sources(base: Vec<Source>, overrides: Vec<Source>) -> Vec<Source> {
    let mut result = base;

    for override_source in overrides {
        if let Some(pos) = result.iter().position(|s| s.id == override_source.id) {
            result[pos] = override_source;
        } else {
            result.push(override_source);
        }
    }

    result
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::models::SourceType;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn write_temp_yaml(content: &str) -> NamedTempFile {
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(content.as_bytes()).unwrap();
        file
    }

    #[test]
    fn test_load_manifest() {
        let yaml = r#"
schema_version: "1.0"
sources:
  - id: central
    name: "MinBZK Central Corpus"
    type: local
    local:
      path: corpus/regulation/nl
    scopes: []
    priority: 1
"#;
        let file = write_temp_yaml(yaml);
        let registry = CorpusRegistry::load(file.path(), None).unwrap();

        assert_eq!(registry.sources().len(), 1);
        assert_eq!(registry.sources()[0].id, "central");
        assert_eq!(registry.sources()[0].priority, 1);
    }

    #[test]
    fn test_load_with_local_override() {
        let base_yaml = r#"
schema_version: "1.0"
sources:
  - id: central
    name: "MinBZK Central Corpus"
    type: local
    local:
      path: corpus/regulation/nl
    scopes: []
    priority: 1
  - id: amsterdam
    name: "Gemeente Amsterdam"
    type: local
    local:
      path: /remote/amsterdam
    scopes: []
    priority: 10
"#;
        let override_yaml = r#"
schema_version: "1.0"
sources:
  - id: amsterdam
    name: "Amsterdam Local Dev"
    type: local
    local:
      path: /local/amsterdam
    scopes: []
    priority: 10
"#;
        let base_file = write_temp_yaml(base_yaml);
        let override_file = write_temp_yaml(override_yaml);

        let registry = CorpusRegistry::load(base_file.path(), Some(override_file.path())).unwrap();

        assert_eq!(registry.sources().len(), 2);

        let amsterdam = registry.get_source("amsterdam").unwrap();
        assert_eq!(amsterdam.name, "Amsterdam Local Dev");
        match &amsterdam.source_type {
            SourceType::Local { local } => {
                assert_eq!(local.path, std::path::PathBuf::from("/local/amsterdam"));
            }
            _ => panic!("Expected local source"),
        }
    }

    #[test]
    fn test_local_override_adds_new_source() {
        let base_yaml = r#"
schema_version: "1.0"
sources:
  - id: central
    name: "Central"
    type: local
    local:
      path: corpus/regulation/nl
    scopes: []
    priority: 1
"#;
        let override_yaml = r#"
schema_version: "1.0"
sources:
  - id: my-gemeente
    name: "My Gemeente"
    type: local
    local:
      path: /local/my-gemeente
    scopes: []
    priority: 20
"#;
        let base_file = write_temp_yaml(base_yaml);
        let override_file = write_temp_yaml(override_yaml);

        let registry = CorpusRegistry::load(base_file.path(), Some(override_file.path())).unwrap();

        assert_eq!(registry.sources().len(), 2);
        assert!(registry.get_source("my-gemeente").is_some());
    }

    #[test]
    fn test_sources_sorted_by_priority() {
        let yaml = r#"
schema_version: "1.0"
sources:
  - id: low-prio
    name: "Low Priority"
    type: local
    local:
      path: /low
    scopes: []
    priority: 100
  - id: central
    name: "Central"
    type: local
    local:
      path: /central
    scopes: []
    priority: 1
  - id: mid-prio
    name: "Mid Priority"
    type: local
    local:
      path: /mid
    scopes: []
    priority: 10
"#;
        let registry = CorpusRegistry::from_yaml(yaml).unwrap();
        let sources = registry.sources();

        assert_eq!(sources[0].id, "central");
        assert_eq!(sources[0].priority, 1);
        assert_eq!(sources[1].id, "mid-prio");
        assert_eq!(sources[1].priority, 10);
        assert_eq!(sources[2].id, "low-prio");
        assert_eq!(sources[2].priority, 100);
    }

    #[test]
    fn test_missing_local_override_is_ok() {
        let yaml = r#"
schema_version: "1.0"
sources:
  - id: central
    name: "Central"
    type: local
    local:
      path: /central
    scopes: []
    priority: 1
"#;
        let file = write_temp_yaml(yaml);
        let nonexistent = Path::new("/nonexistent/override.yaml");

        let registry = CorpusRegistry::load(file.path(), Some(nonexistent)).unwrap();
        assert_eq!(registry.sources().len(), 1);
    }

    #[test]
    fn test_invalid_yaml_returns_error() {
        let yaml = "not: [valid: yaml: {{{";
        let file = write_temp_yaml(yaml);

        let result = CorpusRegistry::load(file.path(), None);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_source_by_id() {
        let yaml = r#"
schema_version: "1.0"
sources:
  - id: central
    name: "Central"
    type: local
    local:
      path: /central
    scopes: []
    priority: 1
  - id: amsterdam
    name: "Amsterdam"
    type: local
    local:
      path: /amsterdam
    scopes: []
    priority: 10
"#;
        let registry = CorpusRegistry::from_yaml(yaml).unwrap();

        assert!(registry.get_source("central").is_some());
        assert!(registry.get_source("amsterdam").is_some());
        assert!(registry.get_source("nonexistent").is_none());
    }
}
