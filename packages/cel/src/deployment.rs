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
use crate::load;
use crate::schema::Kind;

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
    /// the policy gives it in `identifies`. A field carries no `name` (the
    /// key is the name) and no `legal_basis` (that is policy). Every message
    /// names the cell and the channel.
    pub fn identification_fields(
        &self,
        cell: &str,
        channel: &str,
        basis: &BTreeMap<String, Vec<String>>,
    ) -> Result<Vec<IdentificationField>, String> {
        let at = format!("{cell}: channel '{channel}'");
        let mut out = Vec::new();
        for (name, value) in &self.fields {
            let name = name
                .as_str()
                .ok_or_else(|| format!("{at}: a field name is not text"))?
                .to_string();
            let mut m = match value {
                serde_yaml_ng::Value::Mapping(m) => m.clone(),
                _ => return Err(format!("{at}: field '{name}': not a mapping")),
            };
            for key in ["name", "legal_basis"] {
                if m.contains_key(key) {
                    return Err(format!(
                        "{at}: field '{name}': '{key}' does not belong in the deployment (the {})",
                        if key == "name" {
                            "key is the name"
                        } else {
                            "policy gives it in identifies"
                        }
                    ));
                }
            }
            m.insert("name".into(), name.clone().into());
            if let Some(b) = basis.get(&name).filter(|b| !b.is_empty()) {
                m.insert(
                    "legal_basis".into(),
                    serde_yaml_ng::to_value(b).map_err(|e| format!("{at}: {e}"))?,
                );
            }
            out.push(
                serde_yaml_ng::from_value(serde_yaml_ng::Value::Mapping(m))
                    .map_err(|e| format!("{at}: field '{name}': {e}"))?,
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
/// `rows` of `process.yaml` had, validated against
/// `schema/chronolex/v0.3.0/synthesis.json`.
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

/// `synthesis.yaml`, validated against its schema: a source names a cell or
/// a regulation, a source of the case (`case: true`) names no input,
/// parameters, extra fields or url, and a synthesis per row has columns.
pub fn load_synthesis(file: &Path) -> Result<BTreeMap<String, SynthesisDeployment>, Vec<String>> {
    load::load(file, |text, source| {
        load::definition(text, source, Kind::Synthesis)
    })
}

/// `channels.yaml`, validated against its schema, with the fields of each
/// channel in the order of the file.
pub fn load_channels(file: &Path) -> Result<Channels, Vec<String>> {
    load::load(file, |text, source| {
        load::ordered_definition(text, source, Kind::Channels)
    })
}

/// `examples.yaml`, validated against its schema, with every path made
/// absolute against its directory.
pub fn load_examples(file: &Path) -> Result<BTreeMap<String, ExamplesDefinition>, Vec<String>> {
    let dir = file.parent().unwrap_or(Path::new("."));
    let abs = |p: &String| dir.join(p).display().to_string();
    let mut out: BTreeMap<String, ExamplesDefinition> = load::load(file, |text, source| {
        load::definition(text, source, Kind::Examples)
    })?;
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
    let channels = load_channels(&channels_file)
        .map_err(|e| errors.extend(e))
        .unwrap_or_default();
    let synthesis = match &config.synthesis {
        Some(f) => load_synthesis(f)
            .map_err(|e| errors.extend(e))
            .unwrap_or_default(),
        None => BTreeMap::new(),
    };
    let examples = match &config.examples {
        Some(f) => load_examples(f)
            .map_err(|e| errors.extend(e))
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
        let f = d
            .identification_fields("test_afnemer", "eherkenning", &basis)
            .unwrap();
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
    fn an_unknown_key_is_an_error() {
        let typo = CHANNELS.replace("    label: Inloggen", "    lable: Inloggen");
        let e = serde_yaml_ng::from_str::<Channels>(&typo).unwrap_err();
        assert!(e.to_string().contains("lable"), "{e}");
        let c: Channels =
            serde_yaml_ng::from_str(&CHANNELS.replace("numeric: true", "numerik: true")).unwrap();
        let e = c["test_afnemer"]["eherkenning"]
            .identification_fields("test_afnemer", "eherkenning", &BTreeMap::new())
            .unwrap_err();
        assert!(
            e.contains("numerik") && e.contains("test_afnemer") && e.contains("'eherkenning'"),
            "{e}"
        );
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("synthesis.yaml");
        std::fs::write(&file, "test_afnemer:\n  synthese: []\n").unwrap();
        let e = load_synthesis(&file).unwrap_err();
        assert!(e.iter().any(|f| f.contains("synthese")), "{e:?}");
    }

    /// The schema of `synthesis.yaml` keeps the rules of the synthesis: a
    /// source names a cell or a regulation, and a source of the case names
    /// no input.
    #[test]
    fn the_synthesis_is_validated_against_its_schema() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("synthesis.yaml");
        for (text, expect) in [
            (
                "test_afnemer:\n  synthesis:\n    - {cell: test_afnemer, lexostatus: besluit, case: true}\n",
                "",
            ),
            // Neither a cell nor a regulation.
            (
                "test_afnemer:\n  synthesis:\n    - {lexostatus: besluit, case: true}\n",
                "/test_afnemer/synthesis/0",
            ),
            // A source of the case with input.
            (
                "test_afnemer:\n  synthesis:\n    - {cell: test_afnemer, lexostatus: besluit, case: true, input: {root: {value: x}}}\n",
                "/test_afnemer/synthesis/0",
            ),
        ] {
            std::fs::write(&file, text).unwrap();
            match load_synthesis(&file) {
                Ok(s) => assert!(expect.is_empty(), "{text}: {s:?}"),
                Err(e) => assert!(
                    !expect.is_empty() && e.iter().all(|f| f.contains(expect)),
                    "{text}: {e:?}"
                ),
            }
        }
    }

    /// `channels.yaml` is validated against its schema, every message names
    /// the file, and the fields keep their order.
    #[test]
    fn the_channels_are_validated_against_their_schema() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("channels.yaml");
        std::fs::write(&file, CHANNELS).unwrap();
        let c = load_channels(&file).unwrap();
        assert_eq!(
            c["test_afnemer"]["eherkenning"].field_names(),
            ["kvk", "persoon"]
        );
        for (bad, expect) in [
            // A checksum without a modulus.
            (
                CHANNELS.replace("numeric: true}", "numeric: true, checksum: {weights: [1]}}"),
                "/test_afnemer/eherkenning/fields/kvk/checksum",
            ),
            (
                CHANNELS.replace("simulated", "echt"),
                "/test_afnemer/eherkenning/adapter",
            ),
            (
                CHANNELS.replace("label: Uw naam", "lable: Uw naam"),
                "/test_afnemer/eherkenning/fields/persoon",
            ),
        ] {
            std::fs::write(&file, &bad).unwrap();
            let e = load_channels(&file).unwrap_err();
            let name = file.display().to_string();
            assert!(
                e.iter().all(|f| f.starts_with(&name)) && e.iter().any(|f| f.contains(expect)),
                "{bad}: {e:?}"
            );
        }
    }

    /// The channels of the fixtures validate.
    #[test]
    fn the_fixture_channels_validate() {
        let file = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/deployment/channels.yaml");
        assert!(!load_channels(&file).unwrap().is_empty());
    }

    /// The synthesis of the fixtures validates.
    #[test]
    fn the_fixture_synthesis_validates() {
        let file = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/deployment/synthesis.yaml");
        let s = load_synthesis(&file).unwrap();
        assert!(!s.is_empty());
    }

    #[test]
    fn the_policy_gives_name_and_legal_basis_of_a_field() {
        for key in ["name: kvk", "legal_basis: ['testregeling_register#1']"] {
            let c: Channels = serde_yaml_ng::from_str(
                &CHANNELS.replace("numeric: true}", &format!("numeric: true, {key}}}")),
            )
            .unwrap();
            let e = c["test_afnemer"]["eherkenning"]
                .identification_fields("test_afnemer", "eherkenning", &BTreeMap::new())
                .unwrap_err();
            let k = key.split(':').next().unwrap();
            assert!(
                e.contains(&format!("'{k}'")) && e.contains("test_afnemer"),
                "{e}"
            );
        }
    }

    /// `examples.yaml` is validated against its schema; the message names
    /// the file. The fixture validates.
    #[test]
    fn the_examples_are_validated_against_their_schema() {
        let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/deployment/examples.yaml");
        assert!(!load_examples(&fixture).unwrap().is_empty());
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("examples.yaml");
        for (text, expect) in [
            ("test_afnemer:\n  login: [a.json]\n", "/test_afnemer"),
            (
                "test_afnemer:\n  logins: [a.yaml]\n",
                "/test_afnemer/logins/0",
            ),
            (
                "test_afnemer:\n  actions: {besluit: [a.json]}\n",
                "/test_afnemer/actions/besluit",
            ),
        ] {
            std::fs::write(&file, text).unwrap();
            let e = load_examples(&file).unwrap_err();
            let name = file.display().to_string();
            assert!(
                e.iter().all(|f| f.starts_with(&name)) && e.iter().any(|f| f.contains(expect)),
                "{text}: {e:?}"
            );
        }
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
