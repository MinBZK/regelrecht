//! Experiment A: een lexostatus als gewone engine-run in plaats van de
//! reductie-DSL ([`crate::reductie`]).
//!
//! De lexostatus is dan een artikel (een engine-regeling) met de kroniek als
//! parameter: een lijst grammen. Deze module doet alleen wat de engine niet
//! kan: elk gram een `volgorde` geven, zijn plaats in de tijd (de engine kent
//! geen tijdstip met tijdzone), zodat "het laatste gram" het gram met de
//! hoogste volgorde is. De rest staat in het artikel. Niet in gebruik door de runtime; zie het verslag in de
//! superpowers-specs.

use std::collections::BTreeMap;

use regelrecht_engine::LawExecutionService;
use serde_json::{json, Map, Value};

use crate::gram::Gram;
use crate::toets;

/// De grammen van `kroniek` als parameter voor de engine, in de volgorde van
/// de kroniek, elk met `volgorde`, `name`, `type`, `op_moment`,
/// `zaakkenmerk` (als het er is) en `fields`. `volgorde` is de plaats in de
/// tijd, zoals `kies: laatste` die leest ([`Gram::tijdvolgorde`]: eerst
/// `op_moment`, dan `vastgelegd_op`, en bij gelijke tijden de volgorde van de
/// kroniek). Zo is "het laatste gram" het gram met de hoogste volgorde, en
/// blijft een verzameling (`verzamel`) in de volgorde van de kroniek.
pub fn als_kroniek(grammen: &[Gram], kroniek: &str) -> Result<Value, String> {
    let door: Vec<&Gram> = grammen.iter().filter(|g| g.chronicle == kroniek).collect();
    for g in &door {
        // Een ongeldig moment is een fout, geen plaats achteraan.
        g.moment()?;
        g.vastgelegd()?;
    }
    let mut in_de_tijd: Vec<usize> = (0..door.len()).collect();
    // Stabiel: bij gelijke tijden blijft de volgorde van de kroniek.
    in_de_tijd.sort_by(|&a, &b| {
        door[a]
            .tijdvolgorde(door[b])
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut volgorde = vec![0; door.len()];
    for (plaats, i) in in_de_tijd.into_iter().enumerate() {
        volgorde[i] = plaats;
    }
    Ok(Value::Array(
        door.into_iter()
            .zip(volgorde)
            .map(|(g, v)| {
                let mut o = Map::new();
                o.insert("volgorde".into(), json!(v));
                o.insert("name".into(), json!(g.name));
                o.insert("type".into(), json!(g.type_));
                o.insert("op_moment".into(), json!(g.op_moment));
                if let Some(z) = &g.zaakkenmerk {
                    o.insert("zaakkenmerk".into(), json!(z));
                }
                o.insert("fields".into(), Value::Object(g.fields.clone()));
                Value::Object(o)
            })
            .collect(),
    ))
}

/// Reduceer via de engine: evalueer `uitkomsten` van `regeling` met de
/// inputs en de kroniek als parameter `grammen`. Een uitkomst null blijft
/// weg, zoals een parameter waarover de kroniek niets zegt in de reductie.
pub fn reduceer(
    service: &LawExecutionService,
    regeling: &str,
    uitkomsten: &[&str],
    inputs: &Map<String, Value>,
    grammen: &[Gram],
    kroniek: &str,
    datum: &str,
) -> Result<BTreeMap<String, Value>, String> {
    let mut parameters: BTreeMap<String, Value> =
        inputs.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
    parameters.insert("grammen".into(), als_kroniek(grammen, kroniek)?);
    let e = toets::evalueer(service, regeling, uitkomsten, &parameters, datum);
    if let Some(f) = e.fout {
        return Err(f);
    }
    if !e.mist.is_empty() {
        return Err(format!("de engine mist {:?}", e.mist));
    }
    Ok(e.waarden
        .into_iter()
        .filter(|(_, v)| !v.is_null())
        .collect())
}

/// Een kroniek als databron voor een lexostatus-regeling: de input `grammen`
/// (`source: {}`) van die ene regeling krijgt de grammen van de kroniek. Zo
/// werkt `source` (RFC-022 §4.2) tussen lexostatussen zonder dat de afnemer
/// de grammen ziet; welke regeling bij welke cel-kroniek hoort, is
/// deploymentconfiguratie. In dit experiment een momentopname; in een
/// runtime zou de bron de kroniek live lezen.
pub struct KroniekBron {
    naam: String,
    regeling: String,
    grammen: regelrecht_engine::Value,
}

impl KroniekBron {
    pub fn new(regeling: &str, grammen: &[Gram], kroniek: &str) -> Result<Self, String> {
        Ok(Self {
            naam: format!("kroniek:{kroniek}"),
            regeling: regeling.to_string(),
            grammen: regelrecht_engine::Value::from(&als_kroniek(grammen, kroniek)?),
        })
    }
}

impl regelrecht_engine::DataSource for KroniekBron {
    fn name(&self) -> &str {
        &self.naam
    }
    fn priority(&self) -> i32 {
        10
    }
    fn source_type(&self) -> &str {
        "kroniek"
    }
    fn has_field(&self, field: &str) -> bool {
        field == "grammen"
    }
    fn get(
        &self,
        field: &str,
        _criteria: &BTreeMap<String, regelrecht_engine::Value>,
    ) -> Option<regelrecht_engine::Value> {
        (field == "grammen").then(|| self.grammen.clone())
    }
    fn fields(&self) -> Vec<&str> {
        vec!["grammen"]
    }
    fn law_scope(&self) -> Option<&str> {
        Some(&self.regeling)
    }
    fn key_fields(&self) -> Option<&[String]> {
        Some(&[])
    }
}
