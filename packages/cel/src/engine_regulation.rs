//! A lexostatus definition as an engine regulation, in memory: the
//! patterns that experiment A wrote out with a helper script, now as code (see
//! the report, "Engine-route in de runtime"). For a lexostatus from the law
//! ([`crate::law`]) there is then no engine regulation on disk anymore: the
//! reading is in the article, and this module translates it.
//!
//! The patterns, per derivation:
//! - `pick: latest` is a helper output with the highest `sequence` of the
//!   grams through the filter (`latest`, or `latest_<name>` with its own
//!   filter); a field of that gram is a FOREACH on that sequence;
//! - a number or yes/no goes with MAX or OR, a text or date with ADD, with
//!   an IF in front because ADD over nothing gives 0;
//! - `exists` is OR, `sum` is ADD, `filled` is "not null and not empty".
//!
//! The type of an output comes from the parameter of the reading article;
//! an extra field is text.

use std::collections::BTreeMap;

use serde_json::{json, Map, Value};

use crate::lexostatus_engine::{helper_of, LATEST};
use crate::reduction::{is_gram_key, Derivation, Filter, LexostatusDefinition, Moment};

/// How an output is combined in the engine.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Text,
    Date,
    Number,
    YesNo,
    List,
}

impl Kind {
    fn from_type(t: &str) -> Self {
        match t {
            "date" => Kind::Date,
            "number" | "amount" => Kind::Number,
            "boolean" => Kind::YesNo,
            "array" => Kind::List,
            _ => Kind::Text,
        }
    }

    fn engine_type(self) -> &'static str {
        match self {
            Kind::Text => "string",
            Kind::Date => "date",
            Kind::Number => "number",
            Kind::YesNo => "boolean",
            Kind::List => "array",
        }
    }
}

fn eq(subject: &str, value: Value) -> Value {
    json!({"operation": "EQUALS", "subject": subject, "value": value})
}

fn not_null(subject: &str) -> Value {
    json!({"operation": "NOT", "value": eq(subject, Value::Null)})
}

/// An AND over the conditions (a nested AND flattened); a single
/// condition stays itself.
fn all_of(conditions: impl IntoIterator<Item = Value>) -> Value {
    let mut flat = Vec::new();
    for v in conditions {
        match v.get("operation").and_then(Value::as_str) {
            Some("AND") => flat.extend(v["conditions"].as_array().cloned().unwrap_or_default()),
            _ => flat.push(v),
        }
    }
    if flat.len() == 1 {
        flat.remove(0)
    } else {
        json!({"operation": "AND", "conditions": flat})
    }
}

/// Not null, and for text also not empty. EQUALS is structural: a number
/// is never equal to "", so this is safe for every type.
fn filled(subject: &str) -> Value {
    all_of([
        not_null(subject),
        json!({"operation": "NOT", "value": eq(subject, json!(""))}),
    ])
}

fn foreach(
    collection: &str,
    alias: &str,
    body: Value,
    filter: Option<Value>,
    combine: Option<&str>,
) -> Value {
    let mut o = Map::new();
    o.insert("operation".into(), json!("FOREACH"));
    o.insert("collection".into(), json!(collection));
    o.insert("as".into(), json!(alias));
    if let Some(f) = filter {
        o.insert("filter".into(), f);
    }
    o.insert("body".into(), body);
    if let Some(c) = combine {
        o.insert("combine".into(), json!(c));
    }
    Value::Object(o)
}

fn grams(body: Value, filter: Value, combine: &str) -> Value {
    foreach("$grams", "g", body, Some(filter), Some(combine))
}

fn as_then(when: Value, then: Value) -> Value {
    json!({"operation": "IF", "cases": [{"when": when, "then": then}]})
}

/// The path of a key or field of the gram `g` in the engine.
fn path(p: &str) -> String {
    if is_gram_key(p) || matches!(p, "effective_date" | "recorded_date" | "sequence") {
        format!("$g.{p}")
    } else {
        format!("$g.fields.{p}")
    }
}

/// A filter as conditions; `name` first, so that the engine reads a field only
/// for a gram of that event.
fn filter_of(f: &Filter) -> Vec<Value> {
    let mut keys: Vec<&String> = f.keys().collect();
    keys.sort_by_key(|k| (k.as_str() != "name", !is_gram_key(k)));
    keys.into_iter()
        .map(|k| eq(&path(k), Value::String(f[k].clone())))
        .collect()
}

fn chosen(helper: &str, extra: Option<Value>) -> Value {
    all_of(std::iter::once(eq("$g.sequence", json!(format!("${helper}")))).chain(extra))
}

/// A field of the chosen gram.
fn field(helper: &str, p: &str, kind: Kind) -> Value {
    let f = path(p);
    if kind == Kind::Number {
        return grams(json!(f), chosen(helper, Some(not_null(&f))), "MAX");
    }
    let cond = chosen(
        helper,
        Some(if matches!(kind, Kind::Text | Kind::Date) {
            filled(&f)
        } else {
            not_null(&f)
        }),
    );
    as_then(
        grams(json!(true), cond.clone(), "OR"),
        grams(
            json!(f),
            cond,
            if kind == Kind::YesNo { "OR" } else { "ADD" },
        ),
    )
}

fn moment_path(m: Moment) -> &'static str {
    match m {
        Moment::EffectiveAt => "effective_date",
        Moment::RecordedAt => "recorded_date",
    }
}

fn year_of(helper: &str, p: &str) -> Value {
    let f = path(p);
    let cond = chosen(helper, Some(not_null(&f)));
    as_then(
        grams(json!(true), cond.clone(), "OR"),
        grams(
            json!({"operation": "DATE_PART", "date": f, "in": "year"}),
            cond,
            "ADD",
        ),
    )
}

/// A legal basis as the `legal_basis` of an action.
fn legal_basis(legal_basis: &str, names: &BTreeMap<String, String>) -> Option<Value> {
    let g = crate::regulations::parse(legal_basis).ok()?;
    let mut o = Map::new();
    o.insert(
        "law".into(),
        json!(names
            .get(g.regulation)
            .cloned()
            .unwrap_or_else(|| g.regulation.to_string())),
    );
    o.insert("article".into(), json!(g.article));
    if let Some(l) = g.paragraph {
        o.insert("paragraph".into(), json!(l));
    }
    Some(Value::Object(o))
}

/// The version date of a generated lexostatus regulation. It is a
/// translation of a reduction, not a regulation with a history of its own:
/// it holds on every reference date the engine may be asked for, so that a
/// reduction before some arbitrary date does not fail to find a version.
const SINCE_ALWAYS: &str = "0001-01-01";

/// The engine regulation of a lexostatus as YAML text (JSON is YAML), with
/// `$id` `id`. `names` gives per regulation id the name for `legal_basis`. A
/// derivation the engine route does not know (`period_of`, `contains`,
/// `collect`, a list) is an error.
pub fn regulation(
    def: &LexostatusDefinition,
    id: &str,
    names: &BTreeMap<String, String>,
) -> Result<String, String> {
    let r = &def.reduction;
    let types = def.law.as_ref().map(|w| &w.types);
    let mut outputs: Vec<Value> = Vec::new();
    let mut actions: Vec<Value> = Vec::new();
    if r.pick.is_some() {
        outputs.push(json!({"name": LATEST, "type": "number", "nullable": true}));
        actions.push(json!({"output": LATEST, "value":
            grams(json!("$g.sequence"), all_of(filter_of(&r.filter)), "MAX")}));
    }
    for (name, a) in r.derivations.iter().chain(&r.extra_fields) {
        let kind = types
            .and_then(|t| t.get(name))
            .map(|t| Kind::from_type(t))
            .unwrap_or(Kind::Text);
        let filter_own = || -> Filter {
            let mut f = r.filter.clone();
            f.extend(a.filter().cloned().unwrap_or_default());
            f
        };
        // A derivation with its own filter and `pick`: its own gram.
        let mut helper = LATEST.to_string();
        if matches!(
            &a.derivation,
            Derivation::LatestField { .. }
                | Derivation::LatestMoment { .. }
                | Derivation::LatestYearOf { .. }
        ) {
            helper = helper_of(name);
            outputs.push(json!({"name": helper, "type": "number", "nullable": true}));
            actions.push(json!({"output": helper, "value":
                grams(json!("$g.sequence"), all_of(filter_of(&filter_own())), "MAX")}));
        }
        let (value, nullable, kind) = match &a.derivation {
            Derivation::Field { field: v } | Derivation::LatestField { field: v, .. } => {
                (field(&helper, v, kind), true, kind)
            }
            Derivation::Moment { moment } | Derivation::LatestMoment { moment, .. } => (
                field(&helper, moment_path(*moment), Kind::Date),
                true,
                Kind::Date,
            ),
            Derivation::YearOf { year_of: p } | Derivation::LatestYearOf { year_of: p, .. } => {
                (year_of(&helper, p), true, Kind::Number)
            }
            Derivation::Filled { filled: p } => (
                grams(json!(true), chosen(&helper, Some(filled(&path(p)))), "OR"),
                false,
                Kind::YesNo,
            ),
            Derivation::Equals { equals } => {
                let f = path(&equals.field);
                let cond = chosen(&helper, Some(not_null(&f)));
                (
                    as_then(
                        grams(json!(true), cond.clone(), "OR"),
                        grams(eq(&f, equals.value.clone()), cond, "OR"),
                    ),
                    true,
                    Kind::YesNo,
                )
            }
            Derivation::EachRow {
                table,
                each_row,
                only_where,
            } => {
                let t = path(table);
                let col = format!("$r.{each_row}");
                let rf = only_where
                    .as_ref()
                    .map(|w| eq(&format!("$r.{w}"), json!(true)));
                (
                    grams(
                        all_of([
                            foreach(&t, "r", json!(true), None, Some("OR")),
                            foreach(&t, "r", filled(&col), rf, Some("AND")),
                        ]),
                        chosen(&helper, None),
                        "OR",
                    ),
                    false,
                    Kind::YesNo,
                )
            }
            Derivation::OneRow { table, one_row } => (
                grams(
                    foreach(
                        &path(table),
                        "r",
                        eq(&format!("$r.{one_row}"), json!(true)),
                        None,
                        Some("OR"),
                    ),
                    chosen(&helper, None),
                    "OR",
                ),
                false,
                Kind::YesNo,
            ),
            Derivation::Exists {
                exists: true,
                filled: g,
                ..
            } => {
                let mut c = filter_of(&filter_own());
                if let Some(g) = g {
                    c.push(filled(&path(g)));
                }
                (grams(json!(true), all_of(c), "OR"), false, Kind::YesNo)
            }
            Derivation::Sum { sum, .. } => {
                let f = path(sum);
                let mut c = filter_of(&filter_own());
                c.push(not_null(&f));
                (grams(json!(f), all_of(c), "ADD"), false, Kind::Number)
            }
            other => {
                return Err(format!(
                    "derivation '{name}' ({}) has no engine pattern",
                    serde_json::to_string(other).unwrap_or_default()
                ))
            }
        };
        let mut o = json!({"name": name, "type": kind.engine_type()});
        if nullable {
            o["nullable"] = json!(true);
        }
        outputs.push(o);
        let mut act = Map::new();
        act.insert("output".into(), json!(name));
        if let Some(lb) = a.legal_basis.first().and_then(|g| legal_basis(g, names)) {
            act.insert("legal_basis".into(), lb);
        }
        act.insert("value".into(), value);
        actions.push(Value::Object(act));
    }
    let parameters: Vec<Value> = def
        .inputs
        .iter()
        .map(|i| json!({"name": i.name, "type": i.kind, "required": true}))
        .collect();
    let text = match &def.law {
        Some(w) => format!("How {} reads its parameters from the chronicle (produces.extensions.chronolex.reads), translated to the engine.", w.article),
        None => format!("The lexostatus {} of the cell, translated to the engine.", def.name),
    };
    let doc = json!({
        "$id": id,
        "name": format!("Lexostatus {}", def.name),
        "regulatory_layer": "UITVOERINGSBELEID",
        "publication_date": SINCE_ALWAYS,
        "valid_from": SINCE_ALWAYS,
        "url": format!("urn:regelrecht:cel:lexostatus:{}", def.name),
        "articles": [{
            "number": "1",
            "text": text,
            "url": format!("urn:regelrecht:cel:lexostatus:{}:1", def.name),
            "machine_readable": {"execution": {
                "parameters": parameters,
                "input": [{"name": "grams", "type": "array", "source": {}}],
                "output": outputs,
                "actions": actions,
            }},
        }],
    });
    serde_json::to_string_pretty(&doc).map_err(|e| e.to_string())
}
