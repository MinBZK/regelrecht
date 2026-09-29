//! De toets: een lexostatus als parameters aan een artikel geven en de
//! engine een uitkomst laten evalueren.
//!
//! De engine voert bij een gevraagde uitkomst het hele artikel uit: elke
//! actie en elke invoer, ook wat die uitkomst niet nodig heeft. Een
//! parameter of invoer die de lexostatus niet levert, maakt de uitkomst dan
//! niet te beoordelen. De toets meldt wat er mist en vult niets aan: een
//! verzonnen waarde zou een feit suggereren dat niemand heeft vastgelegd.

use std::collections::BTreeMap;

use regelrecht_engine::{EngineError, LawExecutionService, Value as EngineValue};
use serde::Serialize;
use serde_json::Value;

/// De uitslag van een toets.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Uitslag {
    pub regulation: String,
    pub output: String,
    /// Of de engine een waarde gaf.
    pub to_assess: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<Value>,
    /// Wat de engine miste, als hij dat noemde.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub missing: Vec<String>,
    /// Waarom niet te beoordelen, in woorden.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// De parameters uit de lexostatus die zeggen dat iets ontbreekt
    /// (zie [`crate::reductie::ontbreekt`]). Staat los van de uitkomst: ook
    /// een niet te beoordelen toets noemt wat er aan de aanvraag ontbreekt.
    pub absent: Vec<String>,
    /// De trace van de engine-run, als tekst.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_text: Option<String>,
}

/// Wat de engine van een of meer uitkomsten maakte.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Evaluatie {
    /// De uitkomsten met een waarde zonder onbekende feiten.
    pub waarden: BTreeMap<String, Value>,
    /// Wat de engine miste, zonder dubbelen, in de volgorde van de uitkomsten.
    pub missing: Vec<String>,
    /// Wat de engine miste, per uitkomst zonder waarde. Brak de run af op een
    /// ontbrekend feit, dan mist elke gevraagde uitkomst dat feit.
    pub mist_per: BTreeMap<String, Vec<String>>,
    /// Waarom een uitkomst geen waarde kreeg, als de engine dat niet als
    /// ontbrekend feit noemde.
    pub error: Option<String>,
    /// De trace van de engine-run als tekst, als die gevraagd was.
    pub trace_text: Option<String>,
}

impl Evaluatie {
    /// Of elke gevraagde uitkomst een waarde heeft.
    pub fn volledig(&self, outputs: &[&str]) -> bool {
        self.error.is_none()
            && self.missing.is_empty()
            && outputs.iter().all(|u| self.waarden.contains_key(*u))
    }

    /// Wat `uitkomst` miste; leeg als ze niets miste.
    pub fn mist_van(&self, output: &str) -> &[String] {
        self.mist_per.get(output).map_or(&[], Vec::as_slice)
    }

    /// Waarom niet volledig, in woorden; `voorvoegsel` is bijvoorbeeld "niet
    /// te beoordelen".
    pub fn reason(&self, voorvoegsel: &str) -> String {
        match (&self.error, self.missing.is_empty()) {
            (_, false) => format!("{voorvoegsel}: mist {}", self.missing.join(", ")),
            (Some(f), true) => format!("{voorvoegsel}: {f}"),
            (None, true) => voorvoegsel.to_string(),
        }
    }
}

/// Laat de engine `uitkomsten` van `regeling` evalueren met deze parameters
/// op `datum` (JJJJ-MM-DD). Er wordt niets aangevuld: een uitkomst met een
/// onbekend feit heeft geen waarde, en dat feit staat in `mist`.
pub fn evalueer(
    service: &LawExecutionService,
    regulation: &str,
    outputs: &[&str],
    parameters: &BTreeMap<String, Value>,
    date: &str,
) -> Evaluatie {
    evalueer_als(service, regulation, outputs, parameters, date, false)
}

/// Als [`evalueer`], met de trace van de engine erbij (voor wie wil zien hoe
/// de uitkomst tot stand kwam).
pub fn evalueer_met_trace(
    service: &LawExecutionService,
    regulation: &str,
    outputs: &[&str],
    parameters: &BTreeMap<String, Value>,
    date: &str,
) -> Evaluatie {
    evalueer_als(service, regulation, outputs, parameters, date, true)
}

fn evalueer_als(
    service: &LawExecutionService,
    regulation: &str,
    outputs: &[&str],
    parameters: &BTreeMap<String, Value>,
    date: &str,
    met_trace: bool,
) -> Evaluatie {
    let mut uit = Evaluatie::default();
    let input: BTreeMap<String, EngineValue> = parameters
        .iter()
        .map(|(k, v)| (k.clone(), EngineValue::from(v)))
        .collect();
    let resultaat = if met_trace {
        service.evaluate_law_with_trace(regulation, outputs, input, date)
    } else {
        service.evaluate_law(regulation, outputs, input, date)
    };
    match resultaat {
        Ok(resultaat) => {
            // Dezelfde tekstweergave als de editor: de box-drawing-trace van
            // de engine.
            uit.trace_text = resultaat.trace.as_ref().map(|t| t.render_box_drawing());
            for u in outputs {
                match resultaat.outputs.get(*u) {
                    Some(w) if w.contains_unknown() => {
                        let eigen = uit.mist_per.entry((*u).to_string()).or_default();
                        for f in w.missing_facts() {
                            if !eigen.contains(&f.name) {
                                eigen.push(f.name.clone());
                            }
                            if !uit.missing.contains(&f.name) {
                                uit.missing.push(f.name.clone());
                            }
                        }
                    }
                    Some(w) => match serde_json::to_value(w) {
                        Ok(v) => {
                            uit.waarden.insert((*u).to_string(), v);
                        }
                        Err(e) => {
                            uit.error.get_or_insert_with(|| {
                                format!("de waarde van '{u}' is niet als JSON te lezen: {e}")
                            });
                        }
                    },
                    None => {
                        uit.error
                            .get_or_insert_with(|| format!("de engine gaf geen waarde voor '{u}'"));
                    }
                }
            }
        }
        Err(e) => {
            let (missing, reason) = verklaar(&e);
            match missing {
                Some(m) => {
                    for u in outputs {
                        uit.mist_per.insert((*u).to_string(), vec![m.clone()]);
                    }
                    uit.missing.push(m);
                }
                None => uit.error = Some(reason),
            }
        }
    }
    uit
}

/// Evalueer `uitkomst` van `regeling` met deze parameters op `datum`
/// (JJJJ-MM-DD). `ontbreekt` gaat ongewijzigd mee in de uitslag.
pub fn assessment(
    service: &LawExecutionService,
    regulation: &str,
    output: &str,
    parameters: &BTreeMap<String, Value>,
    absent: Vec<String>,
    date: &str,
) -> Uitslag {
    let e = evalueer_met_trace(service, regulation, &[output], parameters, date);
    let to_assess = e.volledig(&[output]);
    Uitslag {
        regulation: regulation.to_string(),
        output: output.to_string(),
        to_assess,
        value: e.waarden.get(output).cloned(),
        reason: (!to_assess).then(|| e.reason("niet te beoordelen")),
        missing: e.missing,
        absent,
        trace_text: e.trace_text,
    }
}

/// De ontbrekende naam in een enginefout, als die er een noemt.
fn verklaar(e: &EngineError) -> (Option<String>, String) {
    match e {
        EngineError::TracedError { source, .. } => verklaar(source),
        EngineError::VariableNotFound(name) => (Some(name.clone()), e.to_string()),
        EngineError::MissingParameter { name, .. } => (Some(name.clone()), e.to_string()),
        // Een null waar de regeling er geen toestaat: ook dat feit is er niet.
        EngineError::NullForNonNullable { field, .. } => (Some(field.clone()), e.to_string()),
        ander => (None, ander.to_string()),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::path::Path;

    fn service() -> LawExecutionService {
        crate::regelingen::laad(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/regulation"),
        )
        .unwrap()
        .service
    }

    fn volledig() -> BTreeMap<String, Value> {
        serde_json::from_value(json!({
            "aanvraagjaar": 2025, "aanvraagdatum": "2025-03-12",
            "bevat_naam": true, "bevat_aanduiding": true, "bevat_naam_orgaan": true,
            "bevat_aantal_zetels": true, "is_samengevoegd": false,
            "bevat_aantal_aanduidingen": true, "registratie_categorie_a": false,
        }))
        .unwrap()
    }

    #[test]
    fn volledige_aanvraag() {
        let u = assessment(
            &service(),
            "testregeling_aanvraag",
            "aanvraag_volledig",
            &volledig(),
            Vec::new(),
            "2025-03-12",
        );
        assert!(u.to_assess, "{u:?}");
        assert_eq!(u.value, Some(json!(true)));
    }

    #[test]
    fn onvolledige_aanvraag() {
        let mut p = volledig();
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
    fn ontbrekende_parameter_is_niet_te_beoordelen() {
        let mut p = volledig();
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
            Some("niet te beoordelen: mist aanvraagjaar")
        );
    }

    #[test]
    fn niet_verplichte_parameter_die_ontbreekt_maakt_de_uitkomst_onbekend() {
        let mut p = volledig();
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

    /// De engine-bevinding: bij een gevraagde uitkomst voert de engine het
    /// hele artikel uit. `aanvraag_compleet` hangt alleen van twee
    /// parameters af, maar het artikel heeft ook een invoer uit een andere
    /// regeling en een parameter voor een andere uitkomst; zonder die feiten
    /// is ook `aanvraag_compleet` niet te beoordelen.
    #[test]
    fn engine_eist_het_hele_artikel_bij_een_uitkomst() {
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
    fn onbekende_regeling() {
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
