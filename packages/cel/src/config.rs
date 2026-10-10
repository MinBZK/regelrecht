//! The configuration of a cell: who records, which facts (streams), and how
//! the cell reads them back (lexostatuses).
//!
//! ```yaml
//! # cells/<cell>/cell.yaml
//! id: toeslagen
//! recording_actor: belastingdienst_toeslagen
//! streams: [../../chronicles/zorgtoeslag_aanvragen.yaml]
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

fn read_yaml<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    let text =
        std::fs::read_to_string(path).map_err(|e| setup(format!("{}: {e}", path.display())))?;
    serde_yaml_ng::from_str(&text).map_err(|e| setup(format!("{}: {e}", path.display())))
}

impl CellConfig {
    /// Read `cell.yaml` and the files it names.
    pub fn load(cell_yaml: &Path) -> Result<Self> {
        let file: CellFile = read_yaml(cell_yaml)?;
        let dir = cell_yaml.parent().unwrap_or(Path::new("."));
        let streams = file
            .streams
            .iter()
            .map(|p| read_yaml::<Stream>(&dir.join(p)))
            .collect::<Result<Vec<_>>>()?;
        for s in &streams {
            if s.recording_actor != file.recording_actor {
                return Err(setup(format!(
                    "stream '{}' is recorded by '{}', the cell by '{}'",
                    s.id, s.recording_actor, file.recording_actor
                )));
            }
        }
        let lexostatuses = match &file.lexostatuses {
            None => Vec::new(),
            Some(p) => {
                let l: LexostatusFile = read_yaml(&dir.join(p))?;
                if l.cell != file.id {
                    return Err(setup(format!(
                        "lexostatuses of cell '{}' in the configuration of cell '{}'",
                        l.cell, file.id
                    )));
                }
                l.lexostatus_definitions
            }
        };
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
