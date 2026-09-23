//! Aanvraagmogelijkheden: wat kan een ingelogde organisatie hier aanvragen?
//!
//! Niemand somt dat op. De cel voert de wet uit voor de organisatie, met
//! alleen de feiten die er al zijn: wie er inlogt (de eHerkenning) en wat
//! andere cellen over haar weten (synthese). Het concept van de aanvraag is
//! leeg, op het subsidiejaar na. Per toets zegt de uitkomst dan:
//!
//! - een definitieve nul of onwaar: de regeling sluit het uit, dus geen
//!   aanvraag;
//! - onbekend: het hangt af van feiten die er nog niet zijn, vooral die de
//!   aanvrager zelf levert (Awb 4:2 lid 2). De aanvraag is mogelijk, en de
//!   ontbrekende feiten zijn wat de aanvraag moet vertellen;
//! - een definitieve positieve waarde: mogelijk.
//!
//! De toetsen zijn het mandaat van wie inlogt (mag hij namens deze
//! organisatie handelen?) en het besluit zelf (kan deze organisatie iets
//! krijgen?). Een termijn wordt alleen getoond.
//!
//! Geen mogelijkheid is geen weigering. Wie toch aanvraagt, op papier (Awb
//! 4:1), wordt vastgelegd en beoordeeld; het portaal biedt alleen aan wat de
//! wet toelaat. Niets hiervan wordt vastgelegd.

use std::collections::{BTreeMap, BTreeSet};

use regelrecht_engine::LawExecutionService;
use serde::Serialize;
use serde_json::Value;

use crate::toets::{self, Evaluatie};

/// Wat een toets, of alle toetsen samen, zeggen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Oordeel {
    /// De uitkomst is onbekend of positief: aanvragen kan.
    Mogelijk,
    /// De uitkomst is definitief nul of onwaar: de regeling sluit dit uit.
    Uitgesloten,
    /// De engine gaf geen uitkomst (een fout, geen ontbrekend feit).
    NietTeBepalen,
}

/// Welke vraag een toets beantwoordt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Vraag {
    /// Mag wie inlogt namens deze organisatie handelen?
    Mandaat,
    /// Kan deze organisatie iets krijgen?
    Besluit,
    /// Tot wanneer? Alleen getoond, telt niet mee.
    Termijn,
}

/// Een uitkomst van een regeling die het portaal uitvoert.
#[derive(Debug, Clone)]
pub struct Vraagstelling<'a> {
    pub vraag: Vraag,
    pub regeling: &'a str,
    pub uitkomst: &'a str,
}

/// De uitslag van een toets, met de trace van de engine-run.
#[derive(Debug, Clone, Serialize)]
pub struct Toetsing {
    pub vraag: Vraag,
    pub regeling: String,
    pub uitkomst: String,
    pub oordeel: Oordeel,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub waarde: Option<Value>,
    /// De feiten die de uitkomst nog mist en die de aanvraag kan leveren.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub mist: Vec<String>,
    /// De feiten die ontbreken maar pas bij de behandeling ontstaan, zoals
    /// de oordelen van de behandelaar. Ze tellen niet mee voor het oordeel.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub mist_behandeling: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reden: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace: Option<Value>,
}

/// Een mogelijkheid voor een subsidiejaar.
#[derive(Debug, Clone, Serialize)]
pub struct Mogelijkheid {
    pub subsidiejaar: i64,
    pub oordeel: Oordeel,
    pub toetsen: Vec<Toetsing>,
}

/// Of een definitieve waarde "nee" zegt: nul of onwaar.
fn is_nee(w: &Value) -> bool {
    match w {
        Value::Bool(b) => !b,
        Value::Number(n) => n.as_f64() == Some(0.0),
        _ => false,
    }
}

/// Het oordeel over een evaluatie van `uitkomst`. `niet_van_bronnen` zijn de
/// feiten die een bron niet leverde: mist de uitkomst er een van, dan is ze
/// niet te bepalen, want dat feit komt niet uit de aanvraag.
pub fn oordeel(e: &Evaluatie, uitkomst: &str, niet_van_bronnen: &BTreeSet<String>) -> Oordeel {
    match e.waarden.get(uitkomst) {
        // Leeg is geen nee: de regeling gaf geen waarde.
        Some(Value::Null) => Oordeel::NietTeBepalen,
        Some(w) if is_nee(w) => Oordeel::Uitgesloten,
        Some(_) => Oordeel::Mogelijk,
        None if e.mist.iter().any(|m| niet_van_bronnen.contains(m)) => Oordeel::NietTeBepalen,
        None if !e.mist.is_empty() => Oordeel::Mogelijk,
        None => Oordeel::NietTeBepalen,
    }
}

/// Wat de toetsen vooraf weten over waar een ontbrekend feit vandaan had
/// moeten komen.
#[derive(Debug, Clone, Default)]
pub struct Herkomsten {
    /// Feiten die een bron niet leverde.
    pub niet_van_bronnen: BTreeSet<String>,
    /// Feiten die pas bij de behandeling ontstaan.
    pub van_behandeling: BTreeSet<String>,
}

/// Het oordeel over alle toetsen samen: uitgesloten als een meetellende toets
/// uitsluit, anders niet te bepalen als er een niet te bepalen is.
pub fn samen(toetsen: &[Toetsing]) -> Oordeel {
    let tellen = toetsen.iter().filter(|t| t.vraag != Vraag::Termijn);
    let mut uit = Oordeel::Mogelijk;
    for t in tellen {
        match t.oordeel {
            Oordeel::Uitgesloten => return Oordeel::Uitgesloten,
            Oordeel::NietTeBepalen => uit = Oordeel::NietTeBepalen,
            Oordeel::Mogelijk => {}
        }
    }
    uit
}

/// Voer een toets uit op de samengevoegde parameters.
pub fn toets(
    service: &LawExecutionService,
    v: &Vraagstelling<'_>,
    parameters: &BTreeMap<String, Value>,
    herkomsten: &Herkomsten,
    datum: &str,
) -> Toetsing {
    let e = toets::evalueer_met_trace(service, v.regeling, &[v.uitkomst], parameters, datum);
    let oordeel = oordeel(&e, v.uitkomst, &herkomsten.niet_van_bronnen);
    let bron_mist: Vec<&String> = e
        .mist
        .iter()
        .filter(|m| herkomsten.niet_van_bronnen.contains(*m))
        .collect();
    let reden = match oordeel {
        Oordeel::NietTeBepalen if !bron_mist.is_empty() => Some(format!(
            "niet te bepalen: een bron leverde {} niet",
            bron_mist
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )),
        Oordeel::Uitgesloten => Some(format!(
            "{}: '{}' is {}",
            v.regeling,
            v.uitkomst,
            e.waarden
                .get(v.uitkomst)
                .map(Value::to_string)
                .unwrap_or_default()
        )),
        Oordeel::NietTeBepalen => Some(e.reden("niet te bepalen")),
        Oordeel::Mogelijk => None,
    };
    Toetsing {
        vraag: v.vraag,
        regeling: v.regeling.to_string(),
        uitkomst: v.uitkomst.to_string(),
        oordeel,
        waarde: e.waarden.get(v.uitkomst).cloned(),
        mist: e
            .mist
            .iter()
            .filter(|m| !herkomsten.van_behandeling.contains(*m))
            .cloned()
            .collect(),
        mist_behandeling: e
            .mist
            .iter()
            .filter(|m| herkomsten.van_behandeling.contains(*m))
            .cloned()
            .collect(),
        reden,
        trace: e.trace,
    }
}

/// Bepaal de mogelijkheid voor een subsidiejaar.
pub fn bepaal(
    service: &LawExecutionService,
    subsidiejaar: i64,
    vragen: &[Vraagstelling<'_>],
    parameters: &BTreeMap<String, Value>,
    herkomsten: &Herkomsten,
    datum: &str,
) -> Mogelijkheid {
    let toetsen: Vec<Toetsing> = vragen
        .iter()
        .map(|v| toets(service, v, parameters, herkomsten, datum))
        .collect();
    Mogelijkheid {
        subsidiejaar,
        oordeel: samen(&toetsen),
        toetsen,
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
    fn leeg_is_geen_nee() {
        assert_eq!(
            oordeel(&ev(Some(Value::Null), &[]), "u", &geen()),
            Oordeel::NietTeBepalen
        );
    }

    /// Een feit dat een bron niet leverde, komt niet uit de aanvraag: dan is
    /// de uitkomst niet te bepalen, niet mogelijk.
    #[test]
    fn wat_een_bron_niet_leverde_maakt_niet_te_bepalen() {
        let bron: BTreeSet<String> = ["registerfeit".to_string()].into();
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

    fn t(vraag: Vraag, oordeel: Oordeel) -> Toetsing {
        Toetsing {
            vraag,
            regeling: "r".into(),
            uitkomst: "u".into(),
            oordeel,
            waarde: None,
            mist: Vec::new(),
            mist_behandeling: Vec::new(),
            reden: None,
            trace: None,
        }
    }

    #[test]
    fn definitief_nul_sluit_uit() {
        assert_eq!(
            oordeel(&ev(Some(json!(0)), &[]), "u", &geen()),
            Oordeel::Uitgesloten
        );
        assert_eq!(
            oordeel(&ev(Some(json!(false)), &[]), "u", &geen()),
            Oordeel::Uitgesloten
        );
    }

    #[test]
    fn onbekend_door_ontbrekende_feiten_is_mogelijk() {
        assert_eq!(
            oordeel(&ev(None, &["feit_uit_de_aanvraag"]), "u", &geen()),
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
    fn fout_zonder_ontbrekend_feit_is_niet_te_bepalen() {
        let mut e = ev(None, &[]);
        e.fout = Some("kapot".into());
        assert_eq!(oordeel(&e, "u", &geen()), Oordeel::NietTeBepalen);
    }

    #[test]
    fn een_uitsluitende_toets_beslist() {
        let ts = [
            t(Vraag::Mandaat, Oordeel::Uitgesloten),
            t(Vraag::Besluit, Oordeel::Mogelijk),
        ];
        assert_eq!(samen(&ts), Oordeel::Uitgesloten);
    }

    #[test]
    fn de_termijn_telt_niet_mee() {
        let ts = [
            t(Vraag::Besluit, Oordeel::Mogelijk),
            t(Vraag::Termijn, Oordeel::Uitgesloten),
        ];
        assert_eq!(samen(&ts), Oordeel::Mogelijk);
    }
}
