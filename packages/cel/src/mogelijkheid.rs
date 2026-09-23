//! Aanvraagmogelijkheden: wat biedt het portaal een ingelogde persoon aan?
//!
//! Het beleid van de cel zegt het (`portaal.aanbod`): een uitkomst van een
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Oordeel {
    Mogelijk,
    Uitgesloten,
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

pub fn oordeel(e: &Evaluatie, uitkomst: &str, niet_van_bronnen: &BTreeSet<String>) -> Oordeel {
    match e.waarden.get(uitkomst) {
        Some(Value::Null) => Oordeel::NietTeBepalen,
        Some(w) if is_nee(w) => Oordeel::Uitgesloten,
        Some(_) => Oordeel::Mogelijk,
        None if e.mist.iter().any(|m| niet_van_bronnen.contains(m)) => Oordeel::NietTeBepalen,
        None if !e.mist.is_empty() => Oordeel::Mogelijk,
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
    let reden = match oordeel {
        Oordeel::Mogelijk => None,
        Oordeel::Uitgesloten => Some(format!(
            "{}: '{}' is onwaar",
            aanbod.regeling, aanbod.uitkomst
        )),
        Oordeel::NietTeBepalen => Some(e.reden("niet te bepalen")),
    };
    Mogelijkheid {
        subsidiejaar,
        oordeel,
        regeling: aanbod.regeling.clone(),
        uitkomst: aanbod.uitkomst.clone(),
        waarde: e.waarden.get(&aanbod.uitkomst).cloned(),
        termijn: aanbod
            .termijn
            .as_ref()
            .and_then(|t| e.waarden.get(t).cloned()),
        mist: e.mist.clone(),
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
    fn waar_is_mogelijk() {
        assert_eq!(
            oordeel(&ev(Some(json!(true)), &[]), "u", &geen()),
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
    fn wat_een_bron_niet_leverde_maakt_niet_te_bepalen() {
        let bron: BTreeSet<String> = ["registerfeit".to_string()].into();
        assert_eq!(
            oordeel(&ev(None, &["registerfeit"]), "u", &bron),
            Oordeel::NietTeBepalen
        );
        assert_eq!(
            oordeel(&ev(None, &["feit_uit_de_aanvraag"]), "u", &bron),
            Oordeel::Mogelijk
        );
    }
}
