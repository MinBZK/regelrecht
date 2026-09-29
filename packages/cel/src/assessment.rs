//! The assessment: passing a lexostatus as parameters to an article and
//! having the engine evaluate an output.
//!
//! For a requested output the engine executes the whole article: every
//! action and every input, including what that output does not need. A
//! parameter or input the lexostatus does not deliver then makes the output
//! impossible to judge. The assessment reports what is missing and fills in
//! nothing: an invented value would suggest a fact nobody recorded.

use std::collections::BTreeMap;

use regelrecht_engine::{EngineError, LawExecutionService, Value as EngineValue};
use serde::Serialize;
use serde_json::Value;

/// The result of an assessment.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Outcome {
    pub regulation: String,
    pub output: String,
    /// Whether the engine gave a value.
    pub to_assess: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<Value>,
    /// What the engine was missing, if it named it.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub missing: Vec<String>,
    /// Why it cannot be judged, in words.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// The parameters from the lexostatus that say something is absent
    /// (see [`crate::reduction::absent`]). Independent of the output: an
    /// assessment that cannot be judged also names what the application
    /// lacks.
    pub absent: Vec<String>,
    /// The trace of the engine run, as text.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_text: Option<String>,
}

/// What the engine made of one or more outputs.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Evaluation {
    /// The outputs with a value without unknown facts.
    pub values: BTreeMap<String, Value>,
    /// What the engine was missing, without duplicates, in the order of the outputs.
    pub missing: Vec<String>,
    /// What the engine was missing, per output without a value. If the run
    /// broke off on a missing fact, every requested output misses that fact.
    pub missing_per: BTreeMap<String, Vec<String>>,
    /// Why an output got no value, if the engine did not name it as a
    /// missing fact.
    pub error: Option<String>,
    /// The trace of the engine run as text, if it was requested.
    pub trace_text: Option<String>,
}

impl Evaluation {
    /// Whether every requested output has a value.
    pub fn complete(&self, outputs: &[&str]) -> bool {
        self.error.is_none()
            && self.missing.is_empty()
            && outputs.iter().all(|u| self.values.contains_key(*u))
    }

    /// What `output` was missing; empty if it missed nothing.
    pub fn missing_of(&self, output: &str) -> &[String] {
        self.missing_per.get(output).map_or(&[], Vec::as_slice)
    }

    /// Why not complete, in words; `prefix` is for example "cannot be
    /// judged".
    pub fn reason(&self, prefix: &str) -> String {
        match (&self.error, self.missing.is_empty()) {
            (_, false) => format!("{prefix}: missing {}", self.missing.join(", ")),
            (Some(f), true) => format!("{prefix}: {f}"),
            (None, true) => prefix.to_string(),
        }
    }
}

/// Has the engine evaluate `outputs` of `regulation` with these parameters
/// on `date` (YYYY-MM-DD). Nothing is filled in: an output with an unknown
/// fact has no value, and that fact is in `missing`.
pub fn evaluate(
    service: &LawExecutionService,
    regulation: &str,
    outputs: &[&str],
    parameters: &BTreeMap<String, Value>,
    date: &str,
) -> Evaluation {
    evaluate_as(service, regulation, outputs, parameters, date, false)
}

/// Like [`evaluate`], with the engine's trace included (for whoever wants to
/// see how the output came about).
pub fn evaluate_with_trace(
    service: &LawExecutionService,
    regulation: &str,
    outputs: &[&str],
    parameters: &BTreeMap<String, Value>,
    date: &str,
) -> Evaluation {
    evaluate_as(service, regulation, outputs, parameters, date, true)
}

fn evaluate_as(
    service: &LawExecutionService,
    regulation: &str,
    outputs: &[&str],
    parameters: &BTreeMap<String, Value>,
    date: &str,
    with_trace: bool,
) -> Evaluation {
    let mut out = Evaluation::default();
    let input: BTreeMap<String, EngineValue> = parameters
        .iter()
        .map(|(k, v)| (k.clone(), EngineValue::from(v)))
        .collect();
    let result = if with_trace {
        service.evaluate_law_with_trace(regulation, outputs, input, date)
    } else {
        service.evaluate_law(regulation, outputs, input, date)
    };
    match result {
        Ok(result) => {
            // The same text rendering as the editor: the engine's
            // box-drawing trace.
            out.trace_text = result.trace.as_ref().map(|t| t.render_box_drawing());
            for u in outputs {
                match result.outputs.get(*u) {
                    Some(w) if w.contains_unknown() => {
                        let own = out.missing_per.entry((*u).to_string()).or_default();
                        for f in w.missing_facts() {
                            if !own.contains(&f.name) {
                                own.push(f.name.clone());
                            }
                            if !out.missing.contains(&f.name) {
                                out.missing.push(f.name.clone());
                            }
                        }
                    }
                    Some(w) => match serde_json::to_value(w) {
                        Ok(v) => {
                            out.values.insert((*u).to_string(), v);
                        }
                        Err(e) => {
                            out.error.get_or_insert_with(|| {
                                format!("the value of '{u}' cannot be read as JSON: {e}")
                            });
                        }
                    },
                    None => {
                        out.error
                            .get_or_insert_with(|| format!("the engine gave no value for '{u}'"));
                    }
                }
            }
        }
        Err(e) => {
            let (missing, reason) = declare(&e);
            match missing {
                Some(m) => {
                    for u in outputs {
                        out.missing_per.insert((*u).to_string(), vec![m.clone()]);
                    }
                    out.missing.push(m);
                }
                None => out.error = Some(reason),
            }
        }
    }
    out
}

/// Evaluates `output` of `regulation` with these parameters on `date`
/// (YYYY-MM-DD). `absent` goes along unchanged in the result.
pub fn assessment(
    service: &LawExecutionService,
    regulation: &str,
    output: &str,
    parameters: &BTreeMap<String, Value>,
    absent: Vec<String>,
    date: &str,
) -> Outcome {
    let e = evaluate_with_trace(service, regulation, &[output], parameters, date);
    let to_assess = e.complete(&[output]);
    Outcome {
        regulation: regulation.to_string(),
        output: output.to_string(),
        to_assess,
        value: e.values.get(output).cloned(),
        reason: (!to_assess).then(|| e.reason("cannot be judged")),
        missing: e.missing,
        absent,
        trace_text: e.trace_text,
    }
}

/// The missing name in an engine error, if it names one.
fn declare(e: &EngineError) -> (Option<String>, String) {
    match e {
        EngineError::TracedError { source, .. } => declare(source),
        EngineError::VariableNotFound(name) => (Some(name.clone()), e.to_string()),
        EngineError::MissingParameter { name, .. } => (Some(name.clone()), e.to_string()),
        // A null where the regulation allows none: that fact is not there either.
        EngineError::NullForNonNullable { field, .. } => (Some(field.clone()), e.to_string()),
        other => (None, other.to_string()),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::path::Path;

    fn service() -> LawExecutionService {
        crate::regulations::load(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/regulation"),
        )
        .unwrap()
        .service
    }

    fn complete() -> BTreeMap<String, Value> {
        serde_json::from_value(json!({
            "aanvraagjaar": 2025, "aanvraagdatum": "2025-03-12",
            "bevat_naam": true, "bevat_aanduiding": true, "bevat_naam_orgaan": true,
            "bevat_aantal_zetels": true, "is_samengevoegd": false,
            "bevat_aantal_aanduidingen": true, "registratie_categorie_a": false,
        }))
        .unwrap()
    }

    #[test]
    fn complete_application() {
        let u = assessment(
            &service(),
            "testregeling_aanvraag",
            "aanvraag_volledig",
            &complete(),
            Vec::new(),
            "2025-03-12",
        );
        assert!(u.to_assess, "{u:?}");
        assert_eq!(u.value, Some(json!(true)));
    }

    #[test]
    fn incomplete_application() {
        let mut p = complete();
        p.insert("bevat_aanduiding".into(), json!(false));
        let u = assessment(
            &service(),
            "testregeling_aanvraag",
            "aanvraag_volledig",
            &p,
            vec!["bevat_aanduiding".into()],
            "2025-03-12",
        );
        assert_eq!(u.value, Some(json!(false)));
        assert_eq!(u.absent, vec!["bevat_aanduiding"]);
    }

    #[test]
    fn missing_parameter_cannot_be_judged() {
        let mut p = complete();
        p.remove("aanvraagjaar");
        let u = assessment(
            &service(),
            "testregeling_aanvraag",
            "aanvraag_volledig",
            &p,
            Vec::new(),
            "2025-03-12",
        );
        assert!(!u.to_assess);
        assert_eq!(u.missing, vec!["aanvraagjaar"]);
        assert_eq!(
            u.reason.as_deref(),
            Some("cannot be judged: missing aanvraagjaar")
        );
    }

    #[test]
    fn missing_optional_parameter_makes_the_output_unknown() {
        let mut p = complete();
        p.insert("is_samengevoegd".into(), json!(true));
        p.remove("bevat_aantal_aanduidingen");
        let u = assessment(
            &service(),
            "testregeling_aanvraag",
            "aanvraag_volledig",
            &p,
            Vec::new(),
            "2025-03-12",
        );
        assert!(!u.to_assess);
        assert_eq!(u.missing, vec!["bevat_aantal_aanduidingen"]);
    }

    /// The engine finding: for a requested output the engine executes the
    /// whole article. `aanvraag_compleet` depends on only two parameters,
    /// but the article also has an input from another regulation and a
    /// parameter for another output; without those facts `aanvraag_compleet`
    /// cannot be judged either.
    #[test]
    fn engine_requires_the_whole_article_for_an_output() {
        let s = service();
        let p: BTreeMap<String, Value> =
            serde_json::from_value(json!({"bevat_naam": true, "bevat_aanduiding": true})).unwrap();
        let u = assessment(
            &s,
            "testregeling_aanvraag",
            "aanvraag_compleet",
            &p,
            Vec::new(),
            "2025-03-12",
        );
        assert!(!u.to_assess, "{u:?}");
        assert_eq!(u.missing, vec!["verzuimdagen"]);

        let mut p2 = p.clone();
        p2.insert("verzuimdagen".into(), json!(0));
        let u = assessment(
            &s,
            "testregeling_aanvraag",
            "aanvraag_compleet",
            &p2,
            Vec::new(),
            "2025-03-12",
        );
        assert_eq!(u.missing, vec!["eigen_feit_behandelaar"]);

        p2.insert("eigen_feit_behandelaar".into(), json!(false));
        let u = assessment(
            &s,
            "testregeling_aanvraag",
            "aanvraag_compleet",
            &p2,
            Vec::new(),
            "2025-03-12",
        );
        assert_eq!(u.value, Some(json!(true)));
    }

    #[test]
    fn unknown_regulation() {
        let u = assessment(
            &service(),
            "bestaat_niet",
            "x",
            &BTreeMap::new(),
            Vec::new(),
            "2025-03-12",
        );
        assert!(!u.to_assess);
        assert!(u.reason.unwrap().contains("bestaat_niet"));
    }
}
