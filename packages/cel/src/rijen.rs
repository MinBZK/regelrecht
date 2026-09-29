//! Synthese per regel: een tabelveld van een lexostatus van de zaak wordt
//! een array-parameter, met per regel kolommen uit andere cellen.
//!
//! Een artikel kan een tabel als parameter vragen waarvan de indiener maar
//! een deel invult; de rest stelt de instantie zelf vast, per regel, uit
//! registers van andere cellen. Het proces bouwt die tabel niet in code op:
//! in `proces.yaml` staat welk tabelveld de regels levert, welke kolom onder welke
//! naam meegaat, en welke bron per regel met welke invoer wordt bevraagd.
//!
//! De regels worden tegelijk bevraagd, in de volgorde van het tabelveld
//! teruggegeven. Per regel gaat het proces langs de bronnen, in volgorde. De invoer van een
//! bron komt uit de regel zelf (`kolom`), uit een lexostatus van de zaak
//! (`lexostatus` en `veld`) of uit de samengevoegde parameters
//! (`parameter`); een bron die eerder aan de beurt was kan dus een kolom
//! leveren die een latere bron als invoer gebruikt. Een invoer kan ook een
//! uitkomst van een regeling zijn (`regeling` en `uitkomst`): de wet leidt
//! haar af uit de samengevoegde parameters, zoals een peildatum uit een
//! jaartal, in een eigen run vóór de regels; of een vaste waarde (`waarde`).
//! Ontbreekt een invoer, is
//! een bron onbereikbaar, of levert ze de waarde niet, dan blijft die kolom
//! weg: er wordt niets aangevuld. Welke kolommen dat waren, staat in `mist`.
//!
//! Het blok staat onder het besluit (`behandeling.besluit.rijen`) of onder de
//! toets (`portaal.toets.rijen`); beide voeren het uit met [`pas_toe`], vóór
//! de engine.

use std::collections::{BTreeMap, BTreeSet};

use futures_util::stream::{self, StreamExt};

use serde::Serialize;
use serde_json::{Map, Value};

use regelrecht_engine::LawExecutionService;

use crate::cel::Cell;
use crate::config::{ProcesDefinitie, RijBron, RijInvoer, RijenDefinitie};
use crate::reductie::{Lexostatus, Peil};
use crate::synthese::{Herkomst, Samenvoeging, Status};
use crate::transport::TransportFout;

/// Een bron die per regel wordt bevraagd, met het transport dat de runtime
/// ervoor koos.
pub type Bron = crate::synthese::Bron<RijBron>;

/// Een rijen-definitie met haar bronnen.
#[derive(Clone)]
pub struct Rijen {
    pub definitie: RijenDefinitie,
    pub sources: Vec<Bron>,
}

/// Hoe het bevragen van een bron over alle regels verliep.
#[derive(Debug, Clone, Serialize)]
pub struct BronUitslag {
    pub cell: String,
    pub lexostatus: String,
    pub transport: &'static str,
    /// Het aantal regels waarvoor de bron is bevraagd.
    pub queried: usize,
    /// De ongunstigste status over de regels.
    pub status: Status,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Langs welke route(s) de bron reduceerde, als zij dat zegt (een
    /// runtime met de engine-route, experiment A): `engine`, `dsl`, of beide
    /// met een komma.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reduction: Option<String>,
}

/// Wat een rijen-definitie opleverde.
#[derive(Debug, Clone, Serialize)]
pub struct Uitslag {
    pub parameter: String,
    /// Waarom de tabel niet is opgebouwd: het tabelveld is geen lijst van
    /// regels. Dan gaat de parameter niet naar de engine; een regel
    /// weglaten zou de uitkomst stil veranderen.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// De regels, elk met de kolommen die een bron leverde.
    pub rows: Vec<Value>,
    pub sources: Vec<BronUitslag>,
    /// Kolommen die niet in elke regel staan, zonder dubbelen.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub missing: Vec<String>,
    /// De invoer die de wet afleidde, per `<regeling>#<uitkomst>`, met de
    /// waarde of waarom er geen was.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub from_law: BTreeMap<String, Result<Value, String>>,
}

/// Waartegen de rijen worden opgebouwd: het corpus en de datum waarop de
/// engine de regeling leest voor een invoer uit de wet (die van de toets of
/// het besluit), en het peil waarop elke bron haar kroniek reduceert (zie
/// [`Peil`]).
#[derive(Clone, Copy)]
pub struct Omgeving<'a> {
    pub service: &'a LawExecutionService,
    pub date: &'a str,
    pub peil: &'a Peil,
}

/// De sleutel van een invoer uit de wet.
fn wetsleutel(regulation: &str, output: &str) -> String {
    format!("{regulation}#{output}")
}

/// Reken elke invoer uit de wet van deze rijen-definitie uit, een keer, met
/// de samengevoegde parameters. Er wordt niets aangevuld: mist de uitkomst
/// een feit, dan zegt de fout welk.
fn from_law(
    rows: &Rijen,
    parameters: &BTreeMap<String, Value>,
    law: Omgeving<'_>,
) -> BTreeMap<String, Result<Value, String>> {
    let mut uit = BTreeMap::new();
    for b in rows.sources.iter().map(|b| &b.definitie) {
        for i in b.input.values() {
            let RijInvoer::Wet { regulation, output } = i else {
                continue;
            };
            let sleutel = wetsleutel(regulation, output);
            if uit.contains_key(&sleutel) {
                continue;
            }
            let e =
                crate::toets::evalueer(law.service, regulation, &[output], parameters, law.date);
            let value = match e.waarden.get(output.as_str()) {
                Some(w) if !w.is_null() => Ok(w.clone()),
                _ => Err(e.reason(&format!("{sleutel} heeft geen waarde"))),
            };
            uit.insert(sleutel, value);
        }
    }
    uit
}

/// De waarde die een lexostatus onder een naam levert: een parameter of een
/// extra veld.
fn uit_lexostatus<'l>(
    lexostatuses: &'l [Lexostatus],
    name: &str,
    field: &str,
) -> Option<&'l Value> {
    lexostatuses.iter().find(|l| l.name == name)?.field(field)
}

/// De invoer voor een bron bij een regel. Een fout noemt wat ontbreekt.
fn input(
    source: &RijBron,
    regel: &Map<String, Value>,
    lexostatuses: &[Lexostatus],
    parameters: &BTreeMap<String, Value>,
    law: &BTreeMap<String, Result<Value, String>>,
) -> Result<Map<String, Value>, String> {
    let mut uit = Map::new();
    for (name, verwijzing) in &source.input {
        let (value, wat) = match verwijzing {
            RijInvoer::Kolom { column } => (
                regel.get(column).filter(|w| !w.is_null()),
                format!("kolom '{column}'"),
            ),
            RijInvoer::Own { lexostatus, field } => (
                uit_lexostatus(lexostatuses, lexostatus, field),
                format!("lexostatus '{lexostatus}', veld '{field}'"),
            ),
            RijInvoer::Parameter { parameter } => (
                parameters.get(parameter).filter(|w| !w.is_null()),
                format!("parameter '{parameter}'"),
            ),
            RijInvoer::Wet { regulation, output } => {
                match law.get(&wetsleutel(regulation, output)) {
                    Some(Ok(w)) => (Some(w), String::new()),
                    Some(Err(f)) => return Err(format!("invoer '{name}' ontbreekt: {f}")),
                    None => (None, format!("uitkomst '{output}' van {regulation}")),
                }
            }
            RijInvoer::Waarde { value } => (Some(value), String::new()),
        };
        let Some(value) = value else {
            return Err(format!(
                "invoer '{name}' ontbreekt: {wat} heeft geen waarde"
            ));
        };
        uit.insert(name.clone(), value.clone());
    }
    Ok(uit)
}

/// De ongunstigste van twee statussen: bevraagd is het best, daarna fout,
/// niet bevraagd en onbereikbaar.
fn slechtste(a: Status, b: Status) -> Status {
    fn rang(s: Status) -> u8 {
        match s {
            Status::Queried => 0,
            Status::Error => 1,
            Status::NotQueried => 2,
            Status::Unreachable => 3,
        }
    }
    if rang(b) > rang(a) {
        b
    } else {
        a
    }
}

/// Hoeveel regels tegelijk hun bronnen bevragen.
const GELIJKTIJDIG: usize = 16;

/// Hoe het bevragen van een bron bij een regel verliep.
enum Bevraging {
    /// Met de route van de reductie, als de bron die noemde.
    Queried(Option<String>),
    /// Niet gevraagd: een invoer ontbrak.
    NotQueried(String),
    /// Gevraagd, zonder lexostatus.
    Mislukt(Status, String),
}

/// Wat een regel opleverde: de regel, de kolommen die ontbreken, en per bron
/// (in de volgorde van de definitie) hoe het bevragen verliep.
struct Regeluitslag {
    regel: Map<String, Value>,
    missing: BTreeSet<String>,
    sources: Vec<Bevraging>,
}

/// Stel een regel samen: de kolommen uit de tabel, en daarna die van elke
/// bron, na elkaar.
async fn stel_regel_samen(
    rows: &Rijen,
    source: &Map<String, Value>,
    lexostatuses: &[Lexostatus],
    parameters: &BTreeMap<String, Value>,
    law: &BTreeMap<String, Result<Value, String>>,
    peil: &Peil,
) -> Regeluitslag {
    let mut uit = Regeluitslag {
        regel: Map::new(),
        missing: BTreeSet::new(),
        sources: Vec::new(),
    };
    for (column, name) in &rows.definitie.columns {
        if let Some(w) = source.get(column) {
            uit.regel.insert(name.clone(), w.clone());
        } else {
            uit.missing.insert(name.clone());
        }
    }
    for b in &rows.sources {
        let input = match input(&b.definitie, &uit.regel, lexostatuses, parameters, law) {
            Ok(i) => i,
            Err(f) => {
                uit.sources.push(Bevraging::NotQueried(f));
                uit.missing.extend(b.definitie.columns.values().cloned());
                continue;
            }
        };
        let geleverd = match b.vraag(&input, peil).await {
            Ok(l) => {
                uit.sources
                    .push(Bevraging::Queried(l.reduction.map(|r| r.route)));
                let mut combined = l.parameters;
                combined.extend(l.extra_fields);
                combined
            }
            Err(f) => {
                let status = match f {
                    TransportFout::Unreachable(_) => Status::Unreachable,
                    TransportFout::Antwoord { .. } | TransportFout::Json(_) => Status::Error,
                };
                uit.sources.push(Bevraging::Mislukt(status, f.to_string()));
                BTreeMap::new()
            }
        };
        for (van, to) in &b.definitie.columns {
            match geleverd.get(van).filter(|w| !w.is_null()) {
                Some(w) => {
                    uit.regel.insert(to.clone(), w.clone());
                }
                None => {
                    uit.missing.insert(to.clone());
                }
            }
        }
    }
    uit
}

/// Bouw de array-parameter op. Voor elke regel van het tabelveld gaan de
/// kolommen uit de configuratie mee, en daarna de kolommen die de bronnen
/// per regel leveren. Ontbreekt er iets, dan blijft die kolom weg.
pub async fn stel_samen(
    rows: &Rijen,
    lexostatuses: &[Lexostatus],
    parameters: &BTreeMap<String, Value>,
    law: Omgeving<'_>,
) -> Option<Uitslag> {
    let d = &rows.definitie;
    let waar = format!(
        "lexostatus '{}', veld '{}'",
        d.table.lexostatus, d.table.field
    );
    let niet_op_te_bouwen = |error: String| Uitslag {
        parameter: d.parameter.clone(),
        error: Some(error),
        rows: Vec::new(),
        sources: Vec::new(),
        missing: Vec::new(),
        from_law: BTreeMap::new(),
    };
    let table = uit_lexostatus(lexostatuses, &d.table.lexostatus, &d.table.field)?;
    let Some(table) = table.as_array() else {
        return Some(niet_op_te_bouwen(format!(
            "{waar} is geen lijst van regels"
        )));
    };
    let mut rijregels = Vec::new();
    for (i, r) in table.iter().enumerate() {
        match r.as_object() {
            Some(r) => rijregels.push(r),
            None => {
                return Some(niet_op_te_bouwen(format!(
                    "{waar}: regel {i} is geen object met kolommen"
                )))
            }
        }
    }

    let mut uitslagen: Vec<BronUitslag> = rows
        .sources
        .iter()
        .map(|b| BronUitslag {
            cell: b.definitie.cell.clone(),
            lexostatus: b.definitie.lexostatus.clone(),
            transport: b.transport.soort(),
            queried: 0,
            status: Status::Queried,
            error: None,
            reduction: None,
        })
        .collect();
    let mut out_rows = Vec::new();
    let mut missing: BTreeSet<String> = BTreeSet::new();

    // De regels tegelijk (hooguit GELIJKTIJDIG), in de volgorde van de
    // tabel; binnen een regel de bronnen na elkaar, want een bron kan een
    // kolom van een eerdere als invoer nemen.
    // Eerst de futures in een lijst: een stream over een closure met
    // verwijzingen maakt de future van een route niet Send.
    let afgeleid = from_law(rows, parameters, law);
    let mut vragen = Vec::with_capacity(rijregels.len());
    for r in rijregels {
        vragen.push(stel_regel_samen(
            rows,
            r,
            lexostatuses,
            parameters,
            &afgeleid,
            law.peil,
        ));
    }
    let per_regel: Vec<Regeluitslag> = stream::iter(vragen).buffered(GELIJKTIJDIG).collect().await;
    for r in per_regel {
        for (result, bevraging) in uitslagen.iter_mut().zip(r.sources) {
            match bevraging {
                Bevraging::Queried(route) => {
                    result.queried += 1;
                    if let Some(route) = route {
                        let r = result.reduction.get_or_insert_with(String::new);
                        if !r.split(", ").any(|x| x == route) {
                            if !r.is_empty() {
                                r.push_str(", ");
                            }
                            r.push_str(&route);
                        }
                    }
                }
                Bevraging::NotQueried(f) => {
                    result.status = slechtste(result.status, Status::NotQueried);
                    result.error.get_or_insert(f);
                }
                Bevraging::Mislukt(status, f) => {
                    result.queried += 1;
                    result.status = slechtste(result.status, status);
                    result.error.get_or_insert(f);
                }
            }
        }
        missing.extend(r.missing);
        out_rows.push(Value::Object(r.regel));
    }
    Some(Uitslag {
        parameter: d.parameter.clone(),
        error: None,
        rows: out_rows,
        sources: uitslagen,
        missing: missing.into_iter().collect(),
        from_law: afgeleid,
    })
}

/// Voer de rijen-definities uit op een samenvoeging, vóór de engine: elke
/// array-parameter komt erbij, met herkomst per regel. `eigen` zijn de
/// lexostatussen van de zaak (bij de toets: de proefreductie van het
/// concept); een tabel kan ook een extra veld zijn dat een synthese-bron
/// doorgaf, en dat staat dan naast de eigen lexostatussen onder de naam van
/// die bron. Gedeeld door de toets en het besluit.
pub async fn pas_toe(
    rows: &[Rijen],
    eigen: &[Lexostatus],
    combined: &mut Samenvoeging,
    law: Omgeving<'_>,
) -> Vec<Uitslag> {
    let mut met_bronnen = eigen.to_vec();
    for u in combined
        .sources
        .iter()
        .filter(|u| !u.extra_fields.is_empty())
    {
        met_bronnen.push(Lexostatus {
            extra_fields: u
                .extra_fields
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
            ..Lexostatus::leeg(&u.lexostatus)
        });
    }
    let mut uitslagen = Vec::new();
    for r in rows {
        let Some(result) = stel_samen(r, &met_bronnen, &combined.parameters, law).await else {
            continue;
        };
        if result.error.is_some() {
            uitslagen.push(result);
            continue;
        }
        combined
            .parameters
            .insert(result.parameter.clone(), Value::Array(result.rows.clone()));
        combined.provenance.insert(
            result.parameter.clone(),
            Herkomst::PerRow {
                lexostatus: r.definitie.table.lexostatus.clone(),
                field: r.definitie.table.field.clone(),
            },
        );
        uitslagen.push(result);
    }
    uitslagen
}

/// Controleer een rijen-definitie bij het laden: de tabel komt uit een van de
/// `eigen` lexostatussen (die haar levert) of uit een extra veld dat een
/// synthese-bron doorgeeft, elke kolom komt uit maar een plek, een bron is
/// een andere cel, en elke invoer van een bron wordt gevuld. `wie` is de
/// uitvoering (toets of besluit), `anders` zegt wat een andere lexostatus
/// dan niet is.
pub fn controleer(
    wie: &str,
    r: &RijenDefinitie,
    eigen: &[&str],
    anders: &str,
    d: &ProcesDefinitie,
    cell: &Cell,
) -> Vec<String> {
    let mut fouten = Vec::new();
    let wie = format!("{wie}, rijen '{}'", r.parameter);
    // Een tabel of invoer mag ook uit een extra veld komen dat een
    // synthese-bron doorgeeft (bijvoorbeeld de regels van een uitslag uit een
    // register).
    let doorgegeven_veld = |lexostatus: &str, field: &str| {
        d.andere_bronnen()
            .any(|s| s.lexostatus == lexostatus && s.extra_fields.iter().any(|e| e == field))
    };
    if doorgegeven_veld(&r.table.lexostatus, &r.table.field) {
        // Uit een bron: de synthese controleert de bron zelf.
    } else if !eigen.contains(&r.table.lexostatus.as_str()) {
        fouten.push(format!(
            "{wie}: de tabel komt uit lexostatus '{}', en die is {anders}",
            r.table.lexostatus
        ));
    } else if !cell
        .lexostatuses
        .lexostatus(&r.table.lexostatus)
        .is_some_and(|l| l.levert(&r.table.field))
    {
        fouten.push(format!(
            "{wie}: lexostatus '{}' levert geen '{}' (geen afleiding en geen extra veld)",
            r.table.lexostatus, r.table.field
        ));
    }
    // Elke kolomnaam komt uit maar een plek: de tabel of een bron.
    let mut columns: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    for name in r.columns.values() {
        columns.entry(name).or_default().push("de tabel".into());
    }
    for source in &r.sources {
        let bronwie = format!("{wie}, bron {}/{}", source.cell, source.lexostatus);
        if source.cell == cell.id() {
            fouten.push(format!(
                "{bronwie}: een bron is een andere cel, niet de cel zelf"
            ));
        }
        for name in source.columns.values() {
            columns
                .entry(name)
                .or_default()
                .push(format!("bron {}/{}", source.cell, source.lexostatus));
        }
        for (i, v) in &source.input {
            match v {
                RijInvoer::Kolom { column } if !columns.contains_key(column.as_str()) => {
                    fouten.push(format!(
                        "{bronwie}, invoer '{i}': kolom '{column}' wordt door niets ervoor gevuld"
                    ));
                }
                RijInvoer::Wet { regulation, output }
                    if cell
                        .service
                        .resolver()
                        .get_article_by_output(regulation, output, None)
                        .is_none() =>
                {
                    fouten.push(format!(
                        "{bronwie}, invoer '{i}': regeling '{regulation}' heeft geen uitkomst '{output}'"
                    ));
                }
                RijInvoer::Own { lexostatus, field }
                    if !doorgegeven_veld(lexostatus, field)
                        && !cell
                            .lexostatuses
                            .lexostatus(lexostatus)
                            .is_some_and(|l| l.levert(field)) =>
                {
                    fouten.push(format!(
                        "{bronwie}, invoer '{i}': lexostatus '{lexostatus}' levert geen '{field}'"
                    ));
                }
                _ => {}
            }
        }
    }
    for (name, waar) in &columns {
        if waar.len() > 1 {
            fouten.push(format!(
                "{wie}: kolom '{name}' komt uit meer dan een plek: {}",
                waar.join(", ")
            ));
        }
    }
    fouten
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::transport::proef::Vast;
    use serde_json::json;
    use std::sync::Arc;

    /// Een fictieve regeling die een peildatum afleidt uit een jaartal.
    const PEIL: &str = r#"
$id: testregeling_peil
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Het tarief geldt op 1 januari van het jaar.
    machine_readable:
      execution:
        parameters:
          - {name: jaar, type: number, required: true}
        output:
          - {name: peildatum, type: date}
        actions:
          - output: peildatum
            value: {operation: DATE, year: $jaar, month: 1, day: 1}
"#;

    const GEEN_PEIL: Peil = Peil {
        as_of: None,
        known_at: None,
    };

    fn service() -> LawExecutionService {
        let mut s = LawExecutionService::new();
        s.load_law(PEIL).unwrap();
        s
    }

    fn law(service: &LawExecutionService) -> Omgeving<'_> {
        Omgeving {
            service,
            date: "2026-05-01",
            peil: &GEEN_PEIL,
        }
    }

    fn eigen() -> Vec<Lexostatus> {
        vec![serde_json::from_value(json!({
            "name": "aanvraag",
            "parameters": {},
            "extra_fields": {
                "aanduiding": "EEN LIJST",
                "organen": [
                    {"orgaan": "raad", "gebied": "A", "zetels": 10, "aantal": null},
                    {"orgaan": "raad", "gebied": "B", "zetels": 3, "aantal": 2},
                ],
            },
        }))
        .unwrap()]
    }

    fn definitie() -> RijenDefinitie {
        serde_json::from_value(json!({
            "parameter": "tabel",
            "table": {"lexostatus": "aanvraag", "field": "organen"},
            "columns": {"orgaan": "orgaan", "gebied": "gebiedscode", "zetels": "zetels", "aantal": "samenstellende"},
        }))
        .unwrap()
    }

    fn source(antwoord: Result<Value, TransportFout>, input: Value) -> (Bron, Arc<Vast>) {
        let t = Arc::new(Vast::new(antwoord));
        let definitie: RijBron = serde_json::from_value(json!({
            "cell": "register",
            "lexostatus": "per_gebied",
            "input": input,
            "columns": {"bedrag": "tarief"},
        }))
        .unwrap();
        (
            Bron {
                definitie,
                transport: t.clone(),
            },
            t,
        )
    }

    #[tokio::test]
    async fn elke_regel_krijgt_haar_kolommen() {
        let (b, t) = source(
            Ok(json!({"name": "per_gebied", "parameters": {}, "extra_fields": {"bedrag": 5}})),
            json!({"gebied": {"column": "gebiedscode"}}),
        );
        let rows = Rijen {
            definitie: definitie(),
            sources: vec![b],
        };
        let u = stel_samen(&rows, &eigen(), &BTreeMap::new(), law(&service()))
            .await
            .unwrap();
        assert_eq!(
            t.vragen().as_slice(),
            [
                "/cells/register/api/lexostatus/per_gebied?gebied=A",
                "/cells/register/api/lexostatus/per_gebied?gebied=B"
            ]
        );
        assert_eq!(
            u.rows[0],
            json!({"orgaan": "raad", "gebiedscode": "A", "zetels": 10, "samenstellende": null, "tarief": 5})
        );
        assert_eq!(u.rows[1]["samenstellende"], json!(2));
        assert!(u.missing.is_empty(), "{:?}", u.missing);
        assert_eq!(u.sources[0].queried, 2);
        assert_eq!(u.sources[0].status, Status::Queried);
    }

    #[tokio::test]
    async fn een_onbereikbare_bron_vult_niets_aan() {
        let (b, _) = source(
            Err(TransportFout::Unreachable("weg".into())),
            json!({"gebied": {"column": "gebiedscode"}}),
        );
        let rows = Rijen {
            definitie: definitie(),
            sources: vec![b],
        };
        let u = stel_samen(&rows, &eigen(), &BTreeMap::new(), law(&service()))
            .await
            .unwrap();
        assert!(!u.rows[0].as_object().unwrap().contains_key("tarief"));
        assert_eq!(u.missing, ["tarief"]);
        assert_eq!(u.sources[0].status, Status::Unreachable);
    }

    #[tokio::test]
    async fn invoer_uit_de_wet_en_een_vaste_waarde() {
        // De peildatum leidt de regeling af uit het jaartal; de configuratie
        // zet niets om.
        let (b, t) = source(
            Ok(json!({"name": "per_gebied", "parameters": {"bedrag": 7}})),
            json!({
                "gebied": {"column": "gebiedscode"},
                "peildatum": {"regulation": "testregeling_peil", "output": "peildatum"},
                "naam": {"lexostatus": "aanvraag", "field": "aanduiding"},
                "soort": {"value": "raad"},
            }),
        );
        let rows = Rijen {
            definitie: definitie(),
            sources: vec![b],
        };
        let mut parameters = BTreeMap::new();
        parameters.insert("jaar".to_string(), json!(2026));
        let u = stel_samen(&rows, &eigen(), &parameters, law(&service()))
            .await
            .unwrap();
        assert_eq!(
            t.vragen()[0],
            "/cells/register/api/lexostatus/per_gebied?gebied=A&naam=EEN+LIJST&peildatum=2026-01-01&soort=raad"
        );
        assert_eq!(u.rows[0]["tarief"], json!(7));
        assert_eq!(
            u.from_law["testregeling_peil#peildatum"],
            Ok(json!("2026-01-01"))
        );
    }

    /// Kan de wet de invoer niet afleiden, dan wordt de bron niet bevraagd en
    /// zegt de fout wat de wet miste.
    #[tokio::test]
    async fn invoer_uit_de_wet_die_een_feit_mist() {
        let (b, t) = source(
            Ok(json!({"name": "per_gebied", "parameters": {"bedrag": 7}})),
            json!({"peildatum": {"regulation": "testregeling_peil", "output": "peildatum"}}),
        );
        let rows = Rijen {
            definitie: definitie(),
            sources: vec![b],
        };
        let u = stel_samen(&rows, &eigen(), &BTreeMap::new(), law(&service()))
            .await
            .unwrap();
        assert!(t.vragen().is_empty());
        let error = u.sources[0].error.as_deref().unwrap();
        assert!(error.contains("invoer 'peildatum' ontbreekt"), "{error}");
        assert!(error.contains("jaar"), "{error}");
        assert_eq!(u.missing, ["tarief"]);
    }

    #[tokio::test]
    async fn zonder_invoer_wordt_de_bron_niet_bevraagd() {
        let (b, t) = source(
            Ok(json!({"name": "per_gebied", "parameters": {"bedrag": 7}})),
            json!({"peildatum": {"parameter": "ontbreekt"}}),
        );
        let rows = Rijen {
            definitie: definitie(),
            sources: vec![b],
        };
        let u = stel_samen(&rows, &eigen(), &BTreeMap::new(), law(&service()))
            .await
            .unwrap();
        assert!(t.vragen().is_empty());
        assert_eq!(u.sources[0].status, Status::NotQueried);
        assert!(u.sources[0]
            .error
            .as_ref()
            .unwrap()
            .contains("invoer 'peildatum' ontbreekt"));
        assert_eq!(u.missing, ["tarief"]);
    }

    #[tokio::test]
    async fn een_regel_die_geen_object_is_bouwt_geen_tabel_op() {
        let mut l = eigen();
        l[0].extra_fields.insert(
            "organen".into(),
            json!([{"orgaan": "raad", "gebied": "A", "zetels": 1}, "raad B"]),
        );
        let rows = Rijen {
            definitie: definitie(),
            sources: Vec::new(),
        };
        let u = stel_samen(&rows, &l, &BTreeMap::new(), law(&service()))
            .await
            .unwrap();
        assert!(u.rows.is_empty());
        assert!(
            u.error
                .as_deref()
                .unwrap()
                .contains("regel 1 is geen object"),
            "{u:?}"
        );
        // Dan gaat de parameter niet naar de engine.
        let mut combined = Samenvoeging {
            parameters: BTreeMap::new(),
            provenance: BTreeMap::new(),
            sources: Vec::new(),
        };
        let uit = pas_toe(
            std::slice::from_ref(&rows),
            &l,
            &mut combined,
            law(&service()),
        )
        .await;
        assert!(uit[0].error.is_some());
        assert!(!combined.parameters.contains_key("table"));
    }

    /// Een bron die even wacht, telt hoeveel vragen er tegelijk lopen, en
    /// het gebied uit de vraag als bedrag teruggeeft.
    struct Traag {
        bezig: std::sync::atomic::AtomicUsize,
        hoogste: std::sync::atomic::AtomicUsize,
    }

    impl crate::transport::Transport for Traag {
        fn soort(&self) -> &'static str {
            "internal"
        }
        fn haal<'a>(&'a self, path: &'a str) -> crate::transport::Antwoord<'a> {
            use std::sync::atomic::Ordering::SeqCst;
            Box::pin(async move {
                let nu = self.bezig.fetch_add(1, SeqCst) + 1;
                self.hoogste.fetch_max(nu, SeqCst);
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                self.bezig.fetch_sub(1, SeqCst);
                let gebied = path.rsplit_once("gebied=").unwrap().1.to_string();
                Ok(
                    json!({"name": "per_gebied", "parameters": {}, "extra_fields": {"bedrag": gebied}}),
                )
            })
        }
        fn stuur<'a>(&'a self, path: &'a str, _body: &'a Value) -> crate::transport::Antwoord<'a> {
            self.haal(path)
        }
    }

    #[tokio::test]
    async fn de_regels_worden_tegelijk_bevraagd_in_hun_eigen_volgorde() {
        let organen: Vec<Value> = (0..8)
            .map(|i| json!({"orgaan": "raad", "gebied": format!("G{i}"), "zetels": i}))
            .collect();
        let mut l = eigen();
        l[0].extra_fields
            .insert("organen".into(), Value::Array(organen));
        let t = Arc::new(Traag {
            bezig: 0.into(),
            hoogste: 0.into(),
        });
        let (mut b, _) = source(
            Ok(Value::Null),
            json!({"gebied": {"column": "gebiedscode"}}),
        );
        b.transport = t.clone();
        let rows = Rijen {
            definitie: definitie(),
            sources: vec![b],
        };
        let u = stel_samen(&rows, &l, &BTreeMap::new(), law(&service()))
            .await
            .unwrap();
        assert!(t.hoogste.load(std::sync::atomic::Ordering::SeqCst) > 1);
        // Elke regel houdt haar eigen antwoord, in de volgorde van de tabel.
        let tarieven: Vec<&str> = u
            .rows
            .iter()
            .map(|r| r["tarief"].as_str().unwrap())
            .collect();
        assert_eq!(tarieven, ["G0", "G1", "G2", "G3", "G4", "G5", "G6", "G7"]);
        assert_eq!(u.sources[0].queried, 8);
        assert_eq!(u.sources[0].status, Status::Queried);
    }

    #[tokio::test]
    async fn zonder_tabel_geen_parameter() {
        let mut d = definitie();
        d.table.field = "bestaat_niet".into();
        let rows = Rijen {
            definitie: d,
            sources: Vec::new(),
        };
        assert!(
            stel_samen(&rows, &eigen(), &BTreeMap::new(), law(&service()))
                .await
                .is_none()
        );
    }
}
