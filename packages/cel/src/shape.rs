//! The shape of a gram, from the law.
//!
//! For an application the cell executes the article that establishes it with
//! an empty application. The engine fires the hooks on it and yields with the
//! model of the application ([`regelrecht_engine::Submission`]): the articles
//! that take part and what each asks. Every part that marks itself in
//! `extensions.chronolex` contributes its fields; Awb 4:2 on every
//! application for a beschikking, the establishing article with what is its
//! own. For a decision the fields are what the article says, usually its
//! outputs.

use std::collections::BTreeMap;

use chrono::NaiveDate;
use regelrecht_engine::{Article, ExecutionOutcome, LawExecutionService, Submission, Value};
use regelrecht_law_model::{Origin, OriginRole, OriginValue, Parameter, ParameterType};
use serde::Serialize;

use crate::error::{setup, Result};
use crate::extension::{
    self, EffectiveAt, Establishment, Extends, Fields, PeriodParameter, Reference,
};

/// One field of a gram, as the law declares it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FieldDef {
    pub name: String,
    #[serde(rename = "type")]
    pub type_: Option<ParameterType>,
    /// The provisions the field rests on.
    pub legal_basis: Vec<String>,
    /// The article that declares it (`<regulation>#<article>`).
    pub declared_by: String,
    /// A value the cell fills in itself, not the applicant: the decision
    /// requested (Awb 4:2 lid 1 onder c) is the decision taken on the
    /// application.
    pub fixed: Option<serde_json::Value>,
}

/// The shape of the grams of one event.
#[derive(Debug, Clone, Serialize)]
pub struct Shape {
    pub event: String,
    /// The establishing article, `<regulation>#<article>`.
    pub establishes: String,
    pub law_id: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub subtype: Option<String>,
    pub stage: Option<String>,
    pub fields: Vec<FieldDef>,
    pub effective_at: Option<EffectiveAt>,
    pub refers_to: BTreeMap<String, Reference>,
    /// The period a gram of the event concerns, and the parameter that
    /// gives it.
    pub period: Option<PeriodParameter>,
    pub legal_character: Option<String>,
    pub decision_type: Option<String>,
    /// The outputs of the establishing article.
    pub outputs: Vec<String>,
}

impl Shape {
    pub fn field(&self, name: &str) -> Option<&FieldDef> {
        self.fields.iter().find(|f| f.name == name)
    }

    /// The establishing article, then every provision a field rests on, each
    /// once.
    pub fn legal_basis(&self) -> Vec<String> {
        let mut out = vec![self.establishes.clone()];
        for f in &self.fields {
            push_new(&mut out, &f.legal_basis);
        }
        out
    }
}

fn push_new(to: &mut Vec<String>, from: &[String]) {
    for x in from {
        if !to.contains(x) {
            to.push(x.clone());
        }
    }
}

/// Split `<regulation>#<article>`.
pub fn split_reference(reference: &str) -> Result<(&str, &str)> {
    reference
        .split_once('#')
        .ok_or_else(|| setup(format!("'{reference}' is not <regulation>#<article>")))
}

/// The article a reference names, in the version that applies on `day`.
fn article_on<'s>(
    service: &'s LawExecutionService,
    reference: &str,
    day: NaiveDate,
) -> Result<(&'s regelrecht_engine::ArticleBasedLaw, &'s Article)> {
    let (law_id, number) = split_reference(reference)?;
    let law = service
        .resolver()
        .get_law_for_date(law_id, Some(day))
        .ok_or_else(|| {
            setup(format!(
                "{reference}: no version of '{law_id}' applies on {day}"
            ))
        })?;
    let article = law.find_article_by_number(number).ok_or_else(|| {
        setup(format!(
            "{reference}: the version in force has no such article"
        ))
    })?;
    Ok((law, article))
}

fn outputs(article: &Article) -> Vec<String> {
    article
        .get_execution_spec()
        .and_then(|e| e.output.as_ref())
        .map(|o| o.iter().map(|x| x.name.clone()).collect())
        .unwrap_or_default()
}

fn declared_parameters(article: &Article) -> Vec<Parameter> {
    article
        .get_execution_spec()
        .and_then(|e| e.parameters.clone())
        .unwrap_or_default()
}

/// The names of the parameters the article `reference` declares, in the
/// version that applies on `day`.
pub fn parameter_names(
    service: &LawExecutionService,
    reference: &str,
    day: NaiveDate,
) -> Result<Vec<String>> {
    let (_, article) = article_on(service, reference, day)?;
    Ok(declared_parameters(article)
        .into_iter()
        .map(|p| p.name)
        .collect())
}

/// Derive the shape of `event`, which `establishes` establishes, from the
/// law as it applies on `day`.
pub fn derive(
    service: &LawExecutionService,
    event: &str,
    establishes: &str,
    day: NaiveDate,
) -> Result<Shape> {
    let (law, article) = article_on(service, establishes, day)?;
    let chronolex = extension::of_article(article, establishes)
        .map_err(setup)?
        .ok_or_else(|| setup(format!("{establishes}: no extensions.chronolex")))?;
    let entry = chronolex
        .establishes
        .iter()
        .find(|e| e.event.as_deref() == Some(event))
        .ok_or_else(|| setup(format!("{establishes}: establishes no event '{event}'")))?;
    let produces = article.get_produces();
    let mut shape = Shape {
        event: event.to_string(),
        establishes: establishes.to_string(),
        law_id: law.id.clone(),
        type_: String::new(),
        subtype: None,
        stage: entry.stage.clone(),
        fields: Vec::new(),
        effective_at: entry.effective_at.clone(),
        refers_to: entry.refers_to.clone(),
        period: entry.period.clone(),
        legal_character: produces.and_then(|p| p.legal_character.clone()),
        decision_type: produces.and_then(|p| p.decision_type.clone()),
        outputs: outputs(article),
    };
    match produces.and_then(|p| p.submission.as_ref()) {
        Some(submission) => {
            // What the application is a TOETS of is not what the gram is:
            // the gram is the submission.
            shape.legal_character = None;
            shape.decision_type = None;
            shape.type_ = entry
                .type_
                .clone()
                .unwrap_or_else(|| "submission".to_string());
            shape.subtype = Some(submission.kind.to_lowercase());
            let model = execute_submission(service, &law.id, &shape.outputs, day)
                .map_err(|e| setup(format!("{establishes}: {e}")))?;
            submission_fields(service, &mut shape, &model, day)?;
        }
        None => {
            shape.type_ = entry.type_.clone().ok_or_else(|| {
                setup(format!(
                    "{establishes}: event '{event}' has no type, and the article establishes no submission"
                ))
            })?;
            shape.fields = part_fields(
                entry,
                establishes,
                &declared_parameters(article),
                &shape.outputs,
            )?;
        }
    }
    Ok(shape)
}

/// Execute the article that establishes a submission with nothing: the
/// engine yields with the model of the submission (or computes at once, with
/// the same model on the result, if nothing required is asked).
fn execute_submission(
    service: &LawExecutionService,
    law_id: &str,
    outputs: &[String],
    day: NaiveDate,
) -> std::result::Result<Submission, String> {
    let output = outputs
        .first()
        .ok_or("establishes a submission, but has no output to execute it by")?;
    match service.execute_stage(law_id, output, None, BTreeMap::new(), &day.to_string()) {
        Ok(ExecutionOutcome::Yielded {
            submission: Some(s),
            ..
        }) => Ok(*s),
        Ok(ExecutionOutcome::Complete(r)) => r
            .submission
            .map(|s| *s)
            .ok_or_else(|| "the engine executed it without naming its submission".to_string()),
        Ok(ExecutionOutcome::Yielded { pending_inputs, .. }) => Err(format!(
            "the engine waits for {} on a procedure stage, not on the submission",
            pending_inputs.join(", ")
        )),
        Err(e) => Err(format!("executing it as a submission fails: {e}")),
    }
}

/// The fields of a submission: per article of the model, what its chronolex
/// entries contribute; then the decision requested filled in.
fn submission_fields(
    service: &LawExecutionService,
    shape: &mut Shape,
    model: &Submission,
    day: NaiveDate,
) -> Result<()> {
    // The decision taken on the application: one legal character, or the
    // general law would apply twice.
    let decision = match model.decisions.as_slice() {
        [one] => one,
        [] => {
            return Err(setup(format!(
                "{}: no decision is taken on this submission (produces.decides_on)",
                shape.establishes
            )))
        }
        more => {
            return Err(setup(format!(
                "{}: {} decisions are taken on this submission; this cell records one",
                shape.establishes,
                more.len()
            )))
        }
    };
    let decision_reference = format!("{}#{}", decision.law_id, decision.article_number);
    // The application belongs to the first stage of the procedure of that
    // decision (Awb: AANVRAAG).
    if shape.stage.is_none() {
        shape.stage = service
            .resolver()
            .find_procedure(&decision.legal_character, None)
            .and_then(|p| p.stages.first())
            .map(|s| s.name.clone());
    }

    for part in &model.articles {
        let reference = format!("{}#{}", part.law_id, part.article_number);
        let (_, article) = article_on(service, &reference, day)?;
        let Some(chronolex) = extension::of_article(article, &reference).map_err(setup)? else {
            continue;
        };
        let entries: Vec<&Establishment> = chronolex
            .establishes
            .iter()
            .filter(|e| match (&part.hook, &e.extends) {
                // The establishing article: its own event.
                (None, None) => e.event.as_deref() == Some(shape.event.as_str()),
                // A hook: what extends every submission of this kind.
                (Some(_), Some(Extends { submission })) => {
                    Some(submission.to_lowercase()) == shape.subtype
                }
                _ => false,
            })
            .collect();
        let parameters: Vec<Parameter> = model
            .inputs
            .iter()
            .filter(|i| i.law_id == part.law_id && i.article_number == part.article_number)
            .map(|i| i.parameter.clone())
            .collect();
        for entry in entries {
            for field in part_fields(entry, &reference, &parameters, &outputs(article))? {
                merge_field(&mut shape.fields, field);
            }
            if let Some(e) = &entry.effective_at {
                let basis = &mut shape
                    .effective_at
                    .get_or_insert_with(|| EffectiveAt {
                        legal_basis: Vec::new(),
                    })
                    .legal_basis;
                push_new(basis, &e.legal_basis);
            }
        }
    }

    // The decision requested is the decision taken on the application.
    for input in &model.inputs {
        let role = origin(&input.parameter, &shape.establishes)?.and_then(|o| o.rol);
        if role != Some(OriginRole::GevraagdBesluit) {
            continue;
        }
        if let Some(f) = shape
            .fields
            .iter_mut()
            .find(|f| f.name == input.parameter.name)
        {
            f.fixed = Some(serde_json::Value::String(decision_reference.clone()));
        }
    }

    Ok(())
}

/// A field asked by two articles is one field (one question), with both
/// legal bases.
fn merge_field(fields: &mut Vec<FieldDef>, field: FieldDef) {
    match fields.iter_mut().find(|f| f.name == field.name) {
        Some(existing) => push_new(&mut existing.legal_basis, &field.legal_basis),
        None => fields.push(field),
    }
}

/// The fields one chronolex entry of an article contributes.
fn part_fields(
    entry: &Establishment,
    reference: &str,
    parameters: &[Parameter],
    outputs: &[String],
) -> Result<Vec<FieldDef>> {
    let def = |name: &str, type_: Option<ParameterType>, legal_basis: Vec<String>| FieldDef {
        name: name.to_string(),
        type_,
        legal_basis,
        declared_by: reference.to_string(),
        fixed: None,
    };
    Ok(match entry.fields {
        None => Vec::new(),
        // Only what the applicant or the channel supplies is content of the
        // application.
        Some(Fields::Parameters) => {
            let mut out = Vec::new();
            for p in parameters {
                let Some(o) = origin(p, reference)? else {
                    continue;
                };
                if matches!(o.waarde, OriginValue::Belanghebbende | OriginValue::Kanaal) {
                    out.push(def(&p.name, Some(p.param_type), vec![o.grondslag.clone()]));
                }
            }
            out
        }
        Some(Fields::Outputs) => outputs
            .iter()
            .map(|o| def(o, None, vec![reference.to_string()]))
            .collect(),
    })
}

/// The origin of a parameter. One the law-model kept as invalid is an error
/// here: dropping it would make a field silently disappear from the gram.
fn origin<'p>(p: &'p Parameter, reference: &str) -> Result<Option<&'p Origin>> {
    p.origin
        .as_ref()
        .map(|o| {
            o.valid()
                .map_err(|e| setup(format!("{reference}: origin of '{}': {e}", p.name)))
        })
        .transpose()
}

/// Whether a submitted value fits the type the law declares. `null` fits
/// every type: an application may leave a field out (Awb 4:5 asks to
/// complete it, it is still an application).
pub fn fits(type_: Option<ParameterType>, value: &serde_json::Value) -> bool {
    use serde_json::Value as J;
    match (type_, value) {
        (_, J::Null) | (None, _) => true,
        (Some(ParameterType::String), J::String(_)) => true,
        (Some(ParameterType::Number | ParameterType::Amount), J::Number(_)) => true,
        (Some(ParameterType::Boolean), J::Bool(_)) => true,
        (Some(ParameterType::Date), J::String(s)) => {
            NaiveDate::parse_from_str(s, "%Y-%m-%d").is_ok()
        }
        (Some(ParameterType::Array), J::Array(_)) => true,
        (Some(ParameterType::Object), J::Object(_)) => true,
        _ => false,
    }
}

/// An engine value as JSON.
pub fn to_json(value: &Value) -> serde_json::Value {
    serde_json::to_value(value).unwrap_or(serde_json::Value::Null)
}
