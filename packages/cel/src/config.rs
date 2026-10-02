//! Configuration: the runtime environment, the cell definition (`cell.yaml`)
//! and the process definition (`process.yaml`).
//!
//! A cell is a directory under `CELLS_PATH` with a `cell.yaml`, a process a
//! directory under `PROCESSES_PATH` with a `process.yaml`. Paths inside are
//! relative to that directory.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_json::Value;

use crate::channel::{ChannelDefinition, RoleDefinition, Routes};
use crate::load;
use crate::schema::Kind;

/// Default port, within 7100-7300.
pub const DEFAULT_PORT: u16 = 7170;

/// The name of the file that makes a directory a cell.
pub const CELL_FILE: &str = "cell.yaml";

/// The name of the file that makes a directory a process.
pub const PROCESS_FILE: &str = "process.yaml";

/// The runtime environment.
#[derive(Debug, Clone)]
pub struct Config {
    /// Directory with a subdirectory per cell, each with a `cell.yaml`.
    pub cells_path: PathBuf,
    /// Directory with a subdirectory per process, each with a `process.yaml`.
    /// Without it: no processes, only cells.
    pub processes_path: Option<PathBuf>,
    /// Directory with the regulations (the corpus), shared by all cells.
    pub regulation_path: PathBuf,
    /// Directory for the chronicles: a subdirectory `<id>/` per cell.
    pub data_dir: PathBuf,
    pub port: u16,
    /// The read token (`CELL_READ_TOKEN`) shared by runtimes that may read
    /// each other's cells; without it only the runtime itself reads.
    pub read_token: Option<String>,
    /// The runtimes (base urls) that are sent the read token
    /// (`CELL_READ_TOKEN_SOURCES`, comma-separated). A source with another
    /// url does not get it: the token grants reading in this runtime.
    pub read_token_sources: Vec<String>,
    /// Along which route the cells reduce (`CELL_REDUCTION`, experiment A).
    pub reduction: ReductionMode,
    /// The binding file of the registers (`CELL_REGISTERS`): which system
    /// supplies the register a policy queries (see
    /// [`crate::register`]). Without it: no registers, and a policy that
    /// queries one stops the runtime.
    pub registers: Option<PathBuf>,
    /// The channels of the deployment (`CELL_CHANNELS`, RFC-047): with it,
    /// the processes follow from the policy instead of `PROCESSES_PATH`.
    pub channels: Option<PathBuf>,
    /// The synthesis and its rows per actor (`CELL_SYNTHESIS`), until RFC-045.
    pub synthesis: Option<PathBuf>,
    /// The examples of the demo (`CELL_EXAMPLES`).
    pub examples: Option<PathBuf>,
}

/// How the cells of the runtime reduce a lexostatus (`CELL_REDUCTION`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ReductionMode {
    /// The reduction DSL (`dsl`, the default).
    #[default]
    Dsl,
    /// Each lexostatus as an engine run of the regulation the binding file
    /// (`CELL_ENGINE_BINDING`) names (`engine`); see
    /// [`crate::lexostatus_engine`]. With `compare` the cell also reduces
    /// along the DSL and every difference is an error.
    Engine { binding: PathBuf, compare: bool },
}

impl ReductionMode {
    /// From `CELL_REDUCTION` and `CELL_ENGINE_BINDING`. A binding file without
    /// the engine route, or the engine route without a binding file, is an
    /// error: no silent fallback.
    pub fn out(reduction: Option<&str>, binding: Option<&str>) -> Result<Self, String> {
        let binding = binding.map(str::trim).filter(|k| !k.is_empty());
        let engine = |compare| match binding {
            Some(k) => Ok(Self::Engine {
                binding: PathBuf::from(k),
                compare,
            }),
            None => Err(
                "CELL_REDUCTION asks for the engine, but CELL_ENGINE_BINDING is not set"
                    .to_string(),
            ),
        };
        match reduction.map(str::trim).unwrap_or("") {
            "" | "dsl" => match binding {
                None => Ok(Self::Dsl),
                Some(_) => Err(
                    "CELL_ENGINE_BINDING is set, but CELL_REDUCTION is not 'engine' or 'compare'"
                        .into(),
                ),
            },
            "engine" => engine(false),
            "compare" => engine(true),
            otherwise => Err(format!(
                "CELL_REDUCTION '{otherwise}' is not 'dsl', 'engine' or 'compare'"
            )),
        }
    }
}

impl Config {
    pub fn from_env() -> Result<Self, String> {
        fn path(name: &str) -> Result<PathBuf, String> {
            std::env::var(name)
                .ok()
                .filter(|v| !v.trim().is_empty())
                .map(PathBuf::from)
                .ok_or_else(|| format!("{name} is not set"))
        }
        let port = match std::env::var("CELL_PORT") {
            Ok(v) => v
                .parse()
                .map_err(|_| format!("CELL_PORT '{v}' is not a port number"))?,
            Err(_) => DEFAULT_PORT,
        };
        let channels = path("CELL_CHANNELS").ok();
        let synthesis = path("CELL_SYNTHESIS").ok();
        let examples = path("CELL_EXAMPLES").ok();
        let processes_path = path("PROCESSES_PATH").ok();
        check_deployment(
            channels.is_some(),
            synthesis.is_some() || examples.is_some(),
            processes_path.is_some(),
        )?;
        Ok(Self {
            cells_path: path("CELLS_PATH")?,
            processes_path,
            regulation_path: path("REGULATION_PATH")?,
            data_dir: path("DATA_DIR")?,
            port,
            read_token: match std::env::var("CELL_READ_TOKEN") {
                Ok(t) if t.trim().len() >= 16 => Some(t.trim().to_string()),
                Ok(t) if !t.trim().is_empty() => {
                    return Err("CELL_READ_TOKEN is shorter than 16 characters".into())
                }
                _ => None,
            },
            read_token_sources: std::env::var("CELL_READ_TOKEN_SOURCES")
                .unwrap_or_default()
                .split(',')
                .map(|u| u.trim().trim_end_matches('/').to_string())
                .filter(|u| !u.is_empty())
                .collect(),
            reduction: ReductionMode::out(
                std::env::var("CELL_REDUCTION").ok().as_deref(),
                std::env::var("CELL_ENGINE_BINDING").ok().as_deref(),
            )?,
            registers: path("CELL_REGISTERS").ok(),
            channels,
            synthesis,
            examples,
        })
    }
}

/// The deployment files go together (RFC-047): `CELL_SYNTHESIS` and
/// `CELL_EXAMPLES` only with `CELL_CHANNELS`, and the processes follow from
/// the policy or from `PROCESSES_PATH`, not from both.
fn check_deployment(channels: bool, with_channels: bool, processes: bool) -> Result<(), String> {
    if !channels && with_channels {
        return Err(
            "CELL_SYNTHESIS and CELL_EXAMPLES belong to CELL_CHANNELS, which is not set".into(),
        );
    }
    if channels && processes {
        return Err(
            "set CELL_CHANNELS (processes from policy) or PROCESSES_PATH (process.yaml), not both"
                .into(),
        );
    }
    Ok(())
}

/// A cell definition (`schema/chronolex/v0.3.0/cell.json`): only what the
/// cell itself does. Recording (the streams), keeping (the chronicles) and
/// reducing (the lexostatuses).
#[derive(Debug, Clone, Deserialize)]
pub struct CellDefinition {
    pub id: String,
    pub recording_actor: String,
    pub streams: Vec<String>,
    pub lexostatuses: String,
    #[serde(default)]
    pub initial_state: Option<String>,
}

/// A process definition (`schema/chronolex/v0.3.0/process.json`): who acts
/// and how. Informing (synthesis, assessment, offer), concluding (the
/// decision) and having a cell record.
#[derive(Debug, Clone, Deserialize)]
pub struct ProcessDefinition {
    pub id: String,
    /// The actor of the process. A cell records for the process only in
    /// a stream with this `recording_actor`, and the decision is the
    /// decision order ("beschikking") this actor is competent for.
    pub actor: String,
    /// How strict the origin check is (see [`crate::origin`]).
    #[serde(default)]
    pub origin_check: OriginCheck,
    /// On behalf of which competent authority the process acts (see
    /// [`crate::authority`]). Needed for a decision; without it no
    /// implementing policy counts as the actor's.
    #[serde(default)]
    pub on_behalf_of: Option<OnBehalfOf>,
    /// Authorities for which the process acts under mandate (Awb 10:1), each
    /// with a legal basis.
    #[serde(default)]
    pub mandates: Vec<Mandate>,
    /// Along which channels someone logs in (see [`crate::channel`]).
    #[serde(default)]
    pub channels: BTreeMap<String, ChannelDefinition>,
    /// Who logs in, along which channel, and which routes that role may use.
    /// Without roles there is no login.
    #[serde(default)]
    pub roles: BTreeMap<String, RoleDefinition>,
    #[serde(default)]
    pub portal: Option<Portal>,
    #[serde(default)]
    pub synthesis: Vec<SynthesisSource>,
    #[serde(default)]
    pub handling: Option<Handling>,
    /// Default data per action, for a trial setup.
    #[serde(default)]
    pub examples: Option<ExamplesDefinition>,
}

/// How strict the origin check (RFC-043) is.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OriginCheck {
    /// A parameter without origin is a warning.
    #[default]
    Lenient,
    /// A parameter without origin is an error: who supplies it cannot be
    /// traced.
    Strict,
}

/// The `examples` block: a JSON file per action, relative to the
/// directory of the process (see [`crate::examples`]).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExamplesDefinition {
    /// Logins, each an object with the fields of a channel and optionally
    /// `channel` and `role`.
    #[serde(default)]
    pub logins: Vec<String>,
    /// An application: `{external: {...}}`.
    #[serde(default)]
    pub application: Option<String>,
    /// A form per action: `{form: {...}}`.
    #[serde(default)]
    pub actions: BTreeMap<String, String>,
}

/// On behalf of which competent authority the process acts: a name as a
/// regulation gives it in `competent_authority`, or a regulation whose
/// competent authority it is.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum OnBehalfOf {
    Authority { authority: String },
    Regulation { regulation: String },
}

/// A mandate (Awb 10:1): the process also acts on behalf of this authority,
/// on the basis of `legal_basis` (`<regulation>#<article>`).
#[derive(Debug, Clone, Deserialize, serde::Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Mandate {
    pub authority: String,
    pub legal_basis: String,
}

/// What the handler does in the process: a worklist, and actions in a
/// case.
#[derive(Debug, Clone, Deserialize)]
pub struct Handling {
    /// A list lexostatus of the process's cell.
    pub worklist: LexostatusReference,
    pub actions: Vec<ActionDefinition>,
}

impl Handling {
    /// The action with this name.
    pub fn action(&self, name: &str) -> Option<&ActionDefinition> {
        self.actions.iter().find(|h| h.name == name)
    }
}

/// A lexostatus of a cell.
#[derive(Debug, Clone, Deserialize)]
pub struct LexostatusReference {
    pub cell: String,
    pub lexostatus: String,
}

/// An action in a case (see [`crate::action`]): the outputs of an
/// article, and the event in which the cell records it. What the action
/// needs and from whom is not in `process.yaml`: it follows at load time
/// from the stage of the event (RFC-008) and from the origin of the
/// parameters (RFC-043); see the fields without serde below.
#[derive(Debug, Clone, Deserialize)]
pub struct ActionDefinition {
    /// Unique in the process; the route is `cases/<c>/actions/<name>`.
    pub name: String,
    #[serde(default)]
    pub label: Option<String>,
    /// The role that may take the action (a key of `roles`, with
    /// routes `handling`). Without it: every role that may use the handling.
    #[serde(default)]
    pub role: Option<String>,
    /// The action of the decision this action belongs to: for a
    /// fact that follows a decision (a payment that executes it) and for a
    /// decision that amends another. The process then acts on the latest
    /// decision of that action in the case, an amendment of it
    /// included. A follow-up finds its decision itself (see
    /// [`ActionKind::FollowUp`]).
    #[serde(default)]
    pub decision: Option<String>,
    /// The parameter of the article that receives the id of the decision
    /// the action acts on (note on source and gram id): this is how the
    /// process calls its own policy that reads per decision, such as the
    /// payment administration. The law says which parameter it is (origin
    /// role `BESLUIT`, RFC-047) and `prepare_for` fills it in from there; a
    /// value here that contradicts the law is an error.
    #[serde(default)]
    pub decision_parameter: Option<String>,
    /// Empty in `process.yaml`: the runtime fills it at load time with the
    /// regulation of the decision order for which the process's authority
    /// (`on_behalf_of`) is competent (see [`crate::authority::decision_orders_of`]).
    #[serde(default)]
    pub regulation: String,
    /// Outputs of an article. For a follow-up, the outputs of the hooks
    /// of that stage are added at load time.
    #[serde(default)]
    pub outputs: Vec<String>,
    /// Synthesis per row: a table field becomes an array parameter.
    #[serde(default)]
    pub rows: Vec<RowsDefinition>,
    /// Where the action is recorded as a gram.
    pub record: Record,
    /// The article of the outputs, as `<regulation>#<article>`; set at load
    /// time.
    #[serde(skip)]
    pub article: String,
    /// What kind of action it is; derived from the event and the procedure.
    #[serde(skip)]
    pub kind: ActionKind,
    /// The stage of the recording event, if it has one.
    #[serde(skip)]
    pub stage: Option<String>,
    /// Whether the recording event opens, follows or amends a decision.
    #[serde(skip)]
    pub decision_role: Option<crate::stream::Decision>,
    /// The verdicts: the parameters of the article with origin `OORDEEL`
    /// (see [`crate::origin::verdicts`]). Not for a follow-up: those verdicts
    /// the handler gave at the decision.
    #[serde(skip)]
    pub verdicts: Vec<Verdict>,
    /// The facts the action records and the handler fills in: for
    /// a fact the `$external` fields of the event that are not an output,
    /// for a follow-up what the stage asks for (`requires`).
    #[serde(skip)]
    pub facts: Vec<crate::form::Field>,
    /// Facts that only arise in a later stage, with their state at this
    /// action: derived from the procedure (RFC-008), only for a decision
    /// and only for what no lexostatus of the case supplies (see
    /// [`crate::action::load::not_yet`]).
    #[serde(skip)]
    pub not_yet: BTreeMap<String, NotYet>,
    /// The boolean outputs of an assessment article ("TOETS") that is in the
    /// legal basis of the event: false means it cannot be taken (see
    /// [`crate::action::load::assessments`]).
    #[serde(skip)]
    pub assessments: Vec<String>,
    /// For a follow-up: the hooks the law fires on that stage, as
    /// `<regulation>#<article>` (RFC-008).
    #[serde(skip)]
    pub hooks: Vec<String>,
    /// The type and unit of each output and assessment, from the regulation
    /// (see [`crate::regulations::ValueType`]).
    #[serde(skip)]
    pub types: BTreeMap<String, crate::regulations::ValueType>,
}

impl ActionDefinition {
    /// What the frontend calls it.
    pub fn label(&self) -> &str {
        self.label.as_deref().unwrap_or(&self.name)
    }
}

/// What kind of action it is. It follows from the recording event: with a
/// stage it is a decision or a follow-up on a decision, without one a fact.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ActionKind {
    /// A fact from the course of the case (a request, a receipt, a
    /// payment): the event has no stage. The lexostatuses of the case
    /// read it; on trial the draft counts.
    #[default]
    Fact,
    /// The decision: the first stage of the procedure of the article that
    /// an action records. A case can have more decisions, each from its
    /// own article; a decision that amends another records in an
    /// event with `decision: amends`.
    Decision,
    /// A later stage of the same decision, such as the publication: the
    /// engine executes that stage on the input of the recorded decision
    /// (RFC-008, `execute_stage`). That is the latest decision in the case
    /// that the decision's action recorded.
    FollowUp {
        /// The action of the decision.
        decision: String,
        /// The procedure (RFC-008) both stages belong to.
        procedure: String,
    },
}

/// A fact that has not happened yet at the decision: the stage of the
/// procedure in which it only arises, and the state at the decision (false,
/// or empty).
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct NotYet {
    pub value: Value,
    pub stage: String,
}

/// The cell and the event in which the process has an action recorded. The
/// event has `case: follows`; its `$external` keys are outputs of the
/// action or fields of its form.
#[derive(Debug, Clone, Deserialize)]
pub struct Record {
    pub cell: String,
    pub stream: String,
    pub event: String,
}

/// Synthesis per row: for each row of a table field from an own
/// lexostatus the cell queries sources with values from that row, and merges
/// the columns into an array parameter.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RowsDefinition {
    /// The array parameter the rows form together.
    pub parameter: String,
    /// The table field of a lexostatus of the case or of a source that
    /// passes it on (an extra field or parameter).
    pub table: InputReference,
    /// Per column of the table: under which name it goes into the parameter.
    /// A column not listed here is left out.
    pub columns: BTreeMap<String, String>,
    /// Sources queried per row.
    #[serde(default)]
    pub sources: Vec<RowSource>,
}

/// A source queried per row.
#[derive(Debug, Clone, Deserialize)]
pub struct RowSource {
    pub cell: String,
    /// Without url: the source cell runs in the same runtime (internal transport).
    #[serde(default)]
    pub url: Option<String>,
    pub lexostatus: String,
    /// Per input of the source: where the value comes from.
    pub input: BTreeMap<String, RowInput>,
    /// Per name the source supplies: under which column name it goes into the row.
    pub columns: BTreeMap<String, String>,
    /// What the translation rests on: the articles that ask for the column at
    /// the consumer and that have the source supply its fact, and a fixed value
    /// in the input (see [`translates`](RowSource::translates)).
    #[serde(default)]
    pub legal_basis: Vec<String>,
}

impl RowSource {
    /// What this source translates: a column that has a different name at the
    /// consumer than at the source, and a fixed value in the input. Empty: nothing.
    pub fn translates(&self) -> Vec<String> {
        let mut out: Vec<String> = self
            .columns
            .iter()
            .filter(|(b, a)| b != a)
            .map(|(b, a)| format!("{b} -> {a}"))
            .collect();
        out.extend(self.input.iter().filter_map(|(n, i)| match i {
            RowInput::Value { value } => Some(format!("{n} = {value}")),
            _ => None,
        }));
        out
    }
}

/// Where the input of a source per row comes from.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum RowInput {
    /// A column of the row itself, as it is named after the column names.
    Column { column: String },
    /// A field of a lexostatus of the case, or of a source that passes it
    /// on (a parameter or an extra field).
    Own { lexostatus: String, field: String },
    /// A parameter from the combination: the lexostatuses of the case and the
    /// synthesis of the process.
    Parameter { parameter: String },
    /// An output of a regulation, computed with the combined
    /// parameters: the law derives the input, such as a reference date from a
    /// year. Once per execution, for all rows.
    Law { regulation: String, output: String },
    /// A fixed value.
    Value { value: Value },
}

/// Where the input of a synthesis source comes from.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum SourceInput {
    /// A field of a lexostatus of the case, or of an earlier source.
    Field(InputReference),
    /// A fixed value, such as the body whose register is requested.
    Value { value: Value },
}

impl SourceInput {
    /// The field, if the input points at one.
    pub fn field(&self) -> Option<&InputReference> {
        match self {
            SourceInput::Field(v) => Some(v),
            SourceInput::Value { .. } => None,
        }
    }
}

/// The parameters a synthesis source supplies: per name at the source the name
/// at the consumer. The source speaks the language of its own law; the
/// translation belongs to the consumer. In `process.yaml` a list (the same
/// name) or a table (source: consumer).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Parameters(Vec<(String, String)>);

impl Parameters {
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Pairs (name at the source, name at the consumer).
    pub fn pairs(&self) -> impl Iterator<Item = (&str, &str)> {
        self.0.iter().map(|(b, a)| (b.as_str(), a.as_str()))
    }

    /// The names at the consumer.
    pub fn iter(&self) -> impl Iterator<Item = &String> {
        self.0.iter().map(|(_, a)| a)
    }

    /// The pairs in which the consumer asks for the fact under another name.
    pub fn translated(&self) -> BTreeMap<&str, &str> {
        self.pairs().filter(|(b, a)| b != a).collect()
    }
}

/// As a list of the names at the consumer: what the source supplies to the process.
impl serde::Serialize for Parameters {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_seq(self.iter())
    }
}

/// Over the names at the consumer: the parameters the source supplies to the process.
impl<'a> IntoIterator for &'a Parameters {
    type Item = &'a String;
    type IntoIter = std::iter::Map<
        std::slice::Iter<'a, (String, String)>,
        fn(&'a (String, String)) -> &'a String,
    >;
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter().map(|(_, a)| a)
    }
}

impl<'de> Deserialize<'de> for Parameters {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Shape {
            List(Vec<String>),
            Table(BTreeMap<String, String>),
        }
        Ok(Parameters(match Shape::deserialize(d)? {
            Shape::List(l) => l.into_iter().map(|n| (n.clone(), n)).collect(),
            Shape::Table(t) => t.into_iter().collect(),
        }))
    }
}

/// A field of the decision form: a parameter with a label, from the
/// regulation.
#[derive(Debug, Clone, PartialEq)]
pub struct Verdict {
    pub parameter: String,
    pub label: String,
    pub group: Option<String>,
    pub explanation: Option<String>,
}

/// The portal block: in which cell and which event a submission goes, and which
/// output the assessment asks for.
#[derive(Debug, Clone, Deserialize)]
pub struct Portal {
    pub cell: String,
    pub stream: String,
    pub event: String,
    pub assessment: Assessment,
    /// What the portal offers: an output of the actor's policy,
    /// optionally with the deadline shown with it.
    #[serde(default)]
    pub offer: Option<Offer>,
    #[serde(default)]
    pub form: Option<FormReference>,
}

/// The offer of a portal: an output of a regulation, executed in
/// a run, and optionally a second output of the same regulation that gives the
/// deadline.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Offer {
    pub regulation: String,
    pub output: String,
    #[serde(default)]
    pub deadline: Option<String>,
    /// An output of the same regulation: the windows the policy
    /// offers, if the offer article asks for a window (the parameter with
    /// origin BELANGHEBBENDE and `rol: TIJDVAK`). The portal computes
    /// it in a run without parameters on today's date.
    #[serde(default)]
    pub windows: Option<String>,
    /// An output of the same regulation: the first day of a window, with
    /// the window as its only parameter. The offer for a window that has yet
    /// to begin queries the registers on that day; without it, on today.
    #[serde(default)]
    pub start: Option<String>,
    /// An output of the same regulation: the first day on which an application
    /// for a window can come in, with the window as its only parameter.
    /// A counter does not enter a receipt from before that day.
    #[serde(default)]
    pub opening: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Assessment {
    pub lexostatus: String,
    pub regulation: String,
    pub output: String,
    /// Synthesis per row, as for the decision: a table field of the
    /// assessment lexostatus becomes an array parameter.
    #[serde(default)]
    pub rows: Vec<RowsDefinition>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FormReference {
    pub path: String,
    pub screen: String,
}

/// A lexostatus of a cell that the process combines (synthesis).
///
/// A source with `case: true` is a lexostatus of the case itself, in the cell
/// in which the process records: the process queries it with the root,
/// and it supplies all its parameters and extra fields. Every other source names
/// its input and its parameters.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SynthesisSource {
    /// The cell that supplies the lexostatus. Empty for a source with `regulation`.
    #[serde(default)]
    pub cell: String,
    /// Instead of a cell: the consumer's own policy, computed by the
    /// engine (note on source and gram id; see
    /// [`crate::synthesis::PolicySource`]). `lexostatus` is then the article,
    /// the input is the parameters and `extra_fields` the outputs.
    #[serde(default)]
    pub regulation: Option<String>,
    /// Without url: the source cell runs in the same runtime (internal transport).
    #[serde(default)]
    pub url: Option<String>,
    pub lexostatus: String,
    /// A lexostatus of the case, with `root` as its only input.
    #[serde(default)]
    pub case: bool,
    /// Per input of the source: from which field of a lexostatus of the case
    /// (for the assessment: the assessment lexostatus), of an earlier source, or
    /// a fixed value.
    #[serde(default)]
    pub input: BTreeMap<String, SourceInput>,
    /// The parameters this source supplies, explicitly, with the name at the
    /// consumer.
    #[serde(default)]
    pub parameters: Parameters,
    /// Fields from the response that are not a parameter, but input for a
    /// later source (for example a name for a registration number).
    #[serde(default)]
    pub extra_fields: Vec<String>,
    /// What the translation rests on: the articles that ask for the fact at
    /// the consumer under its name and that have the source supply it, and that
    /// carry a fixed value in the input (see [`translates`](SynthesisSource::translates)).
    #[serde(default)]
    pub legal_basis: Vec<String>,
}

impl SynthesisSource {
    /// What this source translates: a parameter that has a different name at
    /// the consumer than at the source, and a fixed value in the input. Empty: nothing.
    pub fn translates(&self) -> Vec<String> {
        let mut out: Vec<String> = self
            .parameters
            .translated()
            .into_iter()
            .map(|(b, a)| format!("{b} -> {a}"))
            .collect();
        out.extend(self.input.iter().filter_map(|(n, i)| match i {
            SourceInput::Value { value } => Some(format!("{n} = {value}")),
            SourceInput::Field(_) => None,
        }));
        out
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct InputReference {
    pub lexostatus: String,
    pub field: String,
}

impl CellDefinition {
    /// Read a cell definition from text and validate it against the schema.
    pub fn parse(text: &str, source: &str) -> Result<Self, Vec<String>> {
        load::definition(text, source, Kind::Cell)
    }

    /// Load `cell.yaml` from the directory of a cell.
    pub fn load(map: &Path) -> Result<Self, Vec<String>> {
        load::load(&map.join(CELL_FILE), Self::parse)
    }
}

impl ProcessDefinition {
    /// Read a process definition from text and validate it against the schema.
    pub fn parse(text: &str, source: &str) -> Result<Self, Vec<String>> {
        load::definition(text, source, Kind::Process)
    }

    /// Load `process.yaml` from the directory of a process.
    pub fn load(map: &Path) -> Result<Self, Vec<String>> {
        load::load(&map.join(PROCESS_FILE), Self::parse)
    }

    /// The roles that may use a route group.
    pub fn roles_with(&self, r: Routes) -> impl Iterator<Item = (&String, &RoleDefinition)> {
        self.roles.iter().filter(move |(_, d)| d.may(r))
    }

    /// The channels of the roles that may use a route group, each once,
    /// with their id.
    pub fn channels_with(&self, r: Routes) -> Vec<(&str, &ChannelDefinition)> {
        let mut out: Vec<(&str, &ChannelDefinition)> = Vec::new();
        for (_, role) in self.roles_with(r) {
            if let Some((id, k)) = self.channels.get_key_value(&role.channel) {
                if !out.iter().any(|(i, _)| *i == id) {
                    out.push((id, k));
                }
            }
        }
        out
    }

    /// The sources of the case (`case: true`), in the order of the synthesis.
    pub fn case_sources(&self) -> impl Iterator<Item = &SynthesisSource> {
        self.synthesis.iter().filter(|b| b.case)
    }

    /// The sources that are not a lexostatus of the case.
    pub fn other_sources(&self) -> impl Iterator<Item = &SynthesisSource> {
        self.synthesis.iter().filter(|b| !b.case)
    }
}

/// The directories under `PROCESSES_PATH` with a `process.yaml`, sorted. An
/// empty directory is allowed: a runtime with only register cells has no process.
pub fn process_dirs(processes_path: &Path) -> Result<Vec<PathBuf>, String> {
    load::dirs_with(processes_path, PROCESS_FILE)
}

/// The directories under `CELLS_PATH` with a `cell.yaml`, sorted.
pub fn cell_dirs(cells_path: &Path) -> Result<Vec<PathBuf>, String> {
    let dirs = load::dirs_with(cells_path, CELL_FILE)?;
    if dirs.is_empty() {
        return Err(format!(
            "{}: no subdirectory with a {CELL_FILE}",
            cells_path.display()
        ));
    }
    Ok(dirs)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    pub(crate) fn fixtures() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
    }

    #[test]
    fn the_deployment_files_go_together() {
        assert_eq!(check_deployment(true, true, false), Ok(()));
        assert_eq!(check_deployment(false, false, true), Ok(()));
        let e = check_deployment(false, true, false).unwrap_err();
        assert!(e.contains("CELL_CHANNELS, which is not set"), "{e}");
        let e = check_deployment(true, false, true).unwrap_err();
        assert!(e.contains("not both"), "{e}");
    }

    #[test]
    fn fixture_cells_load() {
        let dirs = cell_dirs(&fixtures().join("cells")).unwrap();
        let ids: Vec<String> = dirs
            .iter()
            .map(|m| CellDefinition::load(m).unwrap().id)
            .collect();
        assert_eq!(
            ids,
            [
                "test_afnemer",
                "test_gebieden",
                "test_instantie",
                "test_register",
                "test_toeslag"
            ]
        );
    }

    #[test]
    fn fixture_processes_load() {
        let dirs = process_dirs(&fixtures().join("processes")).unwrap();
        let ids: Vec<String> = dirs
            .iter()
            .map(|m| ProcessDefinition::load(m).unwrap().id)
            .collect();
        assert_eq!(
            ids,
            [
                "test_afnemer_proces",
                "test_instantie_proces",
                "test_toeslag_proces"
            ]
        );
    }

    #[test]
    fn cell_definition_validates_against_the_schema() {
        let error =
            CellDefinition::parse("id: x\nrecording_actor: x\nstreams: []\n", "t").unwrap_err();
        assert!(
            error.iter().any(|f| f.contains("lexostatuses")),
            "{error:?}"
        );
        assert!(error.iter().any(|f| f.contains("/streams")), "{error:?}");
    }

    #[test]
    fn a_cell_has_no_process_blocks() {
        let error = CellDefinition::parse(
            "id: a\nrecording_actor: a\nstreams: [s.yaml]\nlexostatuses: l.yaml\nroles: {aanvrager: {channel: k, routes: [portal]}}\n",
            "t",
        )
        .unwrap_err();
        assert!(error.iter().any(|f| f.contains("roles")), "{error:?}");
    }

    #[test]
    fn synthesis_source_with_url() {
        let d = ProcessDefinition::parse(
            "id: a\nactor: a\nsynthesis:\n  - {cell: b, url: 'http://localhost:7172', lexostatus: l, input: {}, parameters: [p]}\n",
            "t",
        )
        .unwrap();
        assert_eq!(d.synthesis[0].url.as_deref(), Some("http://localhost:7172"));
        assert!(d.portal.is_none());
        assert!(d.examples.is_none());
    }

    /// The parameters of a source: a list (the same name) or per name at
    /// the source the name at the consumer; an input is a field or a fixed
    /// value.
    #[test]
    fn parameters_with_translation_and_a_fixed_input() {
        let d = ProcessDefinition::parse(
            "id: a\nactor: a\nsynthesis:\n  - {cell: b, lexostatus: l, input: {x: {lexostatus: e, field: x}, orgaan: {value: raad}}, parameters: {is_ingeschreven_in_register: is_ingeschreven_raad}}\n  - {cell: c, lexostatus: m, input: {}, parameters: [p]}\n",
            "t",
        )
        .unwrap();
        let b = &d.synthesis[0];
        assert_eq!(
            b.parameters.pairs().collect::<Vec<_>>(),
            [("is_ingeschreven_in_register", "is_ingeschreven_raad")]
        );
        assert_eq!(
            b.parameters.iter().collect::<Vec<_>>(),
            ["is_ingeschreven_raad"]
        );
        assert!(matches!(&b.input["orgaan"], SourceInput::Value { value } if value == "raad"));
        assert_eq!(b.input["x"].field().unwrap().lexostatus, "e");
        assert_eq!(
            d.synthesis[1].parameters.pairs().collect::<Vec<_>>(),
            [("p", "p")]
        );
        assert!(d.synthesis[1].parameters.translated().is_empty());
    }

    /// The windows and the state at decision are not in the configuration.
    #[test]
    fn choices_and_state_at_decision_are_rejected() {
        let error = ProcessDefinition::parse(
            "id: a\nactor: a\nportal:\n  cell: a\n  stream: s\n  event: e\n  assessment: {lexostatus: l, regulation: r, output: u}\n  offer: {regulation: r, output: u, keuzes: {jaren_vanaf_nu: [0]}}\n",
            "t",
        )
        .unwrap_err();
        assert!(error.iter().any(|f| f.contains("keuzes")), "{error:?}");
        let error = ProcessDefinition::parse(
            "id: a\nactor: a\nhandling:\n  worklist: {cell: a, lexostatus: w}\n  actions:\n    - name: b\n      outputs: [u]\n      record: {cell: a, stream: s, event: e}\n      stand_bij_besluit: {x: false}\n",
            "t",
        )
        .unwrap_err();
        assert!(
            error.iter().any(|f| f.contains("stand_bij_besluit")),
            "{error:?}"
        );
    }

    #[test]
    fn a_case_source_names_no_input_or_parameters() {
        let d = ProcessDefinition::parse(
            "id: a\nactor: a\nsynthesis:\n  - {cell: b, lexostatus: l, case: true}\n  - {cell: c, lexostatus: m, input: {x: {lexostatus: l, field: x}}, parameters: [p]}\n",
            "t",
        )
        .unwrap();
        assert_eq!(d.case_sources().count(), 1);
        assert_eq!(d.other_sources().count(), 1);
        let error = ProcessDefinition::parse(
            "id: a\nactor: a\nsynthesis:\n  - {cell: b, lexostatus: l, case: true, parameters: [p]}\n",
            "t",
        )
        .unwrap_err();
        assert!(
            error.iter().any(|f| f.contains("/synthesis/0")),
            "{error:?}"
        );
        let error = ProcessDefinition::parse(
            "id: a\nactor: a\nsynthesis:\n  - {cell: b, lexostatus: l}\n",
            "t",
        )
        .unwrap_err();
        assert!(
            error.iter().any(|f| f.contains("/synthesis/0")),
            "{error:?}"
        );
    }

    /// The decision form is not in `process.yaml`: it follows from the
    /// parameters with origin OORDEEL.
    #[test]
    fn a_decision_form_in_the_configuration_is_rejected() {
        let error = ProcessDefinition::parse(
            "id: a\nactor: a\nhandling:\n  worklist: {cell: a, lexostatus: w}\n  actions:\n    - name: b\n      outputs: [u]\n      record: {cell: a, stream: s, event: e}\n      form: [{parameter: p, label: P}]\n",
            "t",
        )
        .unwrap_err();
        assert!(error.iter().any(|f| f.contains("form")), "{error:?}");
    }

    #[test]
    fn examples_block() {
        let d = ProcessDefinition::parse(
            "id: a\nactor: a\nexamples:\n  logins: [login.json]\n  actions: {besluit: besluit.json}\n",
            "t",
        )
        .unwrap();
        let v = d.examples.unwrap();
        assert_eq!(v.logins, ["login.json"]);
        assert_eq!(v.application, None);
        assert_eq!(v.actions["besluit"], "besluit.json");
        let error =
            ProcessDefinition::parse("id: a\nactor: a\nexamples:\n  inlog: [login.json]\n", "t")
                .unwrap_err();
        assert!(error.iter().any(|f| f.contains("inlog")), "{error:?}");
    }

    #[test]
    fn directory_without_cells() {
        let dir = tempfile::tempdir().unwrap();
        assert!(cell_dirs(dir.path())
            .unwrap_err()
            .contains("no subdirectory"));
        // Without processes: no error.
        assert!(process_dirs(dir.path()).unwrap().is_empty());
    }
}
