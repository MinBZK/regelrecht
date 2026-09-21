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
    pub regeling: String,
    pub uitkomst: String,
    /// Of de engine een waarde gaf.
    pub te_beoordelen: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub waarde: Option<Value>,
    /// Wat de engine miste, als hij dat noemde.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub mist: Vec<String>,
    /// Waarom niet te beoordelen, in woorden.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reden: Option<String>,
    /// De parameters uit de lexostatus die onwaar zijn.
    pub onwaar: Vec<String>,
}

/// Evalueer `uitkomst` van `regeling` met deze parameters op `datum`
/// (JJJJ-MM-DD).
pub fn toets(
    service: &LawExecutionService,
    regeling: &str,
    uitkomst: &str,
    parameters: &BTreeMap<String, Value>,
    datum: &str,
) -> Uitslag {
    let onwaar = parameters
        .iter()
        .filter(|(_, w)| **w == Value::Bool(false))
        .map(|(k, _)| k.clone())
        .collect();
    let mut uitslag = Uitslag {
        regeling: regeling.to_string(),
        uitkomst: uitkomst.to_string(),
        te_beoordelen: false,
        waarde: None,
        mist: Vec::new(),
        reden: None,
        onwaar,
    };
    let invoer: BTreeMap<String, EngineValue> = parameters
        .iter()
        .map(|(k, v)| (k.clone(), EngineValue::from(v)))
        .collect();
    match service.evaluate_law(regeling, &[uitkomst], invoer, datum) {
        Ok(resultaat) => match resultaat.outputs.get(uitkomst) {
            Some(w) if w.contains_unknown() => {
                let mut mist: Vec<String> =
                    w.missing_facts().iter().map(|f| f.name.clone()).collect();
                mist.dedup();
                uitslag.reden = Some(niet_te_beoordelen(&mist));
                uitslag.mist = mist;
            }
            Some(w) => {
                uitslag.te_beoordelen = true;
                uitslag.waarde = serde_json::to_value(w).ok();
            }
            None => uitslag.reden = Some(format!("de engine gaf geen waarde voor '{uitkomst}'")),
        },
        Err(e) => {
            let (mist, reden) = verklaar(&e);
            uitslag.reden = Some(match &mist {
                Some(m) => niet_te_beoordelen(std::slice::from_ref(m)),
                None => format!("niet te beoordelen: {reden}"),
            });
            uitslag.mist = mist.into_iter().collect();
        }
    }
    uitslag
}

fn niet_te_beoordelen(mist: &[String]) -> String {
    if mist.is_empty() {
        "niet te beoordelen".to_string()
    } else {
        format!("niet te beoordelen: mist {}", mist.join(", "))
    }
}

/// De ontbrekende naam in een enginefout, als die er een noemt.
fn verklaar(e: &EngineError) -> (Option<String>, String) {
    match e {
        EngineError::TracedError { source, .. } => verklaar(source),
        EngineError::VariableNotFound(naam) => (Some(naam.clone()), e.to_string()),
        EngineError::MissingParameter { name, .. } => (Some(name.clone()), e.to_string()),
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
        let u = toets(
            &service(),
            "testregeling_aanvraag",
            "aanvraag_volledig",
            &volledig(),
            "2025-03-12",
        );
        assert!(u.te_beoordelen, "{u:?}");
        assert_eq!(u.waarde, Some(json!(true)));
    }

    #[test]
    fn onvolledige_aanvraag_noemt_wat_onwaar_is() {
        let mut p = volledig();
        p.insert("bevat_aanduiding".into(), json!(false));
        let u = toets(
            &service(),
            "testregeling_aanvraag",
            "aanvraag_volledig",
            &p,
            "2025-03-12",
        );
        assert_eq!(u.waarde, Some(json!(false)));
        assert!(u.onwaar.contains(&"bevat_aanduiding".to_string()));
    }

    #[test]
    fn ontbrekende_parameter_is_niet_te_beoordelen() {
        let mut p = volledig();
        p.remove("aanvraagjaar");
        let u = toets(
            &service(),
            "testregeling_aanvraag",
            "aanvraag_volledig",
            &p,
            "2025-03-12",
        );
        assert!(!u.te_beoordelen);
        assert_eq!(u.mist, vec!["aanvraagjaar"]);
        assert_eq!(
            u.reden.as_deref(),
            Some("niet te beoordelen: mist aanvraagjaar")
        );
    }

    #[test]
    fn niet_verplichte_parameter_die_ontbreekt_maakt_de_uitkomst_onbekend() {
        let mut p = volledig();
        p.insert("is_samengevoegd".into(), json!(true));
        p.remove("bevat_aantal_aanduidingen");
        let u = toets(
            &service(),
            "testregeling_aanvraag",
            "aanvraag_volledig",
            &p,
            "2025-03-12",
        );
        assert!(!u.te_beoordelen);
        assert_eq!(u.mist, vec!["bevat_aantal_aanduidingen"]);
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
        let u = toets(
            &s,
            "testregeling_aanvraag",
            "aanvraag_compleet",
            &p,
            "2025-03-12",
        );
        assert!(!u.te_beoordelen, "{u:?}");
        assert_eq!(u.mist, vec!["verzuimdagen"]);

        let mut p2 = p.clone();
        p2.insert("verzuimdagen".into(), json!(0));
        let u = toets(
            &s,
            "testregeling_aanvraag",
            "aanvraag_compleet",
            &p2,
            "2025-03-12",
        );
        assert_eq!(u.mist, vec!["eigen_feit_behandelaar"]);

        p2.insert("eigen_feit_behandelaar".into(), json!(false));
        let u = toets(
            &s,
            "testregeling_aanvraag",
            "aanvraag_compleet",
            &p2,
            "2025-03-12",
        );
        assert_eq!(u.waarde, Some(json!(true)));
    }

    #[test]
    fn onbekende_regeling() {
        let u = toets(
            &service(),
            "bestaat_niet",
            "x",
            &BTreeMap::new(),
            "2025-03-12",
        );
        assert!(!u.te_beoordelen);
        assert!(u.reden.unwrap().contains("bestaat_niet"));
    }
}
