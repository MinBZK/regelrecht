//! Actor-identiteit: namens welk bevoegd gezag een proces handelt, en met
//! welk mandaat.
//!
//! RFC-022 §2 houdt drie assen uit elkaar, en de runtime ook:
//!
//! - `recording_actor`: wie vastlegt. De actor van het proces, die de
//!   `recording_actor` is van elke stroom waarin het vastlegt (een id).
//! - `competent_authority`: het bevoegd gezag volgens de wet, van het artikel
//!   of anders van de regeling (een naam).
//! - de handelende actor: wie in het proces handelt, in een rol, langs een
//!   kanaal, namens een gezag ([`crate::gram::HandelendeActor`]).
//!
//! Wat de actor met het gezag verbindt, zegt de configuratie: `namens` in
//! `proces.yaml` noemt het gezag zoals een regeling het in
//! `competent_authority` noemt, of een regeling waarvan het het bevoegd gezag
//! is. De runtime vergelijkt namen letterlijk; er is geen normalisatie die
//! een id als naam leest. Handelt het proces ook voor een ander gezag, dan
//! noemt `mandaten` dat gezag met een grondslag (Awb 10:1). Zonder mandaat
//! weigert het besluit bij een ander gezag.

use std::collections::BTreeSet;

use regelrecht_engine::LawExecutionService;
use serde_json::Value;

use crate::config::{Mandaat, Namens, ProcesDefinitie};
use crate::regelingen;

/// Een naam van een gezag uit `competent_authority`: een tekst, of een
/// object met `name`. Een verwijzing (`#bevoegd_gezag`) is geen naam.
fn name(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => s.strip_prefix('#').is_none().then(|| s.clone()),
        Value::Object(o) => o.get("name").and_then(Value::as_str).map(str::to_string),
        _ => None,
    }
}

fn naam_van<T: serde::Serialize>(authority: &T) -> Option<String> {
    name(&serde_json::to_value(authority).ok()?)
}

/// Het bevoegd gezag van een regeling zelf, zonder dat van een artikel.
pub fn gezag_van_regeling(service: &LawExecutionService, regulation: &str) -> Option<String> {
    let law = service.resolver().get_law(regulation)?;
    naam_van(law.competent_authority.as_ref()?)
}

/// Het bevoegd gezag volgens de wet: van het artikel zelf, anders van de
/// regeling.
pub fn gezag_van(service: &LawExecutionService, regulation: &str, article: &str) -> Option<String> {
    let law = service.resolver().get_law(regulation)?;
    let authority = law
        .find_article_by_number(article)
        .and_then(|a| a.machine_readable.as_ref())
        .and_then(|m| m.competent_authority.as_ref())
        .or(law.competent_authority.as_ref())?;
    naam_van(authority)
}

/// Elk gezag dat een geladen regeling noemt, bij de regeling of bij een
/// artikel.
pub fn gezagen(service: &LawExecutionService) -> BTreeSet<String> {
    let mut uit = BTreeSet::new();
    for id in service.list_laws() {
        let Some(law) = service.resolver().get_law(id) else {
            continue;
        };
        uit.extend(law.competent_authority.as_ref().and_then(naam_van));
        for a in &law.articles {
            uit.extend(
                a.machine_readable
                    .as_ref()
                    .and_then(|m| m.competent_authority.as_ref())
                    .and_then(naam_van),
            );
        }
    }
    uit
}

/// De beschikkingen waarvoor `gezag` bevoegd is, als (regeling, artikel):
/// elk artikel dat een `BESCHIKKING` produceert en waarvan het bevoegd gezag
/// (van het artikel, anders van de regeling) `gezag` is. Zo vindt een proces
/// zijn besluit in de wet, zonder dat de configuratie het aanwijst.
pub fn beschikkingen_van(service: &LawExecutionService, authority: &str) -> Vec<(String, String)> {
    let mut uit = Vec::new();
    for id in service.list_laws() {
        let Some(law) = service.resolver().get_law(id) else {
            continue;
        };
        for a in &law.articles {
            let beschikking = a
                .get_execution_spec()
                .and_then(|e| e.produces.as_ref())
                .and_then(|p| p.legal_character.as_deref())
                == Some("BESCHIKKING");
            if beschikking && gezag_van(service, id, &a.number).as_deref() == Some(authority) {
                uit.push((id.to_string(), a.number.clone()));
            }
        }
    }
    uit.sort();
    uit.dedup();
    uit
}

/// Het gezag waarvoor het proces handelt, uit `namens`, zonder controle:
/// `None` als het proces er geen noemt of de regeling er geen heeft.
pub fn eigen(d: &ProcesDefinitie, service: &LawExecutionService) -> Option<String> {
    match d.on_behalf_of.as_ref()? {
        Namens::Gezag { authority } => Some(authority.clone()),
        Namens::Regeling { regulation } => gezag_van_regeling(service, regulation),
    }
}

/// Het gezag waarvoor het proces handelt, uit `namens`, gecontroleerd tegen
/// de wet; en de controle op de mandaten. Een naam moet een gezag zijn dat
/// een geladen regeling noemt; een regeling moet geladen zijn en een gezag
/// noemen. Een mandaat noemt zo'n gezag, niet het eigen gezag, en een
/// grondslag die een geladen artikel aanwijst.
pub fn los_op(
    d: &ProcesDefinitie,
    service: &LawExecutionService,
) -> Result<Option<String>, Vec<String>> {
    let bekend = gezagen(service);
    let onbekend = |g: &str| {
        format!(
            "geen geladen regeling noemt '{g}' als bevoegd gezag (competent_authority); bekend: {}",
            if bekend.is_empty() {
                "geen".to_string()
            } else {
                bekend.iter().cloned().collect::<Vec<_>>().join(", ")
            }
        )
    };
    let mut fouten = Vec::new();
    let eigen = match &d.on_behalf_of {
        None => None,
        Some(Namens::Gezag { authority }) => {
            if !bekend.contains(authority) {
                fouten.push(format!("on_behalf_of: {}", onbekend(authority)));
            }
            Some(authority.clone())
        }
        Some(Namens::Regeling { regulation }) => match service.resolver().get_law(regulation) {
            None => {
                fouten.push(format!(
                    "on_behalf_of: regeling '{regulation}' is niet geladen"
                ));
                None
            }
            Some(_) => match gezag_van_regeling(service, regulation) {
                Some(g) => Some(g),
                None => {
                    fouten.push(format!(
                        "on_behalf_of: regeling '{regulation}' noemt geen bevoegd gezag (competent_authority)"
                    ));
                    None
                }
            },
        },
    };
    for m in &d.mandates {
        if !bekend.contains(&m.authority) {
            fouten.push(format!("mandaat: {}", onbekend(&m.authority)));
        }
        if eigen.as_deref() == Some(m.authority.as_str()) {
            fouten.push(format!(
                "mandaat: '{}' is het gezag waarvoor het proces zelf handelt (namens)",
                m.authority
            ));
        }
        if let Err(f) = regelingen::geldig(service, &m.legal_basis) {
            fouten.push(format!("mandaat van '{}': {f}", m.authority));
        }
    }
    if d.handling.is_some() && d.on_behalf_of.is_none() {
        fouten.push(
            "behandeling zonder on_behalf_of: noem het bevoegd gezag waarvoor het proces besluit (namens: {authority: <naam>} of {regulation: <$id>})".into(),
        );
    }
    if fouten.is_empty() {
        Ok(eigen)
    } else {
        Err(fouten)
    }
}

/// Waarom het proces een besluit van `wet` (het gezag dat de wet aanwijst)
/// mag nemen: als dat gezag zelf, of in mandaat.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Bevoegdheid<'a> {
    Own,
    Mandaat(&'a Mandaat),
}

/// Toets het gezag van de wet tegen het eigen gezag en de mandaten. Een
/// ander gezag zonder mandaat is een weigering, met de reden.
pub fn assessment<'a>(
    eigen: Option<&str>,
    mandates: &'a [Mandaat],
    law: &str,
) -> Result<Bevoegdheid<'a>, String> {
    if eigen == Some(law) {
        return Ok(Bevoegdheid::Own);
    }
    if let Some(m) = mandates.iter().find(|m| m.authority == law) {
        return Ok(Bevoegdheid::Mandaat(m));
    }
    Err(match eigen {
        Some(e) => format!(
            "de wet wijst '{law}' aan als bevoegd gezag, en het proces handelt namens '{e}' zonder mandaat van '{law}'"
        ),
        None => format!(
            "de wet wijst '{law}' aan als bevoegd gezag, en het proces noemt niet namens wie het handelt"
        ),
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    /// Een fictieve regeling met een beschikking van "De Instantie van
    /// Voorbeeld", een artikel met een eigen gezag, en een mandaatregeling.
    const REGELING: &str = r#"
$id: testregeling_bevoegd
regulatory_layer: WET
publication_date: '2025-01-01'
competent_authority:
  name: De Instantie van Voorbeeld
articles:
  - number: '1'
    text: Toets
    machine_readable:
      execution:
        produces: {legal_character: TOETS, decision_type: GEEN_BESLUIT}
        parameters: [{name: x, type: number, required: false}]
        output: [{name: toets, type: boolean}]
        actions: [{output: toets, value: {operation: GREATER_THAN, subject: $x, value: 0}}]
  - number: '2'
    text: Besluit
    machine_readable:
      execution:
        produces: {legal_character: BESCHIKKING, decision_type: TOEKENNING}
        parameters: [{name: x, type: number, required: false}]
        output: [{name: bedrag, type: number}]
        actions: [{output: bedrag, value: $x}]
  - number: '3'
    text: Besluit van de raad
    machine_readable:
      competent_authority: De Raad van Voorbeeld
      execution:
        produces: {legal_character: BESCHIKKING, decision_type: TOEKENNING}
        parameters: [{name: x, type: number, required: false}]
        output: [{name: bedrag_raad, type: number}]
        actions: [{output: bedrag_raad, value: $x}]
  - number: '4'
    text: De raad verleent de instantie mandaat voor de besluiten van artikel 3.
"#;

    fn service() -> LawExecutionService {
        let mut s = LawExecutionService::new();
        s.load_law(REGELING).unwrap();
        s
    }

    fn proces(extra: &str) -> ProcesDefinitie {
        ProcesDefinitie::parse(&format!("id: p\nactor: de_instantie\n{extra}"), "t").unwrap()
    }

    #[test]
    fn de_beschikking_van_het_bevoegd_gezag_wordt_gevonden() {
        let s = service();
        assert_eq!(
            beschikkingen_van(&s, "De Instantie van Voorbeeld"),
            vec![("testregeling_bevoegd".to_string(), "2".to_string())]
        );
        // Letterlijk: een id is geen naam.
        assert!(beschikkingen_van(&s, "de_instantie_van_voorbeeld").is_empty());
        assert!(beschikkingen_van(&s, "Een ander orgaan").is_empty());
    }

    #[test]
    fn namens_een_gezag_of_een_regeling() {
        let s = service();
        let d = proces("on_behalf_of: {authority: De Raad van Voorbeeld}\n");
        assert_eq!(
            los_op(&d, &s).unwrap().as_deref(),
            Some("De Raad van Voorbeeld")
        );
        let d = proces("on_behalf_of: {regulation: testregeling_bevoegd}\n");
        assert_eq!(
            los_op(&d, &s).unwrap().as_deref(),
            Some("De Instantie van Voorbeeld")
        );
        let d = proces("on_behalf_of: {authority: de_instantie_van_voorbeeld}\n");
        let f = los_op(&d, &s).unwrap_err();
        assert!(
            f[0].contains("geen geladen regeling noemt 'de_instantie_van_voorbeeld'"),
            "{f:?}"
        );
        let d = proces("on_behalf_of: {regulation: bestaat_niet}\n");
        assert!(los_op(&d, &s).unwrap_err()[0].contains("niet geladen"));
        assert_eq!(los_op(&proces(""), &s).unwrap(), None);
    }

    #[test]
    fn een_mandaat_noemt_een_bekend_gezag_en_een_grondslag() {
        let s = service();
        let ok = proces(
            "on_behalf_of: {regulation: testregeling_bevoegd}\nmandates:\n  - {authority: De Raad van Voorbeeld, legal_basis: 'testregeling_bevoegd#4'}\n",
        );
        assert!(los_op(&ok, &s).is_ok());
        let error = proces(
            "on_behalf_of: {regulation: testregeling_bevoegd}\nmandates:\n  - {authority: De Instantie van Voorbeeld, legal_basis: 'testregeling_bevoegd#9'}\n  - {authority: Niemand, legal_basis: 'testregeling_bevoegd#4'}\n",
        );
        let f = los_op(&error, &s).unwrap_err();
        assert_eq!(f.len(), 3, "{f:?}");
        assert!(f[0].contains("het gezag waarvoor het proces zelf handelt"));
        assert!(f[1].contains("testregeling_bevoegd#9"));
        assert!(f[2].contains("'Niemand'"));
    }

    #[test]
    fn eigen_gezag_mandaat_of_weigeren() {
        let m = [Mandaat {
            authority: "De Raad van Voorbeeld".into(),
            legal_basis: "testregeling_bevoegd#4".into(),
        }];
        let eigen = Some("De Instantie van Voorbeeld");
        assert_eq!(
            assessment(eigen, &m, "De Instantie van Voorbeeld"),
            Ok(Bevoegdheid::Own)
        );
        assert_eq!(
            assessment(eigen, &m, "De Raad van Voorbeeld"),
            Ok(Bevoegdheid::Mandaat(&m[0]))
        );
        assert!(assessment(eigen, &m, "Een ander")
            .unwrap_err()
            .contains("zonder mandaat van 'Een ander'"));
        assert!(assessment(eigen, &[], "De Raad van Voorbeeld").is_err());
        assert!(assessment(None, &m, "Een ander")
            .unwrap_err()
            .contains("niet namens wie"));
    }
}
