//! The configuration of a cell: who records, which facts (streams), and how
//! the cell reads them back (lexostatuses).
//!
//! ```yaml
//! # cells/<cell>/cell.yaml
//! id: toeslagen
//! recording_actor: belastingdienst_toeslagen
//! streams: [streams/zorgtoeslag_aanvragen.yaml]
//! lexostatuses: lexostatuses.yaml
//! # The chronicle a policy of the holder reads back (`source: {}`), bound to
//! # it as a register (`<policy>#<name of the register>`, RFC-045 §1).
//! registers:
//!   fictief_beleid_kroniek_toeslagen#kroniek: {chronicle: toeslagen}
//! ```
//!
//! A stream only registers an event and the article that establishes it;
//! what the fact contains, the law says (see [`crate::shape`]).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{setup, Result};
use crate::extension::PeriodUnit;

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
    /// The chronicles of this cell that a policy reads as a register, per
    /// `<policy>#<name of the register>` (see [`crate::register`]).
    #[serde(default)]
    pub registers: BTreeMap<String, RegisterBinding>,
}

/// Which chronicle of the cell a register is.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegisterBinding {
    pub chronicle: String,
}

/// A register of the cell: the policy that reads it (its one input without a
/// source, `source: {}`), the name the binding gives it, and the chronicle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Register {
    pub policy: String,
    pub name: String,
    pub chronicle: String,
}

impl Register {
    /// `<policy>#<name>`, as the binding and a provenance name it.
    pub fn key(&self) -> String {
        format!("{}#{}", self.policy, self.name)
    }
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
    /// For a decision of an article that decides at more than one stage of
    /// its procedure (Awir: the voorschot and the toekenning): the stage
    /// this event records. Not needed when the article decides at one.
    #[serde(default)]
    pub stage: Option<String>,
    /// What the cell reads the parameters of the decision or execution from
    /// for its case (see [`crate::Cell::decision_inputs`]): a lexostatus, or
    /// a policy of the holder that reads a register of the cell; one or a
    /// list.
    #[serde(default, deserialize_with = "one_or_more")]
    pub reads: Vec<Read>,
    /// For a decision on a submission: the article (`<regulation>#<article>`)
    /// that gives the day the holder takes it, for the period it concerns
    /// (see [`crate::Cell::due_decision`]). Its one date output is that day;
    /// null is no day of its own.
    #[serde(default)]
    pub decided_on: Option<String>,
}

/// One source of the parameters of an event's case.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(untagged)]
pub enum Read {
    /// A lexostatus of the cell, by name.
    Lexostatus(String),
    /// A policy of the holder (in the law format) that reads a register of
    /// the cell: the cell executes it with the case's `root` (and the period
    /// the case is read for) as parameters and takes its outputs
    /// (`{regulation: <policy>}`), or only those of one article
    /// (`{regulation: <policy>, article: <number>}`).
    Regulation {
        regulation: String,
        #[serde(default)]
        article: Option<String>,
    },
}

impl std::fmt::Display for Read {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Read::Lexostatus(name) => write!(f, "lexostatus '{name}'"),
            Read::Regulation {
                regulation,
                article: None,
            } => write!(f, "policy '{regulation}'"),
            Read::Regulation {
                regulation,
                article: Some(article),
            } => write!(f, "policy '{regulation}#{article}'"),
        }
    }
}

/// One read or a list of them.
fn one_or_more<'de, D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Vec<Read>, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum OneOrMore {
        More(Vec<Read>),
        One(Read),
    }
    Ok(match OneOrMore::deserialize(d)? {
        OneOrMore::One(read) => vec![read],
        OneOrMore::More(reads) => reads,
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
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LexostatusDefinition {
    pub name: String,
    /// The names of its inputs, such as `root`.
    #[serde(default)]
    pub inputs: Vec<String>,
    pub reduction: Reduction,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Reduction {
    pub chronicle: String,
    pub filter: Filter,
    pub pick: Pick,
    pub derivations: BTreeMap<String, Derivation>,
}

/// Which grams take part. A value `$<input>` is the input of the lexostatus.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Filter {
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub type_: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subtype: Option<String>,
    /// The event of the gram (its name in the stream).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub event: Option<String>,
    /// The stage of the procedure the gram belongs to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stage: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root: Option<String>,
    /// The value of the period the gram concerns (`$<input>`: the period the
    /// case is read for, such as one berekeningsjaar of an application that
    /// holds for several).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub period: Option<String>,
}

impl Filter {
    /// The input that gives the period, if the filter reads one (`$<input>`).
    pub fn period_input(&self) -> Option<&str> {
        self.period.as_deref().and_then(|p| p.strip_prefix('$'))
    }
}

/// Which of the grams that pass the filter the derivations read, in the
/// order of the chronicle at the moment of reading (`effective_at`, then
/// `recorded_at`; RFC-044).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Pick {
    /// The last one: the state at the moment of reading. None is an error.
    Latest,
    /// All of them, for a `sum`. None is a sum of nothing: zero.
    All,
}

/// How one parameter follows from the picked gram.
#[derive(Debug, Clone, Deserialize, Serialize)]
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
    /// The sum of a numeric field over every picked gram (`pick: all`).
    Sum {
        sum: String,
        #[serde(default)]
        legal_basis: Vec<String>,
    },
    /// The value of the period the gram concerns, if it is of this unit
    /// (`period: year` gives the year).
    Period {
        period: PeriodUnit,
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
            Derivation::Sum { sum, .. } => Some(sum),
            Derivation::Moment { .. } | Derivation::Period { .. } => None,
        }
    }

    /// Whether it reads many grams (`sum`) rather than one.
    fn over_many(&self) -> bool {
        matches!(self, Derivation::Sum { .. })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
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
    pub registers: Vec<Register>,
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
        // `pick: all` reads many grams and only sums them; `pick: latest`
        // reads one and does not sum it.
        for l in &lexostatuses {
            let all = l.reduction.pick == Pick::All;
            if let Some((name, _)) = l
                .reduction
                .derivations
                .iter()
                .find(|(_, d)| d.over_many() != all)
            {
                return Err(setup(format!(
                    "lexostatus '{}', derivation '{name}': `sum` goes with `pick: all`, and `pick: all` only with `sum`",
                    l.name
                )));
            }
        }
        let mut registers = Vec::new();
        for (key, binding) in &file.registers {
            let Some((policy, name)) = key.split_once('#') else {
                return Err(setup(format!(
                    "register '{key}' is not <policy>#<name of the register>"
                )));
            };
            if !streams.iter().any(|s| s.chronicle == binding.chronicle) {
                return Err(setup(format!(
                    "register '{key}' is chronicle '{}', which no stream of cell '{}' records in",
                    binding.chronicle, file.id
                )));
            }
            if registers.iter().any(|r: &Register| r.policy == policy) {
                return Err(setup(format!(
                    "policy '{policy}' reads more than one register of cell '{}'; it reads one",
                    file.id
                )));
            }
            registers.push(Register {
                policy: policy.to_string(),
                name: name.to_string(),
                chronicle: binding.chronicle.clone(),
            });
        }
        for s in &streams {
            for e in &s.events {
                for read in &e.reads {
                    let name = match read {
                        Read::Lexostatus(name) => name,
                        Read::Regulation { regulation, .. } => {
                            if !registers.iter().any(|r| &r.policy == regulation) {
                                return Err(setup(format!(
                                    "stream '{}', event '{}' reads policy '{regulation}', which reads no register of the cell (`registers` in cell.yaml)",
                                    s.id, e.name
                                )));
                            }
                            continue;
                        }
                    };
                    let Some(l) = lexostatuses.iter().find(|l| &l.name == name) else {
                        return Err(setup(format!(
                        "stream '{}', event '{}' reads lexostatus '{name}', which the cell does not define",
                        s.id, e.name
                    )));
                    };
                    // A decision reads the case it is taken on: the cell passes
                    // the root of the gram it refers to, and the period it is
                    // read for if the filter reads one; nothing else.
                    let period = l.reduction.filter.period_input();
                    let known = |i: &String| i == "root" || Some(i.as_str()) == period;
                    if !l.inputs.iter().any(|i| i == "root")
                        || !l.inputs.iter().all(known)
                        || period.is_some_and(|p| !l.inputs.iter().any(|i| i == p))
                        || l.reduction.filter.root.as_deref() != Some("$root")
                    {
                        return Err(setup(format!(
                        "stream '{}', event '{}' reads lexostatus '{name}', which must have `inputs: [root]` (and the period input its filter reads) and filter on `root: $root`",
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
            registers,
        })
    }

    /// The event by name, with its stream.
    pub fn event(&self, name: &str) -> Option<(&Stream, &Event)> {
        self.streams
            .iter()
            .find_map(|s| s.events.iter().find(|e| e.name == name).map(|e| (s, e)))
    }
}
