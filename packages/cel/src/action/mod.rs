//! Actions in a case: on trial, without recording, and taken, after which
//! the process has the cell record them.
//!
//! `handling.actions` in `process.yaml` names per action an article
//! (a regulation and outputs) and the event in which the cell records it. What
//! an action needs and from whom is not stated there: it follows from the
//! stage of the event (RFC-008) and from the origin of the parameters (RFC-043).
//! There are three kinds ([`ActionKind`]), and the event says which:
//!
//! - **The decision**: the event has a stage that the procedure of the
//!   article knows, and it is the first such action on that article. The
//!   form consists of the verdicts (origin `OORDEEL`); what only a later stage
//!   asks for has not happened yet ([`not_yet`]).
//! - **A follow-up**: a later stage of the same decision, such as the
//!   publication. The engine executes that stage on the input of the
//!   recorded decision (RFC-008, `execute_stage`), with what the stage asks for
//!   (`requires`) as the form; the hooks the law fires on that stage
//!   (such as the objection deadline, Awb 6:8) contribute their outputs.
//! - **A fact** from the course of the case: the event has no stage.
//!   The form consists of the fields of the event that are not outputs; on
//!   trial the cell reduces the lexostatuses of the case as if the fact
//!   were already recorded, so the outputs show what the fact does.
//!
//! A parameter comes from exactly one source: a lexostatus of the case, another
//! synthesis source, the per-row synthesis, the form, or the state of
//! what has not happened yet. Nothing is filled in. If the article of
//! the action is in the legal basis of the event and it is a TOETS, then
//! every boolean output counts: false is a conclusion of the process
//! ([`assessments`]). That way the law says that a payment above the established
//! amount is not in accordance with the establishment (Awb 4:52), not the
//! configuration.
//!
//! The process concludes before it acts, and refuses nothing that happened.
//! If the trial says no on substance (an assessment is false, a hook gives no
//! value), the
//! process does not take the action on its own (`takeable` is false). If the
//! handler reports that the fact happened anyway (`happened: true`), the cell
//! records it, and the lexostatuses show the consequences: a payment above the
//! amount is paid without being owed, a publication that does not comply with
//! the law does not start an objection deadline. Only what concerns the form
//! blocks recording: a form that has not been filled in, an output
//! that the law cannot fully compute (a value or a source is missing),
//! a follow-up without a decision, a moment in the future or before the case. A decision
//! is taken by the process itself; that is not reported.
//!
//! The process does not read the case: what it knows about the case (which stages
//! exist, what the decision recorded, how many grams), the cell derives in
//! its lexostatus [`CaseState`].

use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use regelrecht_engine::{
    ExecutionOutcome, HookPoint, LawExecutionService, StageState, Value as EngineValue,
};
use regelrecht_law_model::{ParameterType, ProcedureDefinition};

use crate::assessment;
use crate::authority::{self, Competence};
use crate::cell::Cell;
use crate::cell_client::{self, DecisionFields, RecordRequest, WithYaml};
use crate::channel::Session;
use crate::config::{ActionDefinition, ActionKind, NotYet, ProcessDefinition};
use crate::date::{self, TimePoint};
use crate::form::{readable, Field};
use crate::gram::{ActingActor, Gram, Input, LoadedRegulation, Receipt, StreamReference};
use crate::process::Process;
use crate::reduction::{AsOf, CaseState, DecisionState, Lexostatus};
use crate::regulations::{self, Required, ValueType};
use crate::rows::{self, Rows};
use crate::stream::{Binding, Case, Decision, Event};
use crate::synthesis::{self, Provenance, Source, SourceResult};
use crate::transport::{Transport, TransportError};

mod check;
mod load;
mod state;
mod take;
mod trial;

// Helpers used by more than one part.
use load::{article_with, outputs_of};
use state::already_taken;
use trial::{event_fields, own};

pub use check::{check, sources_for};
pub use load::{
    assessments, decision_parameter_of, field_kind, hooks_at, not_yet, outputs_of_article,
    prepare_for, procedure_of, required, set_form,
};
pub use state::{
    decisions_in_case, procedure_of_the_case, state, ActionStatus, DecisionInCase, LegalProtection,
    ProcedureStatus, StageStatus,
};
pub use take::{take, Taken};
pub use trial::{target, trial};

/// The answer to an action on trial.
#[derive(Debug, Clone, Serialize)]
pub struct TrialAction {
    pub action: String,
    #[serde(flatten)]
    pub kind: ActionKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stage: Option<String>,
    pub regulation: String,
    /// `<regulation>#<article>` of the outputs.
    pub article: String,
    /// The day on which the engine reads the regulation and the cells are queried.
    pub reference_date: String,
    /// Where the reference date comes from: the `effective_at` of the event, with
    /// its legal basis, or today.
    pub reference_date_from: String,
    /// Whether the process takes the action on its own: every output has a
    /// value, every assessment is true and every fact is filled in.
    pub takeable: bool,
    /// Not takeable because of the substance, not the form: if the handler reports that
    /// the fact happened anyway (`happened: true`), the cell records it.
    /// Never for a decision.
    pub reportable: bool,
    /// The outputs with a value, even if the action is not takeable.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub outputs: BTreeMap<String, Value>,
    /// The assessments of the article (see [`assessments`]) with their value.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub assessments: BTreeMap<String, Value>,
    /// The type and unit of every output and assessment, from the regulation:
    /// an amount in eurocents is shown by the frontend in euros.
    pub types: BTreeMap<String, ValueType>,
    /// What the engine was missing.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub missing: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// What went to the engine, and per parameter where it came from.
    pub parameters: BTreeMap<String, Value>,
    pub provenance: BTreeMap<String, Provenance>,
    pub sources: Vec<SourceResult>,
    /// Parameters the caller of the article must supply, without a
    /// value from a source.
    pub not_delivered: Vec<Required>,
    /// The lexostatuses of the case, for a fact including the draft.
    pub lexostatuses: Vec<Lexostatus>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub rows: Vec<rows::RowsOutcome>,
    /// The recorded decision the action acts on: for a follow-up
    /// the decision the stage continues on, for a fact the decision it
    /// follows (such as the payment that executes it), for an amendment the
    /// decision it amends.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decision: Option<DecisionReference>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_text: Option<String>,
}

/// A recorded decision in the case, as the cell names it in the [`CaseState`].
#[derive(Debug, Clone, Serialize)]
pub struct DecisionReference {
    /// The id of the gram that is the decision.
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stage: Option<String>,
    pub effective_at: String,
    pub recorded_at: String,
}

/// An error in the request, or a state that does not allow the action.
#[derive(Debug, Clone, PartialEq)]
pub enum Refusal {
    /// The form names something the action does not ask for, or a value
    /// the cell cannot put in a gram.
    Invalid(String),
    /// Not takeable: something is missing, an assessment is false, or a fact is not
    /// filled in. No gram is recorded.
    NotTakeable(String),
    /// The cell refuses: the stage is already recorded in the case, or the case changed
    /// since the process read it.
    Conflict(String),
    /// The law designates a different competent authority than the actor of the process.
    Unauthorized(String),
    /// The configuration, or the cell: its chronicle, a reduction or the
    /// recording.
    Cell(String),
}

/// What the handler provides with an action: the form, if needed
/// the decision it acts on (the id of the decision gram; without it the
/// latest, see [`target`]), and when taking, whether the fact happened anyway (see
/// [`take`]).
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ActionInput {
    #[serde(default)]
    pub form: Map<String, Value>,
    #[serde(default)]
    pub decision: Option<String>,
    #[serde(default)]
    pub happened: bool,
}

/// The references of a gram of `event` that a process records: a
/// reference that can only point to a decision gets the decision
/// the action acts on (`decision`); every other one gets the root of the
/// group (such as the application). Without a decision a decision reference is
/// left out; if it is required, the cell refuses.
pub fn references(
    cell: &Cell,
    event: &Event,
    root: &str,
    decision: Option<&str>,
) -> BTreeMap<String, String> {
    let events: Vec<&Event> = cell.streams.iter().flat_map(|s| s.events.iter()).collect();
    event
        .refers_to
        .iter()
        .filter_map(|(name, v)| {
            let targets: Vec<&&Event> = events.iter().filter(|d| v.to.fits_event(d)).collect();
            let to_decision = !targets.is_empty()
                && targets
                    .iter()
                    .all(|d| d.stage.as_deref() == Some(crate::stream::DECISION));
            let id = if to_decision {
                decision?.to_string()
            } else {
                root.to_string()
            };
            Some((name.clone(), id))
        })
        .collect()
}

/// What an action needs from the runtime: the cell, the sources and the
/// per-row synthesis of this action, the loaded regulations (for the
/// receipt) and the current moment.
pub struct Environment<'a> {
    pub process: &'a Process,
    pub cell: &'a dyn Transport,
    pub sources: &'a [Source],
    pub rows: &'a [Rows],
    pub regulations: &'a [LoadedRegulation],
    pub now: DateTime<FixedOffset>,
}
