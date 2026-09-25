//! Het optionele formulierbestand: labels en volgorde voor de velden die
//! een indiening via `$external` meegeeft.
//!
//! Het formulier bepaalt nooit het gedrag. Een veld dat het formulier niet
//! kent, krijgt zijn veldnaam als label en komt achteraan; een veld van het
//! formulier dat de stroom niet kent, wordt overgeslagen.
//!
//! Vorm: `schermen: [{id, titel, groepen: [{titel, velden: [{id, label,
//! type, opties, kolommen, uitleg}]}]}]`.

use std::path::Path;

use serde::Serialize;
use serde_json::Value;
use serde_yaml_ng::Value as Y;

use crate::laden;
use crate::stroom::{Event, Vorm};

/// Een scherm uit een formulierbestand.
#[derive(Debug, Clone, Default)]
pub struct Formulier {
    pub titel: Option<String>,
    pub velden: Vec<Veld>,
}

/// Een veld zoals de frontend het toont.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Veld {
    /// De naam onder `external` bij het indienen.
    pub naam: String,
    pub label: String,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub soort: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub opties: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kolommen: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uitleg: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub groep: Option<String>,
}

fn tekst(v: &Y, sleutel: &str) -> Option<String> {
    v.get(sleutel).and_then(Y::as_str).map(str::to_string)
}

fn json(v: &Y, sleutel: &str) -> Option<Value> {
    v.get(sleutel).and_then(|w| serde_json::to_value(w).ok())
}

/// Lees een scherm uit een formulierbestand.
pub fn parse(tekst_: &str, scherm: &str, bron: &str) -> Result<Formulier, String> {
    let doc: Y =
        serde_yaml_ng::from_str(tekst_).map_err(|e| format!("{bron}: geen geldige YAML: {e}"))?;
    let scherm_doc = doc
        .get("schermen")
        .and_then(Y::as_sequence)
        .and_then(|s| {
            s.iter()
                .find(|s| s.get("id").and_then(Y::as_str) == Some(scherm))
        })
        .ok_or_else(|| format!("{bron}: geen scherm '{scherm}'"))?;
    let mut velden = Vec::new();
    let mut voeg_toe = |groep: Option<String>, lijst: Option<&Y>| {
        for v in lijst.and_then(Y::as_sequence).into_iter().flatten() {
            let Some(id) = tekst(v, "id") else { continue };
            velden.push(Veld {
                label: tekst(v, "label").unwrap_or_else(|| id.clone()),
                naam: id,
                soort: tekst(v, "type"),
                opties: json(v, "opties"),
                kolommen: json(v, "kolommen"),
                uitleg: tekst(v, "uitleg"),
                groep: groep.clone(),
            });
        }
    };
    voeg_toe(None, scherm_doc.get("velden"));
    for groep in scherm_doc
        .get("groepen")
        .and_then(Y::as_sequence)
        .into_iter()
        .flatten()
    {
        voeg_toe(tekst(groep, "titel"), groep.get("velden"));
    }
    Ok(Formulier {
        titel: tekst(scherm_doc, "titel"),
        velden,
    })
}

/// Laad een scherm uit een formulierbestand.
pub fn laad(pad: &Path, scherm: &str) -> Result<Formulier, String> {
    let (t, bron) = laden::lees(pad)?;
    parse(&t, scherm, &bron)
}

/// De velden die een indiening voor dit event meegeeft: in de volgorde van
/// het formulier, daarna wat het formulier niet kent in de volgorde van de
/// stroom. Voor een tabelveld geldt hetzelfde per kolom: de kolommen van
/// de stroom, met label en volgorde uit het formulier.
pub fn velden(event: &Event, formulier: Option<&Formulier>) -> Result<Vec<Veld>, String> {
    let sleutels = event.external_sleutels();
    let vorm = event.external_vorm().map_err(|f| f.join("; "))?;
    let mut uit: Vec<Veld> = formulier
        .map(|f| {
            f.velden
                .iter()
                .filter(|v| sleutels.contains(&v.naam))
                .cloned()
                .collect()
        })
        .unwrap_or_default();
    for sleutel in sleutels {
        if !uit.iter().any(|v| v.naam == sleutel) {
            uit.push(Veld {
                label: sleutel.clone(),
                naam: sleutel,
                soort: None,
                opties: None,
                kolommen: None,
                uitleg: None,
                groep: None,
            });
        }
    }
    // Of een veld een tabel is, bepaalt de stroom: anders toont het scherm
    // een invoer die de cel bij het indienen weigert.
    for veld in &mut uit {
        if let Some(Vorm::Tabel(kolommen)) = vorm.get(&veld.naam) {
            veld.soort = Some("tabel".to_string());
            veld.kolommen = Some(tabelkolommen(kolommen, veld.kolommen.as_ref()));
        } else {
            if veld.soort.as_deref() == Some("tabel") {
                veld.soort = None;
            }
            veld.kolommen = None;
        }
    }
    Ok(uit)
}

/// De kolommen van een tabelveld: de kolommen van het formulier die de
/// stroom kent, daarna de kolommen van de stroom die het formulier niet
/// kent, met hun naam als label.
fn tabelkolommen(stroom: &[String], formulier: Option<&Value>) -> Value {
    let mut uit: Vec<Value> = formulier
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|k| {
            k.get("id")
                .and_then(Value::as_str)
                .is_some_and(|id| stroom.iter().any(|s| s == id))
        })
        .cloned()
        .collect();
    for kolom in stroom {
        if !uit
            .iter()
            .any(|k| k.get("id").and_then(Value::as_str) == Some(kolom))
        {
            uit.push(serde_json::json!({"id": kolom, "label": kolom}));
        }
    }
    Value::Array(uit)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::stroom;

    const STROOM: &str = include_str!("../tests/fixtures/chronicles/test_aanvragen.yaml");
    const FORMULIER: &str = include_str!("../tests/fixtures/processes/instantie/formulier.yaml");

    #[test]
    fn volgorde_en_labels_uit_het_formulier() {
        let s = stroom::parse(STROOM, "fixture").unwrap();
        let f = parse(FORMULIER, "aanvraag", "fixture").unwrap();
        let v = velden(&s.events[0], Some(&f)).unwrap();
        let namen: Vec<&str> = v.iter().map(|v| v.naam.as_str()).collect();
        // `telefoon` kent de stroom niet; `rekeningnummer` kent het formulier niet.
        assert_eq!(
            namen,
            vec![
                "naam",
                "aanduiding",
                "adres",
                "aanvraagjaar",
                "dagtekening",
                "registratie",
                "organen",
                "rekeningnummer"
            ]
        );
        assert_eq!(v[0].label, "Naam van de aanvrager");
        assert_eq!(v[0].groep.as_deref(), Some("De aanvrager"));
        assert_eq!(v[7].label, "rekeningnummer");
        // Kolommen: die van de stroom, met label en volgorde uit het
        // formulier; `opmerking` kent de stroom niet.
        let kolommen: Vec<&str> = v[6]
            .kolommen
            .as_ref()
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .map(|k| k["id"].as_str().unwrap())
            .collect();
        assert_eq!(
            kolommen,
            vec!["orgaan", "zetels", "samengevoegd", "aantal_aanduidingen"]
        );
        assert_eq!(v[6].kolommen.as_ref().unwrap()[0]["label"], "Orgaan");
    }

    #[test]
    fn de_stroom_bepaalt_of_een_veld_een_tabel_is() {
        let s = stroom::parse(STROOM, "fixture").unwrap();
        // Het formulier noemt `organen` tekst en `naam` een tabel.
        let f = parse(
            &FORMULIER.replace("type: tabel", "type: tekst").replace(
                "{id: naam, label: Naam van de aanvrager, type: tekst}",
                "{id: naam, label: Naam van de aanvrager, type: tabel, kolommen: [{id: x}]}",
            ),
            "aanvraag",
            "fixture",
        )
        .unwrap();
        let v = velden(&s.events[0], Some(&f)).unwrap();
        let veld = |naam: &str| v.iter().find(|v| v.naam == naam).unwrap();
        assert_eq!(veld("organen").soort.as_deref(), Some("tabel"));
        assert!(veld("organen").kolommen.is_some());
        assert_eq!(veld("naam").soort, None);
        assert_eq!(veld("naam").kolommen, None);
    }

    #[test]
    fn tabelkolommen_zonder_formulier() {
        let s = stroom::parse(STROOM, "fixture").unwrap();
        let v = velden(&s.events[0], None).unwrap();
        let organen = v.iter().find(|v| v.naam == "organen").unwrap();
        assert_eq!(organen.soort.as_deref(), Some("tabel"));
        assert_eq!(
            organen.kolommen.as_ref().unwrap()[1],
            serde_json::json!({"id": "zetels", "label": "zetels"})
        );
    }

    #[test]
    fn zonder_formulier_de_veldnaam() {
        let s = stroom::parse(STROOM, "fixture").unwrap();
        let v = velden(&s.events[0], None).unwrap();
        assert_eq!(v[0].naam, "naam");
        assert_eq!(v[0].label, "naam");
    }

    #[test]
    fn onbekend_scherm() {
        assert!(parse(FORMULIER, "bestaat_niet", "f")
            .unwrap_err()
            .contains("bestaat_niet"));
    }
}
