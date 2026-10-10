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

use chrono::{DateTime, NaiveDate};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::error::{refused, setup, Result};
use crate::extension::PeriodUnit;

/// The period a fact concerns (a calendar year: `{unit: year, value: 2025}`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Period {
    pub unit: PeriodUnit,
    pub value: i32,
}

impl Period {
    /// The day the law of the period is the law on: its first day.
    pub fn first_day(&self) -> Option<NaiveDate> {
        match self.unit {
            PeriodUnit::Year => NaiveDate::from_ymd_opt(self.value, 1, 1),
        }
    }
}

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
    /// The article that establishes the fact (`<regulation>#<article>`).
    pub establishes: String,
    /// The legal basis of the fact: the article that establishes it (for a
    /// decision the article that decides, for an execution or a receipt its
    /// article).
    pub legal_basis: Vec<String>,
    /// Per field, the provisions it rests on. A field that rests on none
    /// has no entry.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub field_basis: BTreeMap<String, Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub legal_character: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decision_type: Option<String>,
    /// The regulation the decision rests on, with the version that applied.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub regulation: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub regulation_valid_from: Option<String>,
    /// The period the fact concerns, if the law says it concerns one: the
    /// regulation applied is the one of that period.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub period: Option<Period>,
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
    /// The file it is kept in; `None` for a chronicle in memory.
    path: Option<PathBuf>,
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
        Ok(Self {
            path: Some(path),
            grams,
        })
    }

    /// A chronicle in memory, holding `grams`.
    pub fn in_memory(grams: Vec<Gram>) -> Self {
        Self { path: None, grams }
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

    /// Append a gram: to the file first (if there is one), then to memory.
    ///
    /// Time only moves forward: a gram recorded before the last gram of the
    /// chronicle was recorded is refused. A fact may hold from an earlier
    /// moment (`effective_at`), but the cell cannot record it in the past.
    pub fn append(&mut self, gram: Gram) -> Result<&Gram> {
        if self.grams.iter().any(|g| g.id == gram.id) {
            return Err(setup(format!(
                "gram '{}' is already in the chronicle",
                gram.id
            )));
        }
        let recorded = |g: &Gram| {
            DateTime::parse_from_rfc3339(&g.recorded_at).map_err(|e| {
                setup(format!(
                    "gram '{}': recorded_at '{}': {e}",
                    g.id, g.recorded_at
                ))
            })
        };
        if let Some(last) = self.grams.last() {
            if recorded(&gram)? < recorded(last)? {
                return Err(refused(format!(
                    "gram '{}' is recorded at {}, before the last gram of the chronicle ('{}', {}): time does not go back",
                    gram.id, gram.recorded_at, last.id, last.recorded_at
                )));
            }
        }
        if let Some(path) = &self.path {
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir)?;
            }
            let line = serde_json::to_string(&gram).map_err(|e| setup(e.to_string()))?;
            let mut file = OpenOptions::new().create(true).append(true).open(path)?;
            writeln!(file, "{line}")?;
        }
        self.grams.push(gram);
        Ok(&self.grams[self.grams.len() - 1])
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    fn gram(id: &str, recorded_at: &str) -> Gram {
        Gram {
            id: id.into(),
            type_: "submission".into(),
            subtype: None,
            stage: None,
            name: "x".into(),
            chronicle: "c".into(),
            recording_actor: "a".into(),
            establishes: "w#1".into(),
            legal_basis: Vec::new(),
            field_basis: BTreeMap::new(),
            legal_character: None,
            decision_type: None,
            regulation: None,
            regulation_valid_from: None,
            period: None,
            effective_at: recorded_at.into(),
            effective_at_legal_basis: Vec::new(),
            recorded_at: recorded_at.into(),
            refers_to: BTreeMap::new(),
            fields: Map::new(),
            inputs: BTreeMap::new(),
        }
    }

    #[test]
    fn time_does_not_go_back() {
        let mut c = Chronicle::in_memory(Vec::new());
        c.append(gram("1", "2025-03-10T09:00:00+01:00")).unwrap();
        // The same moment, and later (also in another offset), are fine.
        c.append(gram("2", "2025-03-10T09:00:00+01:00")).unwrap();
        c.append(gram("3", "2025-03-10T10:30:00+02:00")).unwrap();
        let e = c
            .append(gram("4", "2025-03-10T08:00:00+01:00"))
            .unwrap_err();
        assert!(matches!(e, crate::Error::Refused(_)), "{e}");
        assert_eq!(c.grams().len(), 3);
    }
}
