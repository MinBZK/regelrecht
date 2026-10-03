//! Reduction to lexostatus: load the lexostatus definitions, and reduce a
//! chronicle to the parameters of an article.
//!
//! A lexostatus definition narrows the chronicle with `filter` and, if needed,
//! picks a gram with `pick`. Per parameter a derivation derives a value, in
//! one of two ways:
//!
//! - **on the chosen gram**: `field`, `year_of`, `filled`, `equals`, `table`
//!   with `each_row` or `one_row` (and `only_where`), and `moment`;
//! - **over a collection of grams**, with its own `filter`: `exists`, `sum`,
//!   and `pick: latest` with `field`, `year_of` or `contains`.
//!
//! No gram means, within the cell's own chronicle, "no" (`exists`,
//! `contains`) or zero (`sum`): the cell only speaks about its own chronicle.
//! A value that is not there (`field` on an empty field, `pick` without a
//! gram) is left out; nothing is filled in, unless the definition says with
//! `no_gram` how it reads absence (for example null: did not happen).
//!
//! With `group_by: root` a lexostatus is a **list**: one row per root (an
//! application and what follows it) of which at least one gram passes
//! `filter`, and with `without` no gram passes that filter. `pick` and the
//! derivations then work per root: `pick` among the grams through `filter`,
//! a derivation over a collection over every gram of the case (so a column
//! can say when the case was decided). A list is for the consumer, such as a
//! handler with a worklist; it has no parameters and never goes to the
//! engine.
//!
//! `extra_fields` derives values that are not parameters, such as the input of
//! a synthesis source. They are kept separately in the lexostatus and never go
//! to the engine. Field paths are relative to `fields` of the gram, dotted.
//!
//! `pick: latest` picks in time: the gram with the latest `effective_at`
//! (when the fact legally holds), on an equal moment the last recorded
//! (`recorded_at`), and after that the last added. A reduction can read as of
//! an earlier moment ([`AsOf`]): then only the grams of that moment count.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use chrono::Datelike;

use crate::date;
use crate::gram::Gram;

mod as_of;
mod case_state;
mod definition;
mod worklist;

pub use as_of::*;
pub use case_state::*;
pub use definition::*;
pub use worklist::{cases_definition, worklist, worklist_definition, CASES, WORKLIST};

impl Derivation {
    /// Apply a derivation to the chosen gram. `None`: the gram says nothing
    /// about it and the parameter is left out; nothing is filled in. A
    /// derivation over a collection of grams gives `None` here; see
    /// [`Derivation::apply_at_collection`]. A gram that does not have the
    /// shape the derivation reads (an invalid `effective_at`, a table row that
    /// is not an object) is an error, not a missing value.
    pub fn apply(&self, gram: &Gram) -> Result<Option<Value>, String> {
        Ok(match self {
            Derivation::Field { field } => gram.field(field).filter(|w| filled(w)).cloned(),
            Derivation::YearOf { year_of } => year_from(year_of, gram.field(year_of))?,
            Derivation::PeriodOf { period_of, period } => {
                period_from(period_of, gram.field(period_of), *period)?
            }
            Derivation::Filled { filled: path } => {
                Some(Value::Bool(gram.field(path).is_some_and(filled)))
            }
            Derivation::Equals { equals } => gram
                .field(&equals.field)
                .filter(|w| filled(w))
                .map(|w| Value::Bool(w == &equals.value)),
            Derivation::EachRow {
                table,
                each_row,
                only_where,
            } => {
                let rows = rows(gram, table)?;
                let each = rows
                    .iter()
                    .filter(|r| {
                        only_where
                            .as_ref()
                            .is_none_or(|k| r.get(k) == Some(&Value::Bool(true)))
                    })
                    .all(|r| r.get(each_row).is_some_and(filled));
                Some(Value::Bool(!rows.is_empty() && each))
            }
            Derivation::OneRow { table, one_row } => Some(Value::Bool(
                rows(gram, table)?
                    .iter()
                    .any(|r| r.get(one_row) == Some(&Value::Bool(true))),
            )),
            Derivation::Moment {
                moment: Moment::EffectiveAt,
            } => Some(Value::String(date::reference_date(&gram.moment()?))),
            Derivation::Moment {
                moment: Moment::RecordedAt,
            } => Some(Value::String(date::reference_date(&gram.recorded()?))),
            Derivation::LatestField { .. }
            | Derivation::LatestMoment { .. }
            | Derivation::LatestYearOf { .. }
            | Derivation::LatestPeriodOf { .. }
            | Derivation::LatestContains { .. }
            | Derivation::Exists { .. }
            | Derivation::Sum { .. }
            | Derivation::Collect { .. } => None,
        })
    }

    /// Apply a derivation over a collection to the grams that already passed
    /// the filter of the lexostatus. It filters further itself.
    pub fn apply_at_collection(
        &self,
        inputs: &Map<String, Value>,
        grams: &[&Gram],
    ) -> Result<Option<Value>, String> {
        let Some(filter) = self.filter() else {
            return Ok(None);
        };
        let mut passed = Vec::new();
        for g in grams {
            if fits(filter, inputs, g)? {
                passed.push(*g);
            }
        }
        Ok(match self {
            Derivation::Exists { filled: None, .. } => Some(Value::Bool(!passed.is_empty())),
            Derivation::Exists {
                filled: Some(field),
                ..
            } => Some(Value::Bool(
                passed.iter().any(|g| g.field(field).is_some_and(filled)),
            )),
            Derivation::Collect { collect, .. } => Some(Value::Array(
                passed
                    .iter()
                    .map(|g| {
                        Value::Object(
                            collect
                                .iter()
                                .map(|v| (v.clone(), g.field(v).cloned().unwrap_or(Value::Null)))
                                .collect(),
                        )
                    })
                    .collect(),
            )),
            Derivation::Sum { sum, .. } => {
                let (mut whole, mut real, mut only_whole) = (0_i64, 0.0_f64, true);
                for g in &passed {
                    match g.field(sum) {
                        None | Some(Value::Null) => {}
                        Some(Value::Number(n)) => {
                            match n.as_i64() {
                                Some(i) => whole = whole.saturating_add(i),
                                None => only_whole = false,
                            }
                            real += n.as_f64().unwrap_or(0.0);
                        }
                        // Not a number: the sum cannot be derived.
                        Some(_) => return Ok(None),
                    }
                }
                if only_whole {
                    Some(Value::from(whole))
                } else {
                    serde_json::Number::from_f64(real).map(Value::Number)
                }
            }
            Derivation::LatestField { field, no_gram, .. } => match latest(&passed)? {
                Some(g) => g.field(field).filter(|w| filled(w)).cloned(),
                None => no_gram.clone(),
            },
            Derivation::LatestMoment {
                moment, no_gram, ..
            } => match latest(&passed)? {
                Some(g) => Some(Value::String(date::reference_date(&match moment {
                    Moment::EffectiveAt => g.moment()?,
                    Moment::RecordedAt => g.recorded()?,
                }))),
                None => no_gram.clone(),
            },
            Derivation::LatestYearOf {
                year_of, no_gram, ..
            } => match latest(&passed)? {
                Some(g) => year_from(year_of, g.field(year_of))?,
                None => no_gram.clone(),
            },
            Derivation::LatestPeriodOf {
                period_of,
                period,
                no_gram,
                ..
            } => match latest(&passed)? {
                Some(g) => period_from(period_of, g.field(period_of), *period)?,
                None => no_gram.clone(),
            },
            Derivation::LatestContains { contains, .. } => Some(Value::Bool(
                latest(&passed)?
                    .and_then(|g| g.field(&contains.field))
                    .and_then(Value::as_array)
                    .is_some_and(|list| list.contains(&contains.value)),
            )),
            _ => None,
        })
    }
}

/// The latest gram in time: the latest `effective_at`, on an equal moment the
/// latest `recorded_at`, and after that the one added later.
fn latest<'g>(grams: &[&'g Gram]) -> Result<Option<&'g Gram>, String> {
    let mut chosen: Option<&Gram> = None;
    for gram in grams {
        // Even a single gram is parsed: an invalid moment is an error.
        gram.moment()?;
        gram.recorded()?;
        if chosen.map_or(Ok(true), |g| gram.time_order(g).map(|o| o.is_ge()))? {
            chosen = Some(gram);
        }
    }
    Ok(chosen)
}

/// The date in the field `path` of a gram (`YYYY-MM-DD`, or a moment with a
/// time zone). No value (the field is absent or null): nothing, and the
/// parameter is left out. A value that is not a date is an error: the gram
/// does not have the shape the derivation reads.
fn date_in(path: &str, value: Option<&Value>) -> Result<Option<chrono::NaiveDate>, String> {
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(t)) => date::date_of(t)
            .map(Some)
            .ok_or_else(|| format!("field '{path}': '{t}' is not a date")),
        Some(w) => Err(format!("field '{path}': {w} is not a date")),
    }
}

/// Whether every field of a gram that a lexostatus of its chronicle reads as a
/// date (`year_of`, `period_of`) is a date or empty. A chronicle is not
/// rewritten: such a gram would make every reduction over that chronicle
/// fail, so the cell refuses it when recording.
pub fn dates_in_order<'d>(
    definitions: impl IntoIterator<Item = &'d LexostatusDefinition>,
    gram: &Gram,
) -> Result<(), String> {
    for def in definitions {
        if def.reduction.chronicle != gram.chronicle {
            continue;
        }
        for (name, a) in def.all_derivations() {
            let path = match &a.derivation {
                Derivation::YearOf { year_of } | Derivation::LatestYearOf { year_of, .. } => {
                    year_of
                }
                Derivation::PeriodOf { period_of, .. }
                | Derivation::LatestPeriodOf { period_of, .. } => period_of,
                _ => continue,
            };
            date_in(path, gram.field(path))
                .map_err(|f| format!("{f} (lexostatus '{}' derives '{name}' from it)", def.name))?;
        }
    }
    Ok(())
}

/// The year of the date in the field `path` (see `date_in`).
pub fn year_from(path: &str, value: Option<&Value>) -> Result<Option<Value>, String> {
    Ok(date_in(path, value)?.map(|d| Value::from(i64::from(d.year()))))
}

/// The first day of the period in which the date in the field `path` falls
/// (see `date_in`), as `YYYY-MM-DD`. A derivation without a period is an
/// error: loading sets it from the regulation, and if that failed, there is
/// no period to choose.
pub fn period_from(
    path: &str,
    value: Option<&Value>,
    period: Option<Period>,
) -> Result<Option<Value>, String> {
    let period = period
        .ok_or_else(|| format!("period_of '{path}' without period (year, quarter or month)"))?;
    Ok(date_in(path, value)?
        .and_then(|d| period.start(d))
        .map(|b| Value::String(b.format("%Y-%m-%d").to_string())))
}

/// Filled: not null, not an empty text, not an empty list or empty object.
pub fn filled(w: &Value) -> bool {
    match w {
        Value::Null => false,
        Value::String(s) => !s.trim().is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
        _ => true,
    }
}

/// The rows of a table field. No table (or null): no rows. A value that is
/// not a list of objects is an error.
fn rows<'g>(gram: &'g Gram, table: &str) -> Result<Vec<&'g Map<String, Value>>, String> {
    let rows = match gram.field(table) {
        None | Some(Value::Null) => return Ok(Vec::new()),
        Some(Value::Array(a)) => a,
        Some(_) => {
            return Err(format!(
                "gram '{}': table field '{table}' is not a list of rows",
                gram.name
            ))
        }
    };
    rows.iter()
        .enumerate()
        .map(|(i, r)| {
            r.as_object().ok_or_else(|| {
                format!(
                    "gram '{}': row {table}[{i}] is not an object with columns",
                    gram.name
                )
            })
        })
        .collect()
}

/// A lexostatus: the parameters a reduction yields.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lexostatus {
    pub name: String,
    /// The root of the chosen gram, if the definition picks one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effective_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recorded_at: Option<String>,
    /// The as-of point the reduction was made at (see [`AsOf`]); omitted
    /// without an as-of point.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub as_of: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub known_at: Option<String>,
    pub parameters: BTreeMap<String, Value>,
    /// Values that are not parameters; they never go to the engine.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub extra_fields: BTreeMap<String, Value>,
    /// Parameters with a derivation about which the chronicle says nothing.
    #[serde(default)]
    pub not_derived: Vec<String>,
    /// For a list lexostatus (`group_by`): the rows. A list is for the consumer
    /// and never goes to the engine; `parameters` is then empty.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub list: Option<Vec<Row>>,
    /// Along which route the cell reduced: only in a runtime with the engine
    /// route (`CELL_REDUCTION`, see [`crate::lexostatus_engine`]). Omitted for
    /// the reduction DSL without a switch, so that output stays the same.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reduction: Option<ReductionRoute>,
}

/// The route of a reduction: the reduction DSL or an engine run of a
/// regulation (experiment A).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReductionRoute {
    /// `engine` or `dsl`.
    pub route: String,
    /// For `engine`: the regulation that is the lexostatus.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub regulation: Option<String>,
    /// For `dsl` in a runtime with the engine route: why this lexostatus goes
    /// through the DSL anyway (from the binding file).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// How long the reduction took, in microseconds.
    pub duration_us: u64,
    /// For `compare`: how long the same reduction took through the DSL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dsl_duration_us: Option<u64>,
    /// The trace of the engine run, if it was requested.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trace_text: Option<String>,
}

impl Lexostatus {
    /// A lexostatus without values: the chronicle says nothing about it.
    pub fn empty(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            root: None,
            effective_at: None,
            recorded_at: None,
            as_of: None,
            known_at: None,
            parameters: BTreeMap::new(),
            extra_fields: BTreeMap::new(),
            not_derived: Vec::new(),
            list: None,
            reduction: None,
        }
    }

    /// The value the lexostatus delivers under a name, as a parameter or as an
    /// extra field; `None` if it is not there or is null.
    pub fn field(&self, name: &str) -> Option<&Value> {
        self.parameters
            .get(name)
            .or_else(|| self.extra_fields.get(name))
            .filter(|w| !w.is_null())
    }
}

/// A row of a list lexostatus: a root.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Row {
    pub root: String,
    /// The chosen gram of the root, if the definition picks one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effective_at: Option<String>,
    /// The derivations and extra fields, per case. Not parameters.
    pub fields: BTreeMap<String, Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub not_derived: Vec<String>,
}

/// The parameters of a lexostatus that say something is absent: a presence
/// derivation (see [`Derivation::assesses_presence`]) with the value false.
pub fn absent(
    definition: &LexostatusDefinition,
    parameters: &BTreeMap<String, Value>,
) -> Vec<String> {
    definition
        .reduction
        .derivations
        .iter()
        .filter(|(name, a)| {
            a.assesses_presence() && parameters.get(*name) == Some(&Value::Bool(false))
        })
        .map(|(name, _)| name.clone())
        .collect()
}

/// Whether a gram passes the filter. A value `$x` comes from the inputs.
pub fn fits(filter: &Filter, inputs: &Map<String, Value>, gram: &Gram) -> Result<bool, String> {
    for (key, expected) in filter {
        let expected = match expected.strip_prefix('$') {
            Some(input) => inputs
                .get(input)
                .and_then(Value::as_str)
                .ok_or_else(|| format!("input '{input}' is absent"))?,
            None => expected.as_str(),
        };
        let equals = match gram.attribute(key) {
            Some(value) => value == Some(expected),
            None => match gram.field(key) {
                Some(Value::String(s)) => s == expected,
                Some(w @ (Value::Number(_) | Value::Bool(_))) => *w.to_string() == *expected,
                _ => false,
            },
        };
        if !equals {
            return Ok(false);
        }
    }
    Ok(true)
}

/// What the derivations over a group of grams yield.
struct Yield<'g> {
    chosen: Option<&'g Gram>,
    parameters: BTreeMap<String, Value>,
    extra_fields: BTreeMap<String, Value>,
    not_derived: Vec<String>,
}

/// Pick a gram from `passed` if needed and apply every derivation. A
/// derivation over a collection reads `collection`: the grams through
/// `filter` for a single lexostatus, every gram of the case for a list.
/// `None`: the definition picks a gram and there is none.
fn derive_from<'g>(
    definition: &LexostatusDefinition,
    inputs: &Map<String, Value>,
    passed: &[&'g Gram],
    collection: &[&'g Gram],
) -> Result<Option<Yield<'g>>, String> {
    let r = &definition.reduction;
    let chosen = match r.pick {
        Some(Pick::Latest) => match latest(passed)? {
            Some(g) => Some(g),
            None => return Ok(None),
        },
        None => None,
    };
    let mut out = Yield {
        chosen,
        parameters: BTreeMap::new(),
        extra_fields: BTreeMap::new(),
        not_derived: Vec::new(),
    };
    for (name, derivation, extra) in r
        .derivations
        .iter()
        .map(|(n, a)| (n, a, false))
        .chain(r.extra_fields.iter().map(|(n, a)| (n, a, true)))
    {
        let value = if derivation.at_chosen_gram() {
            let gram = chosen.ok_or_else(|| {
                format!(
                    "derivation '{name}' reads the chosen gram, but lexostatus '{}' picks none",
                    definition.name
                )
            })?;
            derivation.apply(gram)?
        } else {
            derivation.apply_at_collection(inputs, collection)?
        };
        match (value, extra) {
            (Some(w), false) => {
                out.parameters.insert(name.clone(), w);
            }
            (Some(w), true) => {
                out.extra_fields.insert(name.clone(), w);
            }
            (None, false) => out.not_derived.push(name.clone()),
            (None, true) => {}
        }
    }
    Ok(Some(out))
}

/// The grams that pass a filter.
fn through_filter<'g>(
    filter: &Filter,
    inputs: &Map<String, Value>,
    grams: impl IntoIterator<Item = &'g Gram>,
) -> Result<Vec<&'g Gram>, String> {
    let mut passed = Vec::new();
    for gram in grams {
        if fits(filter, inputs, gram)? {
            passed.push(gram);
        }
    }
    Ok(passed)
}

/// Reduce the grams of a chronicle to a lexostatus, without an as-of point:
/// every gram counts. See [`reduce_at`].
pub fn reduce<'g>(
    definition: &LexostatusDefinition,
    inputs: &Map<String, Value>,
    grams: impl IntoIterator<Item = &'g Gram>,
) -> Result<Option<Lexostatus>, String> {
    reduce_at(definition, inputs, grams, &AsOf::default())
}

/// Reduce the grams of a chronicle to a lexostatus, as of an as-of point: only
/// the grams that count at the as-of point take part. `None`: the definition
/// picks a gram (`pick`) and no gram passes the filter. A list lexostatus
/// (`group_by`) always gives a lexostatus, with an empty list if no case
/// fits.
pub fn reduce_at<'g>(
    definition: &LexostatusDefinition,
    inputs: &Map<String, Value>,
    grams: impl IntoIterator<Item = &'g Gram>,
    as_of: &AsOf,
) -> Result<Option<Lexostatus>, String> {
    let r = &definition.reduction;
    let mut in_chronicle = Vec::new();
    for g in grams {
        if g.chronicle == r.chronicle && as_of.let_through(g)? {
            in_chronicle.push(g);
        }
    }
    let as_of = |l: Lexostatus| Lexostatus {
        as_of: as_of.as_of.map(|t| t.to_string()),
        known_at: as_of.known_at.map(|t| t.to_string()),
        ..l
    };
    if r.group_by.is_some() {
        return reduce_list(definition, inputs, in_chronicle).map(|l| Some(as_of(l)));
    }
    let passed = through_filter(&r.filter, inputs, in_chronicle)?;
    let Some(a) = derive_from(definition, inputs, &passed, &passed)? else {
        return Ok(None);
    };
    Ok(Some(as_of(Lexostatus {
        root: a.chosen.and_then(|g| g.root.clone()),
        effective_at: a.chosen.map(|g| g.effective_at.clone()),
        recorded_at: a.chosen.map(|g| g.recorded_at.clone()),
        parameters: a.parameters,
        extra_fields: a.extra_fields,
        not_derived: a.not_derived,
        ..Lexostatus::empty(&definition.name)
    })))
}

/// A list lexostatus: group per root, keep the roots with a gram through
/// `filter` and without a gram through `without`, and derive per root: `pick`
/// chooses among the grams through `filter`, a derivation over a collection
/// (with its own `filter`) reads every gram of the case. The
/// rows are placed at the moment of the chosen gram (without `pick`: the
/// first gram of the root), oldest first.
fn reduce_list<'g>(
    definition: &LexostatusDefinition,
    inputs: &Map<String, Value>,
    grams: impl IntoIterator<Item = &'g Gram>,
) -> Result<Lexostatus, String> {
    let r = &definition.reduction;
    let mut cases: BTreeMap<&str, Vec<&Gram>> = BTreeMap::new();
    for g in grams {
        // A gram without a known root belongs in no row.
        if let Some(z) = g.root.as_deref() {
            cases.entry(z).or_default().push(g);
        }
    }
    let mut rows = Vec::new();
    for (case, group) in cases {
        if !r.without.is_empty()
            && !through_filter(&r.without, inputs, group.iter().copied())?.is_empty()
        {
            continue;
        }
        let passed = through_filter(&r.filter, inputs, group.iter().copied())?;
        if passed.is_empty() {
            continue;
        }
        let Some(a) = derive_from(definition, inputs, &passed, &group)? else {
            continue;
        };
        // Without a chosen gram the case is placed at its first gram: the opening.
        let moment = match a.chosen {
            Some(g) => Some(g.moment()?),
            None => passed
                .iter()
                .map(|g| g.moment())
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .min(),
        };
        let mut fields = a.parameters;
        fields.extend(a.extra_fields);
        rows.push((
            moment,
            Row {
                root: case.to_string(),
                effective_at: a.chosen.map(|g| g.effective_at.clone()),
                fields,
                not_derived: a.not_derived,
            },
        ));
    }
    rows.sort_by(|(a, ra), (b, rb)| a.cmp(b).then_with(|| ra.root.cmp(&rb.root)));
    Ok(Lexostatus {
        list: Some(rows.into_iter().map(|(_, r)| r).collect()),
        ..Lexostatus::empty(&definition.name)
    })
}

/// Reduce a single gram, such as a draft that is not a fact yet. If the gram
/// has a root, that is the input `root`.
pub fn derive(definition: &LexostatusDefinition, gram: &Gram) -> Result<Lexostatus, String> {
    let mut inputs = Map::new();
    if let Some(z) = &gram.root {
        inputs.insert("root".into(), Value::String(z.clone()));
    }
    reduce(definition, &inputs, std::slice::from_ref(gram))?
        .ok_or_else(|| "the reduction did not find the gram".to_string())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::gram::StreamReference;
    use serde_json::json;

    const CELL: &str = include_str!("../../tests/fixtures/cells/instantie/lexostatuses.yaml");

    fn gram(case: &str, moment: &str, fields: Value) -> Gram {
        Gram {
            kind: "chronolexogram".into(),
            id: uuid::Uuid::now_v7().to_string(),
            type_: "submission".into(),
            subtype: Some("aanvraag".into()),
            stage: None,
            name: "aanvraag_ontvangen".into(),
            chronicle: "test_kroniek".into(),
            recording_actor: "test_instantie".into(),
            legal_basis: vec!["testregeling_aanvraag#1".into()],
            legal_character: None,
            decision_type: None,
            regulation: None,
            regulation_valid_from: None,
            competent_authority: None,
            acting_actor: None,
            effective_at: moment.into(),
            effective_at_legal_basis: None,
            recorded_at: moment.into(),
            refers_to: BTreeMap::new(),
            stream: StreamReference {
                id: "test_aanvragen".into(),
                sha256: "0".repeat(64),
            },
            provenance: None,
            fields: fields.as_object().unwrap().clone(),
            field_provenance: BTreeMap::new(),
            inputs: BTreeMap::new(),
            receipt: None,
            times: Default::default(),
            root: Some(case.into()),
        }
    }

    fn der(yaml: &str) -> Derivation {
        serde_yaml_ng::from_str(yaml).unwrap()
    }

    fn a(yaml: &str, fields: Value) -> Option<Value> {
        der(yaml)
            .apply(&gram("z", "2025-03-12T10:14:03+01:00", fields))
            .unwrap()
    }

    const CASE: &str = "00000000-0000-4000-8000-000000000001";

    #[test]
    fn fixture_loads() {
        let c = parse(CELL, "fixture").unwrap();
        assert_eq!(c.cell, "test_instantie");
        assert_eq!(c.lexostatus_definitions[0].reduction.derivations.len(), 9);
    }

    #[test]
    fn invalid_derivation_fails_on_the_schema() {
        let text = CELL.replace(
            "{field: content.aanvraagjaar}",
            "{optellen: content.aanvraagjaar}",
        );
        let error = parse(&text, "t").unwrap_err();
        assert!(
            error.iter().any(|f| f.contains("derivations/aanvraagjaar")),
            "{error:?}"
        );
    }

    #[test]
    fn derivation_field() {
        let f = json!({"a": {"jaar": 2025, "leeg": ""}});
        assert_eq!(a("{field: a.jaar}", f.clone()), Some(json!(2025)));
        assert_eq!(a("{field: a.leeg}", f.clone()), None);
        assert_eq!(a("{field: a.ontbreekt}", f), None);
    }

    #[test]
    fn derivation_filled() {
        let f = json!({"a": {"naam": "X", "leeg": " ", "lijst": []}});
        assert_eq!(a("{filled: a.naam}", f.clone()), Some(json!(true)));
        assert_eq!(a("{filled: a.leeg}", f.clone()), Some(json!(false)));
        assert_eq!(a("{filled: a.lijst}", f.clone()), Some(json!(false)));
        assert_eq!(a("{filled: a.ontbreekt}", f), Some(json!(false)));
    }

    #[test]
    fn derivation_equals() {
        let f = json!({"a": {"reg": "a"}});
        assert_eq!(
            a("{equals: {field: a.reg, value: a}}", f.clone()),
            Some(json!(true))
        );
        assert_eq!(
            a("{equals: {field: a.reg, value: b}}", f.clone()),
            Some(json!(false))
        );
        assert_eq!(a("{equals: {field: a.x, value: b}}", f), None);
    }

    #[test]
    fn derivation_table_each_row() {
        let f = json!({"t": [{"o": "raad", "z": 3}, {"o": "staten", "z": null}]});
        assert_eq!(a("{table: t, each_row: o}", f.clone()), Some(json!(true)));
        assert_eq!(a("{table: t, each_row: z}", f), Some(json!(false)));
        // An empty or absent table contains nothing.
        assert_eq!(
            a("{table: t, each_row: o}", json!({"t": []})),
            Some(json!(false))
        );
        assert_eq!(a("{table: t, each_row: o}", json!({})), Some(json!(false)));
    }

    #[test]
    fn derivation_table_only_where() {
        let f = json!({"t": [{"s": false}, {"s": true, "n": 2}]});
        assert_eq!(
            a("{table: t, each_row: n, only_where: s}", f),
            Some(json!(true))
        );
        let f = json!({"t": [{"s": true}, {"s": true, "n": 2}]});
        assert_eq!(
            a("{table: t, each_row: n, only_where: s}", f),
            Some(json!(false))
        );
        // No row where true: nothing to miss, as long as the table has rows.
        let f = json!({"t": [{"s": false}]});
        assert_eq!(
            a("{table: t, each_row: n, only_where: s}", f),
            Some(json!(true))
        );
    }

    #[test]
    fn derivation_table_one_row() {
        let f = json!({"t": [{"s": false}, {"s": true}]});
        assert_eq!(a("{table: t, one_row: s}", f), Some(json!(true)));
        let f = json!({"t": [{"s": false}, {"s": "true"}]});
        assert_eq!(a("{table: t, one_row: s}", f), Some(json!(false)));
    }

    #[test]
    fn derivation_year_of() {
        let f = json!({"a": {"datum": "2026-03-18", "leeg": null, "tekst": "geen datum"}});
        assert_eq!(a("{year_of: a.datum}", f.clone()), Some(json!(2026)));
        assert_eq!(a("{year_of: a.leeg}", f.clone()), None);
        assert_eq!(a("{year_of: a.ontbreekt}", f.clone()), None);
        // A value that is not a date is an error, not a missing value.
        let error = der("{year_of: a.tekst}")
            .apply(&gram("z", "2025-03-12T10:14:03+01:00", f))
            .unwrap_err();
        assert!(error.contains("'geen datum' is not a date"), "{error}");
        assert_eq!(der("{year_of: a.datum}").read_paths(), vec!["a.datum"]);
    }

    /// A period is named after its first day: a month, a quarter, a year. A
    /// value that is not a date, and a derivation without a period (not set
    /// from the regulation), are an error.
    #[test]
    fn derivation_period_of() {
        let f = json!({"a": {"datum": "2026-08-18", "moment": "2026-11-30T23:30:00+01:00", "tekst": "geen datum"}});
        let per =
            |p: &str, field: &str| a(&format!("{{period_of: {field}, period: {p}}}"), f.clone());
        assert_eq!(per("month", "a.datum"), Some(json!("2026-08-01")));
        assert_eq!(per("quarter", "a.datum"), Some(json!("2026-07-01")));
        assert_eq!(per("year", "a.datum"), Some(json!("2026-01-01")));
        assert_eq!(per("month", "a.moment"), Some(json!("2026-11-01")));
        assert_eq!(per("quarter", "a.moment"), Some(json!("2026-10-01")));
        let error = |yaml: &str| {
            der(yaml)
                .apply(&gram("z", "2025-03-12T10:14:03+01:00", f.clone()))
                .unwrap_err()
        };
        assert!(error("{period_of: a.tekst, period: month}").contains("not a date"));
        assert!(error("{period_of: a.datum}").contains("without period"));
        assert_eq!(der("{period_of: a.datum}").read_paths(), vec!["a.datum"]);
        let a = der(
            "{filter: {name: x}, pick: latest, period_of: datum, period: month, no_gram: null}",
        );
        assert!(matches!(a, Derivation::LatestPeriodOf { .. }));
    }

    #[test]
    fn derivation_year_of_over_a_collection() {
        let a = der("{filter: {name: x}, pick: latest, year_of: datum, no_gram: null}");
        assert!(matches!(a, Derivation::LatestYearOf { .. }));
        assert!(!a.at_chosen_gram());
        let old = decision(
            "x",
            "2024-03-20T09:00:00+01:00",
            json!({"datum": "2024-03-20"}),
        );
        let new = decision(
            "x",
            "2025-03-20T09:00:00+01:00",
            json!({"datum": "2025-01-02"}),
        );
        assert_eq!(
            a.apply_at_collection(&Map::new(), &[&old, &new]).unwrap(),
            Some(json!(2025))
        );
        // No gram: the reading of absence.
        assert_eq!(
            a.apply_at_collection(&Map::new(), &[]).unwrap(),
            Some(Value::Null)
        );
        // A gram without a date yields nothing; nothing is filled in.
        let empty = decision("x", "2024-03-20T09:00:00+01:00", json!({"datum": null}));
        assert_eq!(a.apply_at_collection(&Map::new(), &[&empty]).unwrap(), None);
    }

    #[test]
    fn derivation_moment() {
        // The date in the moment's own time zone.
        let g = gram("z", "2025-03-12T00:30:00+01:00", json!({}));
        assert_eq!(
            der("{moment: effective_at}").apply(&g).unwrap(),
            Some(json!("2025-03-12"))
        );
    }

    #[test]
    fn an_invalid_effective_at_is_an_error_not_an_empty_value() {
        let g = gram("z", "12 maart 2025", json!({}));
        let f = der("{moment: effective_at}").apply(&g).unwrap_err();
        assert!(f.contains("effective_at '12 maart 2025'"), "{f}");
        // `pick: latest` does not silently pick around such a gram either.
        let def: LexostatusDefinition = serde_yaml_ng::from_str(
            "{name: l, inputs: [], reduction: {chronicle: test_kroniek, pick: latest, derivations: {x: {field: a}}}}",
        )
        .unwrap();
        let good = gram("z", "2025-03-12T10:14:03+01:00", json!({"a": 1}));
        assert!(reduce(&def, &Map::new(), &[good, g])
            .unwrap_err()
            .contains("effective_at '12 maart 2025'"));
    }

    #[test]
    fn pick_latest_compares_moments_across_time_zones() {
        // On the clock of their own time zone the order looks different from
        // what it is: 10:15+02:00 is 08:15 UTC, 10:00+01:00 is 09:00 UTC and
        // 09:30+00:00 is 09:30 UTC, the latest.
        let def: LexostatusDefinition = serde_yaml_ng::from_str(
            "{name: l, inputs: [], reduction: {chronicle: test_kroniek, pick: latest, derivations: {x: {field: a}}}}",
        )
        .unwrap();
        let grams = [
            gram("z", "2025-03-12T09:30:00+00:00", json!({"a": "utc"})),
            gram("z", "2025-03-12T10:15:00+02:00", json!({"a": "oost"})),
            gram("z", "2025-03-12T10:00:00+01:00", json!({"a": "nl"})),
        ];
        let l = reduce(&def, &Map::new(), &grams).unwrap().unwrap();
        assert_eq!(l.parameters["x"], json!("utc"));
        assert_eq!(l.effective_at.as_deref(), Some("2025-03-12T09:30:00+00:00"));
    }

    #[test]
    fn a_table_row_that_is_not_an_object_is_an_error() {
        let g = gram(
            "z",
            "2025-03-12T10:14:03+01:00",
            json!({"t": [{"k": true}, 3]}),
        );
        let f = der("{table: t, one_row: k}").apply(&g).unwrap_err();
        assert!(f.contains("row t[1] is not an object"), "{f}");
        let g = gram("z", "2025-03-12T10:14:03+01:00", json!({"t": null}));
        assert_eq!(
            der("{table: t, one_row: k}").apply(&g).unwrap(),
            Some(json!(false))
        );
    }

    #[test]
    fn derivation_reads_paths() {
        assert_eq!(
            der("{equals: {field: a.b, value: 1}}").read_paths(),
            vec!["a.b"]
        );
        assert_eq!(der("{table: t, one_row: s}").read_paths(), vec!["t"]);
        assert!(der("{moment: effective_at}").read_paths().is_empty());
    }

    #[test]
    fn reduction_picks_latest_per_case() {
        let c = parse(CELL, "fixture").unwrap();
        let def = &c.lexostatus_definitions[0];
        let grams = vec![
            gram(
                CASE,
                "2025-03-01T09:00:00+01:00",
                json!({"content": {"naam": "Eerst"}}),
            ),
            gram(
                CASE,
                "2025-03-02T09:00:00+01:00",
                json!({"content": {"naam": "Herstel"}}),
            ),
            gram(
                "00000000-0000-4000-8000-000000000002",
                "2025-03-03T09:00:00+01:00",
                json!({"content": {}}),
            ),
        ];
        let inputs = json!({"root": CASE});
        let l = reduce(def, inputs.as_object().unwrap(), &grams)
            .unwrap()
            .unwrap();
        assert_eq!(l.effective_at.as_deref(), Some("2025-03-02T09:00:00+01:00"));
        assert_eq!(l.parameters["bevat_naam"], json!(true));
        assert_eq!(l.parameters["aanvraagdatum"], json!("2025-03-02"));
        assert!(l.not_derived.contains(&"aanvraagjaar".to_string()));
    }

    #[test]
    fn absent_names_only_presence_derivations() {
        let c = parse(CELL, "fixture").unwrap();
        let def = &c.lexostatus_definitions[0];
        let g = gram(
            CASE,
            "2025-03-01T09:00:00+01:00",
            json!({"content": {"naam": "X", "registratie": "b",
                "organen": [{"orgaan": "raad", "samengevoegd": false}]}}),
        );
        let l = derive(def, &g).unwrap();
        // False, but an answer: registratie_categorie_a (equals) and
        // is_samengevoegd (one_row). False and a gap: the rest.
        assert_eq!(l.parameters["registratie_categorie_a"], json!(false));
        assert_eq!(l.parameters["is_samengevoegd"], json!(false));
        assert_eq!(
            absent(def, &l.parameters),
            vec!["bevat_aanduiding", "bevat_aantal_zetels"]
        );
    }

    #[test]
    fn reduction_without_matching_gram() {
        let c = parse(CELL, "fixture").unwrap();
        let inputs = json!({"root": CASE});
        let l = reduce(
            &c.lexostatus_definitions[0],
            inputs.as_object().unwrap(),
            &[],
        )
        .unwrap();
        assert!(l.is_none());
    }

    #[test]
    fn reduction_without_input_is_an_error() {
        let c = parse(CELL, "fixture").unwrap();
        let g = gram(CASE, "2025-03-01T09:00:00+01:00", json!({}));
        let error = reduce(&c.lexostatus_definitions[0], &Map::new(), &[g]).unwrap_err();
        assert!(error.contains("root"), "{error}");
    }

    #[test]
    fn filter_on_the_root_does_not_pass_a_gram_without_root() {
        let mut filter = Filter::new();
        filter.insert("root".into(), "$root".into());
        let inputs = json!({"root": CASE});
        let inputs = inputs.as_object().unwrap();
        let mut g = gram(CASE, "2025-03-01T09:00:00+01:00", json!({}));
        assert!(fits(&filter, inputs, &g).unwrap());
        g.root = None;
        assert!(!fits(&filter, inputs, &g).unwrap());
    }

    /// A filter can select per decision: on the reference to that decision.
    #[test]
    fn filter_on_a_reference() {
        let mut filter = Filter::new();
        filter.insert("refers_to.decision".into(), "$besluit".into());
        let inputs = json!({"besluit": "b1"});
        let inputs = inputs.as_object().unwrap();
        let mut g = gram(CASE, "2025-03-01T09:00:00+01:00", json!({}));
        assert!(!fits(&filter, inputs, &g).unwrap(), "without reference");
        g.refers_to.insert("decision".into(), "b1".into());
        assert!(fits(&filter, inputs, &g).unwrap());
        g.refers_to.insert("decision".into(), "b2".into());
        assert!(!fits(&filter, inputs, &g).unwrap(), "another decision");
    }

    /// A field a lexostatus reads as a date must be a date or empty when
    /// recording; another field or another chronicle need not be.
    #[test]
    fn a_date_field_is_a_date_or_empty() {
        let def: LexostatusDefinition = serde_yaml_ng::from_str(
            "name: l\ninputs: []\nreduction:\n  chronicle: test_kroniek\n  pick: latest\n  derivations:\n    jaar: {year_of: a.datum}\n",
        )
        .unwrap();
        let g = |f: Value| gram(CASE, "2025-03-01T09:00:00+01:00", f);
        dates_in_order([&def], &g(json!({"a": {"datum": "2025-03-01"}}))).unwrap();
        dates_in_order([&def], &g(json!({"a": {"datum": null}, "b": "geen datum"}))).unwrap();
        let error = dates_in_order([&def], &g(json!({"a": {"datum": "morgen"}}))).unwrap_err();
        assert!(error.contains("'morgen' is not a date"), "{error}");
        assert!(error.contains("lexostatus 'l'"), "{error}");
        let mut other = g(json!({"a": {"datum": "morgen"}}));
        other.chronicle = "andere_kroniek".into();
        dates_in_order([&def], &other).unwrap();
    }

    /// Every key of the gram itself has a value in `Gram::attribute`; any
    /// other key is a field path.
    #[test]
    fn every_gram_key_is_an_attribute() {
        let g = gram(CASE, "2025-03-01T09:00:00+01:00", json!({}));
        for k in GRAM_KEYS {
            assert!(g.attribute(k).is_some(), "{k}");
        }
        assert!(g.attribute("content.naam").is_none());
    }

    const REGISTER: &str = include_str!("../../tests/fixtures/cells/register/lexostatuses.yaml");

    fn decision(name: &str, moment: &str, fields: Value) -> Gram {
        let mut g = gram(CASE, moment, fields);
        g.root = None;
        g.type_ = "decretogram".into();
        g.subtype = None;
        g.name = name.into();
        g.chronicle = "test_register".into();
        g
    }

    fn register() -> Vec<Gram> {
        vec![
            decision(
                "aanduiding_ingeschreven",
                "2024-01-10T09:00:00+01:00",
                json!({"aanduiding": "VOORBEELD", "orgaan": "raad", "gebied": "A"}),
            ),
            decision(
                "uitslag_vastgesteld",
                "2024-03-20T09:00:00+01:00",
                json!({"lijst": "VOORBEELD", "orgaan": "raad", "gebied": "A", "zetels": 4}),
            ),
            decision(
                "uitslag_vastgesteld",
                "2024-03-21T09:00:00+01:00",
                json!({"lijst": "VOORBEELD", "orgaan": "raad", "gebied": "B", "zetels": 2}),
            ),
            decision(
                "uitslag_vastgesteld",
                "2024-03-21T09:00:00+01:00",
                json!({"lijst": "ANDERS", "orgaan": "raad", "gebied": "B", "zetels": 7}),
            ),
            decision(
                "mededeling_gedaan",
                "2024-11-01T09:00:00+01:00",
                json!({"aanduiding": "VOORBEELD", "datum": "2024-11-01", "geblokkeerd_voor": ["staten"]}),
            ),
            decision(
                "mededeling_gedaan",
                "2024-12-01T09:00:00+01:00",
                json!({"aanduiding": "VOORBEELD", "datum": "2024-12-01", "geblokkeerd_voor": ["raad"]}),
            ),
        ]
    }

    /// `collect` turns the grams through the filter into a list with one row
    /// per gram; a field a gram does not have is null.
    #[test]
    fn collect_gives_one_row_per_gram() {
        let a: Derivation = serde_yaml_ng::from_str(
            "{filter: {name: uitslag_vastgesteld, lijst: $aanduiding}, collect: [gebied, zetels, samengevoegd]}",
        )
        .unwrap();
        let grams = register();
        let refs: Vec<&Gram> = grams.iter().collect();
        let inputs = json!({"aanduiding": "VOORBEELD"});
        let w = a
            .apply_at_collection(inputs.as_object().unwrap(), &refs)
            .unwrap();
        assert_eq!(
            w,
            Some(json!([
                {"gebied": "A", "zetels": 4, "samengevoegd": null},
                {"gebied": "B", "zetels": 2, "samengevoegd": null}
            ]))
        );
    }

    /// `exists` with `filled`: only a gram in which that field has a value
    /// counts. A filter compares on equality and cannot do that.
    #[test]
    fn exists_with_filled_counts_only_a_gram_with_a_value() {
        let a = der("{filter: {name: uitslag_vastgesteld, lijst: $aanduiding}, exists: true, filled: samengevoegd}");
        assert_eq!(a.read_paths(), vec!["samengevoegd", "lijst"]);
        let inputs = json!({"aanduiding": "VOORBEELD"});
        let grams = register();
        let refs: Vec<&Gram> = grams.iter().collect();
        // The results of the fixture have no merged designation.
        assert_eq!(
            a.apply_at_collection(inputs.as_object().unwrap(), &refs)
                .unwrap(),
            Some(json!(false))
        );
        let with = decision(
            "uitslag_vastgesteld",
            "2024-03-20T09:00:00+01:00",
            json!({"lijst": "VOORBEELD", "samengevoegd": 2}),
        );
        let mut refs = refs;
        refs.push(&with);
        assert_eq!(
            a.apply_at_collection(inputs.as_object().unwrap(), &refs)
                .unwrap(),
            Some(json!(true))
        );
    }

    /// A lexostatus of the register fixture, reduced with these inputs.
    fn register_lexostatus(name: &str, inputs: Value) -> Lexostatus {
        let c = parse(REGISTER, "register").unwrap();
        reduce(
            c.lexostatus(name).unwrap(),
            inputs.as_object().unwrap(),
            &register(),
        )
        .unwrap()
        .unwrap()
    }

    fn register_status(aanduiding: &str) -> Lexostatus {
        register_lexostatus("registerstatus", json!({"aanduiding": aanduiding}))
    }

    fn register_state(aanduiding: &str) -> Lexostatus {
        register_lexostatus(
            "register",
            json!({"aanduiding": aanduiding, "orgaan": "raad"}),
        )
    }

    #[test]
    fn filter_per_derivation_exists_sum_and_latest() {
        let r = register_state("VOORBEELD");
        assert_eq!(r.root, None, "without pick no gram is chosen");
        assert_eq!(r.parameters["is_ingeschreven_in_register"], json!(true));
        assert_eq!(r.parameters["is_geschrapt"], json!(false));
        let l = register_status("VOORBEELD");
        // Only the results with this designation above the list, across all areas.
        assert_eq!(l.parameters["zetels_toegewezen"], json!(6));
        assert_eq!(l.parameters["datum_mededeling"], json!("2024-12-01"));
        // The latest gram counts: there raad is in the list.
        assert_eq!(l.parameters["geblokkeerd"], json!(true));
        assert!(l.not_derived.is_empty(), "{:?}", l.not_derived);
    }

    #[test]
    fn absence_in_the_own_chronicle_is_no() {
        let r = register_state("ONBEKEND");
        assert_eq!(r.parameters["is_ingeschreven_in_register"], json!(false));
        let l = register_status("ONBEKEND");
        assert_eq!(l.parameters["zetels_toegewezen"], json!(0));
        assert_eq!(l.parameters["geblokkeerd"], json!(false));
        // There is no date: it is left out, nothing is filled in.
        assert!(!l.parameters.contains_key("datum_mededeling"));
        assert_eq!(
            l.not_derived,
            vec!["datum_mededeling", "jaar_van_mededeling"]
        );
    }

    #[test]
    fn sum_without_number_cannot_be_derived() {
        let a = der("{filter: {name: uitslag_vastgesteld}, sum: zetels}");
        let g = decision(
            "uitslag_vastgesteld",
            "2024-03-20T09:00:00+01:00",
            json!({"zetels": "vier"}),
        );
        assert_eq!(a.apply_at_collection(&Map::new(), &[&g]).unwrap(), None);
        let h = decision(
            "uitslag_vastgesteld",
            "2024-03-20T09:00:00+01:00",
            json!({"zetels": 1.5}),
        );
        let i = decision(
            "uitslag_vastgesteld",
            "2024-03-20T09:00:00+01:00",
            json!({"zetels": 2}),
        );
        assert_eq!(
            a.apply_at_collection(&Map::new(), &[&h, &i]).unwrap(),
            Some(json!(3.5))
        );
    }

    #[test]
    fn filter_on_a_field_path_also_compares_numbers() {
        let g = decision(
            "x",
            "2024-03-20T09:00:00+01:00",
            json!({"a": {"jaar": 2024, "ja": true}}),
        );
        let f: Filter = serde_json::from_value(json!({"a.jaar": "2024", "a.ja": "true"})).unwrap();
        assert!(fits(&f, &Map::new(), &g).unwrap());
        let f: Filter = serde_json::from_value(json!({"a.jaar": "2025"})).unwrap();
        assert!(!fits(&f, &Map::new(), &g).unwrap());
        let f: Filter = serde_json::from_value(json!({"a.ontbreekt": "x"})).unwrap();
        assert!(!fits(&f, &Map::new(), &g).unwrap());
    }

    #[test]
    fn derivations_over_a_collection_also_read_their_filter() {
        let a = der("{filter: {name: x, orgaan: raad, aanduiding: $aanduiding}, exists: true}");
        assert!(!a.at_chosen_gram());
        assert_eq!(a.read_paths(), vec!["aanduiding", "orgaan"]);
        let a = der("{filter: {name: x}, pick: latest, contains: {field: lijst, value: raad}}");
        assert!(matches!(a, Derivation::LatestContains { .. }));
        assert_eq!(a.read_paths(), vec!["lijst"]);
        let a = der("{filter: {name: x}, pick: latest, field: datum}");
        assert!(matches!(a, Derivation::LatestField { .. }));
        assert!(der("{field: datum}").at_chosen_gram());
    }

    #[test]
    fn extra_fields_are_separate_from_the_parameters() {
        let mut c = parse(CELL, "fixture").unwrap();
        let def = &mut c.lexostatus_definitions[0];
        def.reduction.extra_fields.insert(
            "aanduiding".into(),
            der("{field: content.aanduiding}").into(),
        );
        let g = gram(
            CASE,
            "2025-03-01T09:00:00+01:00",
            json!({"content": {"aanduiding": "X"}}),
        );
        let l = derive(def, &g).unwrap();
        assert_eq!(l.extra_fields["aanduiding"], json!("X"));
        assert!(!l.parameters.contains_key("aanduiding"));
        assert!(def.delivers("aanduiding") && def.delivers("bevat_naam"));
    }

    #[test]
    fn derivation_on_the_gram_without_pick_is_an_error() {
        let mut c = parse(REGISTER, "register").unwrap();
        let def = &mut c.lexostatus_definitions[1];
        def.reduction
            .derivations
            .insert("x".into(), der("{field: aanduiding}").into());
        let inputs = json!({"aanduiding": "VOORBEELD"});
        let error = reduce(def, inputs.as_object().unwrap(), &register()).unwrap_err();
        assert!(error.contains("picks none"), "{error}");
    }

    #[test]
    fn register_fixture_validates_against_the_schema() {
        let c = parse(REGISTER, "register").unwrap();
        assert_eq!(c.cell, "test_register");
        // A derivation carries its legal basis machine-readably.
        let a = &c.lexostatus_definitions[0].reduction.derivations["is_ingeschreven_in_register"];
        assert_eq!(a.legal_basis, ["testregeling_register#1 lid 1"]);
        assert!(matches!(a.derivation, Derivation::Exists { .. }));
    }

    // --- List lexostatus: group_by and without ---

    const WORKLIST: &str = "cell: c\nlexostatus_definitions:\n  - name: werkvoorraad\n    inputs: []\n    reduction:\n      chronicle: test_kroniek\n      filter: {type: submission, subtype: aanvraag}\n      group_by: root\n      without: {stage: BESLUIT}\n      pick: latest\n      derivations:\n        ontvangen_op: {moment: effective_at}\n        naam: {field: content.naam}\n";

    fn stage(case: &str, moment: &str, stage: &str) -> Gram {
        let mut g = gram(case, moment, json!({}));
        g.type_ = "decretogram".into();
        g.subtype = None;
        g.stage = Some(stage.into());
        g.name = "besluit".into();
        g
    }

    #[test]
    fn group_per_case_without_decision() {
        let c = parse(WORKLIST, "w").unwrap();
        let def = &c.lexostatus_definitions[0];
        assert!(def.is_list());
        let (a, b, d) = (
            "00000000-0000-4000-8000-00000000000a",
            "00000000-0000-4000-8000-00000000000b",
            "00000000-0000-4000-8000-00000000000d",
        );
        let mut without_case = gram(
            a,
            "2025-01-01T09:00:00+01:00",
            json!({"content": {"naam": "Geen"}}),
        );
        without_case.root = None;
        let grams = vec![
            // Case b was submitted later but comes first by identifier; the list
            // is ordered by moment.
            gram(
                b,
                "2025-03-05T09:00:00+01:00",
                json!({"content": {"naam": "B"}}),
            ),
            gram(
                a,
                "2025-03-01T09:00:00+01:00",
                json!({"content": {"naam": "A eerst"}}),
            ),
            gram(
                a,
                "2025-03-02T09:00:00+01:00",
                json!({"content": {"naam": "A herstel"}}),
            ),
            // Case d has a decision and drops out; another stage does not.
            gram(
                d,
                "2025-03-03T09:00:00+01:00",
                json!({"content": {"naam": "D"}}),
            ),
            stage(d, "2025-03-04T09:00:00+01:00", "BESLUIT"),
            stage(b, "2025-03-06T09:00:00+01:00", "BEKENDMAKING"),
            without_case,
        ];
        let l = reduce(def, &Map::new(), &grams).unwrap().unwrap();
        assert!(l.parameters.is_empty());
        let list = l.list.unwrap();
        let cases: Vec<&str> = list.iter().map(|r| r.root.as_str()).collect();
        assert_eq!(cases, [a, b]);
        assert_eq!(list[0].fields["naam"], json!("A herstel"));
        assert_eq!(list[0].fields["ontvangen_op"], json!("2025-03-02"));
        assert_eq!(
            list[0].effective_at.as_deref(),
            Some("2025-03-02T09:00:00+01:00")
        );
        // A case with only a decision and no application does not count.
        let only = vec![stage(a, "2025-03-04T09:00:00+01:00", "BEKENDMAKING")];
        let l = reduce(def, &Map::new(), &only).unwrap().unwrap();
        assert!(l.list.unwrap().is_empty());
    }

    /// In a list, `pick` chooses among the grams through `filter`, but a
    /// derivation over a collection reads every gram of the case: the
    /// decision of a case, which the filter of the submission leaves out.
    #[test]
    fn a_list_derivation_over_a_collection_reads_the_whole_case() {
        let text = WORKLIST.replace("      without: {stage: BESLUIT}\n", "").replace(
            "        naam: {field: content.naam}\n",
            "        besloten: {filter: {stage: BESLUIT}, pick: latest, moment: effective_at, no_gram: null}\n",
        );
        let c = parse(&text, "w").unwrap();
        let def = &c.lexostatus_definitions[0];
        let (a, d) = (
            "00000000-0000-4000-8000-00000000000a",
            "00000000-0000-4000-8000-00000000000d",
        );
        let grams = vec![
            gram(a, "2025-03-01T09:00:00+01:00", json!({})),
            gram(d, "2025-03-03T09:00:00+01:00", json!({})),
            stage(d, "2025-03-04T09:00:00+01:00", "BESLUIT"),
            stage(d, "2025-03-05T09:00:00+01:00", "BEKENDMAKING"),
        ];
        let list = reduce(def, &Map::new(), &grams)
            .unwrap()
            .unwrap()
            .list
            .unwrap();
        let cases: Vec<&str> = list.iter().map(|r| r.root.as_str()).collect();
        assert_eq!(cases, [a, d]);
        assert_eq!(list[0].fields["besloten"], Value::Null);
        assert_eq!(list[1].fields["besloten"], json!("2025-03-04"));
        // The chosen gram is still the submission.
        assert_eq!(list[1].fields["ontvangen_op"], json!("2025-03-03"));
    }

    #[test]
    fn without_requires_group_by() {
        let text = WORKLIST.replace("      group_by: root\n", "");
        let error = parse(&text, "w").unwrap_err();
        assert!(error.iter().any(|f| f.contains("group_by")), "{error:?}");
    }

    #[test]
    fn no_gram_reads_absence() {
        let a = der("{filter: {name: x}, pick: latest, field: datum, no_gram: null}");
        assert!(matches!(
            &a,
            Derivation::LatestField {
                no_gram: Some(Value::Null),
                ..
            }
        ));
        assert_eq!(
            a.apply_at_collection(&Map::new(), &[]).unwrap(),
            Some(Value::Null)
        );
        let g = decision(
            "x",
            "2024-03-20T09:00:00+01:00",
            json!({"datum": "2024-03-20"}),
        );
        assert_eq!(
            a.apply_at_collection(&Map::new(), &[&g]).unwrap(),
            Some(json!("2024-03-20"))
        );
        // A gram with an empty field is not absence of the gram: left out.
        let empty = decision("x", "2024-03-20T09:00:00+01:00", json!({"datum": null}));
        assert_eq!(a.apply_at_collection(&Map::new(), &[&empty]).unwrap(), None);
        let false_ = der("{filter: {name: x}, pick: latest, field: ja, no_gram: false}");
        assert_eq!(
            false_.apply_at_collection(&Map::new(), &[]).unwrap(),
            Some(json!(false))
        );
        // Without no_gram the parameter is left out.
        let without = der("{filter: {name: x}, pick: latest, field: datum}");
        assert!(matches!(
            &without,
            Derivation::LatestField { no_gram: None, .. }
        ));
        assert_eq!(without.apply_at_collection(&Map::new(), &[]).unwrap(), None);
    }

    #[test]
    fn filter_on_stage() {
        let f: Filter = serde_json::from_value(json!({"stage": "BESLUIT"})).unwrap();
        let z = "00000000-0000-4000-8000-00000000000a";
        assert!(fits(
            &f,
            &Map::new(),
            &stage(z, "2025-03-04T09:00:00+01:00", "BESLUIT")
        )
        .unwrap());
        assert!(!fits(
            &f,
            &Map::new(),
            &stage(z, "2025-03-04T09:00:00+01:00", "BEKENDMAKING")
        )
        .unwrap());
        assert!(!fits(
            &f,
            &Map::new(),
            &gram(z, "2025-03-04T09:00:00+01:00", json!({}))
        )
        .unwrap());
    }

    // --- Time: two times per gram and an as-of point (paper P:46, P:94) ---

    /// A gram that legally holds at `effective`, recorded at `recorded`.
    fn timed(effective: &str, recorded: &str, fields: Value) -> Gram {
        let mut g = gram("z", effective, fields);
        g.recorded_at = recorded.into();
        g
    }

    fn pick_a() -> LexostatusDefinition {
        serde_yaml_ng::from_str(
            "{name: l, inputs: [], reduction: {chronicle: test_kroniek, pick: latest, derivations: {a: {field: a}, sinds: {moment: effective_at}, bekend: {moment: recorded_at}}}}",
        )
        .unwrap()
    }

    fn as_of(as_of: Option<&str>, known_at: Option<&str>) -> AsOf {
        let read = |t: Option<&str>| t.map(|t| crate::date::TimePoint::read("t", t).unwrap());
        AsOf {
            as_of: read(as_of),
            known_at: read(known_at),
        }
    }

    /// The same chronicle at two as-of moments: two lexostatuses. What holds
    /// after the as-of moment does not count.
    #[test]
    fn the_same_chronicle_at_two_as_of_moments() {
        let c = parse(REGISTER, "register").unwrap();
        let def = c.lexostatus("registerstatus").unwrap().clone();
        let register_def = c.lexostatus("register").unwrap().clone();
        let grams = register();
        let mut inputs = Map::new();
        inputs.insert("aanduiding".into(), json!("VOORBEELD"));
        let at = |p: &AsOf| reduce_at(&def, &inputs, &grams, p).unwrap().unwrap();
        let mut council = inputs.clone();
        council.insert("orgaan".into(), json!("raad"));
        let november = at(&as_of(Some("2024-11-15"), None));
        let december = at(&as_of(Some("2024-12-15"), None));
        assert_eq!(november.parameters["datum_mededeling"], json!("2024-11-01"));
        assert_eq!(november.parameters["geblokkeerd"], json!(false));
        assert_eq!(december.parameters["datum_mededeling"], json!("2024-12-01"));
        assert_eq!(december.parameters["geblokkeerd"], json!(true));
        assert_eq!(november.as_of.as_deref(), Some("2024-11-15"));
        // Before the result there was nothing: no seats, not registered.
        let january = at(&as_of(Some("2024-01-01"), None));
        assert_eq!(january.parameters["zetels_toegewezen"], json!(0));
        let r = reduce_at(
            &register_def,
            &council,
            &grams,
            &as_of(Some("2024-01-01"), None),
        )
        .unwrap()
        .unwrap();
        assert_eq!(r.parameters["is_ingeschreven_in_register"], json!(false));
        // Without an as-of point everything counts.
        let now = reduce(&def, &inputs, &grams).unwrap().unwrap();
        assert_eq!(now.parameters, december.parameters);
        assert_eq!(now.as_of, None);
    }

    /// A paper application that came in on March 5 and was entered on March
    /// 12: legally March 5 holds, it is only known on March 12.
    #[test]
    fn a_late_recorded_fact_with_an_earlier_effective_at() {
        let def = pick_a();
        let grams = [
            timed(
                "2025-03-10T09:00:00+01:00",
                "2025-03-10T09:00:00+01:00",
                json!({"a": "portaal"}),
            ),
            timed(
                "2025-03-05T00:00:00+01:00",
                "2025-03-12T14:00:00+01:00",
                json!({"a": "papier"}),
            ),
        ];
        let at = |p: AsOf| reduce_at(&def, &Map::new(), &grams, &p).unwrap();
        // Now: the latest in time is the gram of March 10, even though the
        // paper gram was recorded later.
        let now = at(AsOf::default()).unwrap();
        assert_eq!(now.parameters["a"], json!("portaal"));
        // Legally on March 6, with what is known now: the paper application.
        let l = at(as_of(Some("2025-03-06"), None)).unwrap();
        assert_eq!(l.parameters["a"], json!("papier"));
        assert_eq!(l.parameters["sinds"], json!("2025-03-05"));
        assert_eq!(l.parameters["bekend"], json!("2025-03-12"));
        assert_eq!(l.recorded_at.as_deref(), Some("2025-03-12T14:00:00+01:00"));
        // As known on March 11: the paper application was not there yet.
        let l = at(as_of(None, Some("2025-03-11"))).unwrap();
        assert_eq!(l.parameters["a"], json!("portaal"));
        // Bitemporal: legally on March 6, as known on March 11: nothing.
        assert!(at(as_of(Some("2025-03-06"), Some("2025-03-11"))).is_none());
    }

    /// `pick: latest` picks by effective_at; on an equal effective_at by
    /// recorded_at; and if that is equal too, the one added later.
    #[test]
    fn pick_latest_by_effective_at_then_recorded_at() {
        let def = pick_a();
        let pick = |grams: &[Gram]| {
            reduce(&def, &Map::new(), grams)
                .unwrap()
                .unwrap()
                .parameters["a"]
                .clone()
        };
        let m = "2025-03-10T09:00:00+01:00";
        // Equal effective_at: the one recorded later, regardless of order.
        let later = timed(m, "2025-03-11T09:00:00+01:00", json!({"a": "later"}));
        let earlier = timed(m, "2025-03-10T09:00:00+01:00", json!({"a": "eerder"}));
        assert_eq!(pick(&[later.clone(), earlier.clone()]), json!("later"));
        assert_eq!(pick(&[earlier.clone(), later.clone()]), json!("later"));
        // The effective_at takes precedence: an earlier fact that was recorded
        // later is not the latest.
        let late_recorded = timed(
            "2025-03-09T09:00:00+01:00",
            "2025-03-20T09:00:00+01:00",
            json!({"a": "laat"}),
        );
        assert_eq!(pick(&[earlier.clone(), late_recorded]), json!("eerder"));
        // Everything equal: the one added later.
        let second = timed(m, "2025-03-10T09:00:00+01:00", json!({"a": "tweede"}));
        assert_eq!(pick(&[earlier, second]), json!("tweede"));
    }

    #[test]
    fn an_invalid_as_of_is_an_error() {
        let mut q = Map::new();
        q.insert("as_of".into(), json!("morgen"));
        q.insert("aanduiding".into(), json!("X"));
        let f = AsOf::from_query(&mut q).unwrap_err();
        assert!(f.contains("as_of 'morgen'"), "{f}");
        let mut q = Map::new();
        q.insert("known_at".into(), json!("2025-03-10"));
        q.insert("aanduiding".into(), json!("X"));
        let p = AsOf::from_query(&mut q).unwrap();
        assert!(p.as_of.is_none());
        assert_eq!(p.query(), vec![("known_at", "2025-03-10".to_string())]);
        // What remains are the inputs.
        assert_eq!(q.keys().collect::<Vec<_>>(), ["aanduiding"]);
    }

    #[test]
    fn as_of_is_not_an_input_of_a_lexostatus() {
        let f = parse(
            "cell: c\nlexostatus_definitions:\n  - name: l\n    inputs: [{name: as_of, type: date}]\n    reduction: {chronicle: k, derivations: {}}\n",
            "l",
        )
        .unwrap_err()
        .join("; ");
        assert!(f.contains("as_of"), "{f}");
    }
}
