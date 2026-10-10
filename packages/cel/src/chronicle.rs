//! The gram, and the chronicle a cell appends grams to.
//!
//! A chronicle is a JSON Lines file per cell and chronicle name
//! (`<data_dir>/<cell>/<chronicle>.jsonl`): append-only, one gram per line.
//! A gram has its own id (uuid v7) and refers, with a name from the law text,
//! to the gram it belongs to (`refers_to.on_application`). The root of a gram
//! is the gram without a reference it leads to (the application): the
//! chronicle derives it, it is not stored.

use std::collections::BTreeMap;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::error::{setup, Result};

/// A recorded fact.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Gram {
    pub id: String,
    /// `submission` (an application) or `decretogram` (a decision).
    #[serde(rename = "type")]
    pub type_: String,
    /// For a submission: its kind in lower case (`aanvraag`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subtype: Option<String>,
    /// The stage of the procedure the fact belongs to (`AANVRAAG`, `BESLUIT`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stage: Option<String>,
    /// The event (its name in the stream).
    pub name: String,
    pub chronicle: String,
    pub recording_actor: String,
    /// The article that establishes the fact first, then the provisions the
    /// fields rest on.
    pub legal_basis: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub legal_character: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decision_type: Option<String>,
    /// The regulation the decision rests on, with the version that applied.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub regulation: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub regulation_valid_from: Option<String>,
    /// When the fact legally holds: the receipt of an application (Awb 4:13),
    /// or the moment the law binds it to.
    pub effective_at: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub effective_at_legal_basis: Vec<String>,
    /// When the cell recorded it: its own clock.
    pub recorded_at: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub refers_to: BTreeMap<String, String>,
    pub fields: Map<String, Value>,
    /// For a decision: every parameter that took part, with its value and
    /// where it came from.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub inputs: BTreeMap<String, Value>,
}

/// The grams of one chronicle of a cell.
#[derive(Debug)]
pub struct Chronicle {
    path: PathBuf,
    grams: Vec<Gram>,
}

impl Chronicle {
    /// Open the chronicle, reading what is already there.
    pub fn open(path: PathBuf) -> Result<Self> {
        let grams = match std::fs::read_to_string(&path) {
            Ok(text) => text
                .lines()
                .filter(|l| !l.trim().is_empty())
                .enumerate()
                .map(|(i, l)| {
                    serde_json::from_str(l)
                        .map_err(|e| setup(format!("{} line {}: {e}", path.display(), i + 1)))
                })
                .collect::<Result<Vec<Gram>>>()?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(e) => return Err(e.into()),
        };
        Ok(Self { path, grams })
    }

    pub fn grams(&self) -> &[Gram] {
        &self.grams
    }

    pub fn find(&self, id: &str) -> Option<&Gram> {
        self.grams.iter().find(|g| g.id == id)
    }

    /// The root of a gram: follow its references to the gram without one.
    /// A reference to a gram outside this chronicle ends the walk there.
    pub fn root_of<'a>(&'a self, gram: &'a Gram) -> &'a str {
        let mut current = gram;
        // A chronicle is append-only and a reference points backwards, so
        // the walk ends; the bound guards a hand-edited file.
        for _ in 0..=self.grams.len() {
            let Some(next) = current
                .refers_to
                .values()
                .next()
                .and_then(|id| self.find(id))
            else {
                break;
            };
            current = next;
        }
        &current.id
    }

    /// Append a gram: to the file first, then to memory.
    pub fn append(&mut self, gram: Gram) -> Result<&Gram> {
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let line = serde_json::to_string(&gram).map_err(|e| setup(e.to_string()))?;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        writeln!(file, "{line}")?;
        self.grams.push(gram);
        Ok(&self.grams[self.grams.len() - 1])
    }
}
