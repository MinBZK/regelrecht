//! A process from policy (RFC-047): the [`ProcessDefinition`] the runtime
//! used to read from `process.yaml`, derived from the policy of an actor
//! ([`crate::policy`]), the cells and the deployment
//! ([`crate::deployment`]). One process per actor; its id is the id of the
//! cell that records its submissions.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use regelrecht_engine::LawExecutionService;

use crate::cell::Cell;
use crate::channel::{ChannelDefinition, RoleDefinition};
use crate::config::{
    Assessment, FormReference, OnBehalfOf, OriginCheck, Portal, ProcessDefinition,
};
use crate::deployment::{ChannelDeployment, Deployment};
use crate::policy::{ActorPolicy, DeclaredChannel};
use crate::stream::{Event, Stream};

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
        handling: None,
        examples: deployment.examples.get(&id).cloned(),
        declared_by: Some(channel.article.clone()),
    };
    Ok(Derived {
        definition,
        warnings: Vec::new(),
    })
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
mod tests {
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
