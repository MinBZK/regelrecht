//! The configuration of a cell: who records, which facts (streams), and how
//! the cell reads them back (lexostatuses).
//!
//! ```yaml
//! # cells/<cell>/cell.yaml
//! id: toeslagen
//! recording_actor: belastingdienst_toeslagen
//! streams: [streams/zorgtoeslag_aanvragen.yaml]
//! lexostatuses: lexostatuses.yaml
//! ```
//!
//! A stream only registers an event and the article that establishes it;
//! what the fact contains, the law says (see [`crate::shape`]).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::error::{setup, Result};

/// `cell.yaml`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CellFile {
    pub id: String,
    /// Who records the facts of this cell (RFC-022 §2).
    pub recording_actor: String,
    /// Stream files, relative to `cell.yaml`.
    pub streams: Vec<PathBuf>,
    /// The lexostatus file, relative to `cell.yaml`.
    #[serde(default)]
    pub lexostatuses: Option<PathBuf>,
}

/// A stream file: the events a cell records in one chronicle.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Stream {
    #[serde(rename = "$id")]
    pub id: String,
    pub recording_actor: String,
    pub chronicle: String,
    pub events: Vec<Event>,
}

/// An event: its name in the chronicle, and the article that establishes it
/// (`<regulation>#<article>`).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Event {
    pub name: String,
    pub establishes: String,
    /// For a decision: the lexostatuses the cell reads the parameters of the
    /// decision from (see [`crate::Cell::decision_inputs`]); one name or a
    /// list.
    #[serde(default, deserialize_with = "one_or_more")]
    pub reads: Vec<String>,
}

/// A name or a list of names.
fn one_or_more<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> std::result::Result<Vec<String>, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum OneOrMore {
        One(String),
        More(Vec<String>),
    }
    Ok(match OneOrMore::deserialize(d)? {
        OneOrMore::One(name) => vec![name],
        OneOrMore::More(names) => names,
    })
}

/// A lexostatus file: how the cell reads its own chronicle back.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LexostatusFile {
    pub cell: String,
    pub lexostatus_definitions: Vec<LexostatusDefinition>,
}

/// One lexostatus: a reduction of the chronicle to the parameters an article
/// reads.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LexostatusDefinition {
    pub name: String,
    /// The names of its inputs, such as `root`.
    #[serde(default)]
    pub inputs: Vec<String>,
    pub reduction: Reduction,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reduction {
    pub chronicle: String,
    pub filter: Filter,
    pub pick: Pick,
    pub derivations: BTreeMap<String, Derivation>,
}

/// Which grams take part. A value `$<input>` is the input of the lexostatus.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Filter {
    #[serde(rename = "type", default)]
    pub type_: Option<String>,
    #[serde(default)]
    pub subtype: Option<String>,
    #[serde(default)]
    pub root: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Pick {
    Latest,
}

/// How one parameter follows from the picked gram.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged, deny_unknown_fields)]
pub enum Derivation {
    /// The value of a field.
    Field {
        field: String,
        #[serde(default)]
        legal_basis: Vec<String>,
    },
    /// A moment of the gram (`effective_at`), as a date.
    Moment {
        moment: Moment,
        #[serde(default)]
        legal_basis: Vec<String>,
    },
    /// Whether a field is filled in.
    Filled {
        filled: String,
        #[serde(default)]
        legal_basis: Vec<String>,
    },
}

impl Derivation {
    /// The field of the gram it reads, if any.
    pub fn field(&self) -> Option<&str> {
        match self {
            Derivation::Field { field, .. } => Some(field),
            Derivation::Filled { filled, .. } => Some(filled),
            Derivation::Moment { .. } => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Moment {
    EffectiveAt,
}

/// A cell's configuration, with its streams and lexostatuses read.
#[derive(Debug, Clone)]
pub struct CellConfig {
    pub id: String,
    pub recording_actor: String,
    pub streams: Vec<Stream>,
    pub lexostatuses: Vec<LexostatusDefinition>,
}

fn read_text(path: &Path) -> Result<String> {
    std::fs::read_to_string(path).map_err(|e| setup(format!("{}: {e}", path.display())))
}

fn parse_yaml<T: serde::de::DeserializeOwned>(what: &str, text: &str) -> Result<T> {
    serde_yaml_ng::from_str(text).map_err(|e| setup(format!("{what}: {e}")))
}

impl CellConfig {
    /// Read `cell.yaml` and the files it names.
    pub fn load(cell_yaml: &Path) -> Result<Self> {
        let cell_text = read_text(cell_yaml)?;
        let file: CellFile = parse_yaml(&cell_yaml.display().to_string(), &cell_text)?;
        let dir = cell_yaml.parent().unwrap_or(Path::new("."));
        let streams = file
            .streams
            .iter()
            .map(|p| read_text(&dir.join(p)))
            .collect::<Result<Vec<_>>>()?;
        let lexostatuses = file
            .lexostatuses
            .as_ref()
            .map(|p| read_text(&dir.join(p)))
            .transpose()?;
        Self::from_yaml(
            &cell_text,
            &streams.iter().map(String::as_str).collect::<Vec<_>>(),
            lexostatuses.as_deref(),
        )
    }

    /// A configuration from its texts: `cell.yaml`, each stream it names
    /// (the paths in `cell.yaml` are not followed), and the lexostatuses.
    pub fn from_yaml(cell: &str, streams: &[&str], lexostatuses: Option<&str>) -> Result<Self> {
        let file: CellFile = parse_yaml("cell", cell)?;
        let streams = streams
            .iter()
            .map(|s| parse_yaml::<Stream>("stream", s))
            .collect::<Result<Vec<_>>>()?;
        for s in &streams {
            if s.recording_actor != file.recording_actor {
                return Err(setup(format!(
                    "stream '{}' is recorded by '{}', the cell by '{}'",
                    s.id, s.recording_actor, file.recording_actor
                )));
            }
        }
        let lexostatuses = match lexostatuses {
            None => Vec::new(),
            Some(text) => {
                let l: LexostatusFile = parse_yaml("lexostatuses", text)?;
                if l.cell != file.id {
                    return Err(setup(format!(
                        "lexostatuses of cell '{}' in the configuration of cell '{}'",
                        l.cell, file.id
                    )));
                }
                l.lexostatus_definitions
            }
        };
        for s in &streams {
            for e in &s.events {
                for name in &e.reads {
                    let Some(l) = lexostatuses.iter().find(|l| &l.name == name) else {
                        return Err(setup(format!(
                        "stream '{}', event '{}' reads lexostatus '{name}', which the cell does not define",
                        s.id, e.name
                    )));
                    };
                    // A decision reads the case it is taken on: the cell passes
                    // the root of the gram it refers to, and nothing else.
                    if l.inputs != ["root"] || l.reduction.filter.root.as_deref() != Some("$root") {
                        return Err(setup(format!(
                        "stream '{}', event '{}' reads lexostatus '{name}', which must have `inputs: [root]` and filter on `root: $root`",
                        s.id, e.name
                    )));
                    }
                }
            }
        }
        Ok(Self {
            id: file.id,
            recording_actor: file.recording_actor,
            streams,
            lexostatuses,
        })
    }

    /// The event by name, with its stream.
    pub fn event(&self, name: &str) -> Option<(&Stream, &Event)> {
        self.streams
            .iter()
            .find_map(|s| s.events.iter().find(|e| e.name == name).map(|e| (s, e)))
    }
}
