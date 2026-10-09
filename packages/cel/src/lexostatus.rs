//! Reading a cell's own chronicle back: a lexostatus reduces the grams to the
//! parameters an article reads ("the reduction belongs to the source"). The
//! law asks a datum ("de dag van ontvangst"); that the cell keeps it in a
//! chronicle is the cell's business, so the reduction is cell configuration.
//!
//! A reading is the state at a moment (`peilmoment`, RFC-044): a gram counts
//! when it legally holds by then (`effective_at` on or before the moment), and
//! the grams are in the order they legally hold, then the order the cell
//! recorded them, so a fact recorded late about an earlier moment is not the
//! latest state.

use chrono::{DateTime, FixedOffset};
use serde_json::{Map, Number, Value};

use crate::chronicle::{Chronicle, Gram};
use crate::config::{Derivation, LexostatusDefinition, Moment, Pick};
use crate::error::{refused, setup, Result};

/// The value of a filter: `$<input>` reads the input of the lexostatus.
fn resolve<'a>(value: &'a str, inputs: &'a Map<String, Value>) -> Result<&'a str> {
    match value.strip_prefix('$') {
        None => Ok(value),
        Some(name) => inputs
            .get(name)
            .and_then(Value::as_str)
            .ok_or_else(|| refused(format!("input '{name}' is missing or not a string"))),
    }
}

/// The value of a period filter: a whole number, or `$<input>` for the
/// input that gives it (a number, or a number as text).
fn period_value(value: &str, inputs: &Map<String, Value>) -> Result<i64> {
    let (given, what) = match value.strip_prefix('$') {
        None => (Value::String(value.to_string()), "filter"),
        Some(name) => (
            inputs
                .get(name)
                .cloned()
                .ok_or_else(|| refused(format!("input '{name}' is missing")))?,
            name,
        ),
    };
    given
        .as_i64()
        .or_else(|| given.as_str().and_then(|s| s.parse().ok()))
        .ok_or_else(|| refused(format!("period '{what}' is {given}, not a whole number")))
}

/// A moment of a gram, as the chronicle holds it (RFC 3339).
pub(crate) fn moment(gram: &Gram, value: &str) -> Result<DateTime<FixedOffset>> {
    DateTime::parse_from_rfc3339(value)
        .map_err(|e| setup(format!("gram '{}': moment '{value}': {e}", gram.id)))
}

/// The grams of `chronicle` that hold at `as_of`, in the order they hold:
/// by `effective_at`, then by `recorded_at`, then in recording order.
pub(crate) fn in_force(chronicle: &Chronicle, as_of: DateTime<FixedOffset>) -> Result<Vec<&Gram>> {
    let mut grams = Vec::new();
    for gram in chronicle.grams() {
        let effective = moment(gram, &gram.effective_at)?;
        if effective <= as_of {
            grams.push((effective, moment(gram, &gram.recorded_at)?, gram));
        }
    }
    // A stable sort keeps the recording order between equal moments.
    grams.sort_by_key(|(effective, recorded, _)| (*effective, *recorded));
    Ok(grams.into_iter().map(|(_, _, g)| g).collect())
}

/// Reduce `chronicle` by `definition` to its parameters, as it holds at
/// `as_of`, with the grams the parameters come from: the one picked gram
/// (`pick: latest`), or every gram summed (`pick: all`, none for a sum of
/// nothing), in the order they hold.
pub fn reduce<'c>(
    definition: &LexostatusDefinition,
    inputs: &Map<String, Value>,
    chronicle: &'c Chronicle,
    as_of: DateTime<FixedOffset>,
) -> Result<(Map<String, Value>, Vec<&'c Gram>)> {
    for name in &definition.inputs {
        if !inputs.contains_key(name) {
            return Err(refused(format!(
                "lexostatus '{}' needs input '{name}'",
                definition.name
            )));
        }
    }
    let filter = &definition.reduction.filter;
    fn value<'a>(v: &'a Option<String>, inputs: &'a Map<String, Value>) -> Result<Option<&'a str>> {
        v.as_deref().map(|t| resolve(t, inputs)).transpose()
    }
    let type_ = value(&filter.type_, inputs)?;
    let subtype = value(&filter.subtype, inputs)?;
    let event = value(&filter.event, inputs)?;
    let stage = value(&filter.stage, inputs)?;
    let root = value(&filter.root, inputs)?;
    let period = filter
        .period
        .as_deref()
        .map(|p| period_value(p, inputs))
        .transpose()?;
    let matches = |g: &Gram| {
        type_.is_none_or(|t| g.type_ == t)
            && subtype.is_none_or(|s| g.subtype.as_deref() == Some(s))
            && event.is_none_or(|e| g.name == e)
            && stage.is_none_or(|s| g.stage.as_deref() == Some(s))
            && root.is_none_or(|r| chronicle.root_of(g) == r)
            && period.is_none_or(|p| g.period.is_some_and(|gp| i64::from(gp.value) == p))
    };
    let picked: Vec<&Gram> = in_force(chronicle, as_of)?
        .into_iter()
        .filter(|g| matches(g))
        .collect();
    match definition.reduction.pick {
        Pick::Latest => {
            let gram = picked.last().ok_or_else(|| {
                refused(format!(
                    "lexostatus '{}': no gram in chronicle '{}' matches",
                    definition.name, definition.reduction.chronicle
                ))
            })?;
            Ok((derive_one(definition, gram), vec![*gram]))
        }
        Pick::All => Ok((derive_all(definition, &picked)?, picked)),
    }
}

/// The derivations over the one picked gram. A field the gram does not have
/// is left out: a fact nobody has, not a null the applicant stated.
fn derive_one(definition: &LexostatusDefinition, gram: &Gram) -> Map<String, Value> {
    let mut out = Map::new();
    for (name, derivation) in &definition.reduction.derivations {
        let value = match derivation {
            Derivation::Field { field, .. } => match gram.fields.get(field) {
                Some(v) => v.clone(),
                None => continue,
            },
            Derivation::Moment {
                moment: Moment::EffectiveAt,
                ..
            } => Value::String(gram.effective_at.chars().take(10).collect()),
            Derivation::Filled { filled, .. } => Value::Bool(match gram.fields.get(filled) {
                None | Some(Value::Null) => false,
                Some(Value::String(s)) => !s.trim().is_empty(),
                Some(_) => true,
            }),
            Derivation::Period { period, .. } => match gram.period {
                Some(p) if p.unit == *period => Value::from(p.value),
                _ => continue,
            },
            // The configuration allows a sum only with `pick: all`.
            Derivation::Sum { .. } => continue,
        };
        out.insert(name.clone(), value);
    }
    out
}

/// The sums over every picked gram. A gram whose field is not a number makes
/// the sum unknown, and the reading is refused rather than short.
fn derive_all(definition: &LexostatusDefinition, grams: &[&Gram]) -> Result<Map<String, Value>> {
    let mut out = Map::new();
    for (name, derivation) in &definition.reduction.derivations {
        let Derivation::Sum { sum: field, .. } = derivation else {
            continue;
        };
        let mut whole: i64 = 0;
        let mut fraction: Option<f64> = None;
        for gram in grams {
            let value = gram.fields.get(field).ok_or_else(|| {
                refused(format!(
                    "lexostatus '{}': gram '{}' has no '{field}' to sum",
                    definition.name, gram.id
                ))
            })?;
            match (value.as_i64(), value.as_f64()) {
                (Some(i), _) => {
                    whole = whole.checked_add(i).ok_or_else(|| {
                        refused(format!(
                            "lexostatus '{}': the sum of '{field}' overflows",
                            definition.name
                        ))
                    })?;
                }
                (None, Some(f)) => *fraction.get_or_insert(0.0) += f,
                (None, None) => {
                    return Err(refused(format!(
                        "lexostatus '{}': '{field}' of gram '{}' is {value}, not a number",
                        definition.name, gram.id
                    )))
                }
            }
        }
        let value = match fraction {
            None => Value::from(whole),
            // Exact for any sum of eurocents the cell records.
            #[allow(clippy::cast_precision_loss)]
            Some(f) => Number::from_f64(f + whole as f64)
                .map(Value::Number)
                .ok_or_else(|| refused(format!("the sum of '{field}' is not a number")))?,
        };
        out.insert(name.clone(), value);
    }
    Ok(out)
}
