//! What the deployment says about a process (RFC-047): with which technique
//! a channel runs (`CELL_CHANNELS`), the synthesis and its rows until the
//! RFC-045 migration (`CELL_SYNTHESIS`), and the examples of the demo
//! (`CELL_EXAMPLES`). Each file is grouped per actor, under the id of its
//! cell. What a channel is and supplies is policy ([`crate::policy`]).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::channel::IdentificationField;
use crate::config::{Config, ExamplesDefinition, RowsDefinition, SynthesisSource};

/// The technique of a channel. Only a simulated login in this PoC.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Adapter {
    Simulated,
}

/// A channel in `channels.yaml`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChannelDeployment {
    pub adapter: Adapter,
    pub label: String,
    #[serde(default)]
    pub explanation: Option<String>,
    /// The path under `$intake`; without: the channel id.
    #[serde(default)]
    pub intake: Option<String>,
    /// How the frontend names the role of this channel; without: its name.
    #[serde(default)]
    pub role_label: Option<String>,
    /// Per field: label, pattern, check, message, numeric; in login order.
    #[serde(default)]
    pub fields: serde_yaml_ng::Mapping,
}

impl ChannelDeployment {
    /// The identification fields, in file order, each with the legal basis
    /// the policy gives it in `identifies`.
    pub fn identification_fields(
        &self,
        basis: &BTreeMap<String, Vec<String>>,
    ) -> Result<Vec<IdentificationField>, String> {
        let mut out = Vec::new();
        for (name, value) in &self.fields {
            let name = name.as_str().ok_or("a field name is not text")?.to_string();
            let mut m = match value {
                serde_yaml_ng::Value::Mapping(m) => m.clone(),
                _ => return Err(format!("field '{name}': not a mapping")),
            };
            m.insert("name".into(), name.clone().into());
            if let Some(b) = basis.get(&name).filter(|b| !b.is_empty()) {
                m.insert(
                    "legal_basis".into(),
                    serde_yaml_ng::to_value(b).map_err(|e| e.to_string())?,
                );
            }
            out.push(
                serde_yaml_ng::from_value(serde_yaml_ng::Value::Mapping(m))
                    .map_err(|e| format!("field '{name}': {e}"))?,
            );
        }
        Ok(out)
    }

    pub fn field_names(&self) -> Vec<String> {
        self.fields
            .keys()
            .filter_map(|k| k.as_str().map(str::to_string))
            .collect()
    }
}

/// `channels.yaml`: per cell id, per channel.
pub type Channels = BTreeMap<String, BTreeMap<String, ChannelDeployment>>;

/// `synthesis.yaml` for one actor: the same shape as `synthesis` and the
/// `rows` of `process.yaml` had.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SynthesisDeployment {
    #[serde(default)]
    pub synthesis: Vec<SynthesisSource>,
    #[serde(default)]
    pub assessment_rows: Vec<RowsDefinition>,
    /// Per action (the event name) its synthesis per row.
    #[serde(default)]
    pub action_rows: BTreeMap<String, Vec<RowsDefinition>>,
}

/// The deployment of a runtime with policy-based processes.
#[derive(Debug, Clone)]
pub struct Deployment {
    pub channels: Channels,
    pub channels_file: PathBuf,
    pub synthesis: BTreeMap<String, SynthesisDeployment>,
    pub synthesis_file: Option<PathBuf>,
    pub examples: BTreeMap<String, ExamplesDefinition>,
    pub examples_file: Option<PathBuf>,
}

fn read<T: serde::de::DeserializeOwned>(file: &Path) -> Result<T, String> {
    let text = std::fs::read_to_string(file).map_err(|e| format!("{}: {e}", file.display()))?;
    serde_yaml_ng::from_str(&text).map_err(|e| format!("{}: {e}", file.display()))
}

/// `examples.yaml`, with every path made absolute against its directory.
pub fn load_examples(file: &Path) -> Result<BTreeMap<String, ExamplesDefinition>, String> {
    let dir = file.parent().unwrap_or(Path::new("."));
    let abs = |p: &String| dir.join(p).display().to_string();
    let mut out: BTreeMap<String, ExamplesDefinition> = read(file)?;
    for v in out.values_mut() {
        v.logins = v.logins.iter().map(abs).collect();
        v.application = v.application.as_ref().map(abs);
        for p in v.actions.values_mut() {
            *p = abs(p);
        }
    }
    Ok(out)
}

/// The deployment, if `CELL_CHANNELS` is set.
pub fn load(config: &Config) -> Result<Option<Deployment>, Vec<String>> {
    let Some(channels_file) = config.channels.clone() else {
        return Ok(None);
    };
    let mut errors = Vec::new();
    let channels = read(&channels_file)
        .map_err(|e| errors.push(e))
        .unwrap_or_default();
    let synthesis = match &config.synthesis {
        Some(f) => read(f).map_err(|e| errors.push(e)).unwrap_or_default(),
        None => BTreeMap::new(),
    };
    let examples = match &config.examples {
        Some(f) => load_examples(f)
            .map_err(|e| errors.push(e))
            .unwrap_or_default(),
        None => BTreeMap::new(),
    };
    if !errors.is_empty() {
        return Err(errors);
    }
    Ok(Some(Deployment {
        channels,
        channels_file,
        synthesis,
        synthesis_file: config.synthesis.clone(),
        examples,
        examples_file: config.examples.clone(),
    }))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    const CHANNELS: &str = "test_afnemer:\n  eherkenning:\n    adapter: simulated\n    label: Inloggen namens een organisatie\n    fields:\n      kvk: {label: Organisatienummer, pattern: '[0-9]{8}', numeric: true}\n      persoon: {label: Uw naam}\n";

    #[test]
    fn channels_per_actor_keep_their_field_order() {
        let c: Channels = serde_yaml_ng::from_str(CHANNELS).unwrap();
        let d = &c["test_afnemer"]["eherkenning"];
        let basis = BTreeMap::from([(
            "kvk".to_string(),
            vec!["testregeling_register#1".to_string()],
        )]);
        let f = d.identification_fields(&basis).unwrap();
        let names: Vec<&str> = f.iter().map(|v| v.name.as_str()).collect();
        assert_eq!(names, ["kvk", "persoon"]);
        assert!(f[0].numeric);
        assert_eq!(f[0].legal_basis, ["testregeling_register#1"]);
    }

    #[test]
    fn an_unknown_adapter_is_an_error() {
        let e = serde_yaml_ng::from_str::<Channels>(&CHANNELS.replace("simulated", "echt"))
            .unwrap_err();
        assert!(e.to_string().contains("echt"), "{e}");
    }

    #[test]
    fn example_paths_are_relative_to_the_index() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("examples.yaml");
        std::fs::write(
            &file,
            "test_afnemer:\n  logins: [afnemer/login.json]\n  actions: {besluit_genomen: afnemer/besluit.json}\n",
        )
        .unwrap();
        let e = load_examples(&file).unwrap();
        let v = &e["test_afnemer"];
        assert_eq!(
            v.logins,
            [dir.path().join("afnemer/login.json").display().to_string()]
        );
        assert_eq!(
            v.actions["besluit_genomen"],
            dir.path()
                .join("afnemer/besluit.json")
                .display()
                .to_string()
        );
    }
}
