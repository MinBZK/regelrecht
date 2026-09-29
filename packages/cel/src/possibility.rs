//! Application possibilities: what does the portal offer a logged-in person?
//!
//! The actor's policy says so (`portal.offer` in `process.yaml`): an output
//! of a regulation, executed in a run with what is fixed beforehand: who logs
//! in, what other cells know and the chosen window (the parameter with
//! origin BELANGHEBBENDE and legal basis Awb 4:2 lid 1). True: offer.
//! Definitively false or zero: no offer. Everything else (empty, something is
//! missing, an engine error) is undeterminable. Whether an application is
//! complete you do not know beforehand; that is the assessment after filling
//! it in. The runtime therefore does not start if the offer output asks for a
//! fact that is not known beforehand (see [`crate::origin`]). No offer is not
//! a refusal. None of this is recorded.

use std::collections::BTreeMap;

use regelrecht_engine::LawExecutionService;
use serde::Serialize;
use serde_json::Value;

use crate::assessment::{self, Evaluation};
use crate::config::Offer;

/// What the policy says about the offer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    /// The output is true (or positive): the portal offers the application.
    Possible,
    /// The output is definitively zero or false: no offer.
    Excluded,
    /// No verdict: the output is empty, a fact is missing, or the engine
    /// gave an error.
    Undeterminable,
}

/// A chosen window: the parameter, the field of the draft the portal fills
/// in beforehand, and the value.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Choice {
    pub parameter: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
    pub value: Value,
}

/// The offer for a window (or without a window), with the trace of the
/// single run.
#[derive(Debug, Clone, Serialize)]
pub struct Possibility {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window: Option<Choice>,
    pub verdict: Verdict,
    pub regulation: String,
    pub output: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deadline: Option<Value>,
    /// What the offer's output is missing (not what the deadline is missing).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub missing: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_text: Option<String>,
}

/// Zero or false.
fn is_no(w: &Value) -> bool {
    match w {
        Value::Bool(b) => !b,
        Value::Number(n) => n.as_f64() == Some(0.0),
        _ => false,
    }
}

/// The verdict on `output` in an evaluation: true is possible, zero or false
/// is excluded, and everything else is undeterminable. Empty is not a no,
/// and unknown is not a yes: an output that misses a fact says nothing about
/// the offer.
pub fn verdict(e: &Evaluation, output: &str) -> Verdict {
    match e.values.get(output) {
        Some(Value::Null) | None => Verdict::Undeterminable,
        Some(w) if is_no(w) => Verdict::Excluded,
        Some(_) => Verdict::Possible,
    }
}

/// The windows the policy offers: the output `windows` of the offer's
/// regulation, in a run without parameters on `date`. Which windows exist is
/// policy (the room Awb 4:2 lid 1 leaves the actor), not configuration. Not a
/// list is an error: then there is nothing to offer.
pub fn windows(
    service: &LawExecutionService,
    regulation: &str,
    windows: &str,
    date: &str,
) -> Result<Vec<Value>, String> {
    let e = assessment::evaluate(service, regulation, &[windows], &BTreeMap::new(), date);
    match e.values.get(windows) {
        Some(Value::Array(list)) => Ok(list.clone()),
        Some(other) => Err(format!(
            "{regulation}: windows '{windows}' is not a list ({other})"
        )),
        None => Err(e.reason(&format!("{regulation}: windows '{windows}' undeterminable"))),
    }
}

/// The start of a window according to the policy: the output `start` of the
/// offer's regulation, with only the chosen window as parameter. The offer
/// for a window that has yet to begin queries the registers on that day. Not
/// a date is an error.
pub fn start(
    service: &LawExecutionService,
    regulation: &str,
    start: &str,
    choice: &Choice,
    date: &str,
) -> Result<chrono::NaiveDate, String> {
    let mut p = BTreeMap::new();
    p.insert(choice.parameter.clone(), choice.value.clone());
    let e = assessment::evaluate(service, regulation, &[start], &p, date);
    match e.values.get(start) {
        Some(Value::String(d)) => chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d")
            .map_err(|_| format!("{regulation}: start '{start}' is not a date ({d})")),
        Some(other) => Err(format!(
            "{regulation}: start '{start}' is not a date ({other})"
        )),
        None => Err(e.reason(&format!("{regulation}: start '{start}' undeterminable"))),
    }
}

/// Runs the offer for a window: output and deadline in one run.
pub fn determine(
    service: &LawExecutionService,
    window: Option<Choice>,
    offer: &Offer,
    parameters: &BTreeMap<String, Value>,
    date: &str,
) -> Possibility {
    let mut outputs = vec![offer.output.as_str()];
    outputs.extend(offer.deadline.as_deref());
    let e = assessment::evaluate_with_trace(service, &offer.regulation, &outputs, parameters, date);
    let verdict = verdict(&e, &offer.output);
    let value = e.values.get(&offer.output).cloned();
    let missing = e.missing_of(&offer.output).to_vec();
    let reason = match verdict {
        Verdict::Possible => None,
        Verdict::Excluded => Some(format!(
            "{}: '{}' is {}",
            offer.regulation,
            offer.output,
            value.as_ref().map(Value::to_string).unwrap_or_default()
        )),
        Verdict::Undeterminable if !missing.is_empty() => {
            Some(format!("undeterminable: missing {}", missing.join(", ")))
        }
        Verdict::Undeterminable => Some(e.reason("undeterminable")),
    };
    Possibility {
        window,
        verdict,
        regulation: offer.regulation.clone(),
        output: offer.output.clone(),
        value,
        deadline: offer
            .deadline
            .as_ref()
            .and_then(|t| e.values.get(t).cloned()),
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

    fn ev(value: Option<Value>, missing: &[&str]) -> Evaluation {
        let mut e = Evaluation::default();
        if let Some(w) = value {
            e.values.insert("u".into(), w);
        }
        e.missing = missing.iter().map(|s| s.to_string()).collect();
        if !missing.is_empty() {
            e.missing_per.insert("u".into(), e.missing.clone());
        }
        e
    }

    #[test]
    fn false_excludes() {
        assert_eq!(
            verdict(&ev(Some(json!(false)), &[]), "u"),
            Verdict::Excluded
        );
    }

    #[test]
    fn zero_excludes() {
        assert_eq!(verdict(&ev(Some(json!(0)), &[]), "u"), Verdict::Excluded);
    }

    #[test]
    fn true_is_possible() {
        assert_eq!(verdict(&ev(Some(json!(true)), &[]), "u"), Verdict::Possible);
    }

    #[test]
    fn positive_is_possible() {
        assert_eq!(verdict(&ev(Some(json!(1200)), &[]), "u"), Verdict::Possible);
    }

    #[test]
    fn empty_is_undeterminable() {
        assert_eq!(
            verdict(&ev(Some(Value::Null), &[]), "u"),
            Verdict::Undeterminable
        );
    }

    #[test]
    fn error_without_missing_fact_is_undeterminable() {
        let mut e = ev(None, &[]);
        e.error = Some("kapot".into());
        assert_eq!(verdict(&e, "u"), Verdict::Undeterminable);
    }

    /// Unknown is not a yes: if the output misses a fact, from the
    /// application or from a source, it says nothing about the offer.
    #[test]
    fn unknown_is_undeterminable() {
        assert_eq!(
            verdict(&ev(None, &["feit_uit_de_aanvraag"]), "u"),
            Verdict::Undeterminable
        );
        assert_eq!(
            verdict(&ev(None, &["registerfeit"]), "u"),
            Verdict::Undeterminable
        );
    }

    /// A fictional regulation with one article: the offer and the deadline.
    const REGULATION: &str = r#"
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
        s.load_law(REGULATION).unwrap();
        s
    }

    fn offer(deadline: bool) -> Offer {
        Offer {
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

    fn determine_with(deadline: bool, p: Value) -> Possibility {
        let choice = Choice {
            parameter: "jaar".into(),
            field: None,
            value: json!(2026),
        };
        determine(
            &service(),
            Some(choice),
            &offer(deadline),
            &params(p),
            "2026-02-01",
        )
    }

    #[test]
    fn not_competent_excludes_and_names_the_value() {
        let m = determine_with(
            true,
            json!({"bevoegd": false, "jaar": 2026, "registerdatum": "2026-01-01"}),
        );
        assert_eq!(m.verdict, Verdict::Excluded, "{m:?}");
        assert_eq!(m.value, Some(json!(false)));
        assert_eq!(m.deadline.as_ref().and_then(Value::as_f64), Some(2026.0));
        assert_eq!(
            m.reason.as_deref(),
            Some("testregeling_aanbod: 'aangeboden' is false")
        );
        assert!(m.trace_text.is_some());
    }

    #[test]
    fn all_conditions_true_is_possible() {
        let m = determine_with(
            true,
            json!({"bevoegd": true, "aanvraagfeit": true, "jaar": 2026, "registerdatum": "2026-01-01"}),
        );
        assert_eq!(m.verdict, Verdict::Possible, "{m:?}");
        assert_eq!(m.value, Some(json!(true)));
        assert_eq!(m.reason, None);
    }

    /// A condition that misses a fact makes the offer undeterminable, even if
    /// that fact would later come from the application. (The runtime does not
    /// start with such an offer; see the check in `origin`.)
    #[test]
    fn a_missing_fact_makes_the_offer_undeterminable() {
        let m = determine_with(
            true,
            json!({"bevoegd": true, "jaar": 2026, "registerdatum": "2026-01-01"}),
        );
        assert_eq!(m.verdict, Verdict::Undeterminable, "{m:?}");
        assert_eq!(m.value, None);
        assert_eq!(m.missing, ["aanvraagfeit"]);
        assert_eq!(
            m.reason.as_deref(),
            Some("undeterminable: missing aanvraagfeit")
        );
        let m = determine_with(true, json!({"jaar": 2026, "registerdatum": "2026-01-01"}));
        assert_eq!(m.verdict, Verdict::Undeterminable, "{m:?}");
        assert!(m.reason.as_deref().unwrap().contains("bevoegd"), "{m:?}");
    }

    /// The windows from the policy: a list from a run without parameters; an
    /// output that is not a list is an error.
    #[test]
    fn windows_from_a_run() {
        const POLICY: &str = r#"
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
        s.load_law(POLICY).unwrap();
        assert_eq!(
            windows(&s, "testbeleid_tijdvakken", "jaren", "2026-09-25").unwrap(),
            [json!(2026), json!(2027)]
        );
        let error = windows(&s, "testbeleid_tijdvakken", "een_jaar", "2026-09-25").unwrap_err();
        assert!(error.contains("is not a list"), "{error}");
    }

    #[test]
    fn without_deadline() {
        let m = determine_with(
            false,
            json!({"bevoegd": true, "aanvraagfeit": true, "jaar": 2026}),
        );
        assert_eq!(m.deadline, None);
        assert_eq!(m.verdict, Verdict::Possible, "{m:?}");
    }

    /// What the deadline is missing does not count for the verdict on the output.
    #[test]
    fn what_the_deadline_misses_does_not_count() {
        let m = determine_with(
            true,
            json!({"bevoegd": false, "aanvraagfeit": true, "jaar": 2026}),
        );
        assert_eq!(m.verdict, Verdict::Excluded, "{m:?}");
        assert_eq!(m.deadline, None);
        assert!(m.missing.is_empty());
    }
}
