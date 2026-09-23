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
    /// De parameters uit de lexostatus die zeggen dat iets ontbreekt
    /// (zie [`crate::reductie::ontbreekt`]). Staat los van de uitkomst: ook
    /// een niet te beoordelen toets noemt wat er aan de aanvraag ontbreekt.
    pub ontbreekt: Vec<String>,
}

/// Wat de engine van een of meer uitkomsten maakte.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Evaluatie {
    /// De uitkomsten met een waarde zonder onbekende feiten.
    pub waarden: BTreeMap<String, Value>,
    /// Wat de engine miste, zonder dubbelen, in de volgorde van de uitkomsten.
    pub mist: Vec<String>,
    /// Waarom een uitkomst geen waarde kreeg, als de engine dat niet als
    /// ontbrekend feit noemde.
    pub fout: Option<String>,
}

impl Evaluatie {
    /// Of elke gevraagde uitkomst een waarde heeft.
    pub fn volledig(&self, uitkomsten: &[&str]) -> bool {
        self.fout.is_none()
            && self.mist.is_empty()
            && uitkomsten.iter().all(|u| self.waarden.contains_key(*u))
    }

    /// Waarom niet volledig, in woorden; `voorvoegsel` is bijvoorbeeld "niet
    /// te beoordelen".
    pub fn reden(&self, voorvoegsel: &str) -> String {
        match (&self.fout, self.mist.is_empty()) {
            (_, false) => format!("{voorvoegsel}: mist {}", self.mist.join(", ")),
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
    regeling: &str,
    uitkomsten: &[&str],
    parameters: &BTreeMap<String, Value>,
    datum: &str,
) -> Evaluatie {
    let mut uit = Evaluatie::default();
    let invoer: BTreeMap<String, EngineValue> = parameters
        .iter()
        .map(|(k, v)| (k.clone(), EngineValue::from(v)))
        .collect();
    match service.evaluate_law(regeling, uitkomsten, invoer, datum) {
        Ok(resultaat) => {
            for u in uitkomsten {
                match resultaat.outputs.get(*u) {
                    Some(w) if w.contains_unknown() => {
                        for f in w.missing_facts() {
                            if !uit.mist.contains(&f.name) {
                                uit.mist.push(f.name.clone());
                            }
                        }
                    }
                    Some(w) => {
                        if let Ok(v) = serde_json::to_value(w) {
                            uit.waarden.insert((*u).to_string(), v);
                        }
                    }
                    None => {
                        uit.fout
                            .get_or_insert_with(|| format!("de engine gaf geen waarde voor '{u}'"));
                    }
                }
            }
        }
        Err(e) => {
            let (mist, reden) = verklaar(&e);
            match mist {
                Some(m) => uit.mist.push(m),
                None => uit.fout = Some(reden),
            }
        }
    }
    uit
}

/// Evalueer `uitkomst` van `regeling` met deze parameters op `datum`
/// (JJJJ-MM-DD). `ontbreekt` gaat ongewijzigd mee in de uitslag.
pub fn toets(
    service: &LawExecutionService,
    regeling: &str,
    uitkomst: &str,
    parameters: &BTreeMap<String, Value>,
    ontbreekt: Vec<String>,
    datum: &str,
) -> Uitslag {
    let e = evalueer(service, regeling, &[uitkomst], parameters, datum);
    let te_beoordelen = e.volledig(&[uitkomst]);
    Uitslag {
        regeling: regeling.to_string(),
        uitkomst: uitkomst.to_string(),
        te_beoordelen,
        waarde: e.waarden.get(uitkomst).cloned(),
        reden: (!te_beoordelen).then(|| e.reden("niet te beoordelen")),
        mist: e.mist,
        ontbreekt,
    }
}

/// De ontbrekende naam in een enginefout, als die er een noemt.
fn verklaar(e: &EngineError) -> (Option<String>, String) {
    match e {
        EngineError::TracedError { source, .. } => verklaar(source),
        EngineError::VariableNotFound(naam) => (Some(naam.clone()), e.to_string()),
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
        let u = toets(
            &service(),
            "testregeling_aanvraag",
            "aanvraag_volledig",
            &volledig(),
            Vec::new(),
            "2025-03-12",
        );
        assert!(u.te_beoordelen, "{u:?}");
        assert_eq!(u.waarde, Some(json!(true)));
    }

    #[test]
    fn onvolledige_aanvraag() {
        let mut p = volledig();
        p.insert("bevat_aanduiding".into(), json!(false));
        let u = toets(
            &service(),
            "testregeling_aanvraag",
            "aanvraag_volledig",
            &p,
            vec!["bevat_aanduiding".into()],
            "2025-03-12",
        );
        assert_eq!(u.waarde, Some(json!(false)));
        assert_eq!(u.ontbreekt, vec!["bevat_aanduiding"]);
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
            Vec::new(),
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
            Vec::new(),
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
            Vec::new(),
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
            Vec::new(),
            "2025-03-12",
        );
        assert_eq!(u.mist, vec!["eigen_feit_behandelaar"]);

        p2.insert("eigen_feit_behandelaar".into(), json!(false));
        let u = toets(
            &s,
            "testregeling_aanvraag",
            "aanvraag_compleet",
            &p2,
            Vec::new(),
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
            Vec::new(),
            "2025-03-12",
        );
        assert!(!u.te_beoordelen);
        assert!(u.reden.unwrap().contains("bestaat_niet"));
    }
}
