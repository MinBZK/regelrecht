//! Aanvraagmogelijkheden: wat biedt het portaal een ingelogde persoon aan?
//!
//! Het beleid van de actor zegt het (`portaal.aanbod` in `proces.yaml`): een
//! uitkomst van een regeling, uitgevoerd in een run met wat er vooraf
//! vaststaat: wie er inlogt, wat andere cellen weten en het gekozen tijdvak
//! (de parameter met origin BELANGHEBBENDE en grondslag Awb 4:2 lid 1).
//! Waar: aanbod. Definitief onwaar of nul: geen aanbod. Al het andere (leeg,
//! er mist iets, een fout van de engine) is niet te bepalen. Of een aanvraag
//! volledig is, weet je vooraf niet; dat is de toets na het invullen. De
//! runtime start daarom niet als de aanbod-uitkomst een feit vraagt dat vooraf
//! niet bekend is (zie [`crate::origin`]). Geen aanbod is geen weigering.
//! Niets hiervan wordt vastgelegd.

use std::collections::BTreeMap;

use regelrecht_engine::LawExecutionService;
use serde::Serialize;
use serde_json::Value;

use crate::config::Aanbod;
use crate::toets::{self, Evaluatie};

/// Wat het beleid over het aanbod zegt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Oordeel {
    /// De uitkomst is waar (of positief): het portaal biedt de aanvraag aan.
    Possible,
    /// De uitkomst is definitief nul of onwaar: geen aanbod.
    Excluded,
    /// Geen oordeel: de uitkomst is leeg, er mist een feit, of de engine gaf
    /// een fout.
    Undeterminable,
}

/// Een gekozen tijdvak: de parameter, het veld van het concept dat het
/// portaal vooraf invult, en de waarde.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Choice {
    pub parameter: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
    pub value: Value,
}

/// Het aanbod voor een tijdvak (of zonder tijdvak), met de trace van de ene
/// run.
#[derive(Debug, Clone, Serialize)]
pub struct Mogelijkheid {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window: Option<Choice>,
    pub verdict: Oordeel,
    pub regulation: String,
    pub output: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deadline: Option<Value>,
    /// Wat de uitkomst van het aanbod mist (niet wat de termijn mist).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub missing: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
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

/// Het oordeel over `uitkomst` in een evaluatie: waar is mogelijk, nul of
/// onwaar is uitgesloten, en al het andere is niet te bepalen. Leeg is geen
/// nee, en onbekend is geen ja: een uitkomst die een feit mist, zegt niets
/// over het aanbod.
pub fn verdict(e: &Evaluatie, output: &str) -> Oordeel {
    match e.waarden.get(output) {
        Some(Value::Null) | None => Oordeel::Undeterminable,
        Some(w) if is_nee(w) => Oordeel::Excluded,
        Some(_) => Oordeel::Possible,
    }
}

/// De tijdvakken die het beleid aanbiedt: de uitkomst `tijdvakken` van de
/// regeling van het aanbod, in een run zonder parameters op `datum`. Welke
/// tijdvakken er zijn, is beleid (de ruimte die Awb 4:2 lid 1 de actor laat),
/// geen configuratie. Geen lijst is een fout: dan valt er niets aan te bieden.
pub fn windows(
    service: &LawExecutionService,
    regulation: &str,
    windows: &str,
    date: &str,
) -> Result<Vec<Value>, String> {
    let e = toets::evalueer(service, regulation, &[windows], &BTreeMap::new(), date);
    match e.waarden.get(windows) {
        Some(Value::Array(list)) => Ok(list.clone()),
        Some(ander) => Err(format!(
            "{regulation}: tijdvakken '{windows}' is geen lijst ({ander})"
        )),
        None => Err(e.reason(&format!(
            "{regulation}: tijdvakken '{windows}' niet te bepalen"
        ))),
    }
}

/// Het begin van een tijdvak volgens het beleid: de uitkomst `begin` van de
/// regeling van het aanbod, met alleen het gekozen tijdvak als parameter. Het
/// aanbod voor een tijdvak dat nog moet beginnen, peilt de registers op die
/// dag. Geen datum is een fout.
pub fn start(
    service: &LawExecutionService,
    regulation: &str,
    start: &str,
    keuze: &Choice,
    date: &str,
) -> Result<chrono::NaiveDate, String> {
    let mut p = BTreeMap::new();
    p.insert(keuze.parameter.clone(), keuze.value.clone());
    let e = toets::evalueer(service, regulation, &[start], &p, date);
    match e.waarden.get(start) {
        Some(Value::String(d)) => chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d")
            .map_err(|_| format!("{regulation}: begin '{start}' is geen datum ({d})")),
        Some(ander) => Err(format!(
            "{regulation}: begin '{start}' is geen datum ({ander})"
        )),
        None => Err(e.reason(&format!("{regulation}: begin '{start}' niet te bepalen"))),
    }
}

/// Voer het aanbod uit voor een tijdvak: uitkomst en termijn in een run.
pub fn bepaal(
    service: &LawExecutionService,
    window: Option<Choice>,
    offer: &Aanbod,
    parameters: &BTreeMap<String, Value>,
    date: &str,
) -> Mogelijkheid {
    let mut outputs = vec![offer.output.as_str()];
    outputs.extend(offer.deadline.as_deref());
    let e = toets::evalueer_met_trace(service, &offer.regulation, &outputs, parameters, date);
    let verdict = verdict(&e, &offer.output);
    let value = e.waarden.get(&offer.output).cloned();
    let missing = e.mist_van(&offer.output).to_vec();
    let reason = match verdict {
        Oordeel::Possible => None,
        Oordeel::Excluded => Some(format!(
            "{}: '{}' is {}",
            offer.regulation,
            offer.output,
            value.as_ref().map(Value::to_string).unwrap_or_default()
        )),
        Oordeel::Undeterminable if !missing.is_empty() => {
            Some(format!("niet te bepalen: mist {}", missing.join(", ")))
        }
        Oordeel::Undeterminable => Some(e.reason("niet te bepalen")),
    };
    Mogelijkheid {
        window,
        verdict,
        regulation: offer.regulation.clone(),
        output: offer.output.clone(),
        value,
        deadline: offer
            .deadline
            .as_ref()
            .and_then(|t| e.waarden.get(t).cloned()),
        missing,
        reason,
        trace_text: e.trace_text,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use serde_json::json;

    fn ev(value: Option<Value>, missing: &[&str]) -> Evaluatie {
        let mut e = Evaluatie::default();
        if let Some(w) = value {
            e.waarden.insert("u".into(), w);
        }
        e.missing = missing.iter().map(|s| s.to_string()).collect();
        if !missing.is_empty() {
            e.mist_per.insert("u".into(), e.missing.clone());
        }
        e
    }

    #[test]
    fn onwaar_sluit_uit() {
        assert_eq!(
            verdict(&ev(Some(json!(false)), &[]), "u"),
            Oordeel::Excluded
        );
    }

    #[test]
    fn nul_sluit_uit() {
        assert_eq!(verdict(&ev(Some(json!(0)), &[]), "u"), Oordeel::Excluded);
    }

    #[test]
    fn waar_is_mogelijk() {
        assert_eq!(verdict(&ev(Some(json!(true)), &[]), "u"), Oordeel::Possible);
    }

    #[test]
    fn positief_is_mogelijk() {
        assert_eq!(verdict(&ev(Some(json!(1200)), &[]), "u"), Oordeel::Possible);
    }

    #[test]
    fn leeg_is_niet_te_bepalen() {
        assert_eq!(
            verdict(&ev(Some(Value::Null), &[]), "u"),
            Oordeel::Undeterminable
        );
    }

    #[test]
    fn fout_zonder_ontbrekend_feit_is_niet_te_bepalen() {
        let mut e = ev(None, &[]);
        e.error = Some("kapot".into());
        assert_eq!(verdict(&e, "u"), Oordeel::Undeterminable);
    }

    /// Onbekend is geen ja: mist de uitkomst een feit, uit de aanvraag of uit
    /// een bron, dan zegt ze niets over het aanbod.
    #[test]
    fn onbekend_is_niet_te_bepalen() {
        assert_eq!(
            verdict(&ev(None, &["feit_uit_de_aanvraag"]), "u"),
            Oordeel::Undeterminable
        );
        assert_eq!(
            verdict(&ev(None, &["registerfeit"]), "u"),
            Oordeel::Undeterminable
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

    fn offer(deadline: bool) -> Aanbod {
        Aanbod {
            regulation: "testregeling_aanbod".into(),
            output: "aangeboden".into(),
            deadline: deadline.then(|| "termijn".into()),
            windows: None,
            start: None,
            opening: None,
        }
    }

    fn params(v: Value) -> BTreeMap<String, Value> {
        serde_json::from_value(v).unwrap()
    }

    fn bepaal_met(deadline: bool, p: Value) -> Mogelijkheid {
        let keuze = Choice {
            parameter: "jaar".into(),
            field: None,
            value: json!(2026),
        };
        bepaal(
            &service(),
            Some(keuze),
            &offer(deadline),
            &params(p),
            "2026-02-01",
        )
    }

    #[test]
    fn niet_bevoegd_sluit_uit_en_noemt_de_waarde() {
        let m = bepaal_met(
            true,
            json!({"bevoegd": false, "jaar": 2026, "registerdatum": "2026-01-01"}),
        );
        assert_eq!(m.verdict, Oordeel::Excluded, "{m:?}");
        assert_eq!(m.value, Some(json!(false)));
        assert_eq!(m.deadline.as_ref().and_then(Value::as_f64), Some(2026.0));
        assert_eq!(
            m.reason.as_deref(),
            Some("testregeling_aanbod: 'aangeboden' is false")
        );
        assert!(m.trace_text.is_some());
    }

    #[test]
    fn alle_voorwaarden_waar_is_mogelijk() {
        let m = bepaal_met(
            true,
            json!({"bevoegd": true, "aanvraagfeit": true, "jaar": 2026, "registerdatum": "2026-01-01"}),
        );
        assert_eq!(m.verdict, Oordeel::Possible, "{m:?}");
        assert_eq!(m.value, Some(json!(true)));
        assert_eq!(m.reason, None);
    }

    /// Een voorwaarde die een feit mist, maakt het aanbod niet te bepalen, ook
    /// als dat feit later uit de aanvraag zou komen. (De runtime start niet
    /// met zo'n aanbod; zie de controle in `origin`.)
    #[test]
    fn een_ontbrekend_feit_maakt_het_aanbod_niet_te_bepalen() {
        let m = bepaal_met(
            true,
            json!({"bevoegd": true, "jaar": 2026, "registerdatum": "2026-01-01"}),
        );
        assert_eq!(m.verdict, Oordeel::Undeterminable, "{m:?}");
        assert_eq!(m.value, None);
        assert_eq!(m.missing, ["aanvraagfeit"]);
        assert_eq!(
            m.reason.as_deref(),
            Some("niet te bepalen: mist aanvraagfeit")
        );
        let m = bepaal_met(true, json!({"jaar": 2026, "registerdatum": "2026-01-01"}));
        assert_eq!(m.verdict, Oordeel::Undeterminable, "{m:?}");
        assert!(m.reason.as_deref().unwrap().contains("bevoegd"), "{m:?}");
    }

    /// De tijdvakken uit het beleid: een lijst uit een run zonder parameters;
    /// een uitkomst die geen lijst is, is een fout.
    #[test]
    fn tijdvakken_uit_een_run() {
        const BELEID: &str = r#"
$id: testbeleid_tijdvakken
regulatory_layer: UITVOERINGSBELEID
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Het portaal biedt het lopende jaar en het volgende aan.
    machine_readable:
      execution:
        output:
          - {name: jaren, type: array}
          - {name: een_jaar, type: number}
        actions:
          - output: jaren
            value:
              operation: FOREACH
              collection: [0, 1]
              as: verder
              body: {operation: ADD, values: [$referencedate.year, $verder]}
          - output: een_jaar
            value: $referencedate.year
"#;
        let mut s = LawExecutionService::new();
        s.load_law(BELEID).unwrap();
        assert_eq!(
            windows(&s, "testbeleid_tijdvakken", "jaren", "2026-09-25").unwrap(),
            [json!(2026), json!(2027)]
        );
        let error = windows(&s, "testbeleid_tijdvakken", "een_jaar", "2026-09-25").unwrap_err();
        assert!(error.contains("is geen lijst"), "{error}");
    }

    #[test]
    fn zonder_termijn() {
        let m = bepaal_met(
            false,
            json!({"bevoegd": true, "aanvraagfeit": true, "jaar": 2026}),
        );
        assert_eq!(m.deadline, None);
        assert_eq!(m.verdict, Oordeel::Possible, "{m:?}");
    }

    /// Wat de termijn mist, telt niet voor het oordeel over de uitkomst.
    #[test]
    fn wat_de_termijn_mist_telt_niet_mee() {
        let m = bepaal_met(
            true,
            json!({"bevoegd": false, "aanvraagfeit": true, "jaar": 2026}),
        );
        assert_eq!(m.verdict, Oordeel::Excluded, "{m:?}");
        assert_eq!(m.deadline, None);
        assert!(m.missing.is_empty());
    }
}
