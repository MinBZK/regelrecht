//! Een lexostatus-definitie als engine-regeling, in het geheugen: de
//! patronen die experiment A met een hulpscript uitschreef, nu als code (zie
//! het verslag, "Engine-route in de runtime"). Voor een lexostatus uit de wet
//! ([`crate::wet`]) staat er dan geen engine-regeling meer op schijf: de
//! lezing staat in het artikel, en deze module vertaalt haar.
//!
//! De patronen, per afleiding:
//! - `kies: laatste` is een hulpuitkomst met de hoogste `volgorde` van de
//!   grammen door het filter (`laatste`, of `laatste_<naam>` bij een eigen
//!   filter); een veld van dat gram is een FOREACH op die volgorde;
//! - een getal of ja/nee gaat met MAX of OR, een tekst of datum met ADD, met
//!   een IF ervoor omdat ADD over niets 0 geeft;
//! - `bestaat` is OR, `som` is ADD, `gevuld` is "niet null en niet leeg".
//!
//! Het type van een uitkomst komt uit de parameter van het lezende artikel;
//! een extra veld is tekst.

use std::collections::BTreeMap;

use serde_json::{json, Map, Value};

use crate::lexostatus_engine::{hulp_van, LAATSTE};
use crate::reductie::{is_gram_sleutel, Afleiding, Filter, LexostatusDefinitie, Moment};

/// Hoe een uitkomst in de engine gecombineerd wordt.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Soort {
    Tekst,
    Datum,
    Getal,
    JaNee,
    Lijst,
}

impl Soort {
    fn uit_type(t: &str) -> Self {
        match t {
            "date" => Soort::Datum,
            "number" | "amount" => Soort::Getal,
            "boolean" => Soort::JaNee,
            "array" => Soort::Lijst,
            _ => Soort::Tekst,
        }
    }

    fn engine_type(self) -> &'static str {
        match self {
            Soort::Tekst => "string",
            Soort::Datum => "date",
            Soort::Getal => "number",
            Soort::JaNee => "boolean",
            Soort::Lijst => "array",
        }
    }
}

fn eq(subject: &str, value: Value) -> Value {
    json!({"operation": "EQUALS", "subject": subject, "value": value})
}

fn niet_null(subject: &str) -> Value {
    json!({"operation": "NOT", "value": eq(subject, Value::Null)})
}

/// Een AND over de voorwaarden (een geneste AND platgeslagen); een enkele
/// voorwaarde blijft zichzelf.
fn en(voorwaarden: impl IntoIterator<Item = Value>) -> Value {
    let mut plat = Vec::new();
    for v in voorwaarden {
        match v.get("operation").and_then(Value::as_str) {
            Some("AND") => plat.extend(v["conditions"].as_array().cloned().unwrap_or_default()),
            _ => plat.push(v),
        }
    }
    if plat.len() == 1 {
        plat.remove(0)
    } else {
        json!({"operation": "AND", "conditions": plat})
    }
}

/// Niet null, en bij tekst ook niet leeg. EQUALS is structureel: een getal
/// is nooit gelijk aan "", dus dit is voor elk type veilig.
fn filled(subject: &str) -> Value {
    en([
        niet_null(subject),
        json!({"operation": "NOT", "value": eq(subject, json!(""))}),
    ])
}

fn foreach(
    collectie: &str,
    als: &str,
    body: Value,
    filter: Option<Value>,
    combine: Option<&str>,
) -> Value {
    let mut o = Map::new();
    o.insert("operation".into(), json!("FOREACH"));
    o.insert("collection".into(), json!(collectie));
    o.insert("as".into(), json!(als));
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

fn als_dan(wanneer: Value, dan: Value) -> Value {
    json!({"operation": "IF", "cases": [{"when": wanneer, "then": dan}]})
}

/// Het pad van een sleutel of veld van het gram `g` in de engine.
fn path(p: &str) -> String {
    if is_gram_sleutel(p) || matches!(p, "effective_date" | "recorded_date" | "sequence") {
        format!("$g.{p}")
    } else {
        format!("$g.fields.{p}")
    }
}

/// Een filter als voorwaarden; `name` eerst, zodat de engine een veld alleen
/// leest bij een gram van dat event.
fn filter_van(f: &Filter) -> Vec<Value> {
    let mut sleutels: Vec<&String> = f.keys().collect();
    sleutels.sort_by_key(|k| (k.as_str() != "name", !is_gram_sleutel(k)));
    sleutels
        .into_iter()
        .map(|k| eq(&path(k), Value::String(f[k].clone())))
        .collect()
}

fn gekozen(hulp: &str, extra: Option<Value>) -> Value {
    en(std::iter::once(eq("$g.sequence", json!(format!("${hulp}")))).chain(extra))
}

/// Een veld van het gekozen gram.
fn field(hulp: &str, p: &str, soort: Soort) -> Value {
    let f = path(p);
    if soort == Soort::Getal {
        return grams(json!(f), gekozen(hulp, Some(niet_null(&f))), "MAX");
    }
    let cond = gekozen(
        hulp,
        Some(if matches!(soort, Soort::Tekst | Soort::Datum) {
            filled(&f)
        } else {
            niet_null(&f)
        }),
    );
    als_dan(
        grams(json!(true), cond.clone(), "OR"),
        grams(
            json!(f),
            cond,
            if soort == Soort::JaNee { "OR" } else { "ADD" },
        ),
    )
}

fn moment_pad(m: Moment) -> &'static str {
    match m {
        Moment::EffectiveAt => "effective_date",
        Moment::RecordedAt => "recorded_date",
    }
}

fn year_of(hulp: &str, p: &str) -> Value {
    let f = path(p);
    let cond = gekozen(hulp, Some(niet_null(&f)));
    als_dan(
        grams(json!(true), cond.clone(), "OR"),
        grams(
            json!({"operation": "DATE_PART", "date": f, "in": "year"}),
            cond,
            "ADD",
        ),
    )
}

/// Een grondslag als `legal_basis` van een actie.
fn legal_basis(legal_basis: &str, namen: &BTreeMap<String, String>) -> Option<Value> {
    let g = crate::regelingen::ontleed(legal_basis).ok()?;
    let mut o = Map::new();
    o.insert(
        "law".into(),
        json!(namen
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

/// De engine-regeling van een lexostatus als YAML-tekst (JSON is YAML), met
/// `$id` `id`. `namen` geeft per regeling-id de naam voor `legal_basis`. Een
/// afleiding die de engine-route niet kent (`periode_van`, `bevat`,
/// `verzamel`, een lijst) is een fout.
pub fn regulation(
    def: &LexostatusDefinitie,
    id: &str,
    namen: &BTreeMap<String, String>,
) -> Result<String, String> {
    let r = &def.reduction;
    let types = def.law.as_ref().map(|w| &w.types);
    let mut outputs: Vec<Value> = Vec::new();
    let mut actions: Vec<Value> = Vec::new();
    if r.pick.is_some() {
        outputs.push(json!({"name": LAATSTE, "type": "number", "nullable": true}));
        actions.push(json!({"output": LAATSTE, "value":
            grams(json!("$g.sequence"), en(filter_van(&r.filter)), "MAX")}));
    }
    for (name, a) in r.derivations.iter().chain(&r.extra_fields) {
        let soort = types
            .and_then(|t| t.get(name))
            .map(|t| Soort::uit_type(t))
            .unwrap_or(Soort::Tekst);
        let filter_eigen = || -> Filter {
            let mut f = r.filter.clone();
            f.extend(a.filter().cloned().unwrap_or_default());
            f
        };
        // Een afleiding met een eigen filter en `kies`: haar eigen gram.
        let mut hulp = LAATSTE.to_string();
        if matches!(
            &a.derivation,
            Afleiding::LaatsteVeld { .. }
                | Afleiding::LaatsteMoment { .. }
                | Afleiding::LaatsteJaarVan { .. }
        ) {
            hulp = hulp_van(name);
            outputs.push(json!({"name": hulp, "type": "number", "nullable": true}));
            actions.push(json!({"output": hulp, "value":
                grams(json!("$g.sequence"), en(filter_van(&filter_eigen())), "MAX")}));
        }
        let (value, nullable, soort) = match &a.derivation {
            Afleiding::Veld { field: v } | Afleiding::LaatsteVeld { field: v, .. } => {
                (field(&hulp, v, soort), true, soort)
            }
            Afleiding::Moment { moment } | Afleiding::LaatsteMoment { moment, .. } => (
                field(&hulp, moment_pad(*moment), Soort::Datum),
                true,
                Soort::Datum,
            ),
            Afleiding::JaarVan { year_of: p } | Afleiding::LaatsteJaarVan { year_of: p, .. } => {
                (year_of(&hulp, p), true, Soort::Getal)
            }
            Afleiding::Gevuld { filled: p } => (
                grams(json!(true), gekozen(&hulp, Some(filled(&path(p)))), "OR"),
                false,
                Soort::JaNee,
            ),
            Afleiding::Gelijk { equals } => {
                let f = path(&equals.field);
                let cond = gekozen(&hulp, Some(niet_null(&f)));
                (
                    als_dan(
                        grams(json!(true), cond.clone(), "OR"),
                        grams(eq(&f, equals.value.clone()), cond, "OR"),
                    ),
                    true,
                    Soort::JaNee,
                )
            }
            Afleiding::ElkeRegel {
                table,
                each_row,
                only_where,
            } => {
                let t = path(table);
                let kol = format!("$r.{each_row}");
                let rf = only_where
                    .as_ref()
                    .map(|w| eq(&format!("$r.{w}"), json!(true)));
                (
                    grams(
                        en([
                            foreach(&t, "r", json!(true), None, Some("OR")),
                            foreach(&t, "r", filled(&kol), rf, Some("AND")),
                        ]),
                        gekozen(&hulp, None),
                        "OR",
                    ),
                    false,
                    Soort::JaNee,
                )
            }
            Afleiding::EenRegel { table, one_row } => (
                grams(
                    foreach(
                        &path(table),
                        "r",
                        eq(&format!("$r.{one_row}"), json!(true)),
                        None,
                        Some("OR"),
                    ),
                    gekozen(&hulp, None),
                    "OR",
                ),
                false,
                Soort::JaNee,
            ),
            Afleiding::Bestaat {
                exists: true,
                filled: g,
                ..
            } => {
                let mut c = filter_van(&filter_eigen());
                if let Some(g) = g {
                    c.push(filled(&path(g)));
                }
                (grams(json!(true), en(c), "OR"), false, Soort::JaNee)
            }
            Afleiding::Som { sum, .. } => {
                let f = path(sum);
                let mut c = filter_van(&filter_eigen());
                c.push(niet_null(&f));
                (grams(json!(f), en(c), "ADD"), false, Soort::Getal)
            }
            ander => {
                return Err(format!(
                    "afleiding '{name}' ({}) heeft geen engine-patroon",
                    serde_json::to_string(ander).unwrap_or_default()
                ))
            }
        };
        let mut o = json!({"name": name, "type": soort.engine_type()});
        if nullable {
            o["nullable"] = json!(true);
        }
        outputs.push(o);
        let mut act = Map::new();
        act.insert("output".into(), json!(name));
        if let Some(lb) = a.legal_basis.first().and_then(|g| legal_basis(g, namen)) {
            act.insert("legal_basis".into(), lb);
        }
        act.insert("value".into(), value);
        actions.push(Value::Object(act));
    }
    let parameters: Vec<Value> = def
        .inputs
        .iter()
        .map(|i| json!({"name": i.name, "type": i.soort, "required": true}))
        .collect();
    let tekst = match &def.law {
        Some(w) => format!("Hoe {} zijn parameters uit de kroniek leest (produces.extensions.chronolex.leest), vertaald naar de engine.", w.article),
        None => format!("De lexostatus {} van de cel, vertaald naar de engine.", def.name),
    };
    let doc = json!({
        "$id": id,
        "name": format!("Lexostatus {}", def.name),
        "regulatory_layer": "UITVOERINGSBELEID",
        "publication_date": "2025-01-01",
        "valid_from": "2025-01-01",
        "url": format!("urn:regelrecht:cel:lexostatus:{}", def.name),
        "articles": [{
            "number": "1",
            "text": tekst,
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
