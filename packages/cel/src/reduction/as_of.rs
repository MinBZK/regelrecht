//! The as-of point of a reduction: at which moment the chronicle is read
//! ("time travel", paper P:94). Facts are never deleted, so every earlier
//! state can be recovered.
//!
//! A gram has two times (see [`crate::gram`]), and a reduction can read as of
//! either of them, or as of both (bitemporal):
//!
//! - `as_of` (valid time): the state as it legally held at T, with everything
//!   the cell knows now. A gram counts if its `effective_at` is at or before
//!   T. A fact that was recorded later but held earlier (a paper application,
//!   received on day 1 and entered on day 5) therefore counts for T = day 3;
//!   a fact from after T (a removal on day 4) does not.
//!   An `effective_at` is never after the recording: what has yet to happen
//!   is not a fact. A decision that takes effect on a later day is recorded on
//!   the day it was taken, with the effective day as a field; a reduction
//!   cannot (yet) read as of that effective day.
//! - `known_at` (transaction time): the state as the cell knew it at T. A
//!   gram counts if its `recorded_at` is at or before T. This shows what an
//!   earlier decision rested on.
//!
//! Without an as-of point every gram counts: the state as it holds now and is
//! known now, including what only takes effect later. A process that means a
//! date (the reference date of a decision, the start of a window) therefore
//! passes it along. An as-of point is a date (`YYYY-MM-DD`, the whole day
//! counts) or a moment with a time zone. Over HTTP they are the query
//! parameters `as_of` and `known_at` of `GET lexostatus`; those names are
//! therefore not an input of a lexostatus (the schema rejects them).

use serde_json::{Map, Value};

use crate::date::TimePoint;
use crate::gram::Gram;

/// What a reduction reads as of. The default is no as-of point: every gram counts.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AsOf {
    /// Valid time: only grams with `effective_at` at or before this point.
    pub as_of: Option<TimePoint>,
    /// Transaction time: only grams with `recorded_at` at or before this
    /// point.
    pub known_at: Option<TimePoint>,
}

impl AsOf {
    /// The state as it legally held at `t`, with what is known now.
    pub fn at(t: TimePoint) -> Self {
        Self {
            as_of: Some(t),
            known_at: None,
        }
    }

    /// Read the as-of point from the query of a request, and remove the keys:
    /// what remains are the inputs of the lexostatus.
    pub fn from_query(query: &mut Map<String, Value>) -> Result<Self, String> {
        let mut read = |key: &str| -> Result<Option<TimePoint>, String> {
            match query.remove(key) {
                None => Ok(None),
                Some(Value::String(t)) => TimePoint::read(key, &t).map(Some),
                Some(w) => Err(format!("invalid {key} '{w}'")),
            }
        };
        Ok(Self {
            as_of: read("as_of")?,
            known_at: read("known_at")?,
        })
    }

    /// The as-of point as query parameters: `as_of`, then `known_at`. The
    /// schema of `lexostatus.json` rejects those names as input.
    pub fn query(&self) -> Vec<(&'static str, String)> {
        [("as_of", self.as_of), ("known_at", self.known_at)]
            .into_iter()
            .filter_map(|(k, t)| t.map(|t| (k, t.to_string())))
            .collect()
    }

    /// Whether a gram counts at this as-of point. An invalid moment is an
    /// error, not a silent exclusion.
    pub fn let_through(&self, gram: &Gram) -> Result<bool, String> {
        if let Some(t) = &self.as_of {
            if !t.covers(&gram.moment()?) {
                return Ok(false);
            }
        }
        if let Some(t) = &self.known_at {
            if !t.covers(&gram.recorded()?) {
                return Ok(false);
            }
        }
        Ok(true)
    }
}
