//! Which actions a requested output depends on (RFC-043).
//!
//! An article executes only the actions in the dependency closure of the
//! outputs it was asked for. The closure follows `$name` references between
//! the article's own outputs; anything else a reference can name (an input, a
//! parameter, a definition) is not an action and is left to resolution.
//!
//! Over-including is always safe: an action that runs without being needed
//! costs work, never a wrong value. Under-including is what must not happen,
//! so the walk goes through [`ActionOperation::operands`], which is exhaustive
//! over the operation variants, and a requested name no action produces makes
//! the article run in full.

use crate::article::{Action, ActionValue, Article};
use crate::types::Value;
use std::collections::{BTreeMap, BTreeSet};

/// The outputs an article's actions produce, in declaration order.
pub(crate) fn action_outputs(article: &Article) -> impl Iterator<Item = &str> {
    article
        .get_execution_spec()
        .and_then(|e| e.actions.as_ref())
        .into_iter()
        .flatten()
        .filter_map(|a| a.output.as_deref())
}

/// The outputs to compute for `requested`, or `None` when every action must run.
///
/// `None` is returned when a requested name is not produced by any action of
/// the article: whatever produces it (a hook, an override) is outside what
/// this closure can see, and running everything keeps the old behavior there.
pub(crate) fn required_outputs(actions: &[Action], requested: &[&str]) -> Option<BTreeSet<String>> {
    // Every action that writes a name counts: an output assigned twice reads
    // what both assignments read.
    let mut produced: BTreeMap<&str, Vec<&Action>> = BTreeMap::new();
    for action in actions {
        if let Some(name) = action.output.as_deref() {
            produced.entry(name).or_default().push(action);
        }
    }
    if requested.iter().any(|name| !produced.contains_key(name)) {
        return None;
    }

    let mut required = BTreeSet::new();
    let mut pending: Vec<&str> = requested.to_vec();
    while let Some(name) = pending.pop() {
        if !required.insert(name.to_string()) {
            continue;
        }
        for action in produced.get(name).into_iter().flatten() {
            for reference in referenced_names(action) {
                if let Some((key, _)) = produced.get_key_value(reference.as_str()) {
                    pending.push(key);
                }
            }
        }
    }
    Some(required)
}

/// The name a `$reference` reads, by its base (`$a.b` gives `a`); `None` for
/// a string that is not a reference.
pub(crate) fn reference_base(reference: &str) -> Option<&str> {
    let path = reference.strip_prefix('$')?;
    Some(path.split('.').next().unwrap_or(path))
}

/// Every `$name` an action refers to, by its base name.
pub(crate) fn referenced_names(action: &Action) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    action
        .operands()
        .for_each(|operand| visit_value(operand, &mut names));
    names
}

fn visit_value(value: &ActionValue, names: &mut BTreeSet<String>) {
    match value {
        ActionValue::Literal(literal) => visit_literal(literal, names),
        ActionValue::Operation(op) => op
            .operands()
            .into_iter()
            .for_each(|operand| visit_value(operand, names)),
    }
}

fn visit_literal(literal: &Value, names: &mut BTreeSet<String>) {
    match literal {
        Value::String(s) => names.extend(reference_base(s).map(str::to_string)),
        Value::Array(items) => items.iter().for_each(|item| visit_literal(item, names)),
        Value::Object(fields) => fields
            .values()
            .for_each(|field| visit_literal(field, names)),
        _ => {}
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    fn actions(yaml: &str) -> Vec<Action> {
        serde_yaml_ng::from_str(yaml).unwrap()
    }

    const CHAIN: &str = r#"
- output: leeftijd_ok
  value:
    operation: GREATER_THAN_OR_EQUAL
    subject: $leeftijd
    value: 18
- output: voldoet
  value:
    operation: AND
    conditions:
      - $leeftijd_ok
      - $is_verzekerde
- output: hoogte
  value:
    operation: IF
    cases:
      - when: $voldoet
        then:
          operation: MULTIPLY
          values: [$inkomen, 0.1]
    default: 0
- output: los
  value: $partner.inkomen
"#;

    #[test]
    fn closure_follows_output_references_only() {
        let acts = actions(CHAIN);
        let required = required_outputs(&acts, &["voldoet"]).unwrap();
        assert_eq!(
            required.into_iter().collect::<Vec<_>>(),
            vec!["leeftijd_ok".to_string(), "voldoet".to_string()]
        );
    }

    #[test]
    fn closure_is_transitive_through_nested_operations() {
        let acts = actions(CHAIN);
        let required = required_outputs(&acts, &["hoogte"]).unwrap();
        assert_eq!(
            required.into_iter().collect::<Vec<_>>(),
            vec![
                "hoogte".to_string(),
                "leeftijd_ok".to_string(),
                "voldoet".to_string()
            ]
        );
    }

    #[test]
    fn several_requested_outputs_union_their_closures() {
        let acts = actions(CHAIN);
        let required = required_outputs(&acts, &["leeftijd_ok", "los"]).unwrap();
        assert_eq!(
            required.into_iter().collect::<Vec<_>>(),
            vec!["leeftijd_ok".to_string(), "los".to_string()]
        );
    }

    #[test]
    fn a_name_no_action_produces_runs_everything() {
        let acts = actions(CHAIN);
        assert!(required_outputs(&acts, &["van_een_hook"]).is_none());
    }

    #[test]
    fn an_output_assigned_twice_reads_what_both_assignments_read() {
        let acts = actions(
            r#"
- output: a
  value: 1
- output: x
  value: $a
- output: x
  value:
    operation: ADD
    values: [$x, 1]
- output: los
  value: 2
"#,
        );
        let required = required_outputs(&acts, &["x"]).unwrap();
        assert_eq!(
            required.into_iter().collect::<Vec<_>>(),
            vec!["a".to_string(), "x".to_string()]
        );
    }

    #[test]
    fn a_reference_inside_a_literal_list_or_object_counts() {
        let acts = actions(
            r#"
- output: a
  value: 1
- output: b
  value: 2
- output: lijst
  value: [$a, {veld: $b}]
"#,
        );
        let required = required_outputs(&acts, &["lijst"]).unwrap();
        assert_eq!(
            required.into_iter().collect::<Vec<_>>(),
            vec!["a".to_string(), "b".to_string(), "lijst".to_string()]
        );
    }

    /// The action-level operation shape (`operation` with `subject`, `values`
    /// or `conditions` on the action itself) is walked like a nested one.
    #[test]
    fn the_action_level_operation_shape_is_walked() {
        let acts = actions(
            r#"
- output: a
  value: 1
- output: b
  value: 2
- output: c
  value: 3
- output: d
  value: 4
- output: vergelijk
  operation: EQUALS
  subject: $a
  value: 1
- output: som
  operation: ADD
  values: [$b, 1]
- output: en
  operation: AND
  conditions: [$c, $d]
"#,
        );
        let required = required_outputs(&acts, &["vergelijk", "som", "en"]).unwrap();
        assert_eq!(
            required.into_iter().collect::<Vec<_>>(),
            ["a", "b", "c", "d", "en", "som", "vergelijk"]
                .map(String::from)
                .to_vec()
        );
    }

    #[test]
    fn dotted_reference_counts_by_its_base() {
        let acts = actions(CHAIN);
        let names = referenced_names(&acts[3]);
        assert!(names.contains("partner"));
    }
}
