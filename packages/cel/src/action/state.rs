//! The state of an action and of the decisions in a case, for the
//! case screen.

use super::*;

/// Whether an action is possible in a case, given the decisions and their stages: a
/// decision as long as the case has no decision of that action (another
/// decision on the same application requires its own legal basis: an
/// amendment), an amendment if there is a decision to amend, a
/// follow-up if the decision exists and its stage does not yet, a fact that
/// follows a decision if that decision exists, and every other fact always (what it
/// does, the trial says).
#[derive(Debug, Clone, Serialize)]
pub struct ActionStatus {
    pub available: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// The grams the action already recorded in this case.
    pub recorded: usize,
    /// The decision the action would now act on (see [`target`]).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decision: Option<String>,
}

/// The state of an action in a case, from the [`CaseState`] the cell
/// derives: which decisions exist, which stages each went through and how often
/// the event of the action occurred.
pub fn state(process: &Process, h: &ActionDefinition, case: &CaseState) -> ActionStatus {
    let recorded = case.count(&h.record.stream, &h.record.event);
    let mut decision = None;
    let reason = match target(process, h, case, None) {
        Err(r) => Some(r),
        Ok(b) => {
            decision = b.map(|b| b.id.clone());
            match (&h.kind, b) {
                (ActionKind::Decision, _) => already_taken(process, h, case),
                (ActionKind::FollowUp { .. }, Some(b)) => h
                    .stage
                    .as_ref()
                    .filter(|s| b.stages.contains_key(*s))
                    .map(|s| format!("stage {s} is already in decision {}", b.id)),
                _ => None,
            }
        }
    };
    ActionStatus {
        available: reason.is_none(),
        reason,
        recorded,
        decision,
    }
}

/// Whether a decision that amends no other is already in the case: the cell does not
/// record a second decision of the same event in a case. Another
/// decision on the same application is an amendment, with its own legal basis.
pub(super) fn already_taken(
    process: &Process,
    h: &ActionDefinition,
    case: &CaseState,
) -> Option<String> {
    if h.decision_role != Some(Decision::Opens) {
        return None;
    }
    own(process, &h.name, case).first().map(|b| {
        format!(
            "decision {} is already in the case; another decision on it requires its own legal basis (an amendment)",
            b.id
        )
    })
}

/// The procedure of a decision (RFC-008): the stages, with per stage whether
/// a gram of it exists and which action records it.
#[derive(Debug, Clone, Serialize)]
pub struct ProcedureStatus {
    pub id: String,
    pub stages: Vec<StageStatus>,
}

#[derive(Debug, Clone, Serialize)]
pub struct StageStatus {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub recorded: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
}

/// The legal protection after the last stage of a decision, derived from the
/// procedure and the law (RFC-022 sec. 3.3): the next stage, if no
/// action records it (such as BEZWAAR, which runs by itself after the
/// publication), with the outputs of the hooks the law fired on the last
/// stage (such as the end of the objection deadline, Awb 6:7 and 6:8). None
/// of this is spelled out per regulation in the configuration.
#[derive(Debug, Clone, Serialize)]
pub struct LegalProtection {
    pub procedure: String,
    /// The stage after which the route runs.
    pub after: String,
    /// The stage that is running now.
    pub stage: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// The hooks that computed the route, as `<regulation>#<article>`.
    pub legal_basis: Vec<String>,
    /// Their outputs, as the gram of the last stage recorded them.
    pub outputs: BTreeMap<String, Value>,
}

/// A decision in the case, for the case screen: which action took it, the
/// procedure with its stages, the legal protection that follows from it, and the
/// actions that now act on this decision (its follow-up, the facts that
/// follow it, an amendment).
#[derive(Debug, Clone, Serialize)]
pub struct DecisionInCase {
    /// The id of the gram that is the decision.
    pub id: String,
    /// The action that recorded the decision, and its article.
    pub action: String,
    pub label: String,
    pub article: String,
    pub event: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effective_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amends: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub procedure: Option<ProcedureStatus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub legal_protection: Option<LegalProtection>,
    /// The actions that now act on this decision.
    pub actions: Vec<String>,
}

/// The decisions in a case, each with its procedure and legal protection.
/// Which decisions exist, which stages each went through and what their grams
/// recorded is said by the [`CaseState`] of the cell; the procedure and the hooks
/// come from the law. A stage that belongs to no decision (the application)
/// counts for every decision of the case.
pub fn decisions_in_case(process: &Process, case: &CaseState) -> Vec<DecisionInCase> {
    let Some(handling) = &process.definition.handling else {
        return Vec::new();
    };
    case.decisions
        .iter()
        .filter_map(|b| {
            let h = handling.actions.iter().find(|h| {
                h.kind == ActionKind::Decision && b.of(&h.record.stream, &h.record.event)
            })?;
            let (procedure, legal_protection) = procedure_and_route(process, h, b, case);
            let actions = handling
                .actions
                .iter()
                .filter(|x| x.name != h.name)
                .filter(|x| {
                    target(process, x, case, None)
                        .ok()
                        .flatten()
                        .is_some_and(|d| d.id == b.id)
                })
                .map(|x| x.name.clone())
                .collect();
            Some(DecisionInCase {
                id: b.id.clone(),
                action: h.name.clone(),
                label: h.label().to_string(),
                article: h.article.clone(),
                event: b.event.clone(),
                effective_at: b.taken().map(|(_, g)| g.effective_at.clone()),
                amends: b.amends.clone(),
                procedure,
                legal_protection,
                actions,
            })
        })
        .collect()
}

/// The procedure and the legal protection of a decision in the case.
fn procedure_and_route(
    process: &Process,
    decision: &ActionDefinition,
    state: &DecisionState,
    case: &CaseState,
) -> (Option<ProcedureStatus>, Option<LegalProtection>) {
    let service = process.service.as_ref();
    let Some(handling) = &process.definition.handling else {
        return (None, None);
    };
    let Some(p) = procedure_of(service, &decision.article) else {
        return (None, None);
    };
    let by = |stage: &str| {
        handling
            .actions
            .iter()
            .find(|h| h.stage.as_deref() == Some(stage) && h.article == decision.article)
    };
    let gram = |stage: &str| state.stages.get(stage).or_else(|| case.stages.get(stage));
    let stages: Vec<StageStatus> = p
        .stages
        .iter()
        .map(|s| StageStatus {
            name: s.name.clone(),
            description: s.description.clone(),
            recorded: gram(&s.name).is_some(),
            action: by(&s.name).map(|h| h.name.clone()),
        })
        .collect();
    let route = stages.iter().rposition(|s| s.recorded).and_then(|i| {
        let after = &p.stages[i];
        let next = p.stages.get(i + 1)?;
        if by(&next.name).is_some() {
            return None;
        }
        let h = by(&after.name)?;
        if h.hooks.is_empty() {
            return None;
        }
        let gram = gram(&after.name)?;
        let outputs = h
            .hooks
            .iter()
            .flat_map(|a| outputs_of(service, a))
            .filter_map(|u| gram.fields.get(&u).map(|w| (u, w.clone())))
            .collect();
        Some(LegalProtection {
            procedure: p.id.clone(),
            after: after.name.clone(),
            stage: next.name.clone(),
            description: next.description.clone(),
            legal_basis: h.hooks.clone(),
            outputs,
        })
    });
    (
        Some(ProcedureStatus {
            id: p.id.clone(),
            stages,
        }),
        route,
    )
}

/// The procedure of the case before a decision exists: the procedure of the
/// first decision the process knows, with the stages that belong to no decision
/// (such as the application).
pub fn procedure_of_the_case(process: &Process, case: &CaseState) -> Option<ProcedureStatus> {
    let b = process
        .definition
        .handling
        .as_ref()?
        .actions
        .iter()
        .find(|h| h.kind == ActionKind::Decision && h.decision_role == Some(Decision::Opens))?;
    let p = procedure_of(process.service.as_ref(), &b.article)?;
    Some(ProcedureStatus {
        id: p.id.clone(),
        stages: p
            .stages
            .iter()
            .map(|s| StageStatus {
                name: s.name.clone(),
                description: s.description.clone(),
                recorded: case.stages.contains_key(&s.name),
                action: None,
            })
            .collect(),
    })
}
