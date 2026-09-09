//! Checks on a law document at load time that the schema cannot express.
//!
//! An Unknown (RFC-036) is a fact nobody has, and only resolution produces it:
//! a `source: {}` input without data, an optional parameter the caller left
//! out. Its serialized form `{"__unknown": true, "missing": [...]}` exists so
//! results and traces round-trip, and `Value`'s deserializer therefore builds
//! the variant from any document that carries the sentinel, a law's YAML
//! included. A law that wrote one would put an invented provenance into an
//! outcome, so the loader walks every literal in the definitions and the
//! actions and refuses the document.

use crate::article::ArticleBasedLaw;
use crate::error::{EngineError, Result};
use regelrecht_law_model::{ActionOperation, ActionValue};

/// Refuse a law that writes an Unknown value as a literal anywhere in its
/// definitions or actions (RFC-036).
pub(crate) fn reject_unknown_literals(law: &ArticleBasedLaw) -> Result<()> {
    for article in &law.articles {
        if let Some(definitions) = article.get_definitions() {
            for (name, definition) in definitions {
                if definition.value().contains_unknown() {
                    return Err(unknown_literal(
                        law,
                        &article.number,
                        format!("definition '{name}'"),
                    ));
                }
            }
        }
        let Some(actions) = article
            .get_execution_spec()
            .and_then(|e| e.actions.as_ref())
        else {
            continue;
        };
        for (index, action) in actions.iter().enumerate() {
            let where_ = match &action.output {
                Some(output) => format!("action for output '{output}'"),
                None => format!("action {}", index + 1),
            };
            let operands = action
                .value
                .iter()
                .chain(action.subject.iter())
                .chain(action.values.iter().flatten())
                .chain(action.conditions.iter().flatten());
            for operand in operands {
                if action_value_contains_unknown(operand) {
                    return Err(unknown_literal(law, &article.number, where_));
                }
            }
        }
    }
    Ok(())
}

fn unknown_literal(law: &ArticleBasedLaw, article: &str, where_: String) -> EngineError {
    EngineError::LoadError(format!(
        "law '{}': article {article} writes an unknown value as a literal ({where_}); \
         an unknown cannot be written in a law, only resolution produces it (RFC-036)",
        law.id
    ))
}

/// Whether an Unknown literal sits anywhere in an action value: a literal at
/// this level, or one nested in an operation's operands.
fn action_value_contains_unknown(value: &ActionValue) -> bool {
    match value {
        ActionValue::Literal(literal) => literal.contains_unknown(),
        ActionValue::Operation(op) => operation_contains_unknown(op),
    }
}

/// Exhaustive over the operation variants, so a new operation cannot carry a
/// literal past this check unnoticed.
fn operation_contains_unknown(op: &ActionOperation) -> bool {
    let any = |values: &[&ActionValue]| values.iter().any(|v| action_value_contains_unknown(v));
    let all = |values: &[ActionValue]| values.iter().any(action_value_contains_unknown);
    match op {
        ActionOperation::Equals { subject, value }
        | ActionOperation::NotEquals { subject, value }
        | ActionOperation::GreaterThan { subject, value }
        | ActionOperation::LessThan { subject, value }
        | ActionOperation::GreaterThanOrEqual { subject, value }
        | ActionOperation::LessThanOrEqual { subject, value } => any(&[subject, value]),
        ActionOperation::Add { values }
        | ActionOperation::Subtract { values }
        | ActionOperation::Multiply { values }
        | ActionOperation::Divide { values }
        | ActionOperation::Max { values }
        | ActionOperation::Min { values } => all(values),
        ActionOperation::Round { value, .. }
        | ActionOperation::Ceil { value, .. }
        | ActionOperation::Floor { value, .. }
        | ActionOperation::Not { value } => any(&[value]),
        ActionOperation::And { conditions } | ActionOperation::Or { conditions } => all(conditions),
        ActionOperation::If { cases, default } => {
            cases.iter().any(|case| any(&[&case.when, &case.then]))
                || default.as_ref().is_some_and(action_value_contains_unknown)
        }
        ActionOperation::IsNull { subject } | ActionOperation::NotNull { subject } => {
            any(&[subject])
        }
        ActionOperation::In {
            subject,
            value,
            values,
        }
        | ActionOperation::NotIn {
            subject,
            value,
            values,
        } => {
            any(&[subject])
                || value.as_ref().is_some_and(action_value_contains_unknown)
                || values.as_deref().is_some_and(all)
        }
        ActionOperation::List { items } => all(items),
        ActionOperation::Foreach {
            collection,
            body,
            filter,
            ..
        } => any(&[collection, body]) || filter.as_ref().is_some_and(action_value_contains_unknown),
        ActionOperation::Age {
            date_of_birth,
            reference_date,
        } => any(&[date_of_birth, reference_date]),
        ActionOperation::DateAdd {
            date,
            years,
            months,
            weeks,
            days,
        } => {
            any(&[date])
                || [years, months, weeks, days]
                    .iter()
                    .any(|part| part.as_ref().is_some_and(action_value_contains_unknown))
        }
        ActionOperation::Date { year, month, day } => any(&[year, month, day]),
        ActionOperation::DayOfWeek { date } => any(&[date]),
        ActionOperation::DateDiff { from, to, unit } => any(&[from, to, unit]),
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::service::LawExecutionService;
    use crate::types::Value;

    /// The review's `literal-unknown.json` law: an Unknown sentinel written
    /// straight into an `AND`.
    const LITERAL_IN_ACTION: &str = r#"
$id: lit
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: t
    machine_readable:
      execution:
        output:
          - name: x
            type: boolean
        actions:
          - output: x
            value:
              operation: AND
              conditions:
                - true
                - {"__unknown": true, "missing": [{"law": "verzonnen", "name": "iets", "kind": "no_data"}]}
"#;

    const LITERAL_IN_DEFINITION: &str = r#"
$id: lit_def
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '2'
    text: t
    machine_readable:
      definitions:
        GRENS:
          - 1
          - {"__unknown": true, "missing": [{"law": "verzonnen", "name": "iets", "kind": "no_data"}]}
      execution:
        output:
          - name: x
            type: number
        actions:
          - output: x
            value: $GRENS
"#;

    const LITERAL_DEEP_IN_IF: &str = r#"
$id: lit_if
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '3'
    text: t
    machine_readable:
      execution:
        output:
          - name: x
            type: number
        actions:
          - output: x
            value:
              operation: IF
              cases:
                - when: true
                  then:
                    operation: FOREACH
                    collection:
                      operation: LIST
                      items:
                        - 1
                    as: i
                    body:
                      operation: ADD
                      values:
                        - $i
                        - {"__unknown": true, "missing": [{"law": "verzonnen", "name": "iets", "kind": "no_data"}]}
"#;

    /// Not an Unknown: a sentinel without a usable `missing` list stays a
    /// plain object, and a law may write whatever objects it likes.
    const MALFORMED_SENTINEL: &str = r#"
$id: obj
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: t
    machine_readable:
      execution:
        output:
          - name: x
            type: object
        actions:
          - output: x
            value: {"__unknown": true, "missing": []}
"#;

    #[test]
    fn a_law_cannot_write_an_unknown_literal() {
        for (yaml, where_) in [
            (
                LITERAL_IN_ACTION,
                "article 1 writes an unknown value as a literal (action for output 'x')",
            ),
            (
                LITERAL_IN_DEFINITION,
                "article 2 writes an unknown value as a literal (definition 'GRENS')",
            ),
            (
                LITERAL_DEEP_IN_IF,
                "article 3 writes an unknown value as a literal (action for output 'x')",
            ),
        ] {
            let mut service = LawExecutionService::new();
            let err = service
                .load_law(yaml)
                .expect_err("a law with an Unknown literal must not load");
            let message = err.to_string();
            assert!(
                matches!(err, EngineError::LoadError(_)) && message.contains(where_),
                "got {err:?}"
            );
            assert!(message.contains("RFC-036"), "got {message}");
        }
    }

    #[test]
    fn a_malformed_sentinel_is_an_ordinary_object_and_loads() {
        let mut service = LawExecutionService::new();
        service.load_law(MALFORMED_SENTINEL).unwrap();
        let result = service
            .evaluate_law_output("obj", "x", Default::default(), "2025-01-01")
            .unwrap();
        assert!(matches!(result.outputs.get("x"), Some(Value::Object(_))));
    }
}
