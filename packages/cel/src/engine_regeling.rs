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
fn gevuld(subject: &str) -> Value {
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

fn grammen(body: Value, filter: Value, combine: &str) -> Value {
    foreach("$grammen", "g", body, Some(filter), Some(combine))
}

fn als_dan(wanneer: Value, dan: Value) -> Value {
    json!({"operation": "IF", "cases": [{"when": wanneer, "then": dan}]})
}

/// Het pad van een sleutel of veld van het gram `g` in de engine.
fn pad(p: &str) -> String {
    if is_gram_sleutel(p) || matches!(p, "op_datum" | "vastgelegd_datum" | "volgorde") {
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
        .map(|k| eq(&pad(k), Value::String(f[k].clone())))
        .collect()
}

fn gekozen(hulp: &str, extra: Option<Value>) -> Value {
    en(std::iter::once(eq("$g.volgorde", json!(format!("${hulp}")))).chain(extra))
}

/// Een veld van het gekozen gram.
fn veld(hulp: &str, p: &str, soort: Soort) -> Value {
    let f = pad(p);
    if soort == Soort::Getal {
        return grammen(json!(f), gekozen(hulp, Some(niet_null(&f))), "MAX");
    }
    let cond = gekozen(
        hulp,
        Some(if matches!(soort, Soort::Tekst | Soort::Datum) {
            gevuld(&f)
        } else {
            niet_null(&f)
        }),
    );
    als_dan(
        grammen(json!(true), cond.clone(), "OR"),
        grammen(
            json!(f),
            cond,
            if soort == Soort::JaNee { "OR" } else { "ADD" },
        ),
    )
}

fn moment_pad(m: Moment) -> &'static str {
    match m {
        Moment::OpMoment => "op_datum",
        Moment::VastgelegdOp => "vastgelegd_datum",
    }
}

fn jaar_van(hulp: &str, p: &str) -> Value {
    let f = pad(p);
    let cond = gekozen(hulp, Some(niet_null(&f)));
    als_dan(
        grammen(json!(true), cond.clone(), "OR"),
        grammen(
            json!({"operation": "DATE_PART", "date": f, "in": "year"}),
            cond,
            "ADD",
        ),
    )
}

/// Een grondslag als `legal_basis` van een actie.
fn legal_basis(grondslag: &str, namen: &BTreeMap<String, String>) -> Option<Value> {
    let g = crate::regelingen::ontleed(grondslag).ok()?;
    let mut o = Map::new();
    o.insert(
        "law".into(),
        json!(namen
            .get(g.regeling)
            .cloned()
            .unwrap_or_else(|| g.regeling.to_string())),
    );
    o.insert("article".into(), json!(g.artikel));
    if let Some(l) = g.lid {
        o.insert("paragraph".into(), json!(l));
    }
    Some(Value::Object(o))
}

/// De engine-regeling van een lexostatus als YAML-tekst (JSON is YAML), met
/// `$id` `id`. `namen` geeft per regeling-id de naam voor `legal_basis`. Een
/// afleiding die de engine-route niet kent (`periode_van`, `bevat`,
/// `verzamel`, een lijst) is een fout.
pub fn regeling(
    def: &LexostatusDefinitie,
    id: &str,
    namen: &BTreeMap<String, String>,
) -> Result<String, String> {
    let r = &def.reduction;
    let typen = def.wet.as_ref().map(|w| &w.typen);
    let mut outputs: Vec<Value> = Vec::new();
    let mut actions: Vec<Value> = Vec::new();
    if r.kies.is_some() {
        outputs.push(json!({"name": LAATSTE, "type": "number", "nullable": true}));
        actions.push(json!({"output": LAATSTE, "value":
            grammen(json!("$g.volgorde"), en(filter_van(&r.filter)), "MAX")}));
    }
    for (naam, a) in r.afleidingen.iter().chain(&r.extra_velden) {
        let soort = typen
            .and_then(|t| t.get(naam))
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
            &a.afleiding,
            Afleiding::LaatsteVeld { .. }
                | Afleiding::LaatsteMoment { .. }
                | Afleiding::LaatsteJaarVan { .. }
        ) {
            hulp = hulp_van(naam);
            outputs.push(json!({"name": hulp, "type": "number", "nullable": true}));
            actions.push(json!({"output": hulp, "value":
                grammen(json!("$g.volgorde"), en(filter_van(&filter_eigen())), "MAX")}));
        }
        let (waarde, nullable, soort) = match &a.afleiding {
            Afleiding::Veld { veld: v } | Afleiding::LaatsteVeld { veld: v, .. } => {
                (veld(&hulp, v, soort), true, soort)
            }
            Afleiding::Moment { moment } | Afleiding::LaatsteMoment { moment, .. } => (
                veld(&hulp, moment_pad(*moment), Soort::Datum),
                true,
                Soort::Datum,
            ),
            Afleiding::JaarVan { jaar_van: p } | Afleiding::LaatsteJaarVan { jaar_van: p, .. } => {
                (jaar_van(&hulp, p), true, Soort::Getal)
            }
            Afleiding::Gevuld { gevuld: p } => (
                grammen(json!(true), gekozen(&hulp, Some(gevuld(&pad(p)))), "OR"),
                false,
                Soort::JaNee,
            ),
            Afleiding::Gelijk { gelijk } => {
                let f = pad(&gelijk.veld);
                let cond = gekozen(&hulp, Some(niet_null(&f)));
                (
                    als_dan(
                        grammen(json!(true), cond.clone(), "OR"),
                        grammen(eq(&f, gelijk.aan.clone()), cond, "OR"),
                    ),
                    true,
                    Soort::JaNee,
                )
            }
            Afleiding::ElkeRegel {
                tabel,
                elke_regel,
                alleen_waar,
            } => {
                let t = pad(tabel);
                let kol = format!("$r.{elke_regel}");
                let rf = alleen_waar
                    .as_ref()
                    .map(|w| eq(&format!("$r.{w}"), json!(true)));
                (
                    grammen(
                        en([
                            foreach(&t, "r", json!(true), None, Some("OR")),
                            foreach(&t, "r", gevuld(&kol), rf, Some("AND")),
                        ]),
                        gekozen(&hulp, None),
                        "OR",
                    ),
                    false,
                    Soort::JaNee,
                )
            }
            Afleiding::EenRegel { tabel, een_regel } => (
                grammen(
                    foreach(
                        &pad(tabel),
                        "r",
                        eq(&format!("$r.{een_regel}"), json!(true)),
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
                bestaat: true,
                gevuld: g,
                ..
            } => {
                let mut c = filter_van(&filter_eigen());
                if let Some(g) = g {
                    c.push(gevuld(&pad(g)));
                }
                (grammen(json!(true), en(c), "OR"), false, Soort::JaNee)
            }
            Afleiding::Som { som, .. } => {
                let f = pad(som);
                let mut c = filter_van(&filter_eigen());
                c.push(niet_null(&f));
                (grammen(json!(f), en(c), "ADD"), false, Soort::Getal)
            }
            ander => {
                return Err(format!(
                    "afleiding '{naam}' ({}) heeft geen engine-patroon",
                    serde_json::to_string(ander).unwrap_or_default()
                ))
            }
        };
        let mut o = json!({"name": naam, "type": soort.engine_type()});
        if nullable {
            o["nullable"] = json!(true);
        }
        outputs.push(o);
        let mut act = Map::new();
        act.insert("output".into(), json!(naam));
        if let Some(lb) = a.grondslag.first().and_then(|g| legal_basis(g, namen)) {
            act.insert("legal_basis".into(), lb);
        }
        act.insert("value".into(), waarde);
        actions.push(Value::Object(act));
    }
    let parameters: Vec<Value> = def
        .inputs
        .iter()
        .map(|i| json!({"name": i.name, "type": i.soort, "required": true}))
        .collect();
    let tekst = match &def.wet {
        Some(w) => format!("Hoe {} zijn parameters uit de kroniek leest (produces.extensions.chronolex.leest), vertaald naar de engine.", w.artikel),
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
                "input": [{"name": "grammen", "type": "array", "source": {}}],
                "output": outputs,
                "actions": actions,
            }},
        }],
    });
    serde_json::to_string_pretty(&doc).map_err(|e| e.to_string())
}
