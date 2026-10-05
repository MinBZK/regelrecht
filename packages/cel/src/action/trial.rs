//! An action on trial in a case: the decision it acts on, the
//! reference date, the lexostatuses and the synthesis, and the engine.

use super::*;

/// The decisions in the case of the action `decision`, in the order in which
/// the cell recorded them, with the decisions that amend them (from an action
/// with `decision: amends` that names `decision`). The last one is the decision
/// as it currently holds.
fn chain<'z>(process: &Process, decision: &str, case: &'z CaseState) -> Vec<&'z DecisionState> {
    let Some(b) = &process.definition.handling else {
        return Vec::new();
    };
    let events: Vec<&ActionDefinition> = b
        .actions
        .iter()
        .filter(|h| {
            h.name == decision
                || (h.decision_role == Some(Decision::Amends)
                    && h.decision.as_deref() == Some(decision))
        })
        .collect();
    case.decisions
        .iter()
        .filter(|s| {
            events
                .iter()
                .any(|h| s.of(&h.record.stream, &h.record.event))
        })
        .collect()
}

/// The decisions in the case that the action `decision` itself recorded.
pub(super) fn own<'z>(
    process: &Process,
    decision: &str,
    case: &'z CaseState,
) -> Vec<&'z DecisionState> {
    let Some(b) = process
        .definition
        .handling
        .as_ref()
        .and_then(|b| b.action(decision))
    else {
        return Vec::new();
    };
    case.decisions
        .iter()
        .filter(|s| s.of(&b.record.stream, &b.record.event))
        .collect()
}

/// The decision in the case that an action acts on, from the
/// [`CaseState`]: for a follow-up a decision of the action of the
/// decision (the stage continues on it), for a fact that follows a decision
/// and for an amendment a decision of the action they name, including
/// an amendment of it. If the handler names a decision (an id)
/// (`chosen`), that decision, if it is one of them; otherwise the latest.
/// `Ok(None)`: the action belongs to no decision, or opens one itself.
/// `Err`: the decision does not exist yet, or the chosen decision is not one
/// the action acts on.
pub fn target<'z>(
    process: &Process,
    h: &ActionDefinition,
    case: &'z CaseState,
    chosen: Option<&str>,
) -> Result<Option<&'z DecisionState>, String> {
    let (list, of) = match (&h.kind, h.decision_role) {
        (ActionKind::FollowUp { decision, .. }, _) => (own(process, decision, case), decision),
        (_, Some(Decision::Follows | Decision::Amends)) => {
            let Some(b) = h.decision.as_ref() else {
                return Ok(None);
            };
            (chain(process, b, case), b)
        }
        _ => {
            return match chosen {
                Some(k) => Err(format!(
                    "action '{}' acts on no decision, and decision {k} was named",
                    h.name
                )),
                None => Ok(None),
            }
        }
    };
    if let Some(k) = chosen {
        return list
            .iter()
            .find(|b| b.id == k)
            .map(|b| Some(*b))
            .ok_or_else(|| {
                let can: Vec<&str> = list.iter().map(|b| b.id.as_str()).collect();
                format!(
                    "decision {k} is not a decision that action '{}' acts on (valid: {})",
                    h.name,
                    if can.is_empty() {
                        "none".to_string()
                    } else {
                        can.join(", ")
                    }
                )
            });
    }
    match list.last() {
        Some(b) => Ok(Some(b)),
        None => {
            let label = process
                .definition
                .handling
                .as_ref()
                .and_then(|b| b.action(of))
                .map_or(of.as_str(), |b| b.label());
            Err(format!("waiting for the decision ({label})"))
        }
    }
}

fn reference(b: &DecisionState) -> Option<DecisionReference> {
    let (stage, gram) = b.taken()?;
    Some(DecisionReference {
        id: b.id.clone(),
        name: gram.event.clone(),
        stage: Some(stage.clone()),
        effective_at: gram.effective_at.clone(),
        recorded_at: gram.recorded_at.clone(),
    })
}

/// A lexostatus of the case, requested from the cell. If the cell has no
/// gram for it (404), it delivers nothing: an empty lexostatus. With a
/// draft the cell reduces on trial, as if the draft were already recorded.
async fn case_lexostatus(
    env: &Environment<'_>,
    source: &crate::config::SynthesisSource,
    root: &str,
    as_of: &AsOf,
    concept: Option<&RecordRequest>,
) -> Result<(Lexostatus, Option<Gram>), Refusal> {
    let def = env
        .process
        .cell
        .lexostatuses
        .lexostatus(&source.lexostatus)
        .ok_or_else(|| {
            Refusal::Cell(format!("lexostatus '{}' does not exist", source.lexostatus))
        })?;
    let mut inputs = Map::new();
    inputs.insert("root".into(), Value::String(root.to_string()));
    let response = match concept {
        None => env
            .cell
            .fetch(&synthesis::path(
                &source.cell,
                &source.lexostatus,
                &inputs,
                as_of,
            ))
            .await
            .and_then(|v| {
                serde_json::from_value::<Lexostatus>(v)
                    .map(|l| (l, None))
                    .map_err(|e| TransportError::Json(e.to_string()))
            }),
        Some(c) => {
            for (k, v) in as_of.query() {
                inputs.insert(k.into(), Value::String(v));
            }
            cell_client::trial(env.cell, &source.cell, &source.lexostatus, c, &inputs)
                .await
                .map(|p| (p.lexostatus, Some(p.gram)))
        }
    };
    match response {
        Ok(l) => Ok(l),
        // If the definition picks a gram and there is none, it delivers
        // nothing. Any other 404 (a wrong route, an unknown lexostatus) is
        // an error of the cell.
        Err(TransportError::Response { status: 404, error }) if error == synthesis::NO_GRAM => {
            Ok((
                Lexostatus {
                    not_derived: def.reduction.derivations.keys().cloned().collect(),
                    ..Lexostatus::empty(&def.name)
                },
                None,
            ))
        }
        // The draft does not fit in a gram: an error in the form.
        Err(TransportError::Response { status: 400, error }) => Err(Refusal::Invalid(error)),
        Err(TransportError::Response { status: 409, error }) => Err(Refusal::Conflict(error)),
        Err(f) => Err(Refusal::Cell(format!(
            "cell '{}', lexostatus '{}': {f}",
            source.cell, source.lexostatus
        ))),
    }
}

/// The reference date of an action: the day of the `effective_at` that the event
/// binds to a field of the form (the day of the decision, the
/// publication, the payment), with the legal basis the stream gives
/// for it; otherwise today. A decision thus reads the law and the cells on the day
/// it is taken, even if the handler records it later.
/// The third part is an objection to that moment: it lies after today (what
/// has yet to happen is not a fact). The fourth is the form field and the
/// day, if the form binds the moment: whether it lies before a gram it
/// refers to follows once the decision the action acts on is known (see
/// [`before_a_reference`]).
#[allow(clippy::type_complexity)]
fn reference_date(
    event: &Event,
    form: &Map<String, Value>,
    nu: &DateTime<FixedOffset>,
) -> Result<(String, String, Option<String>, Option<(String, String)>), Refusal> {
    let bound =
        crate::stream::bound_moment(event, None, form, *nu.offset()).map_err(Refusal::Invalid)?;
    let Some((moment, b)) = bound else {
        return Ok((date::reference_date(nu), "today".to_string(), None, None));
    };
    let path = b.source.strip_prefix("$external.").unwrap_or(&b.source);
    let day = date::reference_date(&moment);
    let objection = (moment > *nu)
        .then(|| format!("{path} {day} lies after today: what has yet to happen is not a fact"));
    Ok((
        day.clone(),
        format!("{path} (effective_at, {})", b.legal_basis.join(", ")),
        objection,
        Some((path.to_string(), day)),
    ))
}

/// The rule of the cell, said beforehand: a gram does not lie on a day before
/// a gram it refers to (the root of the case, or the decision the action
/// acts on); what follows moves forward in time. The cell refuses such a
/// gram when recording (`not_for` in the cell API); a later fact of the
/// case that the gram does not refer to does not bind it.
async fn before_a_reference(
    env: &Environment<'_>,
    event: &Event,
    root: &str,
    decision: Option<&DecisionReference>,
    (path, day): (&str, &str),
) -> Result<Option<String>, Refusal> {
    let refers_to = super::references(
        &env.process.cell,
        event,
        root,
        decision.map(|b| b.id.as_str()),
    );
    if refers_to.is_empty() {
        return Ok(None);
    }
    let grams = cell_client::read_case(env.cell, env.process.cell.id(), root)
        .await
        .map_err(|f| Refusal::Cell(format!("case {root}: {f}")))?;
    for (name, id) in &refers_to {
        let Some(g) = grams.iter().map(|g| &g.gram).find(|g| &g.id == id) else {
            continue;
        };
        let d = date::reference_date_of(&g.effective_at).map_err(Refusal::Cell)?;
        if day < d.as_str() {
            return Ok(Some(format!(
                "{path} {day} lies before the gram it refers to ({name}: '{}', {d}); what follows moves forward in time",
                g.name
            )));
        }
    }
    Ok(None)
}

/// Why an action is not taken on its own.
enum Objection {
    /// The form: the form fields, the order of the case, the moment. Then
    /// nothing is reported either.
    Shape(String),
    /// The substance: the law says no, or cannot say what the fact does.
    /// A fact that happened is then recorded by the cell anyway.
    Content(String),
}

/// Compute an action in a case, without recording anything. `case`
/// is the state of the case as the cell derives it.
pub async fn trial(
    env: &Environment<'_>,
    h: &ActionDefinition,
    root: &str,
    case: &CaseState,
    action_input: &ActionInput,
) -> Result<TrialAction, Refusal> {
    let process = env.process;
    let form = &action_input.form;
    for name in form.keys() {
        if !h.verdicts.iter().any(|o| &o.parameter == name)
            && !h.facts.iter().any(|f| &f.name == name)
        {
            return Err(Refusal::Invalid(format!(
                "'{name}' is not a field of the form of action '{}'",
                h.name
            )));
        }
    }
    let (_, event) = process
        .cell
        .event(&h.record.stream, &h.record.event)
        .ok_or_else(|| Refusal::Cell(format!("action '{}': no record event", h.name)))?;
    let (reference_date, reference_date_from, time, bound) = reference_date(event, form, &env.now)?;
    let mut p = TrialAction {
        action: h.name.clone(),
        kind: h.kind.clone(),
        stage: h.stage.clone(),
        regulation: h.regulation.clone(),
        article: h.article.clone(),
        reference_date: reference_date.clone(),
        reference_date_from,
        takeable: false,
        reportable: false,
        outputs: BTreeMap::new(),
        assessments: BTreeMap::new(),
        types: h.types.clone(),
        missing: Vec::new(),
        reason: None,
        parameters: BTreeMap::new(),
        provenance: BTreeMap::new(),
        sources: Vec::new(),
        not_delivered: Vec::new(),
        lexostatuses: Vec::new(),
        rows: Vec::new(),
        decision: None,
        trace_text: None,
    };
    let absent: Vec<String> = h
        .facts
        .iter()
        .filter(|f| form.get(&f.name).is_none_or(Value::is_null))
        .map(|f| f.name.clone())
        .collect();
    // The decision the action acts on. If it does not exist yet, there is
    // nothing to compute: that is the form (the order of the case).
    let target = match target(process, h, case, action_input.decision.as_deref()) {
        Ok(b) => b,
        Err(r) => {
            p.reason = Some(format!("not takeable: {r}"));
            return Ok(p);
        }
    };
    p.decision = target.and_then(reference);
    let time = match (time, bound) {
        (Some(t), _) => Some(t),
        (None, Some((path, day))) => {
            before_a_reference(env, event, root, p.decision.as_ref(), (&path, &day)).await?
        }
        (None, None) => None,
    };
    if let Some(r) = already_taken(process, h, case) {
        p.reason = Some(format!("not takeable: {r}"));
        return Ok(p);
    }
    let output = match (&h.kind, target) {
        (ActionKind::FollowUp { decision, .. }, Some(b)) => {
            follow_up(env, h, decision, b, form, &mut p)?
        }
        (ActionKind::FollowUp { .. }, None) => {
            return Err(Refusal::Cell(format!("action '{}': no decision", h.name)))
        }
        // An incomplete output (a value is missing, a source did not answer)
        // is not a conclusion about the substance: then nothing is recorded, not even
        // when reported, because the input and the receipt would not be correct.
        _ => at_the_case(env, h, event, root, form, &mut p)
            .await?
            .map(Objection::Shape),
    };
    // An amendment for which the law leaves an output empty is not taken by the
    // process: the law then amends nothing (there are no grounds for an amendment).
    // Just like a hook that gives no value for a follow-up. A decision that
    // amends nothing may well have an empty auxiliary value (a deadline).
    let empty: Vec<&str> = h
        .outputs
        .iter()
        .filter(|u| p.outputs.get(*u) == Some(&Value::Null))
        .map(String::as_str)
        .collect();
    let output = match output {
        None if h.decision_role == Some(Decision::Amends) && !empty.is_empty() => {
            Some(Objection::Content(format!(
                "not takeable: {} gives no value for {}",
                h.article,
                empty.join(", ")
            )))
        }
        u => u,
    };
    let false_: Vec<&String> = p
        .assessments
        .iter()
        .filter(|(_, w)| **w == Value::Bool(false))
        .map(|(n, _)| n)
        .collect();
    let assessment = (!false_.is_empty()).then(|| {
        format!(
            "not takeable: {} says no ({})",
            h.article,
            false_
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )
    });
    let shape = time
        .map(|t| format!("not takeable: {t}"))
        .or((!absent.is_empty()).then(|| format!("not takeable: fill in: {}", absent.join(", "))))
        .or(match &output {
            Some(Objection::Shape(r)) => Some(r.clone()),
            _ => None,
        });
    let content = match output {
        Some(Objection::Content(r)) => Some(r),
        _ => None,
    }
    .or(assessment);
    p.takeable = shape.is_none() && content.is_none();
    p.reportable = shape.is_none() && content.is_some() && h.kind != ActionKind::Decision;
    p.reason = shape.or(content);
    Ok(p)
}

/// A decision or a fact: the lexostatuses of the case (for a fact including
/// the draft), the synthesis, the per-row synthesis, the form and
/// what has not happened yet, and then the engine. Returns the reason if the
/// outputs are not complete.
async fn at_the_case(
    env: &Environment<'_>,
    h: &ActionDefinition,
    event: &Event,
    root: &str,
    form: &Map<String, Value>,
    p: &mut TrialAction,
) -> Result<Option<String>, Refusal> {
    let process = env.process;
    let service = process.service.as_ref();
    let as_of =
        AsOf::at(TimePoint::read("reference date", &p.reference_date).map_err(Refusal::Cell)?);

    // 1. The lexostatuses of the case. A fact counts on trial: the cell
    // reduces as if the draft were already recorded.
    let concept = (!h.facts.is_empty()).then(|| RecordRequest {
        actor: process.definition.actor.clone(),
        stream: h.record.stream.clone(),
        event: h.record.event.clone(),
        intake: Value::Null,
        external: event_fields(event, &h.outputs, form, &BTreeMap::new()),
        refers_to: super::references(
            &process.cell,
            event,
            root,
            p.decision.as_ref().map(|b| b.id.as_str()),
        ),
        decision: None,
        root_grams: None,
    });
    let mut own = Vec::new();
    // The gram of the draft, as the cell built it: a register that
    // the policy queries counts it on trial (see [`crate::register`]).
    let mut trial_gram: Option<Gram> = None;
    for source in process.definition.case_sources() {
        let (l, g) = case_lexostatus(env, source, root, &as_of, concept.as_ref()).await?;
        trial_gram = trial_gram.or(g);
        own.push(l);
    }

    // 2. Synthesis, with the input from the lexostatus of the case that
    // supplies it (the check at load time says: at most one). If no source asks for
    // an input from the case, the synthesis starts empty; the parameters of
    // the case are added afterwards.
    let head = env
        .sources
        .iter()
        .flat_map(|s| s.definition.input.values().filter_map(|v| v.field()))
        .find_map(|v| own.iter().position(|l| l.name == v.lexostatus));
    let mut combined = match head.and_then(|i| own.get(i)) {
        Some(l) => synthesis::combine(l, env.sources, &as_of).await,
        None => synthesis::combine(&Lexostatus::empty(""), env.sources, &as_of).await,
    };
    for (i, l) in own.iter().enumerate() {
        if Some(i) == head {
            continue;
        }
        for (name, w) in &l.parameters {
            combined.parameters.insert(name.clone(), w.clone());
            combined.provenance.insert(
                name.clone(),
                Provenance::Own {
                    lexostatus: l.name.clone(),
                },
            );
        }
    }

    // 3. Per-row synthesis.
    let law = rows::Environment {
        service,
        date: &p.reference_date,
        as_of: &as_of,
    };
    p.rows = rows::apply(env.rows, &own, &mut combined, law).await;

    // 4. The verdicts of the handler; an empty field is not passed along.
    for o in &h.verdicts {
        if let Some(w) = form.get(&o.parameter).filter(|w| !w.is_null()) {
            combined.parameters.insert(o.parameter.clone(), w.clone());
            combined
                .provenance
                .insert(o.parameter.clone(), Provenance::Handler);
        }
    }

    // 4b. The decision the action acts on, if the article asks for it as a
    // parameter (the own policy that reads per decision).
    if let (Some(name), Some(b)) = (&h.decision_parameter, &p.decision) {
        combined
            .parameters
            .insert(name.clone(), Value::String(b.id.clone()));
        combined
            .provenance
            .insert(name.clone(), Provenance::Decision);
    }

    // 5. What only a later stage asks for has not happened yet.
    for (name, n) in &h.not_yet {
        combined.parameters.insert(name.clone(), n.value.clone());
        combined.provenance.insert(
            name.clone(),
            Provenance::StateAtDecision {
                stage: n.stage.clone(),
            },
        );
    }

    // 6. The engine: the outputs and the assessments, in one run.
    let requested: Vec<&str> = h
        .outputs
        .iter()
        .chain(h.assessments.iter())
        .map(String::as_str)
        .collect();
    let e =
        crate::register::with_trial(process.cell.id(), trial_gram.into_iter().collect(), || {
            assessment::evaluate_with_trace(
                service,
                &h.regulation,
                &requested,
                &combined.parameters,
                &p.reference_date,
            )
        });
    let complete = e.complete(&requested);
    // A source that should have delivered a missing value is the reason;
    // one that failed but delivers nothing that is missing is not.
    let reason = (!complete).then(|| {
        combined
            .reason_for(env.sources, &e.missing)
            .map(|r| format!("not takeable: {r}"))
            .unwrap_or_else(|| e.reason("not takeable"))
    });
    for (name, w) in e.values {
        if h.assessments.contains(&name) {
            p.assessments.insert(name, w);
        } else {
            p.outputs.insert(name, w);
        }
    }
    p.trace_text = e.trace_text;
    p.missing = e.missing;
    p.not_delivered = required(service, h)
        .map_err(Refusal::Cell)?
        .into_values()
        .filter(|b| !combined.parameters.contains_key(&b.name))
        .collect();
    p.parameters = combined.parameters;
    p.provenance = combined.provenance;
    p.sources = combined.sources;
    p.lexostatuses = own;
    Ok(reason)
}

/// A follow-up: the engine executes the stage of the action on the input
/// and the outputs of the recorded decision (RFC-008: the decision is the
/// state, the orchestration keeps it and supplies what the stage asks for). That
/// state is derived by the cell (the stage of the decision in the [`CaseState`]);
/// the process reads no gram. What the stage asks for comes from the form.
/// The hooks of that stage fire (RFC-007), such as the start and the end of
/// the objection deadline.
fn follow_up(
    env: &Environment<'_>,
    h: &ActionDefinition,
    decision: &str,
    state: &DecisionState,
    form: &Map<String, Value>,
    p: &mut TrialAction,
) -> Result<Option<Objection>, Refusal> {
    let process = env.process;
    let service = process.service.as_ref();
    let b = process
        .definition
        .handling
        .as_ref()
        .and_then(|b| b.action(decision))
        .ok_or_else(|| Refusal::Cell(format!("no action '{decision}'")))?;
    let Some((_, gram)) = state.taken() else {
        return Err(Refusal::Cell(format!(
            "decision {} has no decision stage",
            state.id
        )));
    };
    if let Some(s) = h.stage.as_ref().filter(|s| state.stages.contains_key(*s)) {
        return Ok(Some(Objection::Shape(format!(
            "not takeable: stage {s} is already in decision {}",
            state.id
        ))));
    }
    let (ActionKind::FollowUp { procedure, .. }, Some(stage)) = (&h.kind, h.stage.clone()) else {
        return Err(Refusal::Cell(format!(
            "action '{}' is not a follow-up with a stage",
            h.name
        )));
    };
    let state = StageState {
        procedure_id: procedure.clone(),
        contextual_law: gram
            .regulation
            .clone()
            .unwrap_or_else(|| h.regulation.clone()),
        current_stage: stage.clone(),
        accumulated_outputs: gram
            .fields
            .iter()
            .map(|(k, v)| (k.clone(), EngineValue::from(v)))
            .collect(),
        parameters: gram
            .input
            .iter()
            .map(|(k, w)| (k.clone(), EngineValue::from(w)))
            .collect(),
    };
    let mut input = BTreeMap::new();
    for f in &h.facts {
        if let Some(w) = form.get(&f.name).filter(|w| !w.is_null()) {
            input.insert(f.name.clone(), EngineValue::from(w));
            p.parameters.insert(f.name.clone(), w.clone());
            p.provenance.insert(f.name.clone(), Provenance::Handler);
        }
    }
    let output = b
        .outputs
        .first()
        .ok_or_else(|| Refusal::Cell(format!("action '{decision}' names no output")))?;
    let outputs =
        match service.execute_stage(&h.regulation, output, Some(state), input, &p.reference_date) {
            Ok(ExecutionOutcome::Complete(r)) => r.outputs,
            Ok(ExecutionOutcome::Yielded {
                state,
                outputs,
                pending_inputs,
                ..
            }) => {
                if state.current_stage == stage {
                    p.missing = pending_inputs;
                    return Ok(Some(Objection::Shape(format!(
                        "not takeable: stage {stage} asks for {}",
                        p.missing.join(", ")
                    ))));
                }
                outputs
            }
            Err(e) => return Ok(Some(Objection::Shape(format!("not takeable: {e}")))),
        };
    for u in &h.outputs {
        match outputs.get(u) {
            Some(w) if w.contains_unknown() => {
                for f in w.missing_facts() {
                    if !p.missing.contains(&f.name) {
                        p.missing.push(f.name.clone());
                    }
                }
            }
            Some(w) => {
                let v = serde_json::to_value(w)
                    .map_err(|e| Refusal::Cell(format!("output '{u}': {e}")))?;
                p.outputs.insert(u.clone(), v);
            }
            None => {}
        }
    }
    let empty: Vec<&str> = h
        .outputs
        .iter()
        .filter(|u| p.outputs.get(*u).is_none_or(Value::is_null))
        .map(String::as_str)
        .collect();
    if !p.missing.is_empty() {
        return Ok(Some(Objection::Shape(format!(
            "not takeable: missing {}",
            p.missing.join(", ")
        ))));
    }
    if !empty.is_empty() {
        // A hook that gives no value says the stage did not take place in the
        // prescribed manner (such as a publication that does not comply
        // with Awb 3:41: the objection deadline then does not start). A
        // conclusion about the substance: if it happened anyway, it is recorded, with
        // an empty deadline.
        return Ok(Some(Objection::Content(format!(
            "not takeable: no value for {} ({})",
            empty.join(", "),
            h.hooks.join(", ")
        ))));
    }
    Ok(None)
}

/// The fields of the gram: per `$external` key of the event an
/// output, or the value from the form.
pub(super) fn event_fields(
    event: &Event,
    outputs_names: &[String],
    form: &Map<String, Value>,
    outputs: &BTreeMap<String, Value>,
) -> Map<String, Value> {
    event
        .external_keys()
        .into_iter()
        .map(|k| {
            let w = if outputs_names.contains(&k) {
                outputs.get(&k).cloned().unwrap_or(Value::Null)
            } else {
                form.get(&k).cloned().unwrap_or(Value::Null)
            };
            (k, w)
        })
        .collect()
}
