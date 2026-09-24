//! Aanvraagmogelijkheden: wat biedt het portaal een ingelogde persoon aan?
//!
//! Het beleid van de actor zegt het (`portaal.aanbod` in `proces.yaml`): een uitkomst van een
//! regeling, uitgevoerd in een run met wat er vooraf bekend is: wie er
//! inlogt en wat andere cellen weten. Definitief onwaar of nul: geen aanbod.
//! Onbekend door feiten die de aanvraag nog levert, of waar: aanbod. Een
//! feit dat een bron niet leverde, of een lege uitkomst: niet te bepalen.
//! Geen aanbod is geen weigering. Niets hiervan wordt vastgelegd.

use std::collections::{BTreeMap, BTreeSet};

use regelrecht_engine::LawExecutionService;
use serde::Serialize;
use serde_json::Value;

use crate::config::Aanbod;
use crate::toets::{self, Evaluatie};

/// Wat het beleid over het aanbod zegt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Oordeel {
    /// De uitkomst is positief, of onbekend door feiten die de aanvraag nog
    /// levert: het portaal biedt de aanvraag aan.
    Mogelijk,
    /// De uitkomst is definitief nul of onwaar: geen aanbod.
    Uitgesloten,
    /// Geen oordeel: de uitkomst is leeg, de engine gaf een fout, of er mist
    /// een feit dat een bron had moeten leveren.
    NietTeBepalen,
}

/// Het aanbod voor een subsidiejaar, met de trace van de ene run.
#[derive(Debug, Clone, Serialize)]
pub struct Mogelijkheid {
    pub subsidiejaar: i64,
    pub oordeel: Oordeel,
    pub regeling: String,
    pub uitkomst: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub waarde: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub termijn: Option<Value>,
    /// Wat de uitkomst van het aanbod mist (niet wat de termijn mist).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub mist: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reden: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_text: Option<String>,
}

/// Nul of onwaar.
fn is_nee(w: &Value) -> bool {
    match w {
        Value::Bool(b) => !b,
        Value::Number(n) => n.as_f64() == Some(0.0),
        _ => false,
    }
}

/// Het oordeel over `uitkomst` in een evaluatie. Alleen wat deze uitkomst
/// mist telt, niet wat andere uitkomsten van dezelfde run missen. Leeg is
/// geen nee: een lege uitkomst is niet te bepalen. Mist de uitkomst een feit
/// uit `niet_van_bronnen` (een bron leverde het niet), dan is ze niet te
/// bepalen, want dat feit komt niet uit de aanvraag.
pub fn oordeel(e: &Evaluatie, uitkomst: &str, niet_van_bronnen: &BTreeSet<String>) -> Oordeel {
    let mist = e.mist_van(uitkomst);
    match e.waarden.get(uitkomst) {
        Some(Value::Null) => Oordeel::NietTeBepalen,
        Some(w) if is_nee(w) => Oordeel::Uitgesloten,
        Some(_) => Oordeel::Mogelijk,
        None if mist.iter().any(|m| niet_van_bronnen.contains(m)) => Oordeel::NietTeBepalen,
        None if !mist.is_empty() => Oordeel::Mogelijk,
        None => Oordeel::NietTeBepalen,
    }
}

/// Voer het aanbod uit voor een subsidiejaar: uitkomst en termijn in een run.
pub fn bepaal(
    service: &LawExecutionService,
    subsidiejaar: i64,
    aanbod: &Aanbod,
    parameters: &BTreeMap<String, Value>,
    niet_van_bronnen: &BTreeSet<String>,
    datum: &str,
) -> Mogelijkheid {
    let mut uitkomsten = vec![aanbod.uitkomst.as_str()];
    uitkomsten.extend(aanbod.termijn.as_deref());
    let e = toets::evalueer_met_trace(service, &aanbod.regeling, &uitkomsten, parameters, datum);
    let oordeel = oordeel(&e, &aanbod.uitkomst, niet_van_bronnen);
    let waarde = e.waarden.get(&aanbod.uitkomst).cloned();
    let mist = e.mist_van(&aanbod.uitkomst).to_vec();
    let reden = match oordeel {
        Oordeel::Mogelijk => None,
        Oordeel::Uitgesloten => Some(format!(
            "{}: '{}' is {}",
            aanbod.regeling,
            aanbod.uitkomst,
            waarde.as_ref().map(Value::to_string).unwrap_or_default()
        )),
        Oordeel::NietTeBepalen if !mist.is_empty() => {
            Some(format!("niet te bepalen: mist {}", mist.join(", ")))
        }
        Oordeel::NietTeBepalen => Some(e.reden("niet te bepalen")),
    };
    Mogelijkheid {
        subsidiejaar,
        oordeel,
        regeling: aanbod.regeling.clone(),
        uitkomst: aanbod.uitkomst.clone(),
        waarde,
        termijn: aanbod
            .termijn
            .as_ref()
            .and_then(|t| e.waarden.get(t).cloned()),
        mist,
        reden,
        trace_text: e.trace_text,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use serde_json::json;

    fn ev(waarde: Option<Value>, mist: &[&str]) -> Evaluatie {
        let mut e = Evaluatie::default();
        if let Some(w) = waarde {
            e.waarden.insert("u".into(), w);
        }
        e.mist = mist.iter().map(|s| s.to_string()).collect();
        if !mist.is_empty() {
            e.mist_per.insert("u".into(), e.mist.clone());
        }
        e
    }

    fn geen() -> BTreeSet<String> {
        BTreeSet::new()
    }

    #[test]
    fn onwaar_sluit_uit() {
        assert_eq!(
            oordeel(&ev(Some(json!(false)), &[]), "u", &geen()),
            Oordeel::Uitgesloten
        );
    }

    #[test]
    fn nul_sluit_uit() {
        assert_eq!(
            oordeel(&ev(Some(json!(0)), &[]), "u", &geen()),
            Oordeel::Uitgesloten
        );
    }

    #[test]
    fn waar_is_mogelijk() {
        assert_eq!(
            oordeel(&ev(Some(json!(true)), &[]), "u", &geen()),
            Oordeel::Mogelijk
        );
    }

    #[test]
    fn positief_is_mogelijk() {
        assert_eq!(
            oordeel(&ev(Some(json!(1200)), &[]), "u", &geen()),
            Oordeel::Mogelijk
        );
    }

    #[test]
    fn leeg_is_niet_te_bepalen() {
        assert_eq!(
            oordeel(&ev(Some(Value::Null), &[]), "u", &geen()),
            Oordeel::NietTeBepalen
        );
    }

    #[test]
    fn fout_zonder_ontbrekend_feit_is_niet_te_bepalen() {
        let mut e = ev(None, &[]);
        e.fout = Some("kapot".into());
        assert_eq!(oordeel(&e, "u", &geen()), Oordeel::NietTeBepalen);
    }

    /// Een feit dat een bron niet leverde, komt niet uit de aanvraag: dan is
    /// de uitkomst niet te bepalen, ook als er daarnaast een feit uit de
    /// aanvraag mist.
    #[test]
    fn wat_een_bron_niet_leverde_maakt_niet_te_bepalen() {
        let bron: BTreeSet<String> = ["registerfeit".to_string()].into();
        assert_eq!(
            oordeel(&ev(None, &["registerfeit"]), "u", &bron),
            Oordeel::NietTeBepalen
        );
        assert_eq!(
            oordeel(
                &ev(None, &["registerfeit", "feit_uit_de_aanvraag"]),
                "u",
                &bron
            ),
            Oordeel::NietTeBepalen
        );
        assert_eq!(
            oordeel(&ev(None, &["feit_uit_de_aanvraag"]), "u", &bron),
            Oordeel::Mogelijk
        );
    }

    /// Een fictieve regeling met een artikel: het aanbod en de termijn.
    const REGELING: &str = r#"
$id: testregeling_aanbod
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Aanbod
    machine_readable:
      execution:
        parameters:
          - {name: bevoegd, type: boolean, required: false}
          - {name: aanvraagfeit, type: boolean, required: false}
          - {name: jaar, type: number, required: false}
          - {name: registerdatum, type: date, required: false}
        output:
          - {name: aangeboden, type: boolean}
          - {name: termijn, type: number}
        actions:
          - output: aangeboden
            value:
              operation: AND
              conditions:
                - {operation: EQUALS, subject: $bevoegd, value: true}
                - {operation: EQUALS, subject: $aanvraagfeit, value: true}
          - output: termijn
            value:
              operation: IF
              cases:
                - when:
                    operation: LESS_THAN
                    subject: $registerdatum
                    value: {operation: DATE, year: $jaar, month: 4, day: 1}
                  then: $jaar
              default: $jaar
"#;

    fn service() -> LawExecutionService {
        let mut s = LawExecutionService::new();
        s.load_law(REGELING).unwrap();
        s
    }

    fn aanbod(termijn: bool) -> Aanbod {
        Aanbod {
            regeling: "testregeling_aanbod".into(),
            uitkomst: "aangeboden".into(),
            termijn: termijn.then(|| "termijn".into()),
        }
    }

    fn params(v: Value) -> BTreeMap<String, Value> {
        serde_json::from_value(v).unwrap()
    }

    fn bepaal_met(termijn: bool, p: Value, bronnen: &[&str]) -> Mogelijkheid {
        let bron: BTreeSet<String> = bronnen.iter().map(|s| s.to_string()).collect();
        bepaal(
            &service(),
            2026,
            &aanbod(termijn),
            &params(p),
            &bron,
            "2026-02-01",
        )
    }

    #[test]
    fn niet_bevoegd_sluit_uit_en_noemt_de_waarde() {
        let m = bepaal_met(
            true,
            json!({"bevoegd": false, "jaar": 2026, "registerdatum": "2026-01-01"}),
            &[],
        );
        assert_eq!(m.oordeel, Oordeel::Uitgesloten, "{m:?}");
        assert_eq!(m.waarde, Some(json!(false)));
        assert_eq!(m.termijn.as_ref().and_then(Value::as_f64), Some(2026.0));
        assert_eq!(
            m.reden.as_deref(),
            Some("testregeling_aanbod: 'aangeboden' is false")
        );
        assert!(m.trace_text.is_some());
    }

    #[test]
    fn bevoegd_is_mogelijk_en_mist_het_aanvraagfeit() {
        let m = bepaal_met(
            true,
            json!({"bevoegd": true, "jaar": 2026, "registerdatum": "2026-01-01"}),
            &[],
        );
        assert_eq!(m.oordeel, Oordeel::Mogelijk, "{m:?}");
        assert_eq!(m.waarde, None);
        assert_eq!(m.mist, ["aanvraagfeit"]);
        assert_eq!(m.reden, None);
    }

    #[test]
    fn wat_een_bron_niet_leverde_maakt_het_aanbod_niet_te_bepalen() {
        let m = bepaal_met(
            true,
            json!({"jaar": 2026, "registerdatum": "2026-01-01"}),
            &["bevoegd"],
        );
        assert_eq!(m.oordeel, Oordeel::NietTeBepalen, "{m:?}");
        assert!(m.reden.as_deref().unwrap().contains("bevoegd"), "{m:?}");
    }

    #[test]
    fn zonder_termijn() {
        let m = bepaal_met(false, json!({"bevoegd": true, "jaar": 2026}), &[]);
        assert_eq!(m.termijn, None);
        assert_eq!(m.oordeel, Oordeel::Mogelijk, "{m:?}");
        assert_eq!(m.mist, ["aanvraagfeit"]);
    }

    /// Wat de termijn mist, telt niet voor het oordeel over de uitkomst.
    #[test]
    fn wat_de_termijn_mist_telt_niet_mee() {
        let m = bepaal_met(
            true,
            json!({"bevoegd": true, "jaar": 2026}),
            &["registerdatum"],
        );
        assert_eq!(m.oordeel, Oordeel::Mogelijk, "{m:?}");
        assert_eq!(m.termijn, None);
        assert_eq!(m.mist, ["aanvraagfeit"]);
    }
}
