//! Voorbeelden: standaardgegevens per handeling, voor een proefopstelling.
//!
//! `cel.yaml` kan per handeling een JSON-bestand noemen (zie
//! [`crate::config::VoorbeeldenDefinitie`]): logins voor de nep-eHerkenning,
//! een aanvraag en een besluitformulier. De frontend biedt ze aan om een
//! formulier voor in te vullen of de handeling er direct mee te doen. Het
//! zijn gewone invoer, geen feiten: de runtime legt ze niet vast en de
//! handeling toetst ze zoals elke andere invoer.

use std::path::Path;

use serde::Serialize;
use serde_json::{Map, Value};

use crate::config::VoorbeeldenDefinitie;
use crate::eherkenning::Login;

/// De geladen voorbeelden van een cel. Zonder blok `voorbeelden`: leeg.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Voorbeelden {
    pub inloggen: Vec<InlogVoorbeeld>,
    /// De `external` van een aanvraag.
    pub aanvraag: Option<Map<String, Value>>,
    /// Het `formulier` van een besluit.
    pub besluit: Option<Map<String, Value>>,
}

/// Een login, met als label de bestandsnaam zonder extensie.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InlogVoorbeeld {
    pub label: String,
    pub kvk: String,
    pub persoon: String,
}

/// Lees de voorbeelden; paden zijn relatief aan de map van de cel. Elke fout
/// komt terug en noemt het pad.
pub fn laad(map: &Path, definitie: &VoorbeeldenDefinitie) -> Result<Voorbeelden, Vec<String>> {
    let mut fouten = Vec::new();
    let mut uit = Voorbeelden::default();
    // Het label is de sleutel waarmee de frontend een login kiest: uniek.
    let mut labels: Vec<(String, &str)> = Vec::new();
    for pad in &definitie.inloggen {
        match inlog(map, pad) {
            Ok(v) => {
                if let Some((_, eerder)) = labels.iter().find(|(l, _)| *l == v.label) {
                    fouten.push(format!(
                        "voorbeelden {eerder} en {pad} hebben hetzelfde label '{}'",
                        v.label
                    ));
                    continue;
                }
                labels.push((v.label.clone(), pad));
                uit.inloggen.push(v);
            }
            Err(f) => fouten.push(f),
        }
    }
    if let Some(pad) = &definitie.aanvraag {
        uit.aanvraag = object_onder(map, pad, "external")
            .map_err(|f| fouten.push(f))
            .ok();
    }
    if let Some(pad) = &definitie.besluit {
        uit.besluit = object_onder(map, pad, "formulier")
            .map_err(|f| fouten.push(f))
            .ok();
    }
    if fouten.is_empty() {
        Ok(uit)
    } else {
        Err(fouten)
    }
}

fn lees(map: &Path, pad: &str) -> Result<Value, String> {
    let tekst =
        std::fs::read_to_string(map.join(pad)).map_err(|e| format!("voorbeeld {pad}: {e}"))?;
    serde_json::from_str(&tekst).map_err(|e| format!("voorbeeld {pad}: geen geldige JSON: {e}"))
}

/// Een login zoals de nep-eHerkenning haar aanneemt, en geldig.
fn inlog(map: &Path, pad: &str) -> Result<InlogVoorbeeld, String> {
    let login: Login = serde_json::from_value(lees(map, pad)?)
        .map_err(|e| format!("voorbeeld {pad}: geen login met kvk en persoon: {e}"))?;
    let sessie = login
        .valideer()
        .map_err(|e| format!("voorbeeld {pad}: {e}"))?;
    let label = Path::new(pad)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| pad.to_string());
    Ok(InlogVoorbeeld {
        label,
        kvk: sessie.kvk,
        persoon: sessie.persoon,
    })
}

/// Het object onder `sleutel` in een JSON-object.
fn object_onder(map: &Path, pad: &str, sleutel: &str) -> Result<Map<String, Value>, String> {
    match lees(map, pad)? {
        Value::Object(mut o) => match o.remove(sleutel) {
            Some(Value::Object(inhoud)) => Ok(inhoud),
            _ => Err(format!(
                "voorbeeld {pad}: verwacht een object met '{sleutel}' als object"
            )),
        },
        _ => Err(format!(
            "voorbeeld {pad}: verwacht een object met '{sleutel}' als object"
        )),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn map_met(bestanden: &[(&str, &str)]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        for (naam, inhoud) in bestanden {
            std::fs::write(dir.path().join(naam), inhoud).unwrap();
        }
        dir
    }

    fn definitie(
        inloggen: &[&str],
        aanvraag: Option<&str>,
        besluit: Option<&str>,
    ) -> VoorbeeldenDefinitie {
        VoorbeeldenDefinitie {
            inloggen: inloggen.iter().map(|s| s.to_string()).collect(),
            aanvraag: aanvraag.map(str::to_string),
            besluit: besluit.map(str::to_string),
        }
    }

    #[test]
    fn geldige_voorbeelden() {
        let dir = map_met(&[
            (
                "login-een.json",
                r#"{"kvk": " 12345678 ", "persoon": "A. Tester"}"#,
            ),
            ("aanvraag.json", r#"{"external": {"naam": "Voorbeeld"}}"#),
            (
                "besluit.json",
                r#"{"formulier": {"feiten_vergaard": true}}"#,
            ),
        ]);
        let v = laad(
            dir.path(),
            &definitie(
                &["login-een.json"],
                Some("aanvraag.json"),
                Some("besluit.json"),
            ),
        )
        .unwrap();
        assert_eq!(
            v.inloggen,
            [InlogVoorbeeld {
                label: "login-een".into(),
                kvk: "12345678".into(),
                persoon: "A. Tester".into(),
            }]
        );
        assert_eq!(v.aanvraag.unwrap()["naam"], "Voorbeeld");
        assert_eq!(v.besluit.unwrap()["feiten_vergaard"], true);
    }

    #[test]
    fn zonder_voorbeelden_is_alles_leeg() {
        let dir = map_met(&[]);
        let v = laad(dir.path(), &VoorbeeldenDefinitie::default()).unwrap();
        assert!(v.inloggen.is_empty() && v.aanvraag.is_none() && v.besluit.is_none());
    }

    #[test]
    fn ontbrekend_bestand_noemt_het_pad() {
        let dir = map_met(&[]);
        let fouten = laad(
            dir.path(),
            &definitie(&["weg.json"], Some("ook-weg.json"), None),
        )
        .unwrap_err();
        assert_eq!(fouten.len(), 2, "{fouten:?}");
        assert!(fouten[0].contains("weg.json"), "{fouten:?}");
        assert!(fouten[1].contains("ook-weg.json"), "{fouten:?}");
    }

    #[test]
    fn verkeerde_vorm_wordt_geweigerd() {
        let dir = map_met(&[
            ("geen-json.json", "{"),
            ("zonder-persoon.json", r#"{"kvk": "12345678"}"#),
            ("korte-kvk.json", r#"{"kvk": "123", "persoon": "A"}"#),
            ("aanvraag.json", r#"{"formulier": {}}"#),
            ("besluit.json", r#"[{"formulier": {}}]"#),
        ]);
        let fouten = laad(
            dir.path(),
            &definitie(
                &["geen-json.json", "zonder-persoon.json", "korte-kvk.json"],
                Some("aanvraag.json"),
                Some("besluit.json"),
            ),
        )
        .unwrap_err();
        assert_eq!(fouten.len(), 5, "{fouten:?}");
        assert!(
            fouten[0].contains("geen-json.json: geen geldige JSON"),
            "{fouten:?}"
        );
        assert!(
            fouten[1].contains("zonder-persoon.json: geen login"),
            "{fouten:?}"
        );
        assert!(
            fouten[2].contains("korte-kvk.json: een KvK-nummer"),
            "{fouten:?}"
        );
        assert!(
            fouten[3].contains("aanvraag.json: verwacht een object met 'external'"),
            "{fouten:?}"
        );
        assert!(
            fouten[4].contains("besluit.json: verwacht een object met 'formulier'"),
            "{fouten:?}"
        );
    }

    #[test]
    fn twee_logins_met_hetzelfde_label() {
        let dir = map_met(&[("login.json", r#"{"kvk": "12345678", "persoon": "A"}"#)]);
        std::fs::create_dir_all(dir.path().join("ander")).unwrap();
        std::fs::write(
            dir.path().join("ander/login.json"),
            r#"{"kvk": "87654321", "persoon": "B"}"#,
        )
        .unwrap();
        let fouten = laad(
            dir.path(),
            &definitie(&["login.json", "ander/login.json"], None, None),
        )
        .unwrap_err();
        assert_eq!(fouten.len(), 1, "{fouten:?}");
        assert!(fouten[0].contains("login.json"), "{fouten:?}");
        assert!(fouten[0].contains("ander/login.json"), "{fouten:?}");
        assert!(fouten[0].contains("label 'login'"), "{fouten:?}");
    }
}
