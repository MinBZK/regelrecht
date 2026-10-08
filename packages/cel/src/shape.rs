//! The shape of a gram, from the law.
//!
//! For an application the cell executes the article that establishes it with
//! an empty application. The engine fires the hooks on it and yields with the
//! model of the application ([`regelrecht_engine::Submission`]): the articles
//! that take part and what each asks. Every part contributes what it asks of
//! the belanghebbende; Awb 4:2 on every application for a beschikking, the
//! establishing article with what is its own. For a decision the fields are
//! its outputs. What each article establishes or extends is read from what
//! the law says in its own words ([`extension::derive`]), or from an explicit
//! `extensions.chronolex` block if the article has one.

use std::collections::BTreeMap;

use chrono::NaiveDate;
use regelrecht_engine::{
    Article, ExecutionOutcome, LawExecutionService, StageInputs, Submission, Value,
};
use regelrecht_law_model::{
    Origin, OriginRole, OriginValue, Output, Parameter, ParameterType, Stage,
};
use serde::Serialize;

use crate::config::Event;
use crate::error::{setup, Result};
use crate::extension::{
    self, Chronolex, EffectiveAt, Establishment, Every, ExecutedOn, Extends, Fields,
    PeriodParameter, PeriodUnit, Reference, Until,
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
    /// The parameters of the establishing article.
    pub parameters: Vec<String>,
    /// For an execution: the parameter the day goes into (see
    /// [`crate::Cell::execute`]).
    pub executed_on: Option<ExecutedOn>,
    /// For an execution: the boolean output that says whether a gram arises.
    pub record_when: Option<String>,
    /// For a receipt: the parameter whose value identifies the message;
    /// one gram of the article per value.
    pub identified_by: Option<String>,
    /// For an execution: the stage whose gram ends it.
    pub until: Option<Until>,
    /// For a decision: the parameter that is the date it bears, which the
    /// cell fills with the day the decision is taken.
    pub dated_by: Option<String>,
}

impl Shape {
    pub fn field(&self, name: &str) -> Option<&FieldDef> {
        self.fields.iter().find(|f| f.name == name)
    }

    /// Whether a gram of the event arises on receipt of what another party
    /// sends: an execution that says when it arises (`record_when`) but on
    /// no day (`executed_on`). See [`crate::Cell::receive`].
    pub fn is_receipt(&self) -> bool {
        self.record_when.is_some() && self.executed_on.is_none()
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

fn declared_outputs(article: &Article) -> Vec<Output> {
    article
        .get_execution_spec()
        .and_then(|e| e.output.clone())
        .unwrap_or_default()
}

fn declared_parameters(article: &Article) -> Vec<Parameter> {
    article
        .get_execution_spec()
        .and_then(|e| e.parameters.clone())
        .unwrap_or_default()
}

/// Derive the shape of `event` from the law as it applies on `day`, or, if
/// the version in force does not establish it at all, from the first later
/// version that does: the law already enacted for a coming period, such as
/// a decision taken before the period it concerns begins, under the law of
/// that period.
///
/// Only the absence of the event falls through to a later version: no
/// version in force, no such article, or an article that names no such
/// event in its `extensions.chronolex`. A version in force that does name
/// the event but cannot give it a shape (a reference it cannot resolve, an
/// execution without `executed_on`) is an error in the law that applies, and
/// a later version must not hide it.
pub fn derive_for(service: &LawExecutionService, event: &Event, day: NaiveDate) -> Result<Shape> {
    let first = match derive(service, event, day) {
        Ok(shape) => return Ok(shape),
        Err(e) => e,
    };
    if establishes_event(service, event, day) {
        return Err(first);
    }
    let (law_id, _) = split_reference(&event.establishes)?;
    let mut later: Vec<NaiveDate> = service
        .resolver()
        .all_law_versions()
        .filter(|l| l.id == law_id)
        .filter_map(|l| l.valid_from.as_deref())
        .filter_map(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
        .filter(|d| *d > day)
        .collect();
    later.sort();
    later
        .into_iter()
        .find(|d| establishes_event(service, event, *d))
        .map_or(Err(first), |d| derive(service, event, d))
}

/// Whether the version of the law in force on `day` establishes `event`:
/// it has the article, and the article establishes what the event names.
/// A block that cannot be read counts as establishing it, so its error is
/// the one reported.
fn establishes_event(service: &LawExecutionService, event: &Event, day: NaiveDate) -> bool {
    let Ok((_, article)) = article_on(service, &event.establishes, day) else {
        return false;
    };
    match chronolex_of(service, article, &event.establishes, day) {
        Ok(Some(chronolex)) => entry_of(&chronolex, event).is_ok(),
        Ok(None) => false,
        Err(_) => true,
    }
}

/// What an article establishes or extends: its explicit chronolex block, or
/// what the law says in its own words ([`extension::derive`]). For a
/// decision, the stages of its procedure that are a decision say where it
/// is taken, as the procedure reads on `day`. A procedure the article names
/// and no loaded law defines is an error, not a decision without stages: the
/// article would silently establish nothing.
fn chronolex_of(
    service: &LawExecutionService,
    article: &Article,
    reference: &str,
    day: NaiveDate,
) -> Result<Option<Chronolex>> {
    if let Some(explicit) = extension::of_article(article, reference).map_err(setup)? {
        return Ok(Some(explicit));
    }
    let (law_id, _) = split_reference(reference)?;
    let procedure = service
        .procedure_of(law_id, article, Some(day))
        .map_err(|e| setup(format!("{reference}: {e}")))?;
    let decision_stages: Vec<&Stage> = procedure
        .map(|p| {
            p.stages
                .iter()
                .filter(|s| s.name == "BESLUIT" || s.is.as_deref() == Some("BESLUIT"))
                .collect()
        })
        .unwrap_or_default();
    extension::derive(article, &decision_stages).map_err(|e| setup(format!("{reference}: {e}")))
}

/// The establishment of `chronolex` that `event` records. In an explicit
/// block it is the one named after the event; in a derived one it is the
/// article's own establishment, at the stage the stream names if the
/// article decides at more than one.
fn entry_of<'c>(chronolex: &'c Chronolex, event: &Event) -> Result<&'c Establishment> {
    let establishes = &event.establishes;
    if !chronolex.derived {
        let entry = chronolex
            .establishes
            .iter()
            .find(|e| e.event.as_deref() == Some(event.name.as_str()))
            .ok_or_else(|| {
                setup(format!(
                    "{establishes}: establishes no event '{}'",
                    event.name
                ))
            })?;
        // The block names the stage itself; a stream that names another
        // contradicts it, and is not silently overruled.
        if let Some(stage) = &event.stage {
            if entry.stage.as_deref() != Some(stage.as_str()) {
                return Err(setup(format!(
                    "{establishes}: the stream records '{}' at stage {stage}, the article at {}",
                    event.name,
                    entry.stage.as_deref().unwrap_or("no stage")
                )));
            }
        }
        return Ok(entry);
    }
    let own: Vec<&Establishment> = chronolex.own().collect();
    match (&event.stage, own.as_slice()) {
        (Some(stage), _) => own
            .iter()
            .find(|e| e.stage.as_deref() == Some(stage.as_str()))
            .copied()
            .ok_or_else(|| {
                setup(format!(
                    "{establishes}: takes no decision at stage {stage}; it decides at {}",
                    own.iter()
                        .filter_map(|e| e.stage.as_deref())
                        .collect::<Vec<_>>()
                        .join(", ")
                ))
            }),
        (None, [one]) => Ok(one),
        (None, []) => Err(setup(format!(
            "{establishes}: establishes nothing of its own; it only extends a submission"
        ))),
        (None, more) => Err(setup(format!(
            "{establishes}: decides at {} stages ({}); the event '{}' must say which (`stage`)",
            more.len(),
            more.iter()
                .filter_map(|e| e.stage.as_deref())
                .collect::<Vec<_>>()
                .join(", "),
            event.name
        ))),
    }
}

/// Derive the shape of `event` from the law as it applies on `day`.
pub fn derive(service: &LawExecutionService, event: &Event, day: NaiveDate) -> Result<Shape> {
    let establishes = event.establishes.as_str();
    let (law, article) = article_on(service, establishes, day)?;
    let chronolex = chronolex_of(service, article, establishes, day)?.ok_or_else(|| {
        setup(format!(
            "{establishes}: establishes nothing: no produces.submission, no BESCHIKKING with decides_on, no hook on a submission, and no extensions.chronolex"
        ))
    })?;
    let entry = entry_of(&chronolex, event)?;
    let produces = article.get_produces();
    let mut shape = Shape {
        event: event.name.clone(),
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
        outputs: declared_outputs(article)
            .into_iter()
            .map(|o| o.name)
            .collect(),
        parameters: declared_parameters(article)
            .into_iter()
            .map(|p| p.name)
            .collect(),
        executed_on: entry.executed_on.clone(),
        record_when: entry.record_when.clone(),
        identified_by: entry.identified_by.clone(),
        until: entry.until.clone(),
        dated_by: entry.dated_by.clone(),
    };
    for (name, reference) in &shape.refers_to {
        reference
            .check(name)
            .map_err(|e| setup(format!("{establishes}: {e}")))?;
    }
    check_execution(&shape, article)?;
    check_dated_by(service, &shape, day)?;
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
                    "{establishes}: event '{}' has no type, and the article establishes no submission",
                    event.name
                ))
            })?;
            shape.fields = part_fields(
                entry,
                establishes,
                &declared_parameters(article),
                &declared_outputs(article),
            )?;
            // What the decision concerns, if the law says it concerns a
            // period: the parameter with origin role TIJDVAK among what the
            // decision asks at its stage (Awir 14 jo. 15: "een tegemoetkoming
            // met betrekking tot een berekeningsjaar").
            if chronolex.derived {
                if let Some(stage) = &entry.stage {
                    shape.period = tijdvak(service, &shape, stage, day)?;
                }
            }
            // A decision taken at a stage of its procedure is the article
            // with the hooks that fire at that stage (a general law that hooks
            // onto every besluit, or onto one stage). What they produce is
            // part of the decision too, unless the law names the fields one
            // by one.
            let named = matches!(entry.fields, Some(Fields::Named(_)));
            if let (Some(stage), false) = (&entry.stage, named) {
                let at = stage_inputs(service, &shape, stage, day)?;
                for part in at.articles.iter().filter(|a| a.hook.is_some()) {
                    let reference = format!("{}#{}", part.law_id, part.article_number);
                    let (_, hook) = article_on(service, &reference, day)?;
                    for field in part_fields(entry, &reference, &[], &declared_outputs(hook))? {
                        merge_field(&mut shape.fields, field);
                    }
                }
            }
        }
    }
    // The value that identifies a received message is what the cell compares
    // a second message against, so every gram of the event holds it.
    if let Some(key) = &shape.identified_by {
        if shape.field(key).is_none() {
            return Err(setup(format!(
                "{establishes}: identified_by: '{key}' is not a field of '{}'",
                shape.event
            )));
        }
    }
    Ok(shape)
}

/// The period a decision at `stage` concerns: the one parameter with origin
/// role TIJDVAK among what the decision asks there (RFC-048), as a calendar
/// year when it is a number. The applicant chooses it as part of the
/// decision requested (Awb 4:2 lid 1), so it is in the application.
fn tijdvak(
    service: &LawExecutionService,
    shape: &Shape,
    stage: &str,
    day: NaiveDate,
) -> Result<Option<PeriodParameter>> {
    let inputs = stage_inputs(service, shape, stage, day)?;
    let mut names: Vec<(&str, ParameterType)> = Vec::new();
    for input in &inputs.inputs {
        let reference = format!("{}#{}", input.law_id, input.article_number);
        let Some(o) = origin(&input.parameter, &reference)? else {
            continue;
        };
        if o.rol == Some(OriginRole::Tijdvak)
            && !names.iter().any(|(n, _)| *n == input.parameter.name)
        {
            names.push((&input.parameter.name, input.parameter.param_type));
        }
    }
    match names.as_slice() {
        [] => Ok(None),
        [(name, ParameterType::Number)] => Ok(Some(PeriodParameter {
            parameter: name.to_string(),
            unit: PeriodUnit::Year,
        })),
        [(name, other)] => Err(setup(format!(
            "{}: the TIJDVAK '{name}' at stage {stage} is a {other:?}; only a year (a number) is a period the cell knows",
            shape.establishes
        ))),
        more => Err(setup(format!(
            "{}: stage {stage} asks {} parameters with role TIJDVAK ({}); a decision concerns one period",
            shape.establishes,
            more.len(),
            more.iter().map(|(n, _)| *n).collect::<Vec<_>>().join(", ")
        ))),
    }
}

/// An execution says on which parameter its day goes, and when a gram
/// arises: a parameter of type date, and a boolean output of the article.
fn check_execution(shape: &Shape, article: &Article) -> Result<()> {
    let at = |what: String| setup(format!("{}: {what}", shape.establishes));
    if let Some(on) = &shape.executed_on {
        let declared = declared_parameters(article);
        let p = declared
            .iter()
            .find(|p| p.name == on.parameter)
            .ok_or_else(|| {
                at(format!(
                    "executed_on: the article has no parameter '{}'",
                    on.parameter
                ))
            })?;
        if p.param_type != ParameterType::Date {
            return Err(at(format!(
                "executed_on: parameter '{}' is not a date",
                on.parameter
            )));
        }
    }
    if let Some(when) = &shape.record_when {
        let declared = declared_outputs(article);
        let o = declared
            .iter()
            .find(|o| &o.name == when)
            .ok_or_else(|| at(format!("record_when: the article has no output '{when}'")))?;
        if o.output_type != ParameterType::Boolean {
            return Err(at(format!("record_when: output '{when}' is not a boolean")));
        }
    }
    if let Some(on) = &shape.executed_on {
        match (on.day, on.once_per) {
            (None, _) => {}
            (Some(day), Some(Every::Month)) if (1..=31).contains(&day) => {}
            (Some(day), Some(Every::Month)) => {
                return Err(at(format!("executed_on: day {day} is no day of a month")))
            }
            (Some(_), None) => {
                return Err(at(
                    "executed_on: a day needs a period it is the day of (once_per)".to_string(),
                ))
            }
        }
    }
    // The value that identifies a received message comes with the message:
    // a parameter it must carry, not something the article computes.
    if let Some(key) = &shape.identified_by {
        if !shape.is_receipt() {
            return Err(at(format!(
                "identified_by: '{}' is no receipt (record_when, no executed_on)",
                shape.event
            )));
        }
        if declared_outputs(article).iter().any(|o| &o.name == key) {
            return Err(at(format!(
                "identified_by: '{key}' is an output of the article, not what the message carries"
            )));
        }
        let p = declared_parameters(article)
            .into_iter()
            .find(|p| &p.name == key)
            .ok_or_else(|| {
                at(format!(
                    "identified_by: the article has no parameter '{key}'"
                ))
            })?;
        if p.required != Some(true) {
            return Err(at(format!(
                "identified_by: parameter '{key}' is not required; a message without it could not be told from another"
            )));
        }
    }
    // A sender that did not hear its message land delivers it again. Only a
    // receipt that recognises a message it already answered records nothing
    // twice: by a required reference to the gram it answers, or by the value
    // that identifies it.
    if shape.is_receipt()
        && shape.identified_by.is_none()
        && !shape.refers_to.values().any(|r| r.required)
    {
        return Err(at(format!(
            "'{}' is a receipt: it needs a required refers_to or identified_by, so a message delivered again is not recorded twice",
            shape.event
        )));
    }
    // An execution is executed on a day (`executed_on`), or on receipt of
    // what another party sends (no `executed_on`, see `Cell::receive`); it
    // says when a gram arises, and is taken at no stage of a procedure.
    let execution = shape.executed_on.is_some() || shape.record_when.is_some();
    if execution && (shape.record_when.is_none() || shape.stage.is_some()) {
        return Err(at(format!(
            "'{}' is an execution: it needs record_when, and is taken at no stage",
            shape.event
        )));
    }
    Ok(())
}

/// The date a decision bears is a date its stage requires: `dated_by` names
/// a value of type date in the `requires` of the stage it is taken at.
fn check_dated_by(service: &LawExecutionService, shape: &Shape, day: NaiveDate) -> Result<()> {
    let Some(dated_by) = &shape.dated_by else {
        return Ok(());
    };
    let at = |what: String| setup(format!("{}: dated_by: {what}", shape.establishes));
    let stage = shape
        .stage
        .as_deref()
        .ok_or_else(|| at(format!("'{}' is taken at no stage", shape.event)))?;
    let inputs = stage_inputs(service, shape, stage, day)?;
    match inputs.requires.iter().find(|r| &r.name == dated_by) {
        Some(r) if r.req_type == ParameterType::Date => Ok(()),
        Some(_) => Err(at(format!("'{dated_by}' is not a date"))),
        None => Err(at(format!("stage {stage} requires no '{dated_by}'"))),
    }
}

/// What executing the decision `shape` describes at `stage` of its procedure
/// asks, in the law as it applies on `day` ([`LawExecutionService::stage_inputs`]).
pub fn stage_inputs(
    service: &LawExecutionService,
    shape: &Shape,
    stage: &str,
    day: NaiveDate,
) -> Result<StageInputs> {
    let (_, number) = split_reference(&shape.establishes)?;
    service
        .stage_inputs(
            &shape.law_id,
            number,
            stage,
            &BTreeMap::new(),
            &day.to_string(),
        )
        .map_err(|e| setup(format!("{}: {e}", shape.establishes)))
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
            .find_procedure_reported_at(&decision.legal_character, None, Some(day))
            .ok()
            .and_then(|p| p.stages.first())
            .map(|s| s.name.clone());
    }

    for part in &model.articles {
        let reference = format!("{}#{}", part.law_id, part.article_number);
        let (_, article) = article_on(service, &reference, day)?;
        let Some(chronolex) = chronolex_of(service, article, &reference, day)? else {
            continue;
        };
        let entries: Vec<&Establishment> = chronolex
            .establishes
            .iter()
            .filter(|e| match (&part.hook, &e.extends) {
                // The establishing article: its own event.
                (None, None) => {
                    chronolex.derived || e.event.as_deref() == Some(shape.event.as_str())
                }
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
            for field in part_fields(entry, &reference, &parameters, &declared_outputs(article))? {
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
    outputs: &[Output],
) -> Result<Vec<FieldDef>> {
    let def = |name: &str, type_: Option<ParameterType>, legal_basis: Vec<String>| FieldDef {
        name: name.to_string(),
        type_,
        legal_basis,
        declared_by: reference.to_string(),
        fixed: None,
    };
    Ok(match &entry.fields {
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
            .map(|o| def(&o.name, Some(o.output_type), vec![reference.to_string()]))
            .collect(),
        // An output, or a parameter of the article: what it was given (the
        // reference a channel delivered with, say).
        Some(Fields::Named(names)) => names
            .iter()
            .map(|name| {
                if let Some(o) = outputs.iter().find(|o| &o.name == name) {
                    return Ok(def(
                        &o.name,
                        Some(o.output_type),
                        vec![reference.to_string()],
                    ));
                }
                let p = parameters.iter().find(|p| &p.name == name).ok_or_else(|| {
                    setup(format!(
                        "{reference}: fields: the article has no output or parameter '{name}'"
                    ))
                })?;
                let basis = origin(p, reference)?
                    .map_or_else(|| reference.to_string(), |o| o.grondslag.clone());
                Ok(def(&p.name, Some(p.param_type), vec![basis]))
            })
            .collect::<Result<_>>()?,
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

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    /// A version of `testwet` valid from `valid_from`, whose article 1 holds
    /// `chronolex` as its `extensions.chronolex` (or none).
    fn version(valid_from: &str, chronolex: Option<&str>) -> String {
        let extensions = chronolex.map_or(String::new(), |c| {
            format!(
                "        produces:\n          extensions:\n            chronolex:\n{}",
                c.lines()
                    .map(|l| format!("              {l}\n"))
                    .collect::<String>()
            )
        });
        format!(
            "$schema: https://raw.githubusercontent.com/MinBZK/regelrecht/refs/tags/schema-v0.8.0/schema/v0.8.0/schema.json
$id: testwet
regulatory_layer: WET
publication_date: '2024-01-01'
valid_from: '{valid_from}'
name: Testwet
url: https://example.org/testwet
articles:
  - number: '1'
    text: Test.
    url: https://example.org/testwet#1
    machine_readable:
      execution:
{extensions}        parameters:
          - name: x
            type: number
            required: true
        output:
          - name: y
            type: number
        actions:
          - output: y
            value: $x
"
        )
    }

    const ESTABLISHES: &str =
        "establishes:\n  - event: gebeurd\n    type: decretogram\n    fields: outputs";

    fn service(versions: &[String]) -> LawExecutionService {
        let mut service = LawExecutionService::new();
        for v in versions {
            service.load_law(v).unwrap_or_else(|e| panic!("{e}"));
        }
        service
    }

    fn day(s: &str) -> NaiveDate {
        s.parse().unwrap()
    }

    fn gebeurd() -> Event {
        Event {
            name: "gebeurd".into(),
            establishes: "testwet#1".into(),
            stage: None,
            reads: Vec::new(),
        }
    }

    #[test]
    fn a_version_that_does_not_establish_the_event_falls_through_to_a_later_one() {
        let service = service(&[
            version("2024-01-01", None),
            version("2025-01-01", Some(ESTABLISHES)),
        ]);
        let shape = derive_for(&service, &gebeurd(), day("2024-11-20")).unwrap();
        assert_eq!(shape.outputs, vec!["y".to_string()]);
    }

    #[test]
    fn a_version_that_establishes_the_event_but_fails_does_not_fall_through() {
        // The version in force names the event, with an output that is not
        // there: an error in the law that applies, not an absence.
        let broken = format!("{ESTABLISHES}\n    record_when: bestaat_niet");
        let service = service(&[
            version("2024-01-01", Some(&broken)),
            version("2025-01-01", Some(ESTABLISHES)),
        ]);
        let e = derive_for(&service, &gebeurd(), day("2024-11-20")).unwrap_err();
        assert!(e.to_string().contains("bestaat_niet"), "{e}");
    }

    #[test]
    fn without_any_version_that_establishes_the_event_the_first_error_stands() {
        let service = service(&[version("2024-01-01", None), version("2025-01-01", None)]);
        let e = derive_for(&service, &gebeurd(), day("2024-11-20")).unwrap_err();
        assert!(e.to_string().contains("no extensions.chronolex"), "{e}");
    }
    /// A decision on an application of `testwet#1`, in a law with its own
    /// procedure `eigen`; `produces` is the rest of its `produces` block.
    fn decision(produces: &str) -> String {
        format!(
            "$id: testbesluit
regulatory_layer: WET
publication_date: '2024-01-01'
valid_from: '2024-01-01'
procedure:
  - id: eigen
    applies_to: {{legal_character: BESCHIKKING}}
    stages:
      - name: BESLUIT
articles:
  - number: '1'
    text: Test.
    machine_readable:
      execution:
        produces:
          legal_character: BESCHIKKING
{produces}
        output: [{{name: y, type: number}}]
        actions: [{{output: y, value: 1}}]
"
        )
    }

    fn besloten(stage: Option<&str>) -> Event {
        Event {
            name: "besloten".into(),
            establishes: "testbesluit#1".into(),
            stage: stage.map(str::to_string),
            reads: Vec::new(),
        }
    }

    #[test]
    fn a_decision_takes_its_stages_from_the_procedure_it_names() {
        let service = service(&[
            version("2024-01-01", None),
            decision("          procedure_id: eigen\n          decides_on: [testwet#1]"),
        ]);
        let shape = derive(&service, &besloten(None), day("2024-06-01")).unwrap();
        assert_eq!(shape.stage.as_deref(), Some("BESLUIT"));
    }

    #[test]
    fn a_procedure_the_article_names_and_no_law_defines_is_an_error() {
        let service = service(&[
            version("2024-01-01", None),
            decision("          procedure_id: bestaat_niet\n          decides_on: [testwet#1]"),
        ]);
        let e = derive(&service, &besloten(None), day("2024-06-01")).unwrap_err();
        assert!(matches!(e, crate::Error::Setup(_)), "{e}");
        assert!(e.to_string().contains("'bestaat_niet'"), "{e}");
    }

    #[test]
    fn a_decision_on_more_than_one_submission_is_an_error() {
        let service = service(&[
            version("2024-01-01", None),
            decision("          procedure_id: eigen\n          decides_on: [testwet#1, testwet#2]"),
        ]);
        let e = derive(&service, &besloten(None), day("2024-06-01")).unwrap_err();
        assert!(
            e.to_string().contains("decides_on names 2 submissions"),
            "{e}"
        );
    }

    #[test]
    fn a_stream_stage_must_match_the_stage_of_an_explicit_block() {
        let service = service(&[decision(
            "          procedure_id: eigen
          extensions:
            chronolex:
              establishes:
                - {event: besloten, type: decretogram, stage: BESLUIT, fields: outputs}",
        )]);
        derive(&service, &besloten(Some("BESLUIT")), day("2024-06-01")).unwrap();
        derive(&service, &besloten(None), day("2024-06-01")).unwrap();
        let e = derive(&service, &besloten(Some("VOORSCHOT")), day("2024-06-01")).unwrap_err();
        assert!(e.to_string().contains("stage VOORSCHOT"), "{e}");
    }

    /// A law whose article 1 is received (`record_when: ontvangen`, no
    /// `executed_on`) as `gekregen`, with `entry` the rest of its entry;
    /// parameter `kenmerk` is `required` as given, output `bedrag` is computed.
    fn receipt(entry: &str, required: bool) -> String {
        format!(
            "$id: ontvangstwet
regulatory_layer: WET
publication_date: '2024-01-01'
valid_from: '2024-01-01'
articles:
  - number: '1'
    text: Test.
    machine_readable:
      execution:
        produces:
          extensions:
            chronolex:
              establishes:
                - event: gekregen
                  type: executogram
                  fields: [kenmerk, bedrag]
                  record_when: ontvangen
{entry}
        parameters:
          - name: kenmerk
            type: string
            required: {required}
        output:
          - {{name: ontvangen, type: boolean}}
          - {{name: bedrag, type: number}}
        actions:
          - {{output: ontvangen, value: true}}
          - {{output: bedrag, value: 1}}
"
        )
    }

    fn gekregen() -> Event {
        Event {
            name: "gekregen".into(),
            establishes: "ontvangstwet#1".into(),
            stage: None,
            reads: Vec::new(),
        }
    }

    fn derive_receipt(entry: &str, required: bool) -> Result<Shape> {
        let service = service(&[receipt(entry, required)]);
        derive(&service, &gekregen(), day("2024-06-01"))
    }

    #[test]
    fn a_receipt_identified_by_a_required_parameter_is_a_shape() {
        let shape = derive_receipt("                  identified_by: kenmerk", true).unwrap();
        assert_eq!(shape.identified_by.as_deref(), Some("kenmerk"));
    }

    #[test]
    fn a_receipt_that_cannot_tell_a_message_delivered_again_is_an_error() {
        let e = derive_receipt("", true).unwrap_err();
        assert!(matches!(e, crate::Error::Setup(_)), "{e}");
        assert!(
            e.to_string()
                .contains("needs a required refers_to or identified_by"),
            "{e}"
        );
        // A reference that is not required does not tell it either.
        let optional =
            "                  refers_to:\n                    opdracht: {to: ontvangstwet#1}";
        let e = derive_receipt(optional, true).unwrap_err();
        assert!(
            e.to_string()
                .contains("needs a required refers_to or identified_by"),
            "{e}"
        );
        let required = "                  refers_to:\n                    opdracht: {to: ontvangstwet#1, required: true}";
        derive_receipt(required, true).unwrap();
    }

    #[test]
    fn a_receipt_identified_by_a_parameter_that_is_not_required_is_an_error() {
        let e = derive_receipt("                  identified_by: kenmerk", false).unwrap_err();
        assert!(e.to_string().contains("'kenmerk' is not required"), "{e}");
    }

    #[test]
    fn a_receipt_identified_by_an_output_is_an_error() {
        let e = derive_receipt("                  identified_by: bedrag", true).unwrap_err();
        assert!(e.to_string().contains("'bedrag' is an output"), "{e}");
    }
}
