//! The optional form file: labels and order for the fields that a
//! submission passes via `$external`.
//!
//! The form never determines behavior. A field the form does not know gets
//! its label from the law (the part after "Naam:" in the description of its
//! parameter) or its field name, and goes last; a field of the form that the
//! stream does not know is skipped. What the law says about a field (its
//! legal basis, whether the applicant may leave it out) comes with it, and so
//! does, for a logged-in applicant, what the channel or a register supplies
//! (note "het gram uit de wet": the form only keeps the presentation).
//!
//! [`explain`] adds the process layer (portal, form, channel) to the
//! explanation that the law gives of the event.
//!
//! The form document deliberately keeps its Dutch vocabulary (it is a dossier
//! document that other tools read as well); the runtime translates it at the
//! edge (the serde renames below and `document_type`, `document_options`,
//! `document_columns`), so that the API speaks English.
//!
//! Shape: `schermen: [{id, titel, groepen: [{titel, velden: [{id, label,
//! type, opties, kolommen, uitleg, grondslag}]}]}]` (screens, title, groups,
//! fields, options, columns, explanation, legal basis).
//!
//! `grondslag` (on a field or a column) is a legal basis or a list of legal
//! bases in the form `<regulation>#<article>`, optionally with ` lid <n>`, as
//! on an event. At startup the process checks that every article is loaded
//! and the paragraph exists (see [`Form::legal_bases`]).

use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::channel::Routes;
use crate::config::Portal;
use crate::law::{with_executes, SourceRef, Step, StepKind};
use crate::load;
use crate::process::Process;
use crate::stream::{Event, Shape};

/// A screen from a form file.
#[derive(Debug, Clone, Default)]
pub struct Form {
    pub title: Option<String>,
    pub fields: Vec<Field>,
}

impl Form {
    /// Every legal basis of the screen, with where it is: `field 'x'` or
    /// `field 'x', column 'y'`.
    pub fn legal_bases(&self) -> Vec<(String, String)> {
        let mut out = Vec::new();
        for v in &self.fields {
            for g in &v.legal_basis {
                out.push((format!("field '{}'", v.name), g.clone()));
            }
            for k in v.columns.iter().filter_map(Value::as_array).flatten() {
                let id = k.get("id").and_then(Value::as_str).unwrap_or_default();
                for g in legal_basis_from(k.get("legal_basis")) {
                    out.push((format!("field '{}', column '{id}'", v.name), g));
                }
            }
        }
        out
    }
}

/// A legal basis or a list of legal bases.
fn legal_basis_from(v: Option<&Value>) -> Vec<String> {
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

/// A field as the frontend displays it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Field {
    /// The name under `external` when submitting.
    pub name: String,
    pub label: String,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    /// The unit from the regulation (`type_spec.unit`), such as `eurocent` for
    /// an amount: the frontend uses it to convert an input in euros.
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
    /// What the field rests on, as `<regulation>#<article>` (optionally with a paragraph).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub legal_basis: Vec<String>,
    /// The law says the applicant may leave it out (`required: false`, RFC-036):
    /// the form says "(niet verplicht)" (Awb 4:4 lid 2).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub optional: bool,
    /// What the channel or a register supplies for this field, for the
    /// logged-in applicant: `{value, source, legal_basis}`. The portal shows
    /// it as filled in automatically; the submission may not change it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supplied: Option<Value>,
    /// Why the field is on the form and why it has its value
    /// ([`crate::law::FieldExplanation`]), with the presentation and what
    /// the channel supplies; see [`explain`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub why: Option<crate::law::FieldExplanation>,
    /// Whether the form gives the field a label of its own; otherwise the
    /// label comes from the law (or is the name).
    #[serde(skip)]
    pub own_label: bool,
}

impl Field {
    /// A field with only a name and a label, and nothing else known yet.
    pub fn new(name: impl Into<String>, label: impl Into<String>) -> Field {
        Field {
            name: name.into(),
            label: label.into(),
            kind: None,
            unit: None,
            options: None,
            columns: None,
            explanation: None,
            group: None,
            legal_basis: Vec::new(),
            optional: false,
            supplied: None,
            why: None,
            own_label: false,
        }
    }
}

/// Add to the fields what `intake` supplies (`$intake.supplied`), per field,
/// with the legal basis the gram will carry for it.
pub fn with_supplied(fields: &mut [Field], event: &Event, intake: &Value) {
    let Some(supplied) = intake
        .get(crate::stream::SUPPLIED)
        .and_then(Value::as_object)
    else {
        return;
    };
    for f in fields {
        if let Some(s) = supplied.get(&f.name) {
            let mut s = s.clone();
            let source = s
                .get("source")
                .and_then(Value::as_str)
                .unwrap_or("channel")
                .to_string();
            s["legal_basis"] = serde_json::json!(event.supply_legal_basis(&f.name, &source, &s));
            f.supplied = Some(s);
        }
    }
}

/// Why the form looks the way it does: the explanation of the event that
/// the law composed ([`crate::law::Explanation`], the same as in the stream
/// document), with what only the process knows: the portal that records the
/// event, the channel that supplies a field and the form file that gives
/// order, groups and labels. `channel` is the channel of the logged-in
/// applicant; without one, every channel of the portal says what it would
/// supply. After every policy article a step says what it executes
/// (RFC-047, [`with_executes`]), in the version of the day the cell of the
/// process was loaded with. Sets `why` per field and returns
/// `{event, excluded}` for the form as a whole.
pub fn explain(
    fields: &mut [Field],
    event: &Event,
    form: Option<&Form>,
    portal: &Portal,
    process: &Process,
    channel: Option<&str>,
) -> Value {
    let (service, date) = (process.service.as_ref(), process.cell.date);
    let process = &process.definition;
    let stream = SourceRef::stream(&portal.stream, &portal.event);
    let mut chain = vec![Step::new(
        StepKind::Process,
        SourceRef::law(&process.declared_by),
        format!(
            "het portaal legt '{}' vast in stroom '{}' (cel {})",
            portal.event, portal.stream, portal.cell
        ),
    )];
    chain.extend(event.explanation.event.iter().cloned());
    chain.push(match form.and(portal.form.as_ref()) {
        Some(s) => Step::new(
            StepKind::Presentation,
            SourceRef::config("form", &s.screen),
            format!(
                "scherm '{}': volgorde, groepen en labels (het formulier bepaalt geen gedrag)",
                s.screen
            ),
        ),
        None => Step::new(
            StepKind::Presentation,
            stream.clone(),
            "geen formulier: labels uit de wet, volgorde van de stroom",
        ),
    });
    let channels: Vec<_> = process
        .channels_with(Routes::Portal)
        .into_iter()
        .filter(|(id, _)| channel.is_none_or(|c| c == *id))
        .collect();
    for f in fields.iter_mut() {
        let mut why = event
            .explanation
            .fields
            .get(&f.name)
            .cloned()
            .unwrap_or_default();
        let entry = form.and_then(|x| x.fields.iter().find(|v| v.name == f.name));
        let group = f
            .group
            .as_deref()
            .map(|g| format!(", groep '{g}'"))
            .unwrap_or_default();
        // A form entry without a label of its own has the label of the law.
        why.here.push(if entry.is_some_and(|v| v.own_label) {
            Step::new(
                StepKind::Presentation,
                SourceRef::config("form", &f.name),
                format!("label '{}'{group}", f.label),
            )
        } else {
            let silent = if entry.is_some() {
                "het formulier geeft geen label"
            } else {
                "het formulier zwijgt"
            };
            match event.field_defs.iter().find(|d| d.name == f.name) {
                Some(d) => Step::new(
                    StepKind::Presentation,
                    SourceRef::law(&d.declared_by),
                    format!("label '{}' uit de wet: {silent}{group}", f.label),
                ),
                None => Step::new(
                    StepKind::Presentation,
                    stream.clone(),
                    format!("geen artikel noemt het veld: de naam uit de stroom{group}"),
                ),
            }
        });
        for (id, k) in &channels {
            if let Some(from) = k.supplies.get(&f.name) {
                why.value.push(Step::new(
                    StepKind::Supply,
                    SourceRef::law(k.supplied_by.as_deref().unwrap_or(&k.declared_by)),
                    format!(
                        "het kanaal '{id}' levert {}: {from} (supplies, grondslag {})",
                        f.name,
                        k.legal_basis.join(", ")
                    ),
                ));
            }
        }
        // `why` comes from the cell, whose steps already say what they
        // execute; these calls only add it for the presentation and supply
        // steps added here (RFC-047). Idempotent per policy article.
        with_executes(&mut why.here, service, date);
        with_executes(&mut why.value, service, date);
        f.why = Some(why);
    }
    // The cell already did this for the chain of the event; again here for
    // the process step in front of it (a policy article on the route from
    // policy). Idempotent: the steps of the cell stay as they are.
    with_executes(&mut chain, service, date);
    serde_json::json!({"event": chain, "excluded": event.explanation.excluded})
}

/// A name made readable, as the label of a field without a label or of a
/// regulation without a name: `datum_betaling` becomes "Datum betaling".
pub fn readable(name: &str) -> String {
    let text = name.replace('_', " ");
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => text,
    }
}

/// A form file as the runtime reads it (Dutch keys, renamed at the edge).
#[derive(Deserialize)]
struct File {
    #[serde(default, rename = "schermen")]
    screens: Vec<ScreenDoc>,
}

#[derive(Deserialize)]
struct ScreenDoc {
    id: String,
    #[serde(default, rename = "titel")]
    title: Option<String>,
    #[serde(default, rename = "velden")]
    fields: Vec<FieldDoc>,
    #[serde(default, rename = "groepen")]
    groups: Vec<GroupDoc>,
}

#[derive(Deserialize)]
struct GroupDoc {
    #[serde(default, rename = "titel")]
    title: Option<String>,
    #[serde(default, rename = "velden")]
    fields: Vec<FieldDoc>,
}

#[derive(Deserialize)]
struct FieldDoc {
    id: String,
    #[serde(default)]
    label: Option<String>,
    #[serde(default, rename = "type")]
    kind: Option<String>,
    #[serde(default, rename = "opties")]
    options: Option<Value>,
    #[serde(default, rename = "kolommen")]
    columns: Option<Value>,
    #[serde(default, rename = "uitleg")]
    explanation: Option<String>,
    #[serde(default, rename = "grondslag")]
    legal_basis: Option<Value>,
}

impl FieldDoc {
    fn field(self, group: Option<&String>) -> Field {
        let own_label = self.label.is_some();
        Field {
            kind: self.kind.as_deref().map(document_type),
            options: self.options.map(document_options),
            columns: self.columns.map(document_columns),
            explanation: self.explanation,
            group: group.cloned(),
            legal_basis: legal_basis_from(self.legal_basis.as_ref()),
            own_label,
            ..Field::new(self.id.clone(), self.label.unwrap_or(self.id))
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

/// The columns of a table field, with the keys and types of the API. The
/// law names a table's columns but not their types, so the unit of an
/// amount column comes from the form (`eenheid`, as `unit`).
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
                                "eenheid" => ("unit".to_string(), w),
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

/// Read a screen from a form file. A field without an `id`, or a key with a
/// different shape than here, is an error: the form is not read halfway.
/// Other keys (a form file can serve more than the runtime) are ignored.
pub fn parse(text: &str, screen: &str, source: &str) -> Result<Form, String> {
    let file: File = load::yaml(text, source).map_err(|f| f.join("; "))?;
    let s = file
        .screens
        .into_iter()
        .find(|s| s.id == screen)
        .ok_or_else(|| format!("{source}: no screen '{screen}'"))?;
    let mut fields: Vec<Field> = s.fields.into_iter().map(|v| v.field(None)).collect();
    for group in s.groups {
        let title = group.title;
        fields.extend(group.fields.into_iter().map(|v| v.field(title.as_ref())));
    }
    Ok(Form {
        title: s.title,
        fields,
    })
}

/// Load a screen from a form file.
pub fn load(path: &Path, screen: &str) -> Result<Form, String> {
    load::load(path, |t, source| {
        parse(t, screen, source).map_err(|f| vec![f])
    })
    .map_err(|f| f.join("; "))
}

/// The fields a submission for this event passes: in the order of the form,
/// then what the form does not know in the order of the stream. The same
/// holds per column for a table field: the columns of the stream, with label
/// and order from the form.
pub fn fields(event: &Event, form: Option<&Form>) -> Result<Vec<Field>, String> {
    let keys = event.external_keys();
    let shape = event.external_shape().map_err(|f| f.join("; "))?;
    let mut out: Vec<Field> = form
        .map(|f| {
            f.fields
                .iter()
                .filter(|v| keys.contains(&v.name))
                .cloned()
                .collect()
        })
        .unwrap_or_default();
    for key in keys {
        if !out.iter().any(|v| v.name == key) {
            out.push(Field::new(key.clone(), key));
        }
    }
    // What the law says per field: the label if the form has none, the
    // legal basis if the form names none, and whether it may be left out.
    for field in &mut out {
        let Some(d) = event.field_defs.iter().find(|d| d.name == field.name) else {
            continue;
        };
        if !field.own_label {
            field.label = match d.description.as_deref().filter(|t| t.contains("Naam:")) {
                Some(t) => crate::origin::label_from(t),
                None => readable(&field.name),
            };
        }
        if field.legal_basis.is_empty() {
            field.legal_basis = d.legal_basis.clone();
        }
        // The type and unit of the law hold; a form type that contradicts
        // an amount of the law would let the applicant submit in the wrong
        // unit (euros where the law counts eurocents).
        let kind = d.type_.as_deref().map(kind_of_type);
        if kind.as_deref() == Some("amount") && field.kind.as_ref().is_some_and(|k| k != "amount") {
            return Err(format!(
                "form field '{}': type '{}', but the law ({}) makes it an amount",
                field.name,
                field.kind.as_deref().unwrap_or_default(),
                d.declared_by
            ));
        }
        if field.kind.is_none() {
            field.kind = kind;
        }
        field.unit = d.unit.clone();
        field.optional = d.optional_for_applicant();
    }
    // Whether a field is a table is determined by the stream: otherwise the
    // screen shows an input that the cell refuses on submission.
    for field in &mut out {
        if let Some(Shape::Table(columns)) = shape.get(&field.name) {
            field.kind = Some("table".to_string());
            field.columns = Some(table_columns(columns, field.columns.as_ref()));
        } else {
            if field.kind.as_deref() == Some("table") {
                field.kind = None;
            }
            field.columns = None;
        }
    }
    Ok(out)
}

/// The kind of form field for a type as the law writes it (`amount`,
/// `boolean`, ...); see [`crate::action::field_kind`].
fn kind_of_type(t: &str) -> String {
    serde_json::from_value(Value::String(t.to_string()))
        .map(crate::action::field_kind)
        .unwrap_or_else(|_| t.to_string())
}

/// The columns of a table field: the columns of the form that the stream
/// knows, then the columns of the stream that the form does not know, with
/// their name as label.
fn table_columns(stream: &[String], form: Option<&Value>) -> Value {
    let mut out: Vec<Value> = form
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
        if !out
            .iter()
            .any(|k| k.get("id").and_then(Value::as_str) == Some(column))
        {
            out.push(serde_json::json!({"id": column, "label": column}));
        }
    }
    Value::Array(out)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::stream;

    const STREAM: &str = include_str!("../tests/fixtures/chronicles/test_aanvragen.yaml");
    const FORM: &str = include_str!("../tests/fixtures/documents/formulier-instantie.yaml");

    #[test]
    fn order_and_labels_from_the_form() {
        let s = stream::parse(STREAM, "fixture").unwrap();
        let f = parse(FORM, "aanvraag", "fixture").unwrap();
        let v = fields(&s.events[0], Some(&f)).unwrap();
        let names: Vec<&str> = v.iter().map(|v| v.name.as_str()).collect();
        // The stream does not know `telefoon`; the form does not know `rekeningnummer`.
        assert_eq!(
            names,
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
        // Columns: those of the stream, with label and order from the form;
        // the stream does not know `opmerking`.
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
    fn the_stream_determines_whether_a_field_is_a_table() {
        let s = stream::parse(STREAM, "fixture").unwrap();
        // The form calls `organen` text and `naam` a table.
        let f = parse(
            &FORM.replace("type: tabel", "type: tekst").replace(
                "{id: naam, label: Naam van de aanvrager, type: tekst}",
                "{id: naam, label: Naam van de aanvrager, type: tabel, columns: [{id: x}]}",
            ),
            "aanvraag",
            "fixture",
        )
        .unwrap();
        let v = fields(&s.events[0], Some(&f)).unwrap();
        let field = |name: &str| v.iter().find(|v| v.name == name).unwrap();
        assert_eq!(field("organen").kind.as_deref(), Some("table"));
        assert!(field("organen").columns.is_some());
        assert_eq!(field("naam").kind, None);
        assert_eq!(field("naam").columns, None);
    }

    #[test]
    fn table_columns_without_a_form() {
        let s = stream::parse(STREAM, "fixture").unwrap();
        let v = fields(&s.events[0], None).unwrap();
        let organen = v.iter().find(|v| v.name == "organen").unwrap();
        assert_eq!(organen.kind.as_deref(), Some("table"));
        assert_eq!(
            organen.columns.as_ref().unwrap()[1],
            serde_json::json!({"id": "zetels", "label": "zetels"})
        );
    }

    #[test]
    fn without_a_form_the_field_name() {
        let s = stream::parse(STREAM, "fixture").unwrap();
        let v = fields(&s.events[0], None).unwrap();
        assert_eq!(v[0].name, "naam");
        assert_eq!(v[0].label, "naam");
    }

    /// A legal basis in the form is machine-readable: on a field and on a
    /// column, as text or as a list.
    #[test]
    fn legal_bases_of_fields_and_columns() {
        let f = parse(FORM, "aanvraag", "fixture").unwrap();
        let g = f.legal_bases();
        assert!(
            g.contains(&(
                "field 'aanvraagjaar'".into(),
                "testregeling_aanvraag#1 lid 1".into()
            )),
            "{g:?}"
        );
        assert!(
            g.contains(&(
                "field 'organen', column 'zetels'".into(),
                "testregeling_aanvraag#1".into()
            )),
            "{g:?}"
        );
        let s = stream::parse(STREAM, "fixture").unwrap();
        let v = fields(&s.events[0], Some(&f)).unwrap();
        let year = v.iter().find(|v| v.name == "aanvraagjaar").unwrap();
        assert_eq!(year.legal_basis, ["testregeling_aanvraag#1 lid 1"]);
    }

    fn amount(name: &str) -> crate::law::FieldDef {
        crate::law::FieldDef {
            name: name.into(),
            type_: Some("amount".into()),
            unit: Some("eurocent".into()),
            columns: None,
            origin: None,
            origin_policy: None,
            optional: false,
            description: None,
            legal_basis: Vec::new(),
            declared_by: "w#1".into(),
            via: crate::law::Via::Establishes,
        }
    }

    /// The unit of the law reaches the field also when the form gives a
    /// type of its own; a form type that contradicts an amount is refused.
    #[test]
    fn the_unit_of_the_law_holds_whatever_the_form_says() {
        let mut s = stream::parse(STREAM, "fixture").unwrap();
        s.events[0].field_defs.push(amount("aanvraagjaar"));
        let own = parse(
            &FORM.replace(
                "label: Aanvraagjaar, type: getal",
                "label: Aanvraagjaar, type: bedrag",
            ),
            "aanvraag",
            "fixture",
        )
        .unwrap();
        let v = fields(&s.events[0], Some(&own)).unwrap();
        let year = v.iter().find(|v| v.name == "aanvraagjaar").unwrap();
        assert_eq!(year.kind.as_deref(), Some("amount"));
        assert_eq!(year.unit.as_deref(), Some("eurocent"));
        let f = parse(FORM, "aanvraag", "fixture").unwrap();
        let e = fields(&s.events[0], Some(&f)).unwrap_err();
        assert!(e.contains("aanvraagjaar") && e.contains("amount"), "{e}");
    }

    /// A label equal to the id is still the form's own label.
    #[test]
    fn a_label_equal_to_the_id_is_the_forms_own() {
        let f = parse(
            &FORM.replace("label: Adres", "label: adres"),
            "aanvraag",
            "fixture",
        )
        .unwrap();
        let adres = f.fields.iter().find(|v| v.name == "adres").unwrap();
        assert!(adres.own_label);
        let telefoon = parse(
            &FORM.replace(
                "{id: telefoon, label: Telefoonnummer, type: tekst}",
                "{id: telefoon}",
            ),
            "aanvraag",
            "fixture",
        )
        .unwrap();
        assert!(
            !telefoon
                .fields
                .iter()
                .find(|v| v.name == "telefoon")
                .unwrap()
                .own_label
        );
    }

    /// An amount column carries the unit the form names (`eenheid`).
    #[test]
    fn an_amount_column_carries_its_unit() {
        let f = parse(
            &FORM.replace(
                "{id: zetels, label: Zetels, type: getal,",
                "{id: zetels, label: Zetels, type: bedrag, eenheid: eurocent,",
            ),
            "aanvraag",
            "fixture",
        )
        .unwrap();
        let s = stream::parse(STREAM, "fixture").unwrap();
        let v = fields(&s.events[0], Some(&f)).unwrap();
        let organen = v.iter().find(|v| v.name == "organen").unwrap();
        let zetels = &organen.columns.as_ref().unwrap()[1];
        assert_eq!(zetels["type"], "amount");
        assert_eq!(zetels["unit"], "eurocent");
    }

    #[test]
    fn unknown_screen() {
        assert!(parse(FORM, "bestaat_niet", "f")
            .unwrap_err()
            .contains("bestaat_niet"));
    }
}
