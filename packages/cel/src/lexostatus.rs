//! Reading a cell's own chronicle back: a lexostatus reduces the grams to the
//! parameters an article reads ("the reduction belongs to the source"). The
//! law asks a datum ("de dag van ontvangst"); that the cell keeps it in a
//! chronicle is the cell's business, so the reduction is cell configuration.

use serde_json::{Map, Value};

use crate::chronicle::{Chronicle, Gram};
use crate::config::{Derivation, LexostatusDefinition, Moment};
use crate::error::{refused, Result};

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

/// Reduce `chronicle` by `definition` to its parameters.
pub fn read(
    definition: &LexostatusDefinition,
    inputs: &Map<String, Value>,
    chronicle: &Chronicle,
) -> Result<Map<String, Value>> {
    for i in &definition.inputs {
        if !inputs.contains_key(&i.name) {
            return Err(refused(format!(
                "lexostatus '{}' needs input '{}'",
                definition.name, i.name
            )));
        }
    }
    let filter = &definition.reduction.filter;
    let type_ = filter
        .type_
        .as_deref()
        .map(|t| resolve(t, inputs))
        .transpose()?;
    let subtype = filter
        .subtype
        .as_deref()
        .map(|t| resolve(t, inputs))
        .transpose()?;
    let root = filter
        .root
        .as_deref()
        .map(|t| resolve(t, inputs))
        .transpose()?;
    let matches = |g: &Gram| {
        type_.is_none_or(|t| g.type_ == t)
            && subtype.is_none_or(|s| g.subtype.as_deref() == Some(s))
            && root.is_none_or(|r| chronicle.root_of(g) == r)
    };
    // `pick: latest`: the chronicle is in recording order.
    let gram = chronicle
        .grams()
        .iter()
        .rev()
        .find(|g| matches(g))
        .ok_or_else(|| {
            refused(format!(
                "lexostatus '{}': no gram in chronicle '{}' matches",
                definition.name, definition.reduction.chronicle
            ))
        })?;
    let mut out = Map::new();
    for (name, derivation) in &definition.reduction.derivations {
        let value = match derivation {
            Derivation::Field { field, .. } => {
                gram.fields.get(field).cloned().unwrap_or(Value::Null)
            }
            Derivation::Moment {
                moment: Moment::EffectiveAt,
                ..
            } => Value::String(gram.effective_at.chars().take(10).collect()),
            Derivation::Filled { filled, .. } => Value::Bool(match gram.fields.get(filled) {
                None | Some(Value::Null) => false,
                Some(Value::String(s)) => !s.trim().is_empty(),
                Some(_) => true,
            }),
        };
        out.insert(name.clone(), value);
    }
    Ok(out)
}
