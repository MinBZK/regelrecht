//! A process from policy (RFC-047): the [`ProcessDefinition`] the runtime
//! used to read from `process.yaml`, derived from the policy of an actor
//! ([`crate::policy`]), the cells and the deployment
//! ([`crate::deployment`]). One process per actor; its id is the id of the
//! cell that records its submissions.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::Arc;

use regelrecht_engine::LawExecutionService;

use crate::cell::Cell;
use crate::channel::{ChannelDefinition, RoleDefinition};
use crate::config::{
    ActionDefinition, Assessment, FormReference, Handling, LexostatusReference, OnBehalfOf,
    OriginCheck, Portal, ProcessDefinition, Record,
};
use crate::deployment::{ChannelDeployment, Deployment};
use crate::policy::{ActorPolicy, ChannelKind, DeclaredChannel};
use crate::stream::{Binding, Event, Stream, To};

/// A derived process, with what the derivation saw but is no reason not to start.
#[derive(Debug)]
pub struct Derived {
    pub definition: ProcessDefinition,
    pub warnings: Vec<String>,
}

/// `<regulation>#<article>` of the article that establishes an event: the
/// first of `establishes`, otherwise the article of the first legal basis.
pub fn establishing(event: &Event) -> Option<String> {
    let first = event.establishes.first().or(event.legal_basis.first())?;
    Some(crate::regulations::parse(first).ok()?.article_ref())
}

/// Whether an event is a submission: `type: submission` or the stage that
/// opens a case.
fn is_submission(e: &Event) -> bool {
    e.type_ == "submission" || e.stage.as_deref() == Some("AANVRAAG")
}

/// Every process the policies give. Every error is returned, not only the
/// first; each names the policy article or the deployment file.
pub fn processes(
    policies: &BTreeMap<String, ActorPolicy>,
    deployment: &Deployment,
    cells: &BTreeMap<String, Arc<Cell>>,
    service: &LawExecutionService,
    root: &Path,
) -> Result<Vec<Derived>, Vec<String>> {
    let mut out = Vec::new();
    let mut errors = Vec::new();
    // Per cell the authority whose process records in it.
    let mut claimed: BTreeMap<String, &str> = BTreeMap::new();
    for p in policies.values() {
        let at = |m: String| format!("authority '{}': {m}", p.authority);
        if p.channels.is_empty() {
            // Mandates and supplies belong to the process of an actor; without
            // a channel there is none.
            if !p.mandates.is_empty() || !p.supplies.is_empty() {
                errors.push(at(format!(
                    "the policy names mandates or supplies ({}), but no channel; they belong to the process of an actor with channels",
                    p.mandates
                        .iter()
                        .map(|m| m.legal_basis.as_str())
                        .chain(p.supplies.values().map(|(by, _)| by.as_str()))
                        .collect::<Vec<_>>()
                        .join(", ")
                )));
            }
            continue;
        }
        let portal = match portal_of(p, cells, service) {
            Ok(found) => found,
            Err(e) => {
                errors.push(at(e));
                // The deployment with exactly the channels of this policy is
                // its own, so that it does not also count as unclaimed.
                for (cell, deployed) in &deployment.channels {
                    let fits = deployed.len() == p.channels.len()
                        && p.channels.iter().all(|c| deployed.contains_key(&c.id));
                    if fits {
                        claimed.entry(cell.clone()).or_insert(&p.authority);
                    }
                }
                continue;
            }
        };
        let id = portal.cell.id().to_string();
        if let Some(other) = claimed.insert(id.clone(), &p.authority) {
            errors.push(at(format!(
                "{}: the portal records in cell '{id}', as the process of '{other}' does",
                portal.channel.article
            )));
            continue;
        }
        match process(p, &portal, deployment, service, root) {
            Ok(d) => out.push(d),
            Err(f) => errors.extend(f.into_iter().map(at)),
        }
    }
    let files = [
        (
            deployment.channels.keys().collect::<Vec<_>>(),
            Some(&deployment.channels_file),
        ),
        (
            deployment.synthesis.keys().collect(),
            deployment.synthesis_file.as_ref(),
        ),
        (
            deployment.examples.keys().collect(),
            deployment.examples_file.as_ref(),
        ),
    ];
    for (keys, file) in files {
        for cell in keys.into_iter().filter(|c| !claimed.contains_key(*c)) {
            errors.push(format!(
                "{}: cell '{cell}' is the cell of no process in any policy",
                file.map(|f| f.display().to_string()).unwrap_or_default()
            ));
        }
    }
    if errors.is_empty() {
        Ok(out)
    } else {
        Err(errors)
    }
}

/// The portal of an actor: its channel, and the submission it records.
struct PortalOf<'a> {
    channel: &'a DeclaredChannel,
    /// `<regulation>#<article>` of the submitted article.
    submits: &'a str,
    /// Its regulation.
    regulation: &'a str,
    cell: &'a Arc<Cell>,
    stream: &'a Stream,
    event: &'a Event,
}

/// The one channel that names `submits`, a loaded article, and the one
/// submission event, in any cell, that the submitted article establishes.
fn portal_of<'a>(
    p: &'a ActorPolicy,
    cells: &'a BTreeMap<String, Arc<Cell>>,
    service: &LawExecutionService,
) -> Result<PortalOf<'a>, String> {
    let portal: Vec<(&DeclaredChannel, &str)> = p
        .channels
        .iter()
        .filter_map(|c| c.def.submits.as_deref().map(|s| (c, s)))
        .collect();
    let [(channel, submits)] = portal[..] else {
        let named: Vec<String> = if portal.is_empty() {
            let mut articles: Vec<&str> = p.channels.iter().map(|c| c.article.as_str()).collect();
            articles.dedup();
            vec![format!(
                "the channels are declared in {}",
                articles.join(", ")
            )]
        } else {
            portal
                .iter()
                .map(|(c, _)| format!("'{}' in {}", c.id, c.article))
                .collect()
        };
        return Err(format!(
            "{} channels name `submits` ({}); exactly one channel is the portal",
            portal.len(),
            named.join(", ")
        ));
    };
    let at = |m: &str| {
        format!(
            "{}: channel '{}' submits '{submits}', {m}",
            channel.article, channel.id
        )
    };
    let target = crate::regulations::parse(submits)
        .map_err(|_| at("which does not have the form <regulation>#<article>"))?;
    if target.paragraph.is_some() {
        return Err(at("a paragraph; submits names the article itself"));
    }
    crate::regulations::article(service, submits)
        .map_err(|e| at(&format!("which is not a loaded article ({e})")))?;
    let found: Vec<(&Arc<Cell>, &Stream, &Event)> = cells
        .values()
        .flat_map(|c| c.streams.iter().map(move |s| (c, s)))
        .flat_map(|(c, s)| s.events.iter().map(move |e| (c, s, e)))
        .filter(|(_, _, e)| is_submission(e) && establishing(e).as_deref() == Some(submits))
        .collect();
    let [(cell, stream, event)] = found[..] else {
        return Err(format!(
            "{}: {} submission events establish {submits} ({}); exactly one is the portal event",
            channel.article,
            found.len(),
            found
                .iter()
                .map(|(c, s, e)| format!("{}/{}/{}", c.id(), s.id, e.name))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    };
    Ok(PortalOf {
        channel,
        submits,
        regulation: target.regulation,
        cell,
        stream,
        event,
    })
}

/// A channel of the policy with its adapter, and its role: the role name,
/// what the runtime needs of both, and what is wrong.
fn channel_and_role(
    p: &ActorPolicy,
    c: &DeclaredChannel,
    k: &ChannelDeployment,
    cell: &str,
    file: &str,
) -> (ChannelDefinition, String, RoleDefinition, Vec<String>) {
    let mut errors = Vec::new();
    let basis = c
        .def
        .identifies
        .as_ref()
        .map(|i| i.fields())
        .unwrap_or_default();
    let known = k.field_names();
    for name in basis.keys().filter(|n| !known.contains(n)) {
        errors.push(format!(
            "{}: channel '{}' identifies '{name}', which is not a field of the channel under '{cell}' in {file}",
            c.article, c.id
        ));
    }
    let fields = k
        .identification_fields(cell, &c.id, &basis)
        .map_err(|e| errors.push(format!("{}: {e} (in {file})", c.article)))
        .unwrap_or_default();
    let supplied = p.supplies.get(&c.id);
    let mut legal_basis = c.def.legal_basis.clone();
    if let Some((by, _)) = supplied {
        // The supplying article, unless a basis already names it.
        let named = legal_basis
            .iter()
            .any(|b| crate::regulations::parse(b).is_ok_and(|g| g.article_ref() == *by));
        if !named {
            legal_basis.push(by.clone());
        }
    }
    let channel = ChannelDefinition {
        label: k.label.clone(),
        explanation: k.explanation.clone(),
        fields,
        owner: c.def.owner.clone(),
        intake: k.intake.clone(),
        legal_basis,
        supplies: supplied.map(|(_, s)| s.clone()).unwrap_or_default(),
        declared_by: Some(c.article.clone()),
        supplied_by: supplied.map(|(by, _)| by.clone()),
    };
    let role = RoleDefinition {
        channel: c.id.clone(),
        routes: vec![c.def.kind.routes()],
        label: k.role_label.clone(),
        legal_basis: c.def.legal_basis.first().cloned(),
    };
    let name = c.def.role.clone().unwrap_or_else(|| c.id.clone());
    (channel, name, role, errors)
}

fn process(
    p: &ActorPolicy,
    portal: &PortalOf<'_>,
    deployment: &Deployment,
    service: &LawExecutionService,
    root: &Path,
) -> Result<Derived, Vec<String>> {
    let mut errors = Vec::new();
    let id = portal.cell.id().to_string();
    let file = deployment.channels_file.display().to_string();
    let none = BTreeMap::new();
    let deployed = deployment.channels.get(&id).unwrap_or(&none);
    let mut channels = BTreeMap::new();
    let mut roles: BTreeMap<String, RoleDefinition> = BTreeMap::new();
    for c in &p.channels {
        let Some(k) = deployed.get(&c.id) else {
            errors.push(format!(
                "{}: channel '{}' has no adapter under '{id}' in {file}",
                c.article, c.id
            ));
            continue;
        };
        let (channel, name, role, wrong) = channel_and_role(p, c, k, &id, &file);
        errors.extend(wrong);
        if let Some(other) = roles.get(&name) {
            errors.push(format!(
                "{}: role '{name}' of channel '{}' is the role of channel '{}' too",
                c.article, c.id, other.channel
            ));
        }
        channels.insert(c.id.clone(), channel);
        roles.insert(name, role);
    }
    for extra in deployed
        .keys()
        .filter(|k| !p.channels.iter().any(|c| &c.id == *k))
    {
        errors.push(format!(
            "{file}: channel '{extra}' under '{id}' is in no policy of '{}'",
            p.authority
        ));
    }
    let synthesis = deployment.synthesis.get(&id).cloned().unwrap_or_default();
    let (handling, warnings) = match handling(p, portal.cell, service, &synthesis) {
        Ok(h) => h,
        Err(e) => {
            errors.extend(e);
            (None, Vec::new())
        }
    };
    let (channel, submits) = (portal.channel, portal.submits);
    let assessment = match &channel.def.assesses {
        Some(a) => match assessment_lexostatus(portal, service) {
            Ok(lexostatus) => Some(Assessment {
                lexostatus,
                regulation: portal.regulation.to_string(),
                output: a.output.clone(),
                rows: synthesis.assessment_rows,
            }),
            Err(e) => {
                errors.push(format!("{}: {e}", channel.article));
                None
            }
        },
        None => {
            errors.push(format!(
                "{}: the portal channel '{}' names no `assesses`",
                channel.article, channel.id
            ));
            None
        }
    };
    if let Some(a) = &assessment {
        if !crate::action::outputs_of_article(service, submits).contains(&a.output) {
            errors.push(format!(
                "{}: channel '{}' assesses '{}', which is not an output of {submits}",
                channel.article, channel.id, a.output
            ));
        }
    }
    let form = channel.def.form.as_ref().map(|f| FormReference {
        path: root.join(&f.document).display().to_string(),
        screen: f.screen.clone(),
    });
    if let Some(f) = form.as_ref().filter(|f| !Path::new(&f.path).is_file()) {
        errors.push(format!(
            "{}: the form of channel '{}' is {}, which is not a file",
            channel.article, channel.id, f.path
        ));
    }
    if !errors.is_empty() {
        return Err(errors);
    }
    let definition = ProcessDefinition {
        id: id.clone(),
        actor: portal.stream.recording_actor.clone(),
        origin_check: OriginCheck::Strict,
        on_behalf_of: Some(OnBehalfOf::Authority {
            authority: p.authority.clone(),
        }),
        mandates: p.mandates.clone(),
        channels,
        roles,
        portal: assessment.map(|assessment| Portal {
            cell: id.clone(),
            stream: portal.stream.id.clone(),
            event: portal.event.name.clone(),
            assessment,
            offer: channel.def.offers.clone(),
            form,
        }),
        synthesis: synthesis.synthesis,
        handling,
        examples: deployment.examples.get(&id).cloned(),
        declared_by: Some(channel.article.clone()),
    };
    Ok(Derived {
        definition,
        warnings,
    })
}

/// The stage of a decision (RFC-008).
const DECISION: &str = crate::stream::DECISION;

/// What the handler does: one action per event of the cell whose intake is a
/// channel of the policy with `kind: handling`, in the order of the streams
/// (spec §3, decision 12 of the plan). A decision executes its establishing
/// article; a later stage of its procedure is a follow-up on it, one per
/// decision it can follow; a fact executes the article that reads it. The
/// worklist is the one the runtime offers ([`crate::reduction::WORKLIST`]).
fn handling(
    p: &ActorPolicy,
    cell: &Cell,
    service: &LawExecutionService,
    synthesis: &crate::deployment::SynthesisDeployment,
) -> Result<(Option<Handling>, Vec<String>), Vec<String>> {
    let intakes: Vec<&str> = p
        .channels
        .iter()
        .filter(|c| c.def.kind == ChannelKind::Handling)
        .map(|c| c.id.as_str())
        .collect();
    if intakes.is_empty() {
        return Ok((None, Vec::new()));
    }
    let events: Vec<(&Stream, &Event)> = cell
        .streams
        .iter()
        .flat_map(|s| s.events.iter().map(move |e| (s, e)))
        .filter(|(_, e)| intakes.contains(&e.intake.as_str()))
        .collect();
    let mut errors = Vec::new();
    let mut warnings = Vec::new();
    let new = |s: &Stream, e: &Event, name: String, article: String| ActionDefinition {
        name,
        label: None,
        role: None,
        decision: None,
        decision_parameter: None,
        regulation: article
            .split_once('#')
            .map(|(r, _)| r.to_string())
            .unwrap_or_default(),
        outputs: Vec::new(),
        rows: Vec::new(),
        record: Record {
            cell: cell.id().to_string(),
            stream: s.id.clone(),
            event: e.name.clone(),
        },
        article,
        kind: Default::default(),
        stage: None,
        decision_role: None,
        verdicts: Vec::new(),
        facts: Vec::new(),
        not_yet: Default::default(),
        assessments: Vec::new(),
        hooks: Vec::new(),
        types: Default::default(),
    };
    let label = |e: &Event, article: &str| {
        let group = crate::origin::group(service, article).unwrap_or_else(|| article.to_string());
        format!("{} ({group})", crate::form::readable(&e.name))
    };
    // Decisions first: facts and follow-ups refer to them.
    let mut decisions: Vec<ActionDefinition> = Vec::new();
    for (s, e) in events
        .iter()
        .filter(|(_, e)| e.stage.as_deref() == Some(DECISION))
    {
        let Some(article) = establishing(e) else {
            errors.push(format!("event '{}': no establishing article", e.name));
            continue;
        };
        let mut h = new(s, e, e.name.clone(), article.clone());
        h.outputs = external_outputs(e, service, &article);
        h.label = Some(label(e, &article));
        decisions.push(h);
    }
    // A decision that amends another acts on that one.
    let all = decisions.clone();
    for h in &mut decisions {
        let Some((_, e)) = events.iter().find(|(_, e)| e.name == h.record.event) else {
            continue;
        };
        if e.refers_to.contains_key(crate::stream::AMENDS) {
            match decision_of(e, crate::stream::AMENDS, &all, cell) {
                Ok(d) => h.decision = d,
                Err(m) => errors.push(m),
            }
        }
    }
    let mut out: Vec<ActionDefinition> = Vec::new();
    for (s, e) in &events {
        match e.stage.as_deref() {
            Some(DECISION) => out.extend(
                decisions
                    .iter()
                    .filter(|d| d.record.event == e.name)
                    .cloned(),
            ),
            Some(stage) => {
                // A later stage of the procedure of a decision article.
                let of: Vec<&ActionDefinition> = decisions
                    .iter()
                    .filter(|d| {
                        crate::action::procedure_of(service, &d.article).is_some_and(|p| {
                            let i = |n: &str| p.stages.iter().position(|x| x.name == n);
                            matches!((i(DECISION), i(stage)), (Some(a), Some(b)) if b > a)
                        })
                    })
                    .collect();
                if of.is_empty() {
                    errors.push(format!(
                        "event '{}': stage {stage} follows no decision of the actor",
                        e.name
                    ));
                }
                for d in &of {
                    let name = if of.len() == 1 {
                        e.name.clone()
                    } else {
                        format!("{}_{}", e.name, d.name)
                    };
                    let mut h = new(s, e, name, d.article.clone());
                    h.outputs = external_outputs(e, service, &d.article);
                    if h.outputs.is_empty() {
                        // It records nothing of the decision: the follow-up
                        // recomputes all of it (own choice of the plan).
                        h.outputs = d.outputs.clone();
                    }
                    let mut l = label(e, &establishing(e).unwrap_or_else(|| d.article.clone()));
                    if of.len() > 1 {
                        l.push_str(&format!(
                            " bij {}",
                            crate::form::readable(&d.name).to_lowercase()
                        ));
                    }
                    h.label = Some(l);
                    out.push(h);
                }
            }
            None => match fact(p, cell, s, e, service, &decisions) {
                Ok(Some((article, outputs, parameter))) => {
                    let mut h = new(s, e, e.name.clone(), article.clone());
                    h.outputs = outputs;
                    h.decision_parameter = parameter;
                    h.label = Some(label(e, &establishing(e).unwrap_or(article)));
                    if e.decision == Some(crate::stream::Decision::Follows) {
                        match decision_of(e, "", &decisions, cell) {
                            Ok(d) => h.decision = d,
                            Err(m) => errors.push(m),
                        }
                    }
                    out.push(h);
                }
                Ok(None) => warnings.push(format!(
                    "event '{}' has intake '{}', but no article apart from a decision reads it; it is not offered as an action",
                    e.name, e.intake
                )),
                Err(m) => errors.push(m),
            },
        }
    }
    for h in &mut out {
        h.rows = synthesis
            .action_rows
            .get(&h.name)
            .cloned()
            .unwrap_or_default();
    }
    for name in synthesis
        .action_rows
        .keys()
        .filter(|n| !out.iter().any(|h| &h.name == *n))
    {
        errors.push(format!(
            "synthesis.yaml: action_rows for '{name}' under '{}', which is no action of the process",
            cell.id()
        ));
    }
    if !errors.is_empty() {
        return Err(errors);
    }
    Ok((
        Some(Handling {
            worklist: LexostatusReference {
                cell: cell.id().to_string(),
                lexostatus: crate::reduction::WORKLIST.into(),
            },
            actions: out,
        }),
        warnings,
    ))
}

/// The `$external` (or supplied) keys of the event that are outputs of the
/// article, in the order of the event.
fn external_outputs(e: &Event, service: &LawExecutionService, article: &str) -> Vec<String> {
    let outputs = crate::action::outputs_of_article(service, article);
    let mut out: Vec<String> = Vec::new();
    for l in e.leaves() {
        if let Binding::External(k) | Binding::Supplied(k) = l.binding {
            if outputs.contains(&k) && !out.contains(&k) {
                out.push(k);
            }
        }
    }
    out
}

/// The decision action a reference of `e` points to (`key`: that reference;
/// empty: the one that points to a decision). A reference to an event or an
/// article names its decision; one to a stage only if there is exactly one.
fn decision_of(
    e: &Event,
    key: &str,
    decisions: &[ActionDefinition],
    cell: &Cell,
) -> Result<Option<String>, String> {
    let events: Vec<&Event> = cell.streams.iter().flat_map(|s| s.events.iter()).collect();
    for (name, r) in &e.refers_to {
        if !key.is_empty() && name != key {
            continue;
        }
        let targets: Vec<&&Event> = events.iter().filter(|d| r.to.fits_event(d)).collect();
        if targets.is_empty() || !targets.iter().all(|d| d.stage.as_deref() == Some(DECISION)) {
            continue;
        }
        let found: Vec<&ActionDefinition> = match &r.to {
            To::Event(ev) => decisions.iter().filter(|d| &d.record.event == ev).collect(),
            To::Article(a) => decisions.iter().filter(|d| &d.article == a).collect(),
            To::Stage(_) => decisions.iter().collect(),
        };
        return match found[..] {
            [one] => Ok(Some(one.name.clone())),
            _ => Err(format!(
                "event '{}': reference '{name}' points to {} decisions of the actor; name the article in refers_to (to: <regulation>#<article>)",
                e.name,
                found.len()
            )),
        };
    }
    Ok(None)
}

/// The article a fact executes, its outputs and the parameter that receives
/// the decision id (decision 12 of the plan): (a) a policy article of the
/// actor with a parameter of origin role BESLUIT that executes an article
/// in the legal basis of the event, for a fact that follows a decision;
/// otherwise (b) the article that reads the event, through the
/// lexostatuses that read it. `None`: no article apart from a decision
/// reads it.
#[allow(clippy::type_complexity)]
fn fact(
    p: &ActorPolicy,
    cell: &Cell,
    s: &Stream,
    e: &Event,
    service: &LawExecutionService,
    decisions: &[ActionDefinition],
) -> Result<Option<(String, Vec<String>, Option<String>)>, String> {
    let resolver = service.resolver();
    let minus = |article: &str, outputs: Vec<String>| {
        let t = crate::action::assessments(service, article, e);
        outputs
            .into_iter()
            .filter(|o| !t.contains(o))
            .collect::<Vec<_>>()
    };
    // (a)
    if e.decision == Some(crate::stream::Decision::Follows) {
        let basis: Vec<String> = e
            .legal_basis
            .iter()
            .filter_map(|g| crate::regulations::parse(g).ok())
            .map(|g| g.article_ref())
            .collect();
        let mut found = Vec::new();
        for id in service.list_laws() {
            if crate::authority::authority_of_regulation(service, id).as_deref()
                != Some(p.authority.as_str())
            {
                continue;
            }
            let Some(law) = resolver.get_law(id) else {
                continue;
            };
            for a in &law.articles {
                if !a.get_executes().any(|x| basis.contains(&x.article)) {
                    continue;
                }
                let reference = format!("{id}#{}", a.number);
                let Some(parameter) = crate::action::decision_parameter_of(service, &reference)
                    .map_err(|m| format!("event '{}': {m}", e.name))?
                else {
                    continue;
                };
                let outputs = crate::action::outputs_of_article(service, &reference);
                found.push((
                    reference.clone(),
                    minus(&reference, outputs),
                    Some(parameter),
                ));
            }
        }
        match &found[..] {
            [] => {}
            [one] => return Ok(Some(one.clone())),
            more => {
                return Err(format!(
                    "event '{}': more than one policy article of '{}' executes its legal basis with a parameter of origin role BESLUIT ({})",
                    e.name,
                    p.authority,
                    more.iter().map(|(a, _, _)| a.as_str()).collect::<Vec<_>>().join(", ")
                ))
            }
        }
    }
    // (b)
    let mut candidates: Vec<(String, Vec<String>)> = Vec::new();
    for d in cell
        .lexostatuses
        .lexostatus_definitions
        .iter()
        .filter(|d| !d.is_list())
    {
        let names: BTreeSet<String> =
            crate::check::derivations_reading(d, &cell.streams, &s.id, &e.name)
                .into_iter()
                .collect();
        if names.is_empty() {
            continue;
        }
        let articles: Vec<String> = match &d.law {
            // A lexostatus from the law: what the reading article executes.
            Some(reading) => {
                let (r, n) = reading.article.split_once('#').unwrap_or_default();
                resolver
                    .executes_of(r, n)
                    .into_iter()
                    .map(|x| x.target)
                    .collect()
            }
            // One of the cell: every article with a parameter it derives
            // from the event, apart from a decision.
            None => service
                .list_laws()
                .into_iter()
                .filter_map(|id| resolver.get_law(id).map(|l| (id, l)))
                .flat_map(|(id, l)| l.articles.iter().map(move |a| (id, a)))
                .filter(|(_, a)| a.get_parameters().iter().any(|p| names.contains(&p.name)))
                .filter(|(_, a)| {
                    a.get_produces().and_then(|p| p.legal_character.as_deref())
                        != Some("BESCHIKKING")
                })
                .map(|(id, a)| format!("{id}#{}", a.number))
                .collect(),
        };
        for article in articles {
            let Ok(a) = crate::regulations::article(service, &article) else {
                continue;
            };
            let outputs = outputs_depending_on(a, &names);
            if !outputs.is_empty() && !candidates.iter().any(|(c, _)| c == &article) {
                let outputs = minus(&article, outputs);
                candidates.push((article, outputs));
            }
        }
    }
    if candidates.len() > 1 {
        // The regulation of a decision of the actor wins.
        let preferred: Vec<(String, Vec<String>)> = candidates
            .iter()
            .filter(|(a, _)| {
                decisions
                    .iter()
                    .any(|d| a.starts_with(&format!("{}#", d.regulation)))
            })
            .cloned()
            .collect();
        if !preferred.is_empty() {
            candidates = preferred;
        }
    }
    match &candidates[..] {
        [] => Ok(None),
        [(a, o)] => Ok(Some((a.clone(), o.clone(), None))),
        more => Err(format!(
            "event '{}': more than one article reads it ({}); narrow the reading in the cell or the law",
            e.name,
            more.iter().map(|(a, _)| a.as_str()).collect::<Vec<_>>().join(", ")
        )),
    }
}

/// The names an expression refers to (`$name`, `$name.field`).
fn references(v: &serde_json::Value, out: &mut BTreeSet<String>) {
    match v {
        serde_json::Value::String(s) => {
            if let Some(r) = s.strip_prefix('$') {
                out.insert(r.split('.').next().unwrap_or(r).to_string());
            }
        }
        serde_json::Value::Array(a) => a.iter().for_each(|x| references(x, out)),
        serde_json::Value::Object(o) => o.values().for_each(|x| references(x, out)),
        _ => {}
    }
}

/// The outputs of an article that depend on one of `parameters`: through the
/// value of their action, directly or through an input whose `source` passes
/// the parameter on, or through another output, to a fixed point. In
/// declaration order.
pub fn outputs_depending_on(
    article: &regelrecht_engine::Article,
    parameters: &BTreeSet<String>,
) -> Vec<String> {
    let Some(exec) = article.get_execution_spec() else {
        return Vec::new();
    };
    let mut uses: Vec<(String, BTreeSet<String>)> = Vec::new();
    for i in exec.input.iter().flatten() {
        let mut n = BTreeSet::new();
        if let Ok(v) = serde_json::to_value(&i.source) {
            references(&v, &mut n);
        }
        uses.push((i.name.clone(), n));
    }
    for a in exec.actions.iter().flatten() {
        let (Some(output), Ok(v)) = (&a.output, serde_json::to_value(a)) else {
            continue;
        };
        let mut n = BTreeSet::new();
        references(&v, &mut n);
        uses.push((output.clone(), n));
    }
    let mut dependent = parameters.clone();
    loop {
        let before = dependent.len();
        for (name, n) in &uses {
            if !n.is_disjoint(&dependent) {
                dependent.insert(name.clone());
            }
        }
        if dependent.len() == before {
            break;
        }
    }
    exec.output
        .iter()
        .flatten()
        .map(|o| o.name.clone())
        .filter(|o| dependent.contains(o))
        .collect()
}

/// The lexostatus the assessment reduces on trial: of the cell, not a list,
/// reading the portal event and deriving a parameter of the submitted
/// article. Exactly one (UB 16 for NAPP).
fn assessment_lexostatus(
    portal: &PortalOf<'_>,
    service: &LawExecutionService,
) -> Result<String, String> {
    let submits = portal.submits;
    let parameters: Vec<String> = crate::regulations::article(service, submits)?
        .get_parameters()
        .iter()
        .map(|p| p.name.clone())
        .collect();
    let cell = portal.cell;
    let found: Vec<&str> = cell
        .lexostatuses
        .lexostatus_definitions
        .iter()
        .filter(|d| !d.is_list())
        .filter(|d| {
            crate::check::derivations_reading(
                d,
                &cell.streams,
                &portal.stream.id,
                &portal.event.name,
            )
            .iter()
            .any(|n| parameters.contains(n))
        })
        .map(|d| d.name.as_str())
        .collect();
    match found[..] {
        [one] => Ok(one.to_string()),
        _ => Err(format!(
            "{} lexostatuses of cell '{}' read event '{}' for a parameter of {submits} ({}); exactly one is the assessment",
            found.len(),
            cell.id(),
            portal.event.name,
            found.join(", ")
        )),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
pub(crate) mod tests {
    use super::*;
    use crate::config::PROCESS_FILE;

    pub(crate) fn fixtures() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
    }

    pub(crate) fn setup() -> (
        Arc<LawExecutionService>,
        BTreeMap<String, Arc<Cell>>,
        Deployment,
    ) {
        let service = Arc::new(
            crate::regulations::load(&fixtures().join("regulation"))
                .unwrap()
                .service,
        );
        let cells = crate::config::cell_dirs(&fixtures().join("cells"))
            .unwrap()
            .iter()
            .map(|m| {
                let c = Cell::load(m, service.clone()).unwrap();
                (c.id().to_string(), Arc::new(c))
            })
            .collect();
        let d = fixtures().join("deployment");
        let config = crate::config::Config {
            cells_path: fixtures().join("cells"),
            processes_path: None,
            regulation_path: fixtures().join("regulation"),
            data_dir: fixtures(),
            port: 0,
            read_token: None,
            read_token_sources: Vec::new(),
            reduction: Default::default(),
            registers: None,
            channels: Some(d.join("channels.yaml")),
            synthesis: Some(d.join("synthesis.yaml")),
            examples: Some(d.join("examples.yaml")),
        };
        (
            service,
            cells,
            crate::deployment::load(&config).unwrap().unwrap(),
        )
    }

    fn derive(
        s: &LawExecutionService,
        cells: &BTreeMap<String, Arc<Cell>>,
        d: &Deployment,
    ) -> Result<Vec<Derived>, Vec<String>> {
        processes(
            &crate::policy::read(s, None).unwrap(),
            d,
            cells,
            s,
            &fixtures(),
        )
    }

    fn derived(cell: &str) -> ProcessDefinition {
        let (s, cells, d) = setup();
        let all = derive(&s, &cells, &d).unwrap();
        all.into_iter()
            .find(|p| p.definition.id == cell)
            .unwrap()
            .definition
    }

    fn from_yaml(process: &str) -> ProcessDefinition {
        crate::load::load(
            &fixtures()
                .join("processes")
                .join(process)
                .join(PROCESS_FILE),
            ProcessDefinition::parse,
        )
        .unwrap()
    }

    fn file_name(p: &str) -> String {
        Path::new(p)
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned()
    }

    /// The action names of `process.yaml` and the event names that replace
    /// them (the table "Hernoemingen" of the plan).
    const RENAMED: &[(&str, &str)] = &[
        ("besluit", "besluit_genomen"),
        ("bekendmaken", "besluit_bekendgemaakt"),
        ("betalen", "betaling_verricht"),
        ("aanvulling_vragen", "aanvulling_gevraagd"),
        ("voorschot", "voorschot_verleend"),
        (
            "voorschot_bekendmaken",
            "besluit_bekendgemaakt_voorschot_verleend",
        ),
        ("voorschot_betalen", "voorschot_betaald"),
        ("vaststellen", "toeslag_vastgesteld"),
        (
            "vaststelling_bekendmaken",
            "besluit_bekendgemaakt_toeslag_vastgesteld",
        ),
        ("vaststelling_wijzigen", "vaststelling_gewijzigd"),
        (
            "wijziging_bekendmaken",
            "besluit_bekendgemaakt_vaststelling_gewijzigd",
        ),
        ("terugvorderen", "terugvordering_vastgesteld"),
        (
            "terugvordering_bekendmaken",
            "besluit_bekendgemaakt_terugvordering_vastgesteld",
        ),
        ("terugbetalen", "terugbetaling_ontvangen"),
    ];

    /// Until `process.yaml` goes (Task 11): the policy says the same about
    /// channels, roles and the portal as the configuration did, and the
    /// deployment holds the same technique, synthesis and examples.
    #[test]
    fn the_policy_gives_the_same_process_as_the_configuration() {
        let (service, cells, deployment) = setup();
        let all = derive(&service, &cells, &deployment).unwrap();
        // The basis of a role is the first of its channel in the policy;
        // `process.yaml` gave a role none (an intended deviation).
        let role_basis: &[(&str, &str, &str)] = &[
            ("test_afnemer", "aanvrager", "testregeling_afnemer#1"),
            ("test_toeslag", "aanvrager", "testbeleid_toeslag#4 lid 1"),
        ];
        for (cell, process) in [
            ("test_afnemer", "afnemer"),
            ("test_instantie", "instantie"),
            ("test_toeslag", "toeslag"),
        ] {
            let d = &all
                .iter()
                .find(|p| p.definition.id == cell)
                .unwrap()
                .definition;
            let y = from_yaml(process);
            // The process id is the cell id (renamed).
            assert_eq!(d.id, cell);
            assert_eq!(d.actor, y.actor, "{cell}");
            // The authority: as `on_behalf_of` gave it; the instantie had
            // none and now has the one of its policy (renamed).
            let own = crate::authority::own(d, &service);
            if cell == "test_instantie" {
                assert_eq!(own.as_deref(), Some("Test instantie"));
                assert_eq!(crate::authority::own(&y, &service), None);
            } else {
                assert!(own.is_some(), "{cell}");
                assert_eq!(own, crate::authority::own(&y, &service), "{cell}");
            }
            assert_eq!(d.mandates, y.mandates, "{cell}");
            assert_eq!(
                d.channels.keys().collect::<Vec<_>>(),
                y.channels.keys().collect::<Vec<_>>(),
                "{cell}"
            );
            for (id, k) in &y.channels {
                let dk = &d.channels[id];
                assert_eq!(dk.label, k.label, "{cell}/{id}");
                assert_eq!(dk.explanation, k.explanation, "{cell}/{id}");
                assert_eq!(dk.owner, k.owner, "{cell}/{id}");
                assert_eq!(dk.intake, k.intake, "{cell}/{id}");
                assert_eq!(dk.supplies, k.supplies, "{cell}/{id}");
                assert_eq!(dk.legal_basis, k.legal_basis, "{cell}/{id}");
                // Name, label, pattern, check, message, numeric and legal basis.
                let fields =
                    |f: &[crate::channel::IdentificationField]| serde_json::to_value(f).unwrap();
                assert_eq!(fields(&dk.fields), fields(&k.fields), "{cell}/{id}");
                assert!(dk.declared_by.is_some(), "{cell}/{id}");
            }
            assert_eq!(
                d.roles.keys().collect::<Vec<_>>(),
                y.roles.keys().collect::<Vec<_>>(),
                "{cell}"
            );
            for (id, r) in &y.roles {
                assert_eq!(d.roles[id].channel, r.channel, "{cell}/{id}");
                assert_eq!(d.roles[id].routes, r.routes, "{cell}/{id}");
                assert_eq!(d.roles[id].label, r.label, "{cell}/{id}");
                assert_eq!(r.legal_basis, None, "{cell}/{id}");
                let expected = role_basis
                    .iter()
                    .find(|(c, role, _)| *c == cell && role == id)
                    .map(|(_, _, b)| b.to_string());
                assert_eq!(d.roles[id].legal_basis, expected, "{cell}/{id}");
            }
            let (dp, yp) = (d.portal.as_ref().unwrap(), y.portal.as_ref().unwrap());
            assert_eq!(
                (&dp.cell, &dp.stream, &dp.event),
                (&yp.cell, &yp.stream, &yp.event),
                "{cell}"
            );
            assert_eq!(
                (
                    &dp.assessment.lexostatus,
                    &dp.assessment.regulation,
                    &dp.assessment.output
                ),
                (
                    &yp.assessment.lexostatus,
                    &yp.assessment.regulation,
                    &yp.assessment.output
                ),
                "{cell}"
            );
            assert_eq!(
                format!("{:?}", dp.assessment.rows),
                format!("{:?}", yp.assessment.rows),
                "{cell}"
            );
            let offer = |o: Option<&crate::config::Offer>| {
                o.map(|o| {
                    (
                        o.regulation.clone(),
                        o.output.clone(),
                        o.deadline.clone(),
                        o.windows.clone(),
                        o.start.clone(),
                        o.opening.clone(),
                    )
                })
            };
            assert_eq!(offer(dp.offer.as_ref()), offer(yp.offer.as_ref()), "{cell}");
            // The form: the same screen of the same document, now under
            // `documents/` (the yaml path is relative to the process).
            let dir = fixtures().join("processes").join(process);
            assert_eq!(
                dp.form
                    .as_ref()
                    .map(|f| (&f.screen, std::fs::read_to_string(&f.path).unwrap())),
                yp.form.as_ref().map(|f| (
                    &f.screen,
                    std::fs::read_to_string(dir.join(&f.path)).unwrap()
                )),
                "{cell}"
            );
            assert_eq!(
                format!("{:?}", d.synthesis),
                format!("{:?}", y.synthesis),
                "{cell}"
            );
            // The rows per action, under the event name of the action.
            let rows: BTreeMap<String, String> = y
                .handling
                .iter()
                .flat_map(|h| &h.actions)
                .filter(|a| !a.rows.is_empty())
                .map(|a| {
                    let name = RENAMED
                        .iter()
                        .find(|(was, _)| *was == a.name)
                        .map_or(a.name.as_str(), |(_, is)| is);
                    (name.to_string(), format!("{:?}", a.rows))
                })
                .collect();
            let derived_rows: BTreeMap<String, String> = deployment
                .synthesis
                .get(cell)
                .iter()
                .flat_map(|s| &s.action_rows)
                .map(|(a, r)| (a.clone(), format!("{r:?}")))
                .collect();
            assert_eq!(derived_rows, rows, "{cell}");
            if cell == "test_afnemer" {
                assert!(derived_rows.contains_key("besluit_genomen"));
            }
            // The examples: the same files, the actions under their event name.
            let examples = |e: Option<&crate::config::ExamplesDefinition>, rename: bool| {
                e.map(|e| {
                    let actions: BTreeMap<String, String> = e
                        .actions
                        .iter()
                        .map(|(a, p)| {
                            let a = RENAMED
                                .iter()
                                .find(|(was, _)| rename && was == a)
                                .map_or(a.as_str(), |(_, is)| is);
                            (a.to_string(), file_name(p))
                        })
                        .collect();
                    (
                        e.logins.iter().map(|l| file_name(l)).collect::<Vec<_>>(),
                        e.application.as_deref().map(file_name),
                        actions,
                    )
                })
            };
            assert_eq!(
                examples(d.examples.as_ref(), false),
                examples(y.examples.as_ref(), true),
                "{cell}"
            );
        }
    }

    fn renamed(name: &str) -> String {
        RENAMED
            .iter()
            .find(|(was, _)| *was == name)
            .map_or(name, |(_, is)| is)
            .to_string()
    }

    /// Until `process.yaml` goes (Task 11): the derived actions are the
    /// configured ones under their event names (the table "Hernoemingen"),
    /// with the same regulation, outputs, decision and record. What differs
    /// on purpose is listed per fixture, so that a new difference fails.
    #[test]
    fn the_actions_are_those_of_the_configuration() {
        let (service, cells, deployment) = setup();
        let all = derive(&service, &cells, &deployment).unwrap();
        type Summary = (String, Vec<String>, Option<String>, String, String);
        for (cell, process) in [
            ("test_afnemer", "afnemer"),
            ("test_instantie", "instantie"),
            ("test_toeslag", "toeslag"),
        ] {
            let d = &all
                .iter()
                .find(|p| p.definition.id == cell)
                .unwrap()
                .definition;
            let y = from_yaml(process);
            let (Some(dh), Some(yh)) = (&d.handling, &y.handling) else {
                assert!(d.handling.is_none() && y.handling.is_none(), "{cell}");
                continue;
            };
            // The worklist: the runtime's own instead of the cell's
            // `werkvoorraad` (decision 4 of the controller).
            assert_eq!(yh.worklist.lexostatus, "werkvoorraad", "{cell}");
            assert_eq!(dh.worklist.lexostatus, crate::reduction::WORKLIST, "{cell}");
            assert_eq!(dh.worklist.cell, yh.worklist.cell, "{cell}");
            let summary = |h: &ActionDefinition| -> (String, Summary) {
                (
                    renamed(&h.name),
                    (
                        h.regulation.clone(),
                        h.outputs.clone(),
                        h.decision.as_deref().map(renamed),
                        h.record.stream.clone(),
                        h.record.event.clone(),
                    ),
                )
            };
            let derived: BTreeMap<String, Summary> = dh.actions.iter().map(summary).collect();
            let configured: BTreeMap<String, Summary> = yh.actions.iter().map(summary).collect();
            assert_eq!(derived, configured, "{cell}");
            // The article: the configuration finds it at load time through
            // its first output; the derivation sets it.
            for h in &yh.actions {
                let a = &dh.action(&renamed(&h.name)).unwrap().article;
                assert!(
                    crate::action::outputs_of_article(&service, a).contains(&h.outputs[0]),
                    "{cell}/{}: {a}",
                    h.name
                );
            }
            // The order: the streams, not the configuration.
            let order = |h: &Handling| {
                h.actions
                    .iter()
                    .map(|a| renamed(&a.name))
                    .collect::<Vec<_>>()
            };
            let expected_order: &[&str] = match cell {
                "test_afnemer" => &[
                    "aanvulling_gevraagd",
                    "besluit_genomen",
                    "besluit_bekendgemaakt",
                    "betaling_verricht",
                ],
                _ => &[
                    "voorschot_verleend",
                    "toeslag_vastgesteld",
                    "vaststelling_gewijzigd",
                    "terugvordering_vastgesteld",
                    "besluit_bekendgemaakt_voorschot_verleend",
                    "besluit_bekendgemaakt_toeslag_vastgesteld",
                    "besluit_bekendgemaakt_vaststelling_gewijzigd",
                    "besluit_bekendgemaakt_terugvordering_vastgesteld",
                    "voorschot_betaald",
                    "terugbetaling_ontvangen",
                ],
            };
            assert_eq!(order(dh), expected_order, "{cell}");
            assert_ne!(
                order(yh),
                expected_order,
                "{cell}: the configured order differed"
            );
            // The labels follow the law (the table "Hernoemingen").
            for h in &dh.actions {
                assert_eq!(h.role, None, "{cell}/{}", h.name);
                assert!(
                    h.label.as_deref().is_some_and(|l| l.contains(", artikel ")),
                    "{cell}/{}",
                    h.name
                );
            }
        }
    }

    /// The actions follow from the streams, the stages and the law, with the
    /// renamings of the table in the plan (name = event name).
    #[test]
    fn the_actions_follow_from_the_streams_and_the_law() {
        type Row = (String, String, Vec<String>, Option<String>);
        let summary = |d: &ProcessDefinition| -> Vec<Row> {
            d.handling
                .as_ref()
                .unwrap()
                .actions
                .iter()
                .map(|h| {
                    (
                        h.name.clone(),
                        h.article.clone(),
                        h.outputs.clone(),
                        h.decision.clone(),
                    )
                })
                .collect()
        };
        let a = derived("test_afnemer");
        let s = summary(&a);
        assert_eq!(s[0].1, "testregeling_awb#5");
        assert_eq!(s[0].2, ["termijn_opgeschort"]);
        assert_eq!(s[1].1, "testregeling_afnemer#3");
        assert_eq!(
            s[1].2,
            [
                "vastgesteld_bedrag",
                "gebiedsbedrag",
                "besluit_tijdig",
                "besluitdeadline",
                "zorgvuldig"
            ]
        );
        assert_eq!(s[2].1, "testregeling_afnemer#3");
        assert_eq!(s[2].2, ["besluit_tijdig"]);
        assert_eq!(s[3].1, "testregeling_awb#3");
        assert_eq!(s[3].2, ["nog_te_betalen", "onverschuldigd_betaald"]);
        assert_eq!(s[3].3.as_deref(), Some("besluit_genomen"));
        let actions = &a.handling.as_ref().unwrap().actions;
        let labels: Vec<&str> = actions.iter().map(|h| h.label()).collect();
        assert_eq!(
            labels,
            [
                "Aanvulling gevraagd (Testregeling afnemer, artikel 3)",
                "Besluit genomen (Testregeling afnemer, artikel 3)",
                "Besluit bekendgemaakt (Testregeling verzuim, artikel 2)",
                "Betaling verricht (Testregeling verzuim, artikel 3)",
            ]
        );
        assert_eq!(actions[1].rows.len(), 1, "rows from synthesis.yaml");

        let t = derived("test_toeslag");
        let s = summary(&t);
        assert_eq!(s[2].1, "testregeling_toeslag#4");
        assert_eq!(s[2].3.as_deref(), Some("toeslag_vastgesteld"));
        assert_eq!(
            s[4].2,
            ["voorschot"],
            "a follow-up that records nothing of the decision recomputes all of it"
        );
        assert_eq!(s[4].1, "testregeling_toeslag#2");
        assert_eq!(s[8].1, "testregeling_toeslag#8");
        assert_eq!(s[8].2, ["nog_te_betalen_voorschot"]);
        assert_eq!(s[8].3.as_deref(), Some("voorschot_verleend"));
        assert_eq!(s[9].1, "testregeling_awb#6");
        assert_eq!(s[9].2, ["nog_terug_te_betalen"]);
        assert_eq!(s[9].3.as_deref(), Some("terugvordering_vastgesteld"));
        let actions = &t.handling.as_ref().unwrap().actions;
        assert_eq!(
            actions[0].label(),
            "Voorschot verleend (Testregeling maandtoeslag, artikel 2)"
        );
        assert_eq!(
            actions[4].label(),
            "Besluit bekendgemaakt (Testregeling verzuim, artikel 2) bij voorschot verleend"
        );
        // The kind is set later by `prepare_for`, as for process.yaml; it
        // keeps the article the derivation set.
        assert!(actions
            .iter()
            .all(|h| h.kind == crate::config::ActionKind::default()));
        let (service, cells, _) = setup();
        for (mut d, kinds) in [
            (a, vec!["fact", "decision", "follow_up", "fact"]),
            (
                t,
                vec![
                    "decision",
                    "decision",
                    "decision",
                    "decision",
                    "follow_up",
                    "follow_up",
                    "follow_up",
                    "follow_up",
                    "fact",
                    "fact",
                ],
            ),
        ] {
            let before: Vec<String> = d
                .handling
                .as_ref()
                .unwrap()
                .actions
                .iter()
                .map(|h| h.article.clone())
                .collect();
            let authority = crate::authority::own(&d, &service);
            let cell = cells[&d.id].clone();
            let errors = crate::action::prepare_for(&mut d, authority.as_deref(), &service, &cell);
            assert!(errors.is_empty(), "{}: {errors:?}", d.id);
            let actions = &d.handling.as_ref().unwrap().actions;
            let after: Vec<String> = actions.iter().map(|h| h.article.clone()).collect();
            assert_eq!(before, after, "{}", d.id);
            let got: Vec<String> = actions
                .iter()
                .map(|h| {
                    serde_json::to_value(&h.kind).unwrap()["kind"]
                        .as_str()
                        .unwrap()
                        .to_string()
                })
                .collect();
            assert_eq!(got, kinds, "{}", d.id);
        }
    }

    #[test]
    fn a_fact_no_article_reads_is_no_action_but_a_warning() {
        let (s, cells, d) = setup();
        let all = derive(&s, &cells, &d).unwrap();
        let a = all
            .iter()
            .find(|p| p.definition.id == "test_afnemer")
            .unwrap();
        assert!(
            a.warnings
                .iter()
                .any(|w| w.contains("'termijn_opgeschort'")),
            "{:?}",
            a.warnings
        );
        let h = a.definition.handling.as_ref().unwrap();
        assert!(h.action("termijn_opgeschort").is_none());
        // The other processes miss nothing.
        for p in all.iter().filter(|p| p.definition.id != "test_afnemer") {
            assert!(
                p.warnings.is_empty(),
                "{}: {:?}",
                p.definition.id,
                p.warnings
            );
        }
    }

    #[test]
    fn an_output_depends_on_a_parameter_through_inputs_and_outputs() {
        let (s, _, _) = setup();
        let a = crate::regulations::article(&s, "testregeling_awb#3").unwrap();
        let out = outputs_depending_on(a, &BTreeSet::from(["betaald_bedrag".to_string()]));
        assert_eq!(
            out,
            [
                "nog_te_betalen",
                "betaling_conform",
                "onverschuldigd_betaald"
            ]
        );
        // Through an input whose source passes the parameter on.
        let out = outputs_depending_on(a, &BTreeSet::from(["datum_bekendmaking".to_string()]));
        assert_eq!(out, ["betaling_conform"]);
        let a = crate::regulations::article(&s, "testregeling_awb#5").unwrap();
        assert!(outputs_depending_on(a, &BTreeSet::from(["iets_anders".to_string()])).is_empty());
    }

    /// A reference to a stage with more than one decision of the actor does
    /// not say which; the runtime does not start.
    #[test]
    fn a_reference_to_one_of_more_decisions_names_the_article() {
        let e = errors_after(|_, cells, _| {
            let events = &mut cell(cells, "test_toeslag")
                .streams
                .iter_mut()
                .find(|s| s.id == "test_toeslag_zaakverloop")
                .unwrap()
                .events;
            let paid = events
                .iter_mut()
                .find(|e| e.name == "voorschot_betaald")
                .unwrap();
            paid.refers_to.get_mut("decision").unwrap().to = To::Stage("BESLUIT".into());
        });
        assert!(
            has(&e, &["'voorschot_betaald'", "4 decisions", "refers_to"]),
            "{e:?}"
        );
    }

    #[test]
    fn rows_for_no_action_stop_the_derivation() {
        let e = errors_after(|_, _, d| {
            let s = d.synthesis.get_mut("test_afnemer").unwrap();
            let rows = s.action_rows.remove("besluit_genomen").unwrap();
            s.action_rows.insert("besluit".into(), rows);
        });
        assert!(
            has(&e, &["synthesis.yaml", "'besluit'", "no action"]),
            "{e:?}"
        );
    }

    #[test]
    fn the_channels_of_the_policy_carry_their_articles() {
        let d = derived("test_toeslag");
        assert_eq!(d.declared_by.as_deref(), Some("testbeleid_toeslag#6"));
        let persoon = &d.channels["persoon"];
        assert_eq!(persoon.declared_by.as_deref(), Some("testbeleid_toeslag#6"));
        assert_eq!(persoon.supplied_by.as_deref(), Some("testbeleid_toeslag#4"));
        assert!(d.channels["medewerker"].supplied_by.is_none());
        // The basis of the role is the first of the channel.
        assert_eq!(
            d.roles["aanvrager"].legal_basis.as_deref(),
            Some("testbeleid_toeslag#4 lid 1")
        );
        let a = derived("test_afnemer");
        assert!(matches!(
            &a.on_behalf_of,
            Some(crate::config::OnBehalfOf::Authority { authority }) if authority == "Test afnemer"
        ));
        assert_eq!(a.origin_check, crate::config::OriginCheck::Strict);
    }

    #[test]
    fn a_channel_without_an_adapter_stops_the_derivation() {
        let (s, cells, mut d) = setup();
        d.channels
            .get_mut("test_afnemer")
            .unwrap()
            .remove("medewerker");
        let e = derive(&s, &cells, &d).unwrap_err();
        assert!(
            e.iter().any(|f| f.contains("'medewerker'")
                && f.contains("testbeleid_afnemer#1")
                && f.contains("channels.yaml")),
            "{e:?}"
        );
    }

    #[test]
    fn an_adapter_without_a_channel_stops_the_derivation() {
        let (s, cells, mut d) = setup();
        let extra = d.channels["test_afnemer"]["medewerker"].clone();
        d.channels
            .get_mut("test_afnemer")
            .unwrap()
            .insert("fax".into(), extra);
        let e = derive(&s, &cells, &d).unwrap_err();
        assert!(e.iter().any(|f| f.contains("'fax'")), "{e:?}");
        // Only that: the other channels of the cell are claimed.
        assert_eq!(e.len(), 1, "{e:?}");
    }

    #[test]
    fn a_deployment_of_a_cell_without_policy_stops_the_derivation() {
        let (s, cells, mut d) = setup();
        let extra = d.channels["test_afnemer"].clone();
        d.channels.insert("test_register".into(), extra);
        let e = derive(&s, &cells, &d).unwrap_err();
        assert!(
            e.iter()
                .any(|f| f.contains("'test_register'") && f.contains("channels.yaml")),
            "{e:?}"
        );
        let (s, cells, mut d) = setup();
        let extra = d.synthesis["test_afnemer"].clone();
        d.synthesis.insert("test_onbekend".into(), extra);
        let e = derive(&s, &cells, &d).unwrap_err();
        assert!(
            e.iter()
                .any(|f| f.contains("'test_onbekend'") && f.contains("synthesis.yaml")),
            "{e:?}"
        );
    }

    #[test]
    fn identifies_outside_the_fields_stops_the_derivation() {
        let (s, cells, mut d) = setup();
        d.channels
            .get_mut("test_afnemer")
            .unwrap()
            .get_mut("eherkenning")
            .unwrap()
            .fields
            .remove("persoon");
        let e = derive(&s, &cells, &d).unwrap_err();
        assert!(
            e.iter()
                .any(|f| f.contains("identifies") && f.contains("persoon")),
            "{e:?}"
        );
    }

    /// The errors of the derivation after a mutation of the policies, the
    /// cells or the deployment of the fixtures.
    fn errors_after(
        mutate: impl FnOnce(
            &mut BTreeMap<String, ActorPolicy>,
            &mut BTreeMap<String, Arc<Cell>>,
            &mut Deployment,
        ),
    ) -> Vec<String> {
        let (s, mut cells, mut d) = setup();
        let mut policies = crate::policy::read(&s, None).unwrap();
        mutate(&mut policies, &mut cells, &mut d);
        processes(&policies, &d, &cells, &s, &fixtures()).unwrap_err()
    }

    fn channel<'a>(
        policies: &'a mut BTreeMap<String, ActorPolicy>,
        authority: &str,
        id: &str,
    ) -> &'a mut crate::policy::PolicyChannel {
        &mut policies
            .get_mut(authority)
            .unwrap()
            .channels
            .iter_mut()
            .find(|c| c.id == id)
            .unwrap()
            .def
    }

    fn cell<'a>(cells: &'a mut BTreeMap<String, Arc<Cell>>, id: &str) -> &'a mut Cell {
        Arc::get_mut(cells.get_mut(id).unwrap()).unwrap()
    }

    fn has(e: &[String], parts: &[&str]) -> bool {
        e.iter().any(|f| parts.iter().all(|p| f.contains(p)))
    }

    /// The submission stream of the afnemer.
    fn submissions(cells: &mut BTreeMap<String, Arc<Cell>>) -> &mut Vec<Event> {
        &mut cell(cells, "test_afnemer")
            .streams
            .iter_mut()
            .find(|s| s.id == "test_afnemer_aanvragen")
            .unwrap()
            .events
    }

    #[test]
    fn exactly_one_portal_event() {
        let e = errors_after(|_, cells, _| {
            submissions(cells).retain(|e| e.name != "aanvraag_ontvangen");
        });
        assert!(
            has(
                &e,
                &[
                    "testbeleid_afnemer#1",
                    "0 submission events",
                    "testregeling_afnemer#1"
                ]
            ),
            "{e:?}"
        );
        // The portal could not be found, but its deployment is its own.
        assert!(!has(&e, &["cell of no process"]), "{e:?}");
        let e = errors_after(|_, cells, _| {
            let events = submissions(cells);
            let mut twice = events
                .iter()
                .find(|e| e.name == "aanvraag_ontvangen")
                .unwrap()
                .clone();
            twice.name = "aanvraag_nogmaals".into();
            events.push(twice);
        });
        assert!(
            has(
                &e,
                &[
                    "testbeleid_afnemer#1",
                    "2 submission events",
                    "aanvraag_ontvangen",
                    "aanvraag_nogmaals"
                ]
            ),
            "{e:?}"
        );
    }

    #[test]
    fn exactly_one_assessment_lexostatus() {
        let e = errors_after(|_, cells, _| {
            cell(cells, "test_afnemer")
                .lexostatuses
                .lexostatus_definitions
                .retain(|d| d.name != "aanvraag_inhoud");
        });
        assert!(
            has(
                &e,
                &["testbeleid_afnemer#1", "0 lexostatuses", "test_afnemer"]
            ),
            "{e:?}"
        );
        let e = errors_after(|_, cells, _| {
            let defs = &mut cell(cells, "test_afnemer")
                .lexostatuses
                .lexostatus_definitions;
            let mut copy = defs
                .iter()
                .find(|d| d.name == "aanvraag_inhoud")
                .unwrap()
                .clone();
            copy.name = "aanvraag_kopie".into();
            defs.push(copy);
        });
        assert!(
            has(
                &e,
                &[
                    "testbeleid_afnemer#1",
                    "2 lexostatuses",
                    "aanvraag_inhoud, aanvraag_kopie"
                ]
            ),
            "{e:?}"
        );
    }

    #[test]
    fn exactly_one_channel_submits() {
        let e = errors_after(|p, _, _| {
            channel(p, "Test afnemer", "eherkenning").submits = None;
        });
        assert!(
            has(
                &e,
                &[
                    "Test afnemer",
                    "0 channels name `submits`",
                    "testbeleid_afnemer#1"
                ]
            ),
            "{e:?}"
        );
        assert!(!has(&e, &["cell of no process"]), "{e:?}");
        let e = errors_after(|p, _, _| {
            channel(p, "Test afnemer", "medewerker").submits =
                Some("testregeling_afnemer#1".into());
        });
        assert!(
            has(
                &e,
                &[
                    "2 channels name `submits`",
                    "'eherkenning' in testbeleid_afnemer#1",
                    "'medewerker' in testbeleid_afnemer#1"
                ]
            ),
            "{e:?}"
        );
    }

    #[test]
    fn submits_names_a_loaded_article() {
        for (submits, says) in [
            ("testregeling_afnemer#1 lid 1", "a paragraph"),
            ("testregeling_afnemer#99", "not a loaded article"),
            ("testregeling_afnemer", "<regulation>#<article>"),
        ] {
            let e = errors_after(|p, _, _| {
                channel(p, "Test afnemer", "eherkenning").submits = Some(submits.into());
            });
            assert!(
                has(
                    &e,
                    &["testbeleid_afnemer#1", "'eherkenning'", submits, says]
                ),
                "{e:?}"
            );
        }
    }

    #[test]
    fn assesses_is_an_output_of_the_submitted_article() {
        let e = errors_after(|p, _, _| {
            channel(p, "Test afnemer", "eherkenning").assesses = Some(crate::policy::Assesses {
                output: "bestaat_niet".into(),
            });
        });
        assert!(
            has(
                &e,
                &[
                    "testbeleid_afnemer#1",
                    "'bestaat_niet'",
                    "not an output of testregeling_afnemer#1"
                ]
            ),
            "{e:?}"
        );
    }

    #[test]
    fn two_authorities_cannot_record_in_one_cell() {
        let e = errors_after(|p, _, _| {
            let mut other = p["Test afnemer"].clone();
            other.authority = "Test ander".into();
            p.insert(other.authority.clone(), other);
        });
        assert!(
            has(
                &e,
                &[
                    "Test ander",
                    "testbeleid_afnemer#1",
                    "cell 'test_afnemer'",
                    "'Test afnemer'"
                ]
            ),
            "{e:?}"
        );
    }

    #[test]
    fn a_role_belongs_to_one_channel() {
        let e = errors_after(|p, _, _| {
            channel(p, "Test afnemer", "medewerker").role = Some("aanvrager".into());
        });
        assert!(
            has(
                &e,
                &[
                    "testbeleid_afnemer#1",
                    "role 'aanvrager'",
                    "'medewerker'",
                    "'eherkenning'"
                ]
            ),
            "{e:?}"
        );
    }

    #[test]
    fn the_form_document_exists() {
        let e = errors_after(|p, _, _| {
            channel(p, "Test instantie", "eherkenning").form = Some(crate::policy::PolicyForm {
                document: "documents/bestaat-niet.yaml".into(),
                screen: "aanvraag".into(),
            });
        });
        assert!(
            has(
                &e,
                &["testbeleid_instantie#1", "bestaat-niet.yaml", "not a file"]
            ),
            "{e:?}"
        );
    }

    #[test]
    fn mandates_without_channels_are_an_error() {
        let e = errors_after(|p, _, _| {
            p.insert(
                "Test los".into(),
                ActorPolicy {
                    authority: "Test los".into(),
                    mandates: vec![crate::config::Mandate {
                        authority: "Test afnemer".into(),
                        legal_basis: "testbeleid_los#1".into(),
                    }],
                    ..ActorPolicy::default()
                },
            );
        });
        assert!(
            has(&e, &["Test los", "testbeleid_los#1", "no channel"]),
            "{e:?}"
        );
    }

    #[test]
    fn a_wrong_field_of_an_adapter_names_the_file() {
        let e = errors_after(|_, _, d| {
            let field = d
                .channels
                .get_mut("test_afnemer")
                .unwrap()
                .get_mut("medewerker")
                .unwrap()
                .fields
                .get_mut("naam")
                .unwrap();
            field
                .as_mapping_mut()
                .unwrap()
                .insert("name".into(), "naam".into());
        });
        assert!(
            has(
                &e,
                &[
                    "testbeleid_afnemer#1",
                    "'medewerker'",
                    "'name'",
                    "channels.yaml"
                ]
            ),
            "{e:?}"
        );
    }
}
