//! The configuration of a cell: who records, which facts (streams), and
//! which chronicle a policy of the holder reads back (registers).
//!
//! ```yaml
//! # cells/<cell>/cell.yaml
//! id: toeslagen
//! recording_actor: belastingdienst_toeslagen
//! streams: [streams/zorgtoeslag_aanvragen.yaml]
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
    /// for its case besides the application (see
    /// [`crate::Cell::decision_inputs`]): a policy of the holder that reads a
    /// register of the cell, or one article of it; one or a list.
    #[serde(default, deserialize_with = "one_or_more")]
    pub reads: Vec<Read>,
    /// The article (`<regulation>#<article>`) that gives the day the holder
    /// takes the decision, for the period it concerns (see
    /// [`crate::Cell::due_decision`], [`crate::Cell::due_ex_officio`]). Its
    /// one date output is that day; null is no day yet, and then the
    /// decision is not due, except the decision the application asks for,
    /// for the period it asks for, which whoever answers the application
    /// takes (see `Cell::check_due`).
    #[serde(default)]
    pub decided_on: Option<String>,
    /// For a decision on no submission (ex officio, such as the aanslag of
    /// AWR 11): the parameter that gives the period it concerns. The law
    /// names the period of a decision on an application with origin role
    /// TIJDVAK, which the schema allows only for what the applicant
    /// chooses; for a decision nobody applied for, the stream says it (own
    /// choice). Refused when the law already names one.
    #[serde(default)]
    pub period: Option<crate::extension::PeriodParameter>,
    /// For a decision on no submission (with `period`): the parameters that
    /// say whom it concerns, such as the BSN of the aanslag. The cell takes
    /// such a decision once per period per subject: two decisions with the
    /// same values for these parameters and the same period are the same
    /// decision, whatever else they were given. Required with `period`.
    #[serde(default)]
    pub subject: Vec<String>,
    /// For a decision on no submission (with `period`): the first period the
    /// holder decides over, for every subject. Without it, the cell starts
    /// at the first decision about the subject, or, before the first, at the
    /// period before the one of now (see [`crate::Cell::due_ex_officio`],
    /// which asks nothing before it, and refuses more than
    /// [`crate::cell::MAX_EX_OFFICIO_PERIODS`] periods before now).
    #[serde(default)]
    pub first_period: Option<i32>,
}

/// One source of the parameters of an event's case: a policy of the holder
/// (in the law format) that reads a register of the cell. The cell executes
/// it with the case's `root` (and the period the case is read for) as
/// parameters and takes its outputs (`{regulation: <policy>}`), or only
/// those of one article (`{regulation: <policy>, article: <number>}`). What
/// a decision asks of the application it is taken on, it reads from that
/// application without a `reads` (see [`crate::Cell::decision_inputs`]).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Read {
    pub regulation: String,
    #[serde(default)]
    pub article: Option<String>,
}

impl Read {
    /// Whether it reads article `number` of `policy`: that article, or the
    /// whole policy.
    pub fn reads_article(&self, policy: &str, number: &str) -> bool {
        self.regulation == policy && self.article.as_deref().is_none_or(|a| a == number)
    }
}

impl std::fmt::Display for Read {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.article {
            None => write!(f, "policy '{}'", self.regulation),
            Some(article) => write!(f, "policy '{}#{article}'", self.regulation),
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

/// A cell's configuration, with its streams read.
#[derive(Debug, Clone)]
pub struct CellConfig {
    pub id: String,
    pub recording_actor: String,
    pub streams: Vec<Stream>,
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
        Self::from_yaml(
            &cell_text,
            &streams.iter().map(String::as_str).collect::<Vec<_>>(),
        )
    }

    /// A configuration from its texts: `cell.yaml` and each stream it names
    /// (the paths in `cell.yaml` are not followed).
    pub fn from_yaml(cell: &str, streams: &[&str]) -> Result<Self> {
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
                    if !registers.iter().any(|r| r.policy == read.regulation) {
                        return Err(setup(format!(
                            "stream '{}', event '{}' reads policy '{}', which reads no register of the cell (`registers` in cell.yaml)",
                            s.id, e.name, read.regulation
                        )));
                    }
                }
            }
        }
        Ok(Self {
            id: file.id,
            recording_actor: file.recording_actor,
            streams,
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
