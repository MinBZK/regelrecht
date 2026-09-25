//! Het optionele formulierbestand: labels en volgorde voor de velden die
//! een indiening via `$external` meegeeft.
//!
//! Het formulier bepaalt nooit het gedrag. Een veld dat het formulier niet
//! kent, krijgt zijn veldnaam als label en komt achteraan; een veld van het
//! formulier dat de stroom niet kent, wordt overgeslagen.
//!
//! Vorm: `schermen: [{id, titel, groepen: [{titel, velden: [{id, label,
//! type, opties, kolommen, uitleg, grondslag}]}]}]`.
//!
//! `grondslag` (bij een veld of een kolom) is een grondslag of een lijst
//! grondslagen in de vorm `<regeling>#<artikel>`, optioneel met ` lid <n>`,
//! zoals bij een event. Het proces gaat bij het opstarten na dat elk artikel
//! geladen is en het lid bestaat (zie [`Formulier::grondslagen`]).

use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::laden;
use crate::stroom::{Event, Vorm};

/// Een scherm uit een formulierbestand.
#[derive(Debug, Clone, Default)]
pub struct Formulier {
    pub titel: Option<String>,
    pub velden: Vec<Veld>,
}

impl Formulier {
    /// Elke grondslag van het scherm, met waar ze staat: `veld 'x'` of
    /// `veld 'x', kolom 'y'`.
    pub fn grondslagen(&self) -> Vec<(String, String)> {
        let mut uit = Vec::new();
        for v in &self.velden {
            for g in &v.grondslag {
                uit.push((format!("veld '{}'", v.naam), g.clone()));
            }
            for k in v.kolommen.iter().filter_map(Value::as_array).flatten() {
                let id = k.get("id").and_then(Value::as_str).unwrap_or_default();
                for g in grondslag_uit(k.get("grondslag")) {
                    uit.push((format!("veld '{}', kolom '{id}'", v.naam), g));
                }
            }
        }
        uit
    }
}

/// Een grondslag of een lijst grondslagen.
fn grondslag_uit(v: Option<&Value>) -> Vec<String> {
    match v {
        Some(Value::String(g)) => vec![g.clone()],
        Some(Value::Array(l)) => l
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect(),
        _ => Vec::new(),
    }
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
    /// Waarop het veld rust, als `<regeling>#<artikel>` (optioneel met lid).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub grondslag: Vec<String>,
}

/// Een naam leesbaar, als label van een veld zonder label of een regeling
/// zonder naam: `datum_betaling` wordt "Datum betaling".
pub fn leesbaar(naam: &str) -> String {
    let tekst = naam.replace('_', " ");
    let mut tekens = tekst.chars();
    match tekens.next() {
        Some(eerste) => eerste.to_uppercase().chain(tekens).collect(),
        None => tekst,
    }
}

/// Een formulierbestand zoals de runtime het leest.
#[derive(Deserialize)]
struct Bestand {
    #[serde(default)]
    schermen: Vec<SchermDoc>,
}

#[derive(Deserialize)]
struct SchermDoc {
    id: String,
    #[serde(default)]
    titel: Option<String>,
    #[serde(default)]
    velden: Vec<VeldDoc>,
    #[serde(default)]
    groepen: Vec<GroepDoc>,
}

#[derive(Deserialize)]
struct GroepDoc {
    #[serde(default)]
    titel: Option<String>,
    #[serde(default)]
    velden: Vec<VeldDoc>,
}

#[derive(Deserialize)]
struct VeldDoc {
    id: String,
    #[serde(default)]
    label: Option<String>,
    #[serde(default, rename = "type")]
    soort: Option<String>,
    #[serde(default)]
    opties: Option<Value>,
    #[serde(default)]
    kolommen: Option<Value>,
    #[serde(default)]
    uitleg: Option<String>,
    #[serde(default)]
    grondslag: Option<Value>,
}

impl VeldDoc {
    fn veld(self, groep: Option<&String>) -> Veld {
        Veld {
            label: self.label.unwrap_or_else(|| self.id.clone()),
            naam: self.id,
            soort: self.soort,
            opties: self.opties,
            kolommen: self.kolommen,
            uitleg: self.uitleg,
            groep: groep.cloned(),
            grondslag: grondslag_uit(self.grondslag.as_ref()),
        }
    }
}

/// Lees een scherm uit een formulierbestand. Een veld zonder `id`, of een
/// sleutel met een andere vorm dan hier, is een fout: het formulier wordt niet
/// half gelezen. Andere sleutels (een formulierbestand kan meer dienen dan
/// de runtime) blijven buiten beschouwing.
pub fn parse(tekst: &str, scherm: &str, bron: &str) -> Result<Formulier, String> {
    let bestand: Bestand = laden::yaml(tekst, bron).map_err(|f| f.join("; "))?;
    let s = bestand
        .schermen
        .into_iter()
        .find(|s| s.id == scherm)
        .ok_or_else(|| format!("{bron}: geen scherm '{scherm}'"))?;
    let mut velden: Vec<Veld> = s.velden.into_iter().map(|v| v.veld(None)).collect();
    for groep in s.groepen {
        let titel = groep.titel;
        velden.extend(groep.velden.into_iter().map(|v| v.veld(titel.as_ref())));
    }
    Ok(Formulier {
        titel: s.titel,
        velden,
    })
}

/// Laad een scherm uit een formulierbestand.
pub fn laad(pad: &Path, scherm: &str) -> Result<Formulier, String> {
    laden::laad(pad, |t, bron| parse(t, scherm, bron).map_err(|f| vec![f]))
        .map_err(|f| f.join("; "))
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
                grondslag: Vec::new(),
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

    /// Een grondslag in het formulier is machineleesbaar: bij een veld en
    /// bij een kolom, als tekst of als lijst.
    #[test]
    fn grondslagen_van_velden_en_kolommen() {
        let f = parse(FORMULIER, "aanvraag", "fixture").unwrap();
        let g = f.grondslagen();
        assert!(
            g.contains(&(
                "veld 'aanvraagjaar'".into(),
                "testregeling_aanvraag#1 lid 1".into()
            )),
            "{g:?}"
        );
        assert!(
            g.contains(&(
                "veld 'organen', kolom 'zetels'".into(),
                "testregeling_aanvraag#1".into()
            )),
            "{g:?}"
        );
        let s = stroom::parse(STROOM, "fixture").unwrap();
        let v = velden(&s.events[0], Some(&f)).unwrap();
        let jaar = v.iter().find(|v| v.naam == "aanvraagjaar").unwrap();
        assert_eq!(jaar.grondslag, ["testregeling_aanvraag#1 lid 1"]);
    }

    #[test]
    fn onbekend_scherm() {
        assert!(parse(FORMULIER, "bestaat_niet", "f")
            .unwrap_err()
            .contains("bestaat_niet"));
    }
}
