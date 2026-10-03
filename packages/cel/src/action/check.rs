//! The checks on `handling` at startup, and which synthesis sources
//! an action asks for.

use super::*;

/// The checks on the `handling` of a process at startup. An error
/// here stops the runtime:
///
/// - an action that names a role names a role that may do the handling;
/// - the worklist and the list of all cases are list lexostatuses of the
///   cell;
/// - a source of the case requires a handling;
/// - the outputs of an action come from one and the same article (for
///   a follow-up also from the hooks of that stage);
/// - the lexostatuses of the case exist, are not a list and have as their
///   only input `root`;
/// - every parameter from the form, the state of what has not happened yet
///   or a `rows` block is a parameter the caller of the article
///   must supply, and comes from only one source;
/// - the record event follows a case; for a decision it records every output,
///   for a follow-up what the stage asks for and the outputs of the
///   hooks; every field is an output or a field of the form.
pub fn check(process: &Process) -> Vec<String> {
    let mut errors = Vec::new();
    let d = &process.definition;
    let cell = &process.cell;
    let Some(handling) = &d.handling else {
        for b in d.case_sources() {
            errors.push(format!(
                "synthesis source {}/{}: a source of the case (case: true) requires a handling; the assessment reads the draft",
                b.cell, b.lexostatus
            ));
        }
        return errors;
    };
    for list in [&handling.worklist, &handling.cases] {
        match cell.lexostatuses.lexostatus(&list.lexostatus) {
            None => errors.push(format!(
                "handling: list '{}' is not a lexostatus of the cell",
                list.lexostatus
            )),
            Some(l) if !l.is_list() => errors.push(format!(
                "handling: list '{}' is not a list (group_by: root)",
                l.name
            )),
            Some(_) => {}
        }
    }

    // The lexostatuses of the case: once, for all actions.
    let case: Vec<&String> = d.case_sources().map(|z| &z.lexostatus).collect();
    let mut from_the_case: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    for name in case.iter().copied() {
        match cell.lexostatuses.lexostatus(name) {
            None => errors.push(format!("handling: lexostatus '{name}' does not exist")),
            Some(l) => {
                if l.is_list() {
                    errors.push(format!(
                        "handling: lexostatus '{name}' is a list, and a list never goes to the engine"
                    ));
                }
                let inputs: Vec<&str> = l.inputs.iter().map(|i| i.name.as_str()).collect();
                if inputs != ["root"] {
                    errors.push(format!(
                        "handling: lexostatus '{name}' has inputs [{}]; an action only passes 'root'",
                        inputs.join(", ")
                    ));
                }
                for p in l.reduction.derivations.keys() {
                    from_the_case
                        .entry(p)
                        .or_default()
                        .push(format!("the own lexostatus '{name}'"));
                }
            }
        }
    }
    // An input from an earlier source (a passed-on extra field) does not come
    // from an own lexostatus; the synthesis itself checks it.
    let passed_on: Vec<&str> = d
        .other_sources()
        .filter(|s| !s.extra_fields.is_empty())
        .map(|s| s.lexostatus.as_str())
        .collect();
    let mut input_from: Vec<&str> = d
        .other_sources()
        .flat_map(|s| s.input.values().filter_map(|v| v.field()))
        .map(|v| v.lexostatus.as_str())
        .filter(|l| !passed_on.contains(l))
        .collect();
    input_from.sort_unstable();
    input_from.dedup();
    if input_from.len() > 1 {
        errors.push(format!(
            "handling: the input of the synthesis comes from more than one lexostatus ({}); an action passes it from one",
            input_from.join(", ")
        ));
    }
    for source in d.other_sources() {
        for (i, v) in source
            .input
            .iter()
            .filter_map(|(i, v)| Some((i, v.field()?)))
        {
            if !case.contains(&&v.lexostatus) && !passed_on.contains(&v.lexostatus.as_str()) {
                errors.push(format!(
                    "handling: synthesis source {}/{}, input '{i}': comes from lexostatus '{}', which is not a lexostatus of the case (case: true)",
                    source.cell, source.lexostatus, v.lexostatus
                ));
            }
        }
    }

    for h in &handling.actions {
        errors.extend(check_action(process, h, &from_the_case, &case));
    }
    errors
}

fn check_action(
    process: &Process,
    h: &ActionDefinition,
    from_the_case: &BTreeMap<&str, Vec<String>>,
    case: &[&String],
) -> Vec<String> {
    let mut errors = Vec::new();
    let d = &process.definition;
    let cell = &process.cell;
    let service = process.service.as_ref();
    let who = format!("action '{}'", h.name);
    if let Some(r) = &h.role {
        match d.roles.get(r) {
            None => errors.push(format!("{who}: role '{r}' is not listed under roles")),
            Some(role) if !role.may(crate::channel::Routes::Handling) => errors.push(format!(
                "{who}: role '{r}' may not do the handling (routes: handling)"
            )),
            Some(_) => {}
        }
    }
    if h.article.is_empty() {
        // Loading already reported why.
        return errors;
    }
    // An amount in the form names its unit (`type_spec.unit`): the
    // frontend asks for eurocents in euros, and without a unit it does not know whether
    // an entered number is euros or eurocents.
    let amount_verdicts = required(service, h).unwrap_or_default();
    let without_unit = h
        .facts
        .iter()
        .filter(|f| f.kind.as_deref() == Some("amount") && f.unit.is_none())
        .map(|f| f.name.as_str())
        .chain(
            h.verdicts
                .iter()
                .filter(|o| {
                    amount_verdicts.get(&o.parameter).is_some_and(|b| {
                        b.typing.kind == ParameterType::Amount && b.typing.unit.is_none()
                    })
                })
                .map(|o| o.parameter.as_str()),
        );
    for name in without_unit {
        errors.push(format!(
            "{who}: '{name}' is an amount without a unit; give the parameter type_spec.unit in the regulation (such as eurocent)"
        ));
    }
    // The outputs: from one article, for a follow-up also from the hooks.
    let hook_outputs: BTreeSet<String> = h
        .hooks
        .iter()
        .flat_map(|a| outputs_of(service, a))
        .collect();
    let mut articles = BTreeSet::new();
    for u in &h.outputs {
        if hook_outputs.contains(u) {
            continue;
        }
        match article_with(service, &h.regulation, u) {
            None => errors.push(format!(
                "{who}: regulation '{}' has no output '{u}'",
                h.regulation
            )),
            Some(a) => {
                articles.insert(a);
            }
        }
    }
    if articles.len() > 1 {
        errors.push(format!(
            "{who}: the outputs come from more than one article ({}); an action is one article",
            articles.into_iter().collect::<Vec<_>>().join(", ")
        ));
    }

    // The record event.
    let vl = format!("{who}, record {}/{}", h.record.stream, h.record.event);
    if h.record.cell != cell.id() {
        // The cell of the process is reported by `process::the_cell`.
    } else if let Some((_, event)) = cell.event(&h.record.stream, &h.record.event) {
        if event.case != Case::Follows {
            errors.push(format!(
                "{vl}: the event has case: {}, and an action follows the case of the application",
                event.case.as_text()
            ));
        }
        errors.extend(check_decision(process, h, &vl));
        let keys = event.external_keys();
        let facts: Vec<&str> = h.facts.iter().map(|f| f.name.as_str()).collect();
        match &h.kind {
            ActionKind::Decision => {
                let absent: Vec<&str> = h
                    .outputs
                    .iter()
                    .filter(|u| !keys.contains(u))
                    .map(String::as_str)
                    .collect();
                if !absent.is_empty() {
                    errors.push(format!(
                        "{vl}: the event does not record the outputs [{}] of the decision",
                        absent.join(", ")
                    ));
                }
                if !facts.is_empty() {
                    errors.push(format!(
                        "{vl}: the event records [{}], and that is neither an output nor a verdict of the decision",
                        facts.join(", ")
                    ));
                }
            }
            ActionKind::FollowUp { .. } => {
                let absent: Vec<&str> = facts
                    .iter()
                    .copied()
                    .chain(hook_outputs.iter().map(String::as_str))
                    .filter(|k| !keys.iter().any(|s| s == k))
                    .collect();
                if !absent.is_empty() {
                    errors.push(format!(
                        "{vl}: the event does not record [{}]; a follow-up records what the stage asks for and what the hooks compute (RFC-008, RFC-022 sec. 3.3)",
                        absent.join(", ")
                    ));
                }
                let over: Vec<&str> = keys
                    .iter()
                    .map(String::as_str)
                    .filter(|k| !h.outputs.iter().any(|u| u == k) && !facts.contains(k))
                    .collect();
                if !over.is_empty() {
                    errors.push(format!(
                        "{vl}: [{}] is not an output and not anything the stage asks for",
                        over.join(", ")
                    ));
                }
            }
            ActionKind::Fact => {}
        }
    }

    if matches!(h.kind, ActionKind::FollowUp { .. }) {
        // A follow-up reads the recorded decision, not sources.
        if !h.rows.is_empty() {
            errors.push(format!(
                "{who}: a follow-up computes on the input of the decision, without per-row synthesis"
            ));
        }
        return errors;
    }

    // A parameter comes from only one source.
    let mut per: BTreeMap<&str, Vec<String>> = from_the_case.clone();
    for source in d.other_sources() {
        for p in &source.parameters {
            per.entry(p).or_default().push(format!(
                "synthesis source {}/{}",
                source.cell, source.lexostatus
            ));
        }
    }
    for o in &h.verdicts {
        per.entry(&o.parameter).or_default().push("the form".into());
    }
    for p in h.not_yet.keys() {
        per.entry(p)
            .or_default()
            .push("the state of what has not happened yet".into());
    }
    let case: Vec<&str> = case.iter().map(|z| z.as_str()).collect();
    for (i, r) in h.rows.iter().enumerate() {
        per.entry(&r.parameter)
            .or_default()
            .push(format!("the per-row synthesis from '{}'", r.table.field));
        errors.extend(rows::check(
            &who,
            r,
            &h.rows[..i],
            &case,
            "not a lexostatus of the case (case: true)",
            d,
            cell,
        ));
    }
    let required = match required(service, h) {
        Ok(b) => b,
        Err(f) => {
            errors.push(format!("{who}: {f}"));
            return errors;
        }
    };
    for (p, of) in &per {
        if of.len() > 1 && required.contains_key(*p) {
            errors.push(format!(
                "{who}: parameter '{p}' comes from more than one source: {}",
                of.join(", ")
            ));
        }
    }
    let names = h
        .verdicts
        .iter()
        .map(|o| ("form", o.parameter.as_str()))
        .chain(h.not_yet.keys().map(|p| ("not yet happened", p.as_str())))
        .chain(h.rows.iter().map(|r| ("rows", r.parameter.as_str())));
    for (where_, p) in names {
        if !required.contains_key(p) {
            errors.push(format!(
                "{who}, {where_}: '{p}' is not a parameter of {} or of an article it calls without its own parameters",
                h.article
            ));
        }
    }
    errors
}

/// Whether the record event fits the kind of action, and the action fits the
/// decision it names. A case can have more decisions; which gram belongs to
/// which decision is said by the stream (`decision`), and which action belongs to
/// which decision by `decision` in the action (for a follow-up: the article).
///
/// - a decision records in an event that opens or amends a decision;
///   an amendment names the decision it amends, a new decision does not;
/// - a follow-up records in an event that follows a decision;
/// - a fact that follows a decision names the decision; another fact does not;
/// - `decision` names the action of a decision in this process.
fn check_decision(process: &Process, h: &ActionDefinition, vl: &str) -> Vec<String> {
    let mut errors = Vec::new();
    let role = h.decision_role.map_or("none", Decision::as_text);
    let named = h.decision.as_deref();
    let decision_action = |name: &str| {
        process
            .definition
            .handling
            .as_ref()
            .and_then(|b| b.action(name))
            .filter(|b| b.kind == ActionKind::Decision && b.name != h.name)
    };
    match (&h.kind, h.decision_role) {
        (ActionKind::Decision, Some(Decision::Opens)) => {
            if let Some(b) = named {
                errors.push(format!(
                    "{vl}: the event opens a decision, and a new decision names no other (decision: {b}); a decision that amends another records in an event with decision: amends"
                ));
            }
        }
        (ActionKind::Decision, Some(Decision::Amends))
        | (ActionKind::Fact, Some(Decision::Follows)) => match named {
            None => errors.push(format!(
                "{vl}: the event {role} a decision; name the action of that decision with decision"
            )),
            Some(b) if decision_action(b).is_none() => errors.push(format!(
                "action '{}': decision '{b}' is not another action of a decision in this process",
                h.name
            )),
            Some(_) => {}
        },
        (ActionKind::Decision, _) => errors.push(format!(
            "{vl}: a decision records in an event with decision: opens (or amends, if it amends another decision), not decision: {role}"
        )),
        (ActionKind::FollowUp { decision, .. }, Some(Decision::Follows)) => {
            if named.is_some_and(|b| b != decision) {
                errors.push(format!(
                    "action '{}': a follow-up to the decision of '{decision}' (the same article) names decision '{}'",
                    h.name,
                    named.unwrap_or_default()
                ));
            }
        }
        (ActionKind::FollowUp { .. }, _) => errors.push(format!(
            "{vl}: a follow-up is a later stage of a decision, and records in an event with decision: follows, not decision: {role}"
        )),
        (ActionKind::Fact, _) => {
            if let Some(b) = named {
                errors.push(format!(
                    "{vl}: the event follows no decision (decision: {role}), and the action names decision '{b}'"
                ));
            }
        }
    }
    errors
}

/// The synthesis sources an action asks for: those that supply a parameter of its
/// article, and the sources that pass them (or the per-row synthesis) an
/// input. Which source an action needs thus follows from what
/// the article asks for; a payment does not ask for the registers of an
/// application.
pub fn sources_for(process: &Process, h: &ActionDefinition) -> Vec<usize> {
    let d = &process.definition;
    if matches!(h.kind, ActionKind::FollowUp { .. }) {
        return Vec::new();
    }
    let required: BTreeSet<String> = required(&process.service, h)
        .map(|b| b.into_keys().collect())
        .unwrap_or_default();
    let sources: Vec<&crate::config::SynthesisSource> = d.other_sources().collect();
    let mut needed: BTreeSet<usize> = sources
        .iter()
        .enumerate()
        .filter(|(_, b)| b.parameters.iter().any(|p| required.contains(p)))
        .map(|(i, _)| i)
        .collect();
    // Lexostatuses an input comes from: of the chosen sources and of
    // the per-row synthesis.

    let mut requested: BTreeSet<String> = h
        .rows
        .iter()
        .flat_map(|r| {
            std::iter::once(r.table.lexostatus.clone()).chain(r.sources.iter().flat_map(|b| {
                b.input.values().filter_map(|i| match i {
                    crate::config::RowInput::Own { lexostatus, .. } => Some(lexostatus.clone()),
                    _ => None,
                })
            }))
        })
        .collect();
    loop {
        for i in &needed {
            for v in sources[*i].input.values().filter_map(|v| v.field()) {
                requested.insert(v.lexostatus.clone());
            }
        }
        let added: BTreeSet<usize> = sources
            .iter()
            .enumerate()
            .filter(|(i, b)| {
                !needed.contains(i)
                    && !b.extra_fields.is_empty()
                    && requested.contains(&b.lexostatus)
            })
            .map(|(i, _)| i)
            .collect();
        if added.is_empty() {
            break;
        }
        needed.extend(added);
    }
    needed.into_iter().collect()
}
