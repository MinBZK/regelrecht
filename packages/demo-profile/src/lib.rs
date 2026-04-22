//! Demo-profile loader voor de burger-demo (RFC-016).
//!
//! Leest een persona-YAML uit `corpus-demo/profiles/` en registreert de
//! bijbehorende data-sources in een `LawExecutionService`.
//!
//! ```no_run
//! use regelrecht_demo_profile::Profile;
//! use regelrecht_engine::LawExecutionService;
//!
//! let yaml = std::fs::read_to_string("corpus-demo/profiles/merijn.yaml").unwrap();
//! let profile = Profile::from_yaml(&yaml).unwrap();
//!
//! let mut service = LawExecutionService::new();
//! // ... load laws ...
//! profile.register_into(&mut service).unwrap();
//! ```

use regelrecht_engine::{LawExecutionService, Value};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

/// A demo persona. Contains identity metadata and the data-sources that
/// describe the persona's situation (income, household, insurance, etc.).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub bsn: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub avatar: String,
    #[serde(default)]
    pub data_sources: Vec<DataSourceSpec>,
}

/// One registered data-source: a table of records coming from a given service
/// (RVIG, BELASTINGDIENST, ...), keyed by a chosen field (usually `bsn`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataSourceSpec {
    /// Source service (RVIG, RVZ, BELASTINGDIENST, DJI, DUO, ...).
    /// Descriptive only — the engine looks up by field name.
    pub service: String,
    /// Table name under this service (e.g. `personal_data`, `box1`).
    /// Passed to the engine as the data-source name.
    pub table: String,
    /// Field used as the record-key (case-insensitive). Usually `bsn`.
    pub key: String,
    pub records: Vec<BTreeMap<String, Value>>,
}

/// Errors that can occur while loading or registering a profile.
#[derive(Debug, thiserror::Error)]
pub enum ProfileError {
    #[error("failed to read profile file: {0}")]
    Io(#[from] std::io::Error),
    #[error("failed to parse profile YAML: {0}")]
    Parse(#[from] serde_yaml_ng::Error),
    #[error("failed to register data source '{table}': {reason}")]
    Register { table: String, reason: String },
}

impl Profile {
    /// Parse a profile from a YAML string.
    pub fn from_yaml(yaml: &str) -> Result<Self, ProfileError> {
        Ok(serde_yaml_ng::from_str(yaml)?)
    }

    /// Read and parse a profile from disk.
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, ProfileError> {
        let yaml = std::fs::read_to_string(path)?;
        Self::from_yaml(&yaml)
    }

    /// Register every data-source in this profile into the given service.
    pub fn register_into(&self, service: &mut LawExecutionService) -> Result<(), ProfileError> {
        for spec in &self.data_sources {
            service
                .register_dict_source(&spec.table, &spec.key, spec.records.clone())
                .map_err(|e| ProfileError::Register {
                    table: spec.table.clone(),
                    reason: e.to_string(),
                })?;
        }
        Ok(())
    }

    /// JSON-representation suitable for handing to WASM / browser code.
    pub fn to_json(&self) -> Result<String, ProfileError> {
        serde_json::to_string(self).map_err(|e| ProfileError::Register {
            table: "<json>".to_string(),
            reason: e.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use regelrecht_engine::ArticleBasedLaw;
    use std::path::PathBuf;

    fn repo_root() -> PathBuf {
        // CARGO_MANIFEST_DIR = packages/demo-profile → go up two
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .expect("repo root")
            .to_path_buf()
    }

    #[test]
    fn parses_merijn_profile() {
        let path = repo_root().join("corpus-demo/profiles/merijn.yaml");
        let profile = Profile::from_file(&path).expect("profile loads");

        assert_eq!(profile.bsn, "100000001");
        assert_eq!(profile.name, "Merijn van der Meer");
        assert!(!profile.data_sources.is_empty());

        // Sanity: every record contains the keyed field.
        for spec in &profile.data_sources {
            for record in &spec.records {
                let key_present = record.keys().any(|k| k.eq_ignore_ascii_case(&spec.key));
                assert!(
                    key_present,
                    "record in table {:?} missing key {:?}",
                    spec.table, spec.key
                );
            }
        }
    }

    #[test]
    fn merijn_qualifies_for_zorgtoeslag_2025() {
        let root = repo_root();
        let profile =
            Profile::from_file(root.join("corpus-demo/profiles/merijn.yaml")).expect("profile");

        // Load every law in the national corpus — zorgtoeslag depends on
        // zorgverzekeringswet, awir, inkomstenbelasting, and standaardpremie.
        let mut service = LawExecutionService::new();
        let corpus_root = root.join("corpus/regulation/nl");
        let mut loaded = 0usize;
        for entry in walkdir::WalkDir::new(&corpus_root)
            .into_iter()
            .filter_map(Result::ok)
        {
            if entry.file_type().is_file()
                && entry
                    .path()
                    .extension()
                    .map(|e| e == "yaml")
                    .unwrap_or(false)
            {
                let law = ArticleBasedLaw::from_yaml_file(entry.path()).expect("parse law");
                service.load_law_struct(law).expect("load law");
                loaded += 1;
            }
        }
        assert!(loaded >= 5, "expected at least 5 laws loaded, got {loaded}");

        profile
            .register_into(&mut service)
            .expect("register merijn");

        let mut parameters = BTreeMap::new();
        parameters.insert("bsn".to_string(), Value::String(profile.bsn.clone()));

        let result = service
            .evaluate_law_output(
                "zorgtoeslagwet",
                "hoogte_zorgtoeslag",
                parameters,
                "2025-06-01",
            )
            .expect("evaluate");

        let amount = result
            .outputs
            .get("hoogte_zorgtoeslag")
            .expect("hoogte_zorgtoeslag in outputs");

        match amount {
            Value::Int(v) => assert!(*v > 0, "expected positive amount, got {v}"),
            Value::Float(v) => assert!(*v > 0.0, "expected positive amount, got {v}"),
            other => panic!("unexpected amount type: {other:?}"),
        }
    }
}
