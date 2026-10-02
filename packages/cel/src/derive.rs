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
use crate::deployment::Deployment;
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
    let g = crate::regulations::parse(first).ok()?;
    Some(format!("{}#{}", g.regulation, g.article))
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
    for p in policies.values().filter(|p| !p.channels.is_empty()) {
        let at = |m: String| format!("authority '{}': {m}", p.authority);
        let portal = match portal_of(p, cells) {
            Ok(found) => found,
            Err(e) => {
                errors.push(at(e));
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
    submits: String,
    cell: &'a Arc<Cell>,
    stream: &'a Stream,
    event: &'a Event,
}

/// The one channel that names `submits`, and the one submission event, in
/// any cell, that the submitted article establishes.
fn portal_of<'a>(
    p: &'a ActorPolicy,
    cells: &'a BTreeMap<String, Arc<Cell>>,
) -> Result<PortalOf<'a>, String> {
    let portal: Vec<&DeclaredChannel> = p
        .channels
        .iter()
        .filter(|c| c.def.submits.is_some())
        .collect();
    let [channel] = portal[..] else {
        return Err(format!(
            "{} channels name `submits` ({}); exactly one channel is the portal",
            portal.len(),
            portal
                .iter()
                .map(|c| format!("'{}' in {}", c.id, c.article))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    };
    let submits = channel.def.submits.clone().unwrap_or_default();
    let found: Vec<(&Arc<Cell>, &Stream, &Event)> = cells
        .values()
        .flat_map(|c| c.streams.iter().map(move |s| (c, s)))
        .flat_map(|(c, s)| s.events.iter().map(move |e| (c, s, e)))
        .filter(|(_, _, e)| is_submission(e) && establishing(e).as_deref() == Some(&submits))
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
        cell,
        stream,
        event,
    })
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
    let file = deployment.channels_file.display();
    let deployed = deployment.channels.get(&id).cloned().unwrap_or_default();
    let mut channels = BTreeMap::new();
    let mut roles = BTreeMap::new();
    for c in &p.channels {
        let Some(k) = deployed.get(&c.id) else {
            errors.push(format!(
                "{}: channel '{}' has no adapter under '{id}' in {file}",
                c.article, c.id
            ));
            continue;
        };
        let basis = c
            .def
            .identifies
            .as_ref()
            .map(|i| i.fields())
            .unwrap_or_default();
        let known = k.field_names();
        for name in basis.keys().filter(|n| !known.contains(n)) {
            errors.push(format!(
                "{}: channel '{}' identifies '{name}', which is not a field of the channel under '{id}' in {file}",
                c.article, c.id
            ));
        }
        let fields = k
            .identification_fields(&id, &c.id, &basis)
            .map_err(|e| errors.push(format!("{}: {e}", c.article)))
            .unwrap_or_default();
        let supplied = p.supplies.get(&c.id);
        let mut legal_basis = c.def.legal_basis.clone();
        if let Some((by, _)) = supplied {
            // The supplying article, unless a basis already names it.
            let named = legal_basis.iter().any(|b| {
                crate::regulations::parse(b)
                    .is_ok_and(|g| format!("{}#{}", g.regulation, g.article) == *by)
            });
            if !named {
                legal_basis.push(by.clone());
            }
        }
        channels.insert(
            c.id.clone(),
            ChannelDefinition {
                label: k.label.clone(),
                explanation: k.explanation.clone(),
                fields,
                owner: c.def.owner.clone(),
                intake: k.intake.clone(),
                legal_basis,
                supplies: supplied.map(|(_, s)| s.clone()).unwrap_or_default(),
                declared_by: Some(c.article.clone()),
                supplied_by: supplied.map(|(by, _)| by.clone()),
            },
        );
        let role = c.def.role.clone().unwrap_or_else(|| c.id.clone());
        if let Some(other) = roles.get(&role).map(|r: &RoleDefinition| &r.channel) {
            errors.push(format!(
                "{}: role '{role}' of channel '{}' is the role of channel '{other}' too",
                c.article, c.id
            ));
        }
        roles.insert(
            role,
            RoleDefinition {
                channel: c.id.clone(),
                routes: vec![c.def.kind.routes()],
                label: k.role_label.clone(),
                legal_basis: c.def.legal_basis.first().cloned(),
            },
        );
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
    let (channel, submits) = (portal.channel, &portal.submits);
    let assessment = match &channel.def.assesses {
        Some(a) => match assessment_lexostatus(portal, service) {
            Ok(lexostatus) => Some(Assessment {
                lexostatus,
                regulation: submits
                    .split_once('#')
                    .map(|(r, _)| r.to_string())
                    .unwrap_or_default(),
                output: a.output.clone(),
                rows: synthesis.assessment_rows.clone(),
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
    if !errors.is_empty() {
        return Err(errors);
    }
    let form = channel.def.form.as_ref().map(|f| FormReference {
        path: root.join(&f.document).display().to_string(),
        screen: f.screen.clone(),
    });
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
    let submits = &portal.submits;
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
        for (cell, process) in [
            ("test_afnemer", "afnemer"),
            ("test_instantie", "instantie"),
            ("test_toeslag", "toeslag"),
        ] {
            let d = derived(cell);
            let y = from_yaml(process);
            // The process id is the cell id (renamed).
            assert_eq!(d.id, cell);
            assert_eq!(d.actor, y.actor, "{cell}");
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
            assert_eq!(dp.assessment.rows.len(), yp.assessment.rows.len(), "{cell}");
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
            assert_eq!(d.synthesis.len(), y.synthesis.len(), "{cell}");
            for (ds, ys) in d.synthesis.iter().zip(&y.synthesis) {
                assert_eq!(
                    (&ds.cell, &ds.lexostatus, ds.case, &ds.legal_basis),
                    (&ys.cell, &ys.lexostatus, ys.case, &ys.legal_basis),
                    "{cell}"
                );
                assert_eq!(ds.translates(), ys.translates(), "{cell}");
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
}
