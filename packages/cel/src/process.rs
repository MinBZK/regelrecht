//! A process: derived from the policy of its actor (RFC-047, see
//! [`crate::derive`]), loaded and checked.
//!
//! A process belongs to the actor. It informs (synthesis of lexostatuses from
//! cells), concludes (the decision) and has a cell record. It records
//! nothing itself and reduces nothing itself: "reductie vindt altijd plaats ín de
//! cel waar de betreffende chronolexogrammen zijn vastgelegd, op verzoek van
//! een businessproces" (position paper). The cell the process records in is,
//! for the process, a source like any other; only its definitions (which
//! streams and lexostatuses it has) it reads directly, for the
//! checks at startup and for the fields of the form.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use regelrecht_engine::LawExecutionService;

use crate::authority;
use crate::cell::Cell;
use crate::channel;
use crate::check;
use crate::config::{
    ExamplesDefinition, Portal, ProcessDefinition, RowsDefinition, SynthesisSource,
};
use crate::examples::{self, Examples};
use crate::form::{self, Form};
use crate::origin;
use crate::rows;
use crate::stream::{Binding, Event, Stream};

/// A loaded process that passed the checks at startup.
pub struct Process {
    pub definition: ProcessDefinition,
    /// The root of the corpus; the paths of a derived process are absolute.
    pub dir: PathBuf,
    /// The cell the process records in: of the portal, the worklist,
    /// the decision and the sources of the case. It runs in this runtime.
    pub cell: Arc<Cell>,
    /// The corpus, shared by the whole runtime.
    pub service: Arc<LawExecutionService>,
    pub form: Option<Form>,
    /// Default data per action (empty without `examples`).
    pub examples: Examples,
    /// What the origin check of the parameters saw, but is no reason
    /// not to start (see [`crate::origin`]).
    pub warnings: Vec<String>,
    /// The window the portal lets the user choose, if the offer asks for one.
    pub window: Option<Window>,
    /// The competent authority the process acts for, from `on_behalf_of`,
    /// checked against the law (see [`crate::authority`]).
    pub authority: String,
}

/// The window of the offer: the parameter with origin BELANGHEBBENDE and
/// `rol: TIJDVAK` (see [`crate::origin`]).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Window {
    pub parameter: String,
    /// The field of the draft (`external`) from which the assessment lexostatus derives the
    /// parameter, if it does. The portal prefills it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
}

impl Process {
    /// Load a process from policy (RFC-047) and check it against the cells of
    /// the runtime. Every error is returned, and every error names the
    /// process. Its paths (form, examples) are absolute; `root` is the corpus
    /// root. The checks on synthesis and actions are in
    /// [`crate::synthesis::check`] and [`crate::action::check`]; the runtime
    /// calls them.
    pub fn from_derived(
        derived: crate::derive::Derived,
        root: &Path,
        cells: &BTreeMap<String, Arc<Cell>>,
        service: Arc<LawExecutionService>,
    ) -> Result<Self, Vec<String>> {
        let id = derived.definition.id.clone();
        let mut p = Self::load_definition(derived.definition, root, cells, service)
            .map_err(|f| with_process(&id, f))?;
        p.warnings.extend(derived.warnings);
        Ok(p)
    }

    fn load_definition(
        mut definition: ProcessDefinition,
        map: &Path,
        cells: &BTreeMap<String, Arc<Cell>>,
        service: Arc<LawExecutionService>,
    ) -> Result<Self, Vec<String>> {
        let cell = the_cell(&definition, cells)?;
        add_law_sources_to(&mut definition, &cell);
        let mut errors = Vec::new();
        errors.extend(actor_records(&definition, &cell));
        let authority = authority::loose_at(&definition, &service)
            .map_err(|f| errors.extend(f))
            .unwrap_or_default();
        let portal_event = definition
            .portal
            .as_ref()
            .and_then(|p| cell.event(&p.stream, &p.event))
            .map(|(_, e)| e);
        errors.extend(channel::check_process(&definition, portal_event, &service));
        errors.extend(crate::synthesis::legal_bases(&definition, &service));
        if let Some(p) = &definition.portal {
            errors.extend(check::portal(
                &cell.streams,
                &cell.lexostatuses,
                p,
                &service,
                &channel::portal_intake_paths(&definition),
            ));
            for (i, r) in p.assessment.rows.iter().enumerate() {
                errors.extend(rows::check(
                    "assessment",
                    r,
                    &p.assessment.rows[..i],
                    &[p.assessment.lexostatus.as_str()],
                    "not the assessment lexostatus",
                    &definition,
                    &cell,
                ));
            }
        }
        errors.extend(crate::action::prepare_for(&mut definition, &service, &cell));
        let form = match definition.portal.as_ref().and_then(|p| p.form.as_ref()) {
            Some(f) => form::load(&map.join(&f.path), &f.screen)
                .map_err(|e| errors.push(e))
                .ok(),
            None => None,
        };
        // Every legal basis in the form points to a loaded article, and
        // a paragraph the article has.
        for (where_, g) in form.iter().flat_map(Form::legal_bases) {
            if let Err(f) = crate::regulations::valid(&service, &g) {
                errors.push(format!("form, {where_}: {f}"));
            }
        }
        let examples = match &definition.examples {
            Some(v) => {
                errors.extend(examples_without_action(&definition, v));
                examples::load(map, v, &definition)
                    .map_err(|f| errors.extend(f))
                    .unwrap_or_default()
            }
            None => Examples::default(),
        };
        if !errors.is_empty() {
            return Err(errors);
        }
        Ok(Self {
            definition,
            dir: map.to_path_buf(),
            cell,
            service,
            form,
            examples,
            warnings: Vec::new(),
            window: None,
            authority,
        })
    }

    pub fn id(&self) -> &str {
        &self.definition.id
    }

    /// The origin check of the parameters (see [`crate::origin`]).
    /// The form of every action follows from it, and the process keeps the
    /// warnings; the errors are returned.
    pub fn check_provenance(&mut self, cells: &BTreeMap<String, Arc<Cell>>) -> Vec<String> {
        let c = origin::check(&self.definition, &self.cell, cells, &self.service);
        if let Some(b) = self.definition.handling.as_mut() {
            for h in &mut b.actions {
                h.verdicts = origin::verdicts(&c, &self.service, &h.name);
            }
        }
        crate::action::set_form(&mut self.definition, &self.service, &self.cell);
        self.warnings.extend(c.warnings);
        self.window = c.window.map(|parameter| Window {
            field: self.concept_field(&parameter),
            parameter,
        });
        c.errors
    }

    /// The field of the draft that the assessment lexostatus reads to derive a
    /// parameter: an `$external` key of the portal event.
    fn concept_field(&self, parameter: &str) -> Option<String> {
        let p = self.portal()?;
        let (_, event) = self.portal_event()?;
        let derivation = self
            .cell
            .lexostatuses
            .lexostatus(&p.assessment.lexostatus)?
            .reduction
            .derivations
            .get(parameter)?;
        let [path] = derivation.read_paths()[..] else {
            return None;
        };
        event.leaves().into_iter().find_map(|b| match b.binding {
            Binding::External(key) | Binding::Supplied(key)
                if b.path == path && !key.contains('.') =>
            {
                Some(key)
            }
            _ => None,
        })
    }

    /// The portal block, if the process has a portal.
    pub fn portal(&self) -> Option<&Portal> {
        self.definition.portal.as_ref()
    }

    /// The stream and the event in which the portal has the cell record.
    pub fn portal_event(&self) -> Option<(&Stream, &Event)> {
        let p = self.portal()?;
        self.cell.event(&p.stream, &p.event)
    }

    /// The rows definitions of the assessment (per-row synthesis).
    pub fn assessment_rows(&self) -> &[RowsDefinition] {
        self.portal()
            .map(|p| p.assessment.rows.as_slice())
            .unwrap_or_default()
    }

    /// The actions of the handling, in the order of the streams.
    pub fn actions(&self) -> &[crate::config::ActionDefinition] {
        self.definition
            .handling
            .as_ref()
            .map(|b| b.actions.as_slice())
            .unwrap_or_default()
    }
}

/// The lexostatuses the law reads in the cell the process records in,
/// as sources of the case (`case: true`), after the sources of the case that
/// the synthesis of the deployment itself names. Which article reads which fact is in the law
/// (`produces.extensions.chronolex.reads`); the process does not need to list
/// them. A source that is already there stays.
fn add_law_sources_to(definition: &mut ProcessDefinition, cell: &Cell) {
    let after = definition
        .synthesis
        .iter()
        .position(|b| !b.case)
        .unwrap_or(definition.synthesis.len());
    let new: Vec<SynthesisSource> = cell
        .lexostatuses
        .lexostatus_definitions
        .iter()
        .filter(|d| d.law.is_some())
        .filter(|d| {
            !definition
                .synthesis
                .iter()
                .any(|b| b.case && b.lexostatus == d.name)
        })
        .map(|d| SynthesisSource {
            cell: cell.id().to_string(),
            url: None,
            regulation: None,
            lexostatus: d.name.clone(),
            case: true,
            input: Default::default(),
            parameters: Default::default(),
            extra_fields: Vec::new(),
            legal_basis: Vec::new(),
        })
        .collect();
    definition.synthesis.splice(after..after, new);
}

/// The cell the process records in. The portal, the worklist, the
/// decision and the sources of the case name the same cell: at this stage
/// a process acts on the cases of one cell, and that cell runs in this
/// runtime (recording does not go over HTTP).
fn the_cell(
    definition: &ProcessDefinition,
    cells: &BTreeMap<String, Arc<Cell>>,
) -> Result<Arc<Cell>, Vec<String>> {
    let mut named: Vec<(String, &str)> = Vec::new();
    if let Some(p) = &definition.portal {
        named.push(("portal".into(), &p.cell));
    }
    if let Some(b) = &definition.handling {
        named.push(("handling.worklist".into(), &b.worklist.cell));
        named.push(("handling.cases".into(), &b.cases.cell));
        for h in &b.actions {
            named.push((format!("action '{}', record", h.name), &h.record.cell));
        }
    }
    for b in definition.case_sources() {
        named.push((
            format!("synthesis source {}/{}", b.cell, b.lexostatus),
            &b.cell,
        ));
    }
    let Some((_, first)) = named.first() else {
        return Err(vec![
            "the process names no cell: without a portal and without a handling it has nothing recorded anywhere"
                .into(),
        ]);
    };
    let mut errors = Vec::new();
    for (where_, cell) in &named {
        if cell != first {
            errors.push(format!(
                "{where_}: cell '{cell}', and the process records in cell '{first}'; a process acts on the cases of one cell"
            ));
        }
    }
    match cells.get(*first) {
        Some(c) if errors.is_empty() => Ok(c.clone()),
        Some(_) => Err(errors),
        None => {
            errors.push(format!(
                "cell '{first}' does not run in this runtime; a process only has records made in a cell of the same runtime"
            ));
            Err(errors)
        }
    }
}

/// The actor of the process is the `recording_actor` of every stream it
/// records in: that of the portal and that of the decision. What a stream
/// does not name does not exist: the check on portal and decision reports that.
fn actor_records(definition: &ProcessDefinition, cell: &Cell) -> Vec<String> {
    let mut streams: Vec<(String, &str)> = Vec::new();
    if let Some(p) = &definition.portal {
        streams.push(("portal".into(), &p.stream));
    }
    for h in definition.handling.iter().flat_map(|b| b.actions.iter()) {
        streams.push((format!("action '{}', record", h.name), &h.record.stream));
    }
    let mut errors = Vec::new();
    for (where_, id) in streams {
        if let Some(s) = cell.streams.iter().find(|s| s.id == id) {
            if s.recording_actor != definition.actor {
                errors.push(format!(
                    "{where_}: stream '{id}' has recording_actor '{}', and the process acts as '{}'",
                    s.recording_actor, definition.actor
                ));
            }
        }
    }
    errors
}

/// An example for an action the process does not have is an error:
/// logins without roles, an application without a portal, a form for an
/// action that does not exist. (That a portal has a role that may use it
/// is checked by [`crate::channel::check_process`].)
fn examples_without_action(definition: &ProcessDefinition, v: &ExamplesDefinition) -> Vec<String> {
    let mut errors = Vec::new();
    if !v.logins.is_empty() && definition.roles.is_empty() {
        errors.push("examples.logins: the process has no roles".to_string());
    }
    if v.application.is_some() && definition.portal.is_none() {
        errors.push("examples.application: the process has no portal".to_string());
    }
    for name in v.actions.keys() {
        if definition
            .handling
            .as_ref()
            .and_then(|b| b.action(name))
            .is_none()
        {
            errors.push(format!(
                "examples.actions: the process has no action '{name}'"
            ));
        }
    }
    errors
}

/// Put the process in front of every message.
pub fn with_process(process: &str, errors: Vec<String>) -> Vec<String> {
    errors
        .into_iter()
        .map(|f| format!("process '{process}': {f}"))
        .collect()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::config::{RowInput, SourceInput};

    /// A process of the fixtures as the policy gives it, after `adjust`,
    /// loaded as the runtime does.
    fn load(
        cell: &str,
        adjust: impl FnOnce(&mut ProcessDefinition),
    ) -> Result<Process, Vec<String>> {
        let (s, cells, d) = crate::derive::tests::setup();
        let mut x = crate::derive::tests::derive(&s, &cells, &d)
            .unwrap()
            .into_iter()
            .find(|p| p.definition.id == cell)
            .unwrap();
        adjust(&mut x.definition);
        Process::from_derived(x, &crate::derive::tests::fixtures(), &cells, s)
    }

    fn errors(cell: &str, adjust: impl FnOnce(&mut ProcessDefinition)) -> Vec<String> {
        load(cell, adjust).err().unwrap()
    }

    fn source<'a>(d: &'a mut ProcessDefinition, lexostatus: &str) -> &'a mut SynthesisSource {
        d.synthesis
            .iter_mut()
            .find(|b| b.lexostatus == lexostatus)
            .unwrap()
    }

    #[test]
    fn load_fixture_processes() {
        let agency = load("test_instantie", |_| {}).unwrap();
        assert_eq!(agency.cell.id(), "test_instantie");
        assert_eq!(agency.portal_event().unwrap().1.name, "aanvraag_ontvangen");
        assert!(agency.form.is_some());
        let consumer = load("test_afnemer", |_| {}).unwrap();
        assert_eq!(consumer.cell.id(), "test_afnemer");
        assert_eq!(consumer.definition.case_sources().count(), 3);
        assert_eq!(consumer.definition.other_sources().count(), 2);
        // What has not happened yet at the decision follows from the
        // procedure of the beschikking (stage BEKENDMAKING after BESLUIT).
        // The day of publication is derived by the lexostatus besluit (no
        // gram: empty), so it is not included.
        let b = consumer.definition.handling.as_ref().unwrap();
        let decision = b.action("besluit_genomen").unwrap();
        assert_eq!(decision.kind, crate::config::ActionKind::Decision);
        assert_eq!(decision.stage.as_deref(), Some("BESLUIT"));
        let state = &decision.not_yet;
        assert_eq!(state["bekendgemaakt"].value, serde_json::json!(false));
        assert_eq!(state["bekendgemaakt"].stage, "BEKENDMAKING");
        assert_eq!(state.len(), 1);
        // The publication is a follow-up to the decision, with the hook of the
        // objection deadline; the payment a fact with an assessment.
        let known = b.action("besluit_bekendgemaakt").unwrap();
        assert_eq!(
            known.kind,
            crate::config::ActionKind::FollowUp {
                decision: "besluit_genomen".into(),
                procedure: "beschikking".into()
            }
        );
        assert_eq!(known.hooks, ["testregeling_awb#4"]);
        assert_eq!(
            known.outputs,
            [
                "besluit_tijdig",
                "aanvang_bezwaartermijn",
                "einde_bezwaartermijn"
            ]
        );
        let pay = b.action("betaling_verricht").unwrap();
        assert_eq!(pay.kind, crate::config::ActionKind::Fact);
        assert_eq!(pay.assessments, ["betaling_conform"]);
    }

    /// A translation in the synthesis rests on a legal basis: every legal
    /// basis points to a loaded article, and every source that translates
    /// has one. The fixture gives both the register and the register status
    /// one; the test takes away the second.
    #[test]
    fn the_legal_basis_of_a_translation() {
        let f = errors("test_afnemer", |d| {
            source(d, "registerstatus").legal_basis.clear();
        });
        assert_eq!(f.len(), 1, "{f:?}");
        assert!(
            f[0].contains("synthesis source test_register/registerstatus: translates (")
                && f[0].contains("geblokkeerd -> geblokkeerd_raad")
                && f[0].contains("without legal basis"),
            "{f:?}"
        );
        // A legal basis that does not exist.
        let f = errors("test_afnemer", |d| {
            source(d, "register").legal_basis[1] = "testregeling_register#9".into();
        });
        assert_eq!(
            f,
            ["process 'test_afnemer': synthesis source test_register/register: legal basis 'testregeling_register#9': regulation 'testregeling_register' has no article 9"]
        );
        // A fixed value in the input of a per-row source is also a
        // translation.
        let f = errors("test_afnemer", |d| {
            let a = d
                .handling
                .as_mut()
                .unwrap()
                .actions
                .iter_mut()
                .find(|a| a.name == "besluit_genomen")
                .unwrap();
            let tarief = a.rows[0]
                .sources
                .iter_mut()
                .find(|s| s.lexostatus == "tarief")
                .unwrap();
            tarief.input.insert(
                "gebied".into(),
                RowInput::Value {
                    value: "noord".into(),
                },
            );
        });
        assert!(
            f.iter().any(|m| m.contains("action 'besluit_genomen', rows 'gebiedstabel', source test_gebieden/tarief: translates (gebied = \"noord\") without legal basis")),
            "{f:?}"
        );
        // An input from a field is no translation.
        assert!(matches!(
            source(
                &mut load("test_afnemer", |_| {}).unwrap().definition,
                "registerstatus"
            )
            .input
            .values()
            .next(),
            Some(SourceInput::Field(_))
        ));
    }

    /// The legal basis of a channel and of a field points to a loaded article,
    /// with the paragraph it names.
    #[test]
    fn the_legal_basis_of_a_channel() {
        let f = errors("test_afnemer", |d| {
            d.channels.get_mut("eherkenning").unwrap().legal_basis =
                vec!["testregeling_afnemer#1 lid 4".into()];
        });
        assert_eq!(f.len(), 1, "{f:?}");
        assert!(
            f[0].contains("channel 'eherkenning': legal basis 'testregeling_afnemer#1 lid 4': article 1 has no paragraph 4"),
            "{f:?}"
        );
        let f = errors("test_afnemer", |d| {
            let k = d.channels.get_mut("eherkenning").unwrap();
            k.fields[0].legal_basis = vec!["testregeling_onbekend#1".into()];
        });
        assert_eq!(
            f,
            ["process 'test_afnemer': channel 'eherkenning', field 'kvk': legal basis 'testregeling_onbekend#1': regulation 'testregeling_onbekend' is not loaded"]
        );
    }

    #[test]
    fn every_message_names_the_process() {
        let f = errors("test_instantie", |d| {
            d.portal.as_mut().unwrap().cell = "bestaat_niet".into();
        });
        assert!(
            f.iter()
                .any(|m| m.contains("cell 'bestaat_niet' does not run in this runtime")),
            "{f:?}"
        );
        assert!(
            f.iter()
                .all(|m| m.starts_with("process 'test_instantie': ")),
            "{f:?}"
        );
    }

    #[test]
    fn the_actor_records_in_its_own_streams() {
        let f = errors("test_afnemer", |d| d.actor = "iemand_anders".into());
        assert!(
            f.iter().any(|f| f.contains(
                "portal: stream 'test_afnemer_aanvragen' has recording_actor 'test_afnemer', and the process acts as 'iemand_anders'"
            )),
            "{f:?}"
        );
        assert!(
            f.iter().any(|f| f
                .contains("action 'besluit_genomen', record: stream 'test_afnemer_zaakverloop'")),
            "{f:?}"
        );
    }

    #[test]
    fn examples_of_the_fixture() {
        let consumer = load("test_afnemer", |_| {}).unwrap();
        let labels: Vec<&str> = consumer
            .examples
            .logins
            .iter()
            .map(|v| v.label.as_str())
            .collect();
        assert_eq!(labels, ["voorbeeld-login", "voorbeeld-login-ander"]);
        assert!(consumer.examples.application.is_some());
        assert!(consumer.examples.actions.contains_key("besluit_genomen"));
    }

    #[test]
    fn example_for_an_action_the_process_does_not_have() {
        let f = errors("test_instantie", |d| {
            d.examples = Some(ExamplesDefinition {
                actions: [("besluit".to_string(), "weg.json".to_string())].into(),
                ..Default::default()
            });
        });
        assert_eq!(f.len(), 2, "{f:?}");
        assert!(f[0].contains("examples.actions: the process has no action 'besluit'"));
        assert!(f[1].contains("weg.json"), "{f:?}");
    }
}
