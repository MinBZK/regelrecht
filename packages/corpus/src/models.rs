use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// A corpus source definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Source {
    /// Unique identifier for this source.
    pub id: String,
    /// Human-readable name.
    pub name: String,
    /// Source type and configuration.
    #[serde(flatten)]
    pub source_type: SourceType,
    /// Jurisdictional scopes this source is allowed to provide.
    /// Empty means unrestricted.
    #[serde(default)]
    pub scopes: Vec<Scope>,
    /// Priority value. Lower value = higher priority.
    /// The central corpus uses priority 1.
    pub priority: u32,
}

/// Source type discriminator with configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum SourceType {
    Local {
        local: LocalSource,
    },
    #[serde(rename = "github")]
    GitHub {
        github: GitHubSource,
    },
}

/// Configuration for a local filesystem source.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalSource {
    /// Path to the regulation directory (relative to project root or absolute).
    pub path: PathBuf,
}

/// Configuration for a GitHub repository source.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubSource {
    /// GitHub repository in `owner/repo` format.
    pub repo: String,
    /// Branch to read from.
    #[serde(default = "default_branch")]
    pub branch: String,
    /// Path within the repository to the regulation directory.
    #[serde(default)]
    pub path: Option<String>,
}

fn default_branch() -> String {
    "main".to_string()
}

/// Jurisdictional scope definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scope {
    /// Scope type (e.g., "gemeente", "provincie", "waterschap").
    #[serde(rename = "type")]
    pub scope_type: String,
    /// Scope code (e.g., "GM0363" for Amsterdam).
    pub code: String,
}

/// Top-level corpus registry manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryManifest {
    /// Schema version for forward compatibility.
    pub schema_version: String,
    /// List of corpus sources.
    pub sources: Vec<Source>,
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_local_source() {
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
        let manifest: RegistryManifest = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(manifest.schema_version, "1.0");
        assert_eq!(manifest.sources.len(), 1);

        let source = &manifest.sources[0];
        assert_eq!(source.id, "central");
        assert_eq!(source.priority, 1);
        match &source.source_type {
            SourceType::Local { local } => {
                assert_eq!(local.path, PathBuf::from("corpus/regulation/nl"));
            }
            _ => panic!("Expected local source type"),
        }
    }

    #[test]
    fn test_deserialize_github_source() {
        let yaml = r#"
schema_version: "1.0"
sources:
  - id: amsterdam
    name: "Gemeente Amsterdam"
    type: github
    github:
      repo: amsterdam/regelrecht-corpus
      branch: main
      path: regulation/nl
    scopes:
      - type: gemeente
        code: GM0363
    priority: 10
"#;
        let manifest: RegistryManifest = serde_yaml::from_str(yaml).unwrap();
        let source = &manifest.sources[0];
        assert_eq!(source.id, "amsterdam");
        assert_eq!(source.priority, 10);
        assert_eq!(source.scopes.len(), 1);
        assert_eq!(source.scopes[0].code, "GM0363");

        match &source.source_type {
            SourceType::GitHub { github } => {
                assert_eq!(github.repo, "amsterdam/regelrecht-corpus");
                assert_eq!(github.branch, "main");
                assert_eq!(github.path, Some("regulation/nl".to_string()));
            }
            _ => panic!("Expected GitHub source type"),
        }
    }

    #[test]
    fn test_serialize_roundtrip() {
        let manifest = RegistryManifest {
            schema_version: "1.0".to_string(),
            sources: vec![Source {
                id: "test".to_string(),
                name: "Test Source".to_string(),
                source_type: SourceType::Local {
                    local: LocalSource {
                        path: PathBuf::from("test/path"),
                    },
                },
                scopes: vec![],
                priority: 5,
            }],
        };

        let yaml = serde_yaml::to_string(&manifest).unwrap();
        let parsed: RegistryManifest = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(parsed.sources[0].id, "test");
        assert_eq!(parsed.sources[0].priority, 5);
    }
}
