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
    pub title: Option<String>,
    pub fields: Vec<Veld>,
}

impl Formulier {
    /// Elke grondslag van het scherm, met waar ze staat: `veld 'x'` of
    /// `veld 'x', kolom 'y'`.
    pub fn grondslagen(&self) -> Vec<(String, String)> {
        let mut uit = Vec::new();
        for v in &self.fields {
            for g in &v.legal_basis {
                uit.push((format!("veld '{}'", v.name), g.clone()));
            }
            for k in v.columns.iter().filter_map(Value::as_array).flatten() {
                let id = k.get("id").and_then(Value::as_str).unwrap_or_default();
                for g in grondslag_uit(k.get("legal_basis")) {
                    uit.push((format!("veld '{}', kolom '{id}'", v.name), g));
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
    pub name: String,
    pub label: String,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub soort: Option<String>,
    /// De eenheid uit de regeling (`type_spec.unit`), zoals `eurocent` bij
    /// een bedrag: de frontend rekent een invoer in euro ermee om.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub columns: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub explanation: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    /// Waarop het veld rust, als `<regeling>#<artikel>` (optioneel met lid).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub legal_basis: Vec<String>,
}

/// Een naam leesbaar, als label van een veld zonder label of een regeling
/// zonder naam: `datum_betaling` wordt "Datum betaling".
pub fn leesbaar(name: &str) -> String {
    let tekst = name.replace('_', " ");
    let mut tekens = tekst.chars();
    match tekens.next() {
        Some(eerste) => eerste.to_uppercase().chain(tekens).collect(),
        None => tekst,
    }
}

/// Een formulierbestand zoals de runtime het leest.
#[derive(Deserialize)]
struct Bestand {
    #[serde(default, rename = "schermen")]
    screens: Vec<SchermDoc>,
}

#[derive(Deserialize)]
struct SchermDoc {
    id: String,
    #[serde(default, rename = "titel")]
    title: Option<String>,
    #[serde(default, rename = "velden")]
    fields: Vec<VeldDoc>,
    #[serde(default, rename = "groepen")]
    groups: Vec<GroepDoc>,
}

#[derive(Deserialize)]
struct GroepDoc {
    #[serde(default, rename = "titel")]
    title: Option<String>,
    #[serde(default, rename = "velden")]
    fields: Vec<VeldDoc>,
}

#[derive(Deserialize)]
struct VeldDoc {
    id: String,
    #[serde(default)]
    label: Option<String>,
    #[serde(default, rename = "type")]
    soort: Option<String>,
    #[serde(default, rename = "opties")]
    options: Option<Value>,
    #[serde(default, rename = "kolommen")]
    columns: Option<Value>,
    #[serde(default, rename = "uitleg")]
    explanation: Option<String>,
    #[serde(default, rename = "grondslag")]
    legal_basis: Option<Value>,
}

impl VeldDoc {
    fn field(self, group: Option<&String>) -> Veld {
        Veld {
            label: self.label.unwrap_or_else(|| self.id.clone()),
            name: self.id,
            soort: self.soort.as_deref().map(document_type),
            unit: None,
            options: self.options.map(document_options),
            columns: self.columns.map(document_columns),
            explanation: self.explanation,
            group: group.cloned(),
            legal_basis: grondslag_uit(self.legal_basis.as_ref()),
        }
    }
}

/// The form document is written in Dutch (it is a dossier document that other
/// tools read as well); the runtime translates its vocabulary at the edge, so
/// that the API speaks English. Field types:
fn document_type(t: &str) -> String {
    match t {
        "tekst" => "text",
        "getal" => "number",
        "datum" => "date",
        "keuze" => "choice",
        "janee" => "yes_no",
        "vink" => "checkbox",
        "bestand" => "file",
        "tabel" => "table",
        "bedrag" => "amount",
        other => other,
    }
    .to_string()
}

/// The options of a choice: `{waarde, label}` becomes `{value, label}`.
fn document_options(v: Value) -> Value {
    match v {
        Value::Array(l) => Value::Array(
            l.into_iter()
                .map(|o| match o {
                    Value::Object(m) => Value::Object(
                        m.into_iter()
                            .map(|(k, w)| {
                                (
                                    if k == "waarde" {
                                        "value".to_string()
                                    } else {
                                        k
                                    },
                                    w,
                                )
                            })
                            .collect(),
                    ),
                    other => other,
                })
                .collect(),
        ),
        other => other,
    }
}

/// The columns of a table field, with the keys and types of the API.
fn document_columns(v: Value) -> Value {
    match v {
        Value::Array(l) => Value::Array(
            l.into_iter()
                .map(|k| match k {
                    Value::Object(m) => Value::Object(
                        m.into_iter()
                            .map(|(k, w)| match k.as_str() {
                                "type" => {
                                    (k, w.as_str().map(document_type).map_or(w, Value::String))
                                }
                                "opties" => ("options".to_string(), document_options(w)),
                                "uitleg" => ("explanation".to_string(), w),
                                "grondslag" => ("legal_basis".to_string(), w),
                                _ => (k, w),
                            })
                            .collect(),
                    ),
                    other => other,
                })
                .collect(),
        ),
        other => other,
    }
}

/// Lees een scherm uit een formulierbestand. Een veld zonder `id`, of een
/// sleutel met een andere vorm dan hier, is een fout: het formulier wordt niet
/// half gelezen. Andere sleutels (een formulierbestand kan meer dienen dan
/// de runtime) blijven buiten beschouwing.
pub fn parse(tekst: &str, screen: &str, source: &str) -> Result<Formulier, String> {
    let bestand: Bestand = laden::yaml(tekst, source).map_err(|f| f.join("; "))?;
    let s = bestand
        .screens
        .into_iter()
        .find(|s| s.id == screen)
        .ok_or_else(|| format!("{source}: geen scherm '{screen}'"))?;
    let mut fields: Vec<Veld> = s.fields.into_iter().map(|v| v.field(None)).collect();
    for group in s.groups {
        let title = group.title;
        fields.extend(group.fields.into_iter().map(|v| v.field(title.as_ref())));
    }
    Ok(Formulier {
        title: s.title,
        fields,
    })
}

/// Laad een scherm uit een formulierbestand.
pub fn laad(path: &Path, screen: &str) -> Result<Formulier, String> {
    laden::laad(path, |t, source| {
        parse(t, screen, source).map_err(|f| vec![f])
    })
    .map_err(|f| f.join("; "))
}

/// De velden die een indiening voor dit event meegeeft: in de volgorde van
/// het formulier, daarna wat het formulier niet kent in de volgorde van de
/// stroom. Voor een tabelveld geldt hetzelfde per kolom: de kolommen van
/// de stroom, met label en volgorde uit het formulier.
pub fn fields(event: &Event, form: Option<&Formulier>) -> Result<Vec<Veld>, String> {
    let sleutels = event.external_sleutels();
    let vorm = event.external_vorm().map_err(|f| f.join("; "))?;
    let mut uit: Vec<Veld> = form
        .map(|f| {
            f.fields
                .iter()
                .filter(|v| sleutels.contains(&v.name))
                .cloned()
                .collect()
        })
        .unwrap_or_default();
    for sleutel in sleutels {
        if !uit.iter().any(|v| v.name == sleutel) {
            uit.push(Veld {
                label: sleutel.clone(),
                name: sleutel,
                soort: None,
                unit: None,
                options: None,
                columns: None,
                explanation: None,
                group: None,
                legal_basis: Vec::new(),
            });
        }
    }
    // Of een veld een tabel is, bepaalt de stroom: anders toont het scherm
    // een invoer die de cel bij het indienen weigert.
    for field in &mut uit {
        if let Some(Vorm::Tabel(columns)) = vorm.get(&field.name) {
            field.soort = Some("table".to_string());
            field.columns = Some(tabelkolommen(columns, field.columns.as_ref()));
        } else {
            if field.soort.as_deref() == Some("table") {
                field.soort = None;
            }
            field.columns = None;
        }
    }
    Ok(uit)
}

/// De kolommen van een tabelveld: de kolommen van het formulier die de
/// stroom kent, daarna de kolommen van de stroom die het formulier niet
/// kent, met hun naam als label.
fn tabelkolommen(stream: &[String], form: Option<&Value>) -> Value {
    let mut uit: Vec<Value> = form
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|k| {
            k.get("id")
                .and_then(Value::as_str)
                .is_some_and(|id| stream.iter().any(|s| s == id))
        })
        .cloned()
        .collect();
    for column in stream {
        if !uit
            .iter()
            .any(|k| k.get("id").and_then(Value::as_str) == Some(column))
        {
            uit.push(serde_json::json!({"id": column, "label": column}));
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
        let v = fields(&s.events[0], Some(&f)).unwrap();
        let namen: Vec<&str> = v.iter().map(|v| v.name.as_str()).collect();
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
        assert_eq!(v[0].group.as_deref(), Some("De aanvrager"));
        assert_eq!(v[7].label, "rekeningnummer");
        // Kolommen: die van de stroom, met label en volgorde uit het
        // formulier; `opmerking` kent de stroom niet.
        let columns: Vec<&str> = v[6]
            .columns
            .as_ref()
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .map(|k| k["id"].as_str().unwrap())
            .collect();
        assert_eq!(
            columns,
            vec!["orgaan", "zetels", "samengevoegd", "aantal_aanduidingen"]
        );
        assert_eq!(v[6].columns.as_ref().unwrap()[0]["label"], "Orgaan");
    }

    #[test]
    fn de_stroom_bepaalt_of_een_veld_een_tabel_is() {
        let s = stroom::parse(STROOM, "fixture").unwrap();
        // Het formulier noemt `organen` tekst en `naam` een tabel.
        let f = parse(
            &FORMULIER.replace("type: tabel", "type: tekst").replace(
                "{id: naam, label: Naam van de aanvrager, type: tekst}",
                "{id: naam, label: Naam van de aanvrager, type: tabel, columns: [{id: x}]}",
            ),
            "aanvraag",
            "fixture",
        )
        .unwrap();
        let v = fields(&s.events[0], Some(&f)).unwrap();
        let field = |name: &str| v.iter().find(|v| v.name == name).unwrap();
        assert_eq!(field("organen").soort.as_deref(), Some("table"));
        assert!(field("organen").columns.is_some());
        assert_eq!(field("naam").soort, None);
        assert_eq!(field("naam").columns, None);
    }

    #[test]
    fn tabelkolommen_zonder_formulier() {
        let s = stroom::parse(STROOM, "fixture").unwrap();
        let v = fields(&s.events[0], None).unwrap();
        let organen = v.iter().find(|v| v.name == "organen").unwrap();
        assert_eq!(organen.soort.as_deref(), Some("table"));
        assert_eq!(
            organen.columns.as_ref().unwrap()[1],
            serde_json::json!({"id": "zetels", "label": "zetels"})
        );
    }

    #[test]
    fn zonder_formulier_de_veldnaam() {
        let s = stroom::parse(STROOM, "fixture").unwrap();
        let v = fields(&s.events[0], None).unwrap();
        assert_eq!(v[0].name, "naam");
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
        let v = fields(&s.events[0], Some(&f)).unwrap();
        let jaar = v.iter().find(|v| v.name == "aanvraagjaar").unwrap();
        assert_eq!(jaar.legal_basis, ["testregeling_aanvraag#1 lid 1"]);
    }

    #[test]
    fn onbekend_scherm() {
        assert!(parse(FORMULIER, "bestaat_niet", "f")
            .unwrap_err()
            .contains("bestaat_niet"));
    }
}
