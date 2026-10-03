//! Taking an action: the trial, the competent authority, and the recording by
//! the cell.

use super::*;

/// A taken action: the recorded gram, the trial it followed
/// from, and what was worth noting during recording.
#[derive(Debug, Clone, Serialize)]
pub struct Taken {
    pub gram: Gram,
    pub yaml: String,
    pub trial: TrialAction,
    pub warnings: Vec<String>,
}

/// Take an action in a case and have the cell record it.
///
/// The trial must be takeable: then the process acts on its own. If it is
/// not, because of the substance (`reportable`), the cell records the fact only
/// if the handler reports that it happened (`happened`): an executogram
/// records an actual delivery (paper P:54), and what happened
/// the process does not refuse because of what it thinks of it. The reason is passed along as a
/// warning, and the lexostatuses of the case show the consequences. A
/// decision is not reported. For a decision and a follow-up the
/// process checks the competent authority of the law against the authority it acts for
/// (`on_behalf_of`), or a mandate from that authority (see [`crate::authority`]): otherwise
/// it refuses; if the law names no authority, it records with a
/// warning. The gram carries who acted (`acting_actor`): the role,
/// the channel and the identity of the logged-in user, and for a decision
/// or follow-up on behalf of which authority. The cell builds the gram from its stream, and
/// refuses (409) if the stage is already in the case or if the case changed
/// since the process read it: what the process computed held for the case
/// as it was then.
pub async fn take(
    env: &Environment<'_>,
    h: &ActionDefinition,
    root: &str,
    case: &CaseState,
    action_input: &ActionInput,
    acting: &Session,
) -> Result<Taken, Refusal> {
    let (form, happened) = (&action_input.form, action_input.happened);
    let process = env.process;
    let service = process.service.as_ref();
    let actor = &process.definition.actor;
    if happened && h.kind == ActionKind::Decision {
        return Err(Refusal::Invalid(format!(
            "action '{}' is a decision: the process takes it itself, it is not reported as happened",
            h.name
        )));
    }
    let trial = trial(env, h, root, case, action_input).await?;
    let mut warnings = Vec::new();
    if !trial.takeable {
        let reason = trial
            .reason
            .clone()
            .unwrap_or_else(|| "not takeable".to_string());
        if !(happened && trial.reportable) {
            return Err(Refusal::NotTakeable(if trial.reportable {
                format!("{reason}; if it happened anyway, report it as happened (happened: true)")
            } else {
                reason
            }));
        }
        warnings.push(format!(
            "reported as happened, against the conclusion of the process: {reason}"
        ));
    }
    let (stream, event) = process
        .cell
        .event(&h.record.stream, &h.record.event)
        .ok_or_else(|| Refusal::Cell(format!("action '{}': no record event", h.name)))?;

    let own = process.authority.as_str();
    let mut authority = None;
    let (mut on_behalf_of, mut mandate) = (None, None);
    if !matches!(h.kind, ActionKind::Fact) {
        let number = regulations::parse(&h.article)
            .map_err(Refusal::Cell)?
            .article;
        authority = authority::authority_of(service, &h.regulation, number);
        match &authority {
            Some(g) => match authority::assessment(own, &process.definition.mandates, g) {
                Ok(Competence::Own) => on_behalf_of = Some(g.clone()),
                Ok(Competence::Mandate(m)) => {
                    on_behalf_of = Some(g.clone());
                    mandate = Some(m.legal_basis.clone());
                }
                Err(reason) => {
                    return Err(Refusal::Unauthorized(format!("{}: {reason}", h.article)));
                }
            },
            None => {
                warnings.push(format!(
                    "regulation '{}' names no competent authority for {}; recorded without competent_authority",
                    h.regulation, h.article
                ));
                on_behalf_of = Some(own.to_string());
            }
        }
    }
    let acting_actor = ActingActor {
        role: acting.role.clone(),
        channel: acting.channel.clone(),
        identity: acting.fields.clone(),
        legal_basis: process
            .definition
            .roles
            .get(&acting.role)
            .and_then(|r| r.legal_basis.clone()),
        on_behalf_of,
        mandate,
    };

    let external = event_fields(event, &h.outputs, form, &trial.outputs);
    // Every parameter that took part is passed along, with its provenance.
    let mut inputs: BTreeMap<String, Input> = BTreeMap::new();
    for (name, value) in &trial.parameters {
        let provenance = trial
            .provenance
            .get(name)
            .ok_or_else(|| Refusal::Cell(format!("parameter '{name}' has no provenance")))?;
        inputs.insert(
            name.clone(),
            Input {
                value: value.clone(),
                provenance: provenance.clone(),
            },
        );
    }
    let mut streams: Vec<StreamReference> = process
        .cell
        .streams
        .iter()
        .map(|s| StreamReference {
            id: s.id.clone(),
            sha256: s.sha256.clone(),
        })
        .collect();
    streams.sort_by(|a, b| a.id.cmp(&b.id));
    // The legal character and the regulation belong to a decision (a
    // decretogram); input and receipt to every action the engine
    // computed.
    let decretogram = event.type_ == DECRETOGRAM;
    let article = regulations::article(service, &h.article).map_err(Refusal::Cell)?;
    let produces = article
        .get_execution_spec()
        .and_then(|e| e.produces.as_ref())
        .filter(|_| decretogram);
    // The version of the regulation: its `valid_from`. If it names none, then
    // the publication date, with a warning: the version is then an
    // assumption.
    let mut regulation_valid_from = None;
    if decretogram {
        let law = service
            .resolver()
            .get_law(&h.regulation)
            .ok_or_else(|| Refusal::Cell(format!("regulation '{}' is not loaded", h.regulation)))?;
        regulation_valid_from = Some(match &law.valid_from {
            Some(v) => v.clone(),
            None => {
                warnings.push(format!(
                    "regulation '{}' names no valid_from; regulation_valid_from is its publication date ({})",
                    h.regulation, law.publication_date
                ));
                law.publication_date.clone()
            }
        });
    }
    let request = RecordRequest {
        actor: actor.clone(),
        stream: stream.id.clone(),
        event: event.name.clone(),
        intake: Value::Null,
        external,
        // A decision refers to the application (the root); a gram that
        // follows or amends a decision refers to that decision. The cell provides the id.
        refers_to: super::references(
            &process.cell,
            event,
            root,
            match h.decision_role {
                Some(Decision::Follows | Decision::Amends) => {
                    trial.decision.as_ref().map(|b| b.id.as_str())
                }
                _ => None,
            },
        ),
        decision: Some(DecisionFields {
            legal_character: produces.and_then(|p| p.legal_character.clone()),
            decision_type: produces.and_then(|p| p.decision_type.clone()),
            regulation: decretogram.then(|| h.regulation.clone()),
            regulation_valid_from,
            competent_authority: authority.filter(|_| decretogram),
            acting_actor: Some(acting_actor),
            inputs,
            receipt: Some(Receipt::new(env.regulations.to_vec(), streams)),
        }),
        root_grams: Some(case.grams),
    };
    let WithYaml { gram, yaml } = cell_client::record(env.cell, &h.record.cell, &request)
        .await
        .map_err(|f| match f {
            // The form (the stage, the case, the moment) is checked by the cell, under its lock.
            TransportError::Response { status: 409, error } => Refusal::Conflict(error),
            TransportError::Response { status: 400, error } => Refusal::Invalid(error),
            f => Refusal::Cell(format!("the action was not recorded: {f}")),
        })?;
    Ok(Taken {
        gram,
        yaml,
        trial,
        warnings,
    })
}
