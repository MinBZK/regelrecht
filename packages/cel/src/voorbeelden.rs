//! Voorbeelden: standaardgegevens per handeling, voor een proefopstelling.
//!
//! `proces.yaml` kan per handeling een JSON-bestand noemen (zie
//! [`crate::config::VoorbeeldenDefinitie`]): logins langs een kanaal, een
//! aanvraag en per handeling in een zaak een formulier. De frontend biedt ze
//! aan om een formulier voor in te vullen of de handeling er direct mee te
//! doen. Het zijn gewone invoer, geen feiten: de runtime legt ze niet vast en
//! de handeling toetst ze zoals elke andere invoer.
//!
//! Een waarde `"$today"` in een formulier wordt bij het opvragen de datum
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
    pub logins: Vec<InlogVoorbeeld>,
    /// De `external` van een aanvraag.
    pub application: Option<Map<String, Value>>,
    /// Per handeling het `formulier`.
    pub actions: BTreeMap<String, Map<String, Value>>,
}

/// De waarde die in een voorbeeld de datum van vandaag wordt.
pub const VANDAAG: &str = "$today";

impl Voorbeelden {
    /// De voorbeelden zoals de frontend ze krijgt: `"$today"` wordt
    /// `vandaag` (JJJJ-MM-DD).
    pub fn op(&self, vandaag: &str) -> Self {
        let mut uit = self.clone();
        for f in uit.actions.values_mut() {
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
    pub channel: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    pub fields: BTreeMap<String, String>,
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
    for path in &definitie.logins {
        match inlog(map, path, proces) {
            Ok(v) => {
                if let Some((_, eerder)) = labels.iter().find(|(l, _)| *l == v.label) {
                    fouten.push(format!(
                        "voorbeelden {eerder} en {path} hebben hetzelfde label '{}'",
                        v.label
                    ));
                    continue;
                }
                labels.push((v.label.clone(), path));
                uit.logins.push(v);
            }
            Err(f) => fouten.push(f),
        }
    }
    if let Some(path) = &definitie.application {
        uit.application = object_onder(map, path, "external")
            .map_err(|f| fouten.push(f))
            .ok();
    }
    for (name, path) in &definitie.actions {
        if let Ok(f) = object_onder(map, path, "form").map_err(|f| fouten.push(f)) {
            uit.actions.insert(name.clone(), f);
        }
    }
    if fouten.is_empty() {
        Ok(uit)
    } else {
        Err(fouten)
    }
}

fn lees(map: &Path, path: &str) -> Result<Value, String> {
    let tekst =
        std::fs::read_to_string(map.join(path)).map_err(|e| format!("voorbeeld {path}: {e}"))?;
    serde_json::from_str(&tekst).map_err(|e| format!("voorbeeld {path}: geen geldige JSON: {e}"))
}

/// Een login zoals een kanaal haar aanneemt, en geldig. Het kanaal staat in
/// het bestand (`kanaal`), of is het enige kanaal van het portaal.
fn inlog(map: &Path, path: &str, proces: &ProcesDefinitie) -> Result<InlogVoorbeeld, String> {
    let Value::Object(input) = lees(map, path)? else {
        return Err(format!(
            "voorbeeld {path}: verwacht een object met de velden van een kanaal"
        ));
    };
    let portal = proces.kanalen_met(Routes::Portal);
    let channel = match input.get("channel").and_then(Value::as_str) {
        Some(k) => k.to_string(),
        None => match portal.as_slice() {
            [(id, _)] => id.to_string(),
            _ => {
                return Err(format!(
                    "voorbeeld {path}: noem het kanaal; het portaal heeft er {}",
                    portal.len()
                ))
            }
        },
    };
    let k = proces
        .channels
        .get(&channel)
        .ok_or_else(|| format!("voorbeeld {path}: kanaal '{channel}' staat niet onder kanalen"))?;
    let fields = k
        .valideer(&input)
        .map_err(|e| format!("voorbeeld {path}: {e}"))?;
    let role = input
        .get("role")
        .and_then(Value::as_str)
        .map(str::to_string);
    if let Some(r) = &role {
        if proces.roles.get(r).is_none_or(|d| d.channel != channel) {
            return Err(format!(
                "voorbeeld {path}: rol '{r}' logt niet in langs kanaal '{channel}'"
            ));
        }
    }
    let label = Path::new(path)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string());
    Ok(InlogVoorbeeld {
        label,
        channel,
        role,
        fields,
    })
}

/// Het object onder `sleutel` in een JSON-object.
fn object_onder(map: &Path, path: &str, sleutel: &str) -> Result<Map<String, Value>, String> {
    match lees(map, path)? {
        Value::Object(mut o) => match o.remove(sleutel) {
            Some(Value::Object(content)) => Ok(content),
            _ => Err(format!(
                "voorbeeld {path}: verwacht een object met '{sleutel}' als object"
            )),
        },
        _ => Err(format!(
            "voorbeeld {path}: verwacht een object met '{sleutel}' als object"
        )),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn map_met(bestanden: &[(&str, &str)]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        for (name, content) in bestanden {
            std::fs::write(dir.path().join(name), content).unwrap();
        }
        dir
    }

    /// Een proces met een portaalkanaal: een organisatienummer van acht
    /// cijfers en een naam.
    fn proces() -> ProcesDefinitie {
        ProcesDefinitie::parse(
            "id: p\nactor: a\nchannels:\n  org:\n    label: Organisatie\n    fields:\n      - {name: kvk, label: Nummer, pattern: '[0-9]{8}', message: een nummer heeft acht cijfers}\n      - {name: persoon, label: Naam}\nroles:\n  aanvrager: {channel: org, routes: [portal]}\n",
            "t",
        )
        .unwrap()
    }

    fn definitie(
        logins: &[&str],
        application: Option<&str>,
        decision: Option<&str>,
    ) -> VoorbeeldenDefinitie {
        VoorbeeldenDefinitie {
            logins: logins.iter().map(|s| s.to_string()).collect(),
            application: application.map(str::to_string),
            actions: decision
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
                r#"{"form": {"feiten_vergaard": true, "besluitdatum": "$today"}}"#,
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
            v.logins,
            [InlogVoorbeeld {
                label: "login-een".into(),
                channel: "org".into(),
                role: None,
                fields: [
                    ("kvk".to_string(), "12345678".to_string()),
                    ("persoon".to_string(), "A. Tester".to_string())
                ]
                .into(),
            }]
        );
        assert_eq!(v.application.as_ref().unwrap()["naam"], "Voorbeeld");
        assert_eq!(v.actions["besluit"]["feiten_vergaard"], true);
        // "$today" wordt bij het opvragen de datum van vandaag.
        assert_eq!(v.actions["besluit"]["besluitdatum"], "$today");
        assert_eq!(
            v.op("2025-03-20").actions["besluit"]["besluitdatum"],
            "2025-03-20"
        );
    }

    #[test]
    fn zonder_voorbeelden_is_alles_leeg() {
        let dir = map_met(&[]);
        let v = laad(dir.path(), &VoorbeeldenDefinitie::default(), &proces()).unwrap();
        assert!(v.logins.is_empty() && v.application.is_none() && v.actions.is_empty());
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
            ("aanvraag.json", r#"{"form": {}}"#),
            ("besluit.json", r#"[{"form": {}}]"#),
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
            fouten[4].contains("besluit.json: verwacht een object met 'form'"),
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
