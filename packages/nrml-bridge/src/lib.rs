//! Minimal RegelRecht YAML ↔ NRML JSON bridge for the 4LM interop subset.
//!
//! Maps the execution DSL used by the Luxembourg flight-tax example
//! (AND / OR / IF / EQUALS / LESS_THAN / MULTIPLY) into an NRML facts+rules
//! document. Not a full NRML engine.

use std::collections::BTreeMap;

use regelrecht_law_model::{
    ActionOperation, ActionValue, ArticleBasedLaw, ParameterType, Value,
};
use serde_json::{json, Map, Value as JsonValue};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum BridgeError {
    #[error("YAML parse error: {0}")]
    Yaml(#[from] serde_yaml_ng::Error),
    #[error("{0}")]
    Message(String),
}

/// Convert RegelRecht law YAML into an NRML-facts document (subset).
pub fn yaml_to_nrml(yaml: &str) -> Result<JsonValue, BridgeError> {
    let law: ArticleBasedLaw = serde_yaml_ng::from_str(yaml)?;
    law_to_nrml(&law)
}

pub fn law_to_nrml(law: &ArticleBasedLaw) -> Result<JsonValue, BridgeError> {
    let mut facts = Map::new();
    let mut rules = Map::new();

    let amount_fact_id = stable_uuid("amount-fact");
    let amount_item_id = stable_uuid("amount-item");
    facts.insert(
        amount_fact_id.clone(),
        json!({
            "name": { "nl": "Bedrag", "en": "Amount", "fr": "Montant", "de": "Betrag" },
            "items": {
                amount_item_id: {
                    "name": { "nl": "Bedrag definitie", "en": "Amount definition" },
                    "versions": [{ "validFrom": year_from(law), "type": "numeric", "precision": 2, "unit": "€" }]
                }
            }
        }),
    );

    for article in &law.articles {
        let Some(exec) = article.get_execution_spec() else {
            continue;
        };

        let param_fact_id = stable_uuid(&format!("{}-params", law.id));
        let mut param_items = Map::new();
        let mut param_pointers: BTreeMap<String, String> = BTreeMap::new();

        if let Some(parameters) = &exec.parameters {
            for p in parameters {
                let item_id = stable_uuid(&format!("{}-param-{}", law.id, p.name));
                let type_json = match p.param_type {
                    ParameterType::Boolean => json!("boolean"),
                    ParameterType::Number | ParameterType::Amount => json!("numeric"),
                    other => json!(format!("{other:?}").to_ascii_lowercase()),
                };
                param_items.insert(
                    item_id.clone(),
                    json!({
                        "name": {
                            "nl": p.name,
                            "en": p.name,
                            "fr": p.name,
                            "de": p.name
                        },
                        "versions": [{ "validFrom": year_from(law), "type": type_json }]
                    }),
                );
                param_pointers.insert(
                    p.name.clone(),
                    format!("#/facts/{param_fact_id}/items/{item_id}"),
                );
            }
        }

        facts.insert(
            param_fact_id,
            json!({
                "name": {
                    "nl": law.name.clone().unwrap_or_else(|| law.id.clone()),
                    "en": law.id.clone()
                },
                "items": param_items
            }),
        );

        let out_fact_id = stable_uuid(&format!("{}-outputs", law.id));
        let mut out_items = Map::new();
        let mut out_pointers: BTreeMap<String, String> = BTreeMap::new();

        if let Some(outputs) = &exec.output {
            for o in outputs {
                let item_id = stable_uuid(&format!("{}-out-{}", law.id, o.name));
                let type_json = match o.output_type {
                    ParameterType::Boolean => {
                        json!({ "type": "characteristic", "subtype": "possessive" })
                    }
                    ParameterType::Number | ParameterType::Amount => {
                        json!({ "$ref": format!("#/facts/{amount_fact_id}") })
                    }
                    other => json!(format!("{other:?}").to_ascii_lowercase()),
                };
                out_items.insert(
                    item_id.clone(),
                    json!({
                        "name": { "nl": o.name, "en": o.name },
                        "versions": [{ "validFrom": year_from(law), "type": type_json }]
                    }),
                );
                out_pointers.insert(
                    o.name.clone(),
                    format!("#/facts/{out_fact_id}/items/{item_id}"),
                );
            }
        }

        facts.insert(
            out_fact_id,
            json!({
                "name": { "nl": "uitkomsten", "en": "outputs" },
                "items": out_items
            }),
        );

        if let Some(actions) = &exec.actions {
            for action in actions {
                let Some(output_name) = &action.output else {
                    continue;
                };
                let Some(value) = &action.value else {
                    continue;
                };
                let rule_id = stable_uuid(&format!("{}-rule-{output_name}", law.id));
                let expression = action_value_to_nrml(value, &param_pointers, &out_pointers)?;
                let target = out_pointers
                    .get(output_name)
                    .cloned()
                    .unwrap_or_else(|| format!("#/outputs/{output_name}"));
                rules.insert(
                    rule_id,
                    json!({
                        "name": { "nl": output_name, "en": output_name },
                        "versions": [{
                            "validFrom": year_from(law),
                            "expression": expression,
                            "target": target
                        }]
                    }),
                );
            }
        }
    }

    Ok(json!({
        "$schema": "https://example.com/nrml-facts-schema.json",
        "version": "3.0",
        "language": "nl",
        "source": {
            "law_id": law.id,
            "jurisdictie": law.jurisdictie,
            "valid_from": law.valid_from
        },
        "facts": facts,
        "rules": rules
    }))
}

/// Rebuild a minimal RegelRecht YAML skeleton from NRML (subset).
pub fn nrml_to_yaml(nrml: &JsonValue) -> Result<String, BridgeError> {
    let source = nrml
        .get("source")
        .ok_or_else(|| BridgeError::Message("nrml missing source".into()))?;
    let law_id = source
        .get("law_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| BridgeError::Message("source.law_id missing".into()))?;
    let jurisdictie = source
        .get("jurisdictie")
        .and_then(|v| v.as_str())
        .unwrap_or("NL");
    let valid_from = source
        .get("valid_from")
        .and_then(|v| v.as_str())
        .unwrap_or("2026-07-21");

    let skeleton = json!({
        "$id": law_id,
        "jurisdictie": jurisdictie,
        "regulatory_layer": "WET",
        "publication_date": valid_from,
        "valid_from": valid_from,
        "url": "https://example.invalid/nrml-bridge",
        "articles": [{
            "number": "1",
            "text": "NRML bridge skeleton — replace with full machine_readable from source YAML.",
            "url": "https://example.invalid/nrml-bridge#1",
            "machine_readable": {
                "execution": {
                    "parameters": [],
                    "output": [],
                    "actions": []
                }
            }
        }]
    });

    serde_yaml_ng::to_string(&skeleton).map_err(|e| BridgeError::Message(format!("yaml emit: {e}")))
}

fn year_from(law: &ArticleBasedLaw) -> String {
    law.valid_from
        .as_deref()
        .or(Some(law.publication_date.as_str()))
        .and_then(|d| d.get(0..4))
        .unwrap_or("2026")
        .to_string()
}

fn stable_uuid(seed: &str) -> String {
    let hash = Sha256::digest(seed.as_bytes());
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&hash[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x50;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Uuid::from_bytes(bytes).to_string()
}

fn resolve_name(
    name: &str,
    params: &BTreeMap<String, String>,
    outputs: &BTreeMap<String, String>,
) -> String {
    let bare = name.trim_start_matches('$');
    params
        .get(bare)
        .or_else(|| outputs.get(bare))
        .cloned()
        .unwrap_or_else(|| name.to_string())
}

fn value_to_json(v: &Value) -> JsonValue {
    serde_json::Value::from(v)
}

/// `$var` string literals become NRML subject pointers; other literals stay values.
fn action_value_to_nrml(
    value: &ActionValue,
    params: &BTreeMap<String, String>,
    outputs: &BTreeMap<String, String>,
) -> Result<JsonValue, BridgeError> {
    match value {
        ActionValue::Operation(op) => action_operation_to_nrml(op, params, outputs),
        ActionValue::Literal(Value::String(s)) if s.starts_with('$') => Ok(json!({
            "subject": resolve_name(s, params, outputs)
        })),
        ActionValue::Literal(v) => Ok(value_to_json(v)),
    }
}

fn subject_as_pointer(
    subject: &ActionValue,
    params: &BTreeMap<String, String>,
    outputs: &BTreeMap<String, String>,
) -> Result<String, BridgeError> {
    match subject {
        ActionValue::Literal(Value::String(s)) => Ok(resolve_name(s, params, outputs)),
        other => Err(BridgeError::Message(format!(
            "subject must be a $variable, got {other:?}"
        ))),
    }
}

fn action_operation_to_nrml(
    op: &ActionOperation,
    params: &BTreeMap<String, String>,
    outputs: &BTreeMap<String, String>,
) -> Result<JsonValue, BridgeError> {
    match op {
        ActionOperation::And { conditions } => {
            let mapped: Result<Vec<_>, _> = conditions
                .iter()
                .map(|c| action_value_to_nrml(c, params, outputs))
                .collect();
            Ok(json!({ "operation": "AND", "conditions": mapped? }))
        }
        ActionOperation::Or { conditions } => {
            let mapped: Result<Vec<_>, _> = conditions
                .iter()
                .map(|c| action_value_to_nrml(c, params, outputs))
                .collect();
            Ok(json!({ "operation": "OR", "conditions": mapped? }))
        }
        ActionOperation::Equals { subject, value } => Ok(json!({
            "operation": "EQUALS",
            "subject": subject_as_pointer(subject, params, outputs)?,
            "value": action_value_to_nrml(value, params, outputs)?
        })),
        ActionOperation::LessThan { subject, value } => Ok(json!({
            "operation": "LESS_THAN",
            "subject": subject_as_pointer(subject, params, outputs)?,
            "value": action_value_to_nrml(value, params, outputs)?
        })),
        ActionOperation::Multiply { values } => {
            let operands: Result<Vec<_>, _> = values
                .iter()
                .map(|v| action_value_to_nrml(v, params, outputs))
                .collect();
            Ok(json!({ "operation": "MULTIPLY", "operands": operands? }))
        }
        ActionOperation::If { cases, default } => {
            let mut nrml_cases = Vec::new();
            for case in cases {
                nrml_cases.push(json!({
                    "when": action_value_to_nrml(&case.when, params, outputs)?,
                    "then": action_value_to_nrml(&case.then, params, outputs)?
                }));
            }
            let mut obj = json!({ "operation": "IF", "cases": nrml_cases });
            if let Some(d) = default {
                obj.as_object_mut()
                    .expect("object")
                    .insert("default".into(), action_value_to_nrml(d, params, outputs)?);
            }
            Ok(obj)
        }
        other => Err(BridgeError::Message(format!(
            "unsupported ActionOperation for NRML bridge: {}",
            other.operation_name()
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn lu_yaml() -> String {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../corpus/regulation/lu/wet/vliegbelasting_korting_klimaatneutraal_lu/2026-07-21.yaml"
        );
        fs::read_to_string(path).expect("LU yaml")
    }

    #[test]
    fn yaml_to_nrml_contains_rules_and_source() {
        let nrml = yaml_to_nrml(&lu_yaml()).expect("convert");
        assert_eq!(
            nrml["source"]["law_id"],
            json!("vliegbelasting_korting_klimaatneutraal_lu")
        );
        assert_eq!(nrml["source"]["jurisdictie"], json!("LU"));
        assert!(nrml["facts"].as_object().unwrap().len() >= 2);
        assert!(nrml["rules"].as_object().unwrap().len() >= 2);

        let golden_path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/vliegbelasting.nrml.json"
        );
        let fixtures_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures");
        fs::create_dir_all(fixtures_dir).ok();
        if let Ok(golden) = fs::read_to_string(golden_path) {
            let expected: JsonValue = serde_json::from_str(&golden).unwrap();
            pretty_assertions::assert_eq!(nrml, expected);
        } else {
            fs::write(golden_path, serde_json::to_string_pretty(&nrml).unwrap()).unwrap();
            panic!("wrote golden fixture at {golden_path} — re-run test");
        }
    }

    #[test]
    fn nrml_round_trip_preserves_law_id() {
        let nrml = yaml_to_nrml(&lu_yaml()).unwrap();
        let yaml = nrml_to_yaml(&nrml).unwrap();
        assert!(yaml.contains("vliegbelasting_korting_klimaatneutraal_lu"));
        assert!(yaml.contains("LU"));
    }
}
