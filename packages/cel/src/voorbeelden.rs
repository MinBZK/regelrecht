//! Voorbeelden: standaardgegevens per handeling, voor een proefopstelling.
//!
//! `proces.yaml` kan per handeling een JSON-bestand noemen (zie
//! [`crate::config::VoorbeeldenDefinitie`]): logins langs een kanaal, een
//! aanvraag en per handeling in een zaak een formulier. De frontend biedt ze
//! aan om een formulier voor in te vullen of de handeling er direct mee te
//! doen. Het zijn gewone invoer, geen feiten: de runtime legt ze niet vast en
//! de handeling toetst ze zoals elke andere invoer.
//!
//! Een waarde `"$vandaag"` in een formulier wordt bij het opvragen de datum
//! van vandaag ([`Voorbeelden::op`]). Een besluit, een bekendmaking of een
//! betaling in de toekomst is geen feit (de cel weigert een `op_moment` na
//! het vastleggen), en een voorbeeld met een vaste datum veroudert.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Serialize;
use serde_json::{Map, Value};

use crate::config::{ProcesDefinitie, VoorbeeldenDefinitie};
use crate::kanaal::Routes;

/// De geladen voorbeelden van een proces. Zonder blok `voorbeelden`: leeg.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Voorbeelden {
    pub inloggen: Vec<InlogVoorbeeld>,
    /// De `external` van een aanvraag.
    pub aanvraag: Option<Map<String, Value>>,
    /// Per handeling het `formulier`.
    pub handelingen: BTreeMap<String, Map<String, Value>>,
}

/// De waarde die in een voorbeeld de datum van vandaag wordt.
pub const VANDAAG: &str = "$vandaag";

impl Voorbeelden {
    /// De voorbeelden zoals de frontend ze krijgt: `"$vandaag"` wordt
    /// `vandaag` (JJJJ-MM-DD).
    pub fn op(&self, vandaag: &str) -> Self {
        let mut uit = self.clone();
        for f in uit.handelingen.values_mut() {
            for w in f.values_mut() {
                if w.as_str() == Some(VANDAAG) {
                    *w = Value::String(vandaag.to_string());
                }
            }
        }
        uit
    }
}

/// Een login, met als label de bestandsnaam zonder extensie: het kanaal, de
/// rol als het bestand er een noemt, en de velden van het kanaal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InlogVoorbeeld {
    pub label: String,
    pub kanaal: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rol: Option<String>,
    pub velden: BTreeMap<String, String>,
}

/// Lees de voorbeelden; paden zijn relatief aan de map van het proces. Elke fout
/// komt terug en noemt het pad.
pub fn laad(
    map: &Path,
    definitie: &VoorbeeldenDefinitie,
    proces: &ProcesDefinitie,
) -> Result<Voorbeelden, Vec<String>> {
    let mut fouten = Vec::new();
    let mut uit = Voorbeelden::default();
    // Het label is de sleutel waarmee de frontend een login kiest: uniek.
    let mut labels: Vec<(String, &str)> = Vec::new();
    for pad in &definitie.inloggen {
        match inlog(map, pad, proces) {
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
    for (naam, pad) in &definitie.handelingen {
        if let Ok(f) = object_onder(map, pad, "formulier").map_err(|f| fouten.push(f)) {
            uit.handelingen.insert(naam.clone(), f);
        }
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

/// Een login zoals een kanaal haar aanneemt, en geldig. Het kanaal staat in
/// het bestand (`kanaal`), of is het enige kanaal van het portaal.
fn inlog(map: &Path, pad: &str, proces: &ProcesDefinitie) -> Result<InlogVoorbeeld, String> {
    let Value::Object(invoer) = lees(map, pad)? else {
        return Err(format!(
            "voorbeeld {pad}: verwacht een object met de velden van een kanaal"
        ));
    };
    let portaal = proces.kanalen_met(Routes::Portaal);
    let kanaal = match invoer.get("kanaal").and_then(Value::as_str) {
        Some(k) => k.to_string(),
        None => match portaal.as_slice() {
            [(id, _)] => id.to_string(),
            _ => {
                return Err(format!(
                    "voorbeeld {pad}: noem het kanaal; het portaal heeft er {}",
                    portaal.len()
                ))
            }
        },
    };
    let k = proces
        .kanalen
        .get(&kanaal)
        .ok_or_else(|| format!("voorbeeld {pad}: kanaal '{kanaal}' staat niet onder kanalen"))?;
    let velden = k
        .valideer(&invoer)
        .map_err(|e| format!("voorbeeld {pad}: {e}"))?;
    let rol = invoer
        .get("rol")
        .and_then(Value::as_str)
        .map(str::to_string);
    if let Some(r) = &rol {
        if proces.rollen.get(r).is_none_or(|d| d.kanaal != kanaal) {
            return Err(format!(
                "voorbeeld {pad}: rol '{r}' logt niet in langs kanaal '{kanaal}'"
            ));
        }
    }
    let label = Path::new(pad)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| pad.to_string());
    Ok(InlogVoorbeeld {
        label,
        kanaal,
        rol,
        velden,
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

    /// Een proces met een portaalkanaal: een organisatienummer van acht
    /// cijfers en een naam.
    fn proces() -> ProcesDefinitie {
        ProcesDefinitie::parse(
            "id: p\nactor: a\nkanalen:\n  org:\n    label: Organisatie\n    velden:\n      - {naam: kvk, label: Nummer, patroon: '[0-9]{8}', melding: een nummer heeft acht cijfers}\n      - {naam: persoon, label: Naam}\nrollen:\n  aanvrager: {kanaal: org, routes: [portaal]}\n",
            "t",
        )
        .unwrap()
    }

    fn definitie(
        inloggen: &[&str],
        aanvraag: Option<&str>,
        besluit: Option<&str>,
    ) -> VoorbeeldenDefinitie {
        VoorbeeldenDefinitie {
            inloggen: inloggen.iter().map(|s| s.to_string()).collect(),
            aanvraag: aanvraag.map(str::to_string),
            handelingen: besluit
                .map(|b| BTreeMap::from([("besluit".to_string(), b.to_string())]))
                .unwrap_or_default(),
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
                r#"{"formulier": {"feiten_vergaard": true, "besluitdatum": "$vandaag"}}"#,
            ),
        ]);
        let v = laad(
            dir.path(),
            &definitie(
                &["login-een.json"],
                Some("aanvraag.json"),
                Some("besluit.json"),
            ),
            &proces(),
        )
        .unwrap();
        assert_eq!(
            v.inloggen,
            [InlogVoorbeeld {
                label: "login-een".into(),
                kanaal: "org".into(),
                rol: None,
                velden: [
                    ("kvk".to_string(), "12345678".to_string()),
                    ("persoon".to_string(), "A. Tester".to_string())
                ]
                .into(),
            }]
        );
        assert_eq!(v.aanvraag.as_ref().unwrap()["naam"], "Voorbeeld");
        assert_eq!(v.handelingen["besluit"]["feiten_vergaard"], true);
        // "$vandaag" wordt bij het opvragen de datum van vandaag.
        assert_eq!(v.handelingen["besluit"]["besluitdatum"], "$vandaag");
        assert_eq!(
            v.op("2025-03-20").handelingen["besluit"]["besluitdatum"],
            "2025-03-20"
        );
    }

    #[test]
    fn zonder_voorbeelden_is_alles_leeg() {
        let dir = map_met(&[]);
        let v = laad(dir.path(), &VoorbeeldenDefinitie::default(), &proces()).unwrap();
        assert!(v.inloggen.is_empty() && v.aanvraag.is_none() && v.handelingen.is_empty());
    }

    #[test]
    fn ontbrekend_bestand_noemt_het_pad() {
        let dir = map_met(&[]);
        let fouten = laad(
            dir.path(),
            &definitie(&["weg.json"], Some("ook-weg.json"), None),
            &proces(),
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
            &proces(),
        )
        .unwrap_err();
        assert_eq!(fouten.len(), 5, "{fouten:?}");
        assert!(
            fouten[0].contains("geen-json.json: geen geldige JSON"),
            "{fouten:?}"
        );
        assert!(
            fouten[1].contains("zonder-persoon.json: Naam ontbreekt"),
            "{fouten:?}"
        );
        assert!(
            fouten[2].contains("korte-kvk.json: een nummer heeft acht cijfers"),
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
            &proces(),
        )
        .unwrap_err();
        assert_eq!(fouten.len(), 1, "{fouten:?}");
        assert!(fouten[0].contains("login.json"), "{fouten:?}");
        assert!(fouten[0].contains("ander/login.json"), "{fouten:?}");
        assert!(fouten[0].contains("label 'login'"), "{fouten:?}");
    }
}
