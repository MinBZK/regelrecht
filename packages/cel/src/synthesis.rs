//! Synthesis: a process combines the lexostatus of the case (for the
//! assessment: the trial reduction of the draft) with lexostatuses of cells.
//!
//! Synthesis happens at the consumer, not at the source. The source reduces
//! its own chronicle; the process requests that lexostatus through a
//! [`Transport`], with a time limit of three seconds, and takes over only the
//! parameters it explicitly expects from that source in `synthesis.yaml`. For
//! each parameter it keeps track of where it came from. None of this is
//! recorded: it is informing, not a fact. If a source is unreachable, nothing
//! is filled in.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::config::{RowSource, SourceInput, SynthesisSource};
use crate::process::Process;
use crate::reduction::{AsOf, Lexostatus, ReductionRoute};
use crate::regulations;
use crate::transport::{fetch, Transport, TransportError};
use regelrecht_engine::LawExecutionService;

/// How long a source may take to respond.
pub const TIME_LIMIT: Duration = Duration::from_secs(3);

/// A source with the transport the runtime chose for it: a synthesis source
/// ([`SynthesisSource`]) or a source that is queried per row ([`RowSource`],
/// see [`crate::rows`]).
#[derive(Clone)]
pub struct Source<D = SynthesisSource> {
    pub definition: D,
    pub transport: Arc<dyn Transport>,
}

impl<D: Clone> Source<D> {
    /// The same source, along another transport to the same cell (such as
    /// one that remembers responses, see [`crate::transport::Remember`]).
    pub fn along(&self, transport: impl FnOnce(Arc<dyn Transport>) -> Arc<dyn Transport>) -> Self {
        Self {
            definition: self.definition.clone(),
            transport: transport(self.transport.clone()),
        }
    }
}

/// Which lexostatus of which cell a source is.
pub trait SourceReference {
    fn cell(&self) -> &str;
    fn lexostatus(&self) -> &str;
}

impl SourceReference for SynthesisSource {
    fn cell(&self) -> &str {
        &self.cell
    }
    fn lexostatus(&self) -> &str {
        &self.lexostatus
    }
}

impl SourceReference for RowSource {
    fn cell(&self) -> &str {
        &self.cell
    }
    fn lexostatus(&self) -> &str {
        &self.lexostatus
    }
}

impl<D: SourceReference> Source<D> {
    /// Requests the lexostatus of the source with this input, within the
    /// time limit. A response that is not a lexostatus is an error, not an
    /// empty lexostatus.
    pub async fn request(
        &self,
        input: &Map<String, Value>,
        as_of: &AsOf,
    ) -> Result<Lexostatus, TransportError> {
        let d = &self.definition;
        let v = fetch(
            self.transport.as_ref(),
            &path(d.cell(), d.lexostatus(), input, as_of),
            TIME_LIMIT,
        )
        .await?;
        serde_json::from_value(v)
            .map_err(|e| TransportError::Json(format!("not a lexostatus: {e}")))
    }
}

/// A synthesis source that is not a cell but the consumer's own policy
/// (note on source and gram id): the process queries it like any source, and
/// it responds with an engine run of the article instead of a reduction.
/// That way the chain "KvK number, then the name, then the designation" lives
/// in the consumer's policy (with an ordinary `source` to the policy of the
/// keeper of each register) and not in the deployment. The outputs are in the
/// lexostatus as extra fields: input for a later source.
pub struct PolicySource {
    service: Arc<LawExecutionService>,
    regulation: String,
    outputs: Vec<String>,
    name: String,
    /// The runtime's clock, for the date of a query without `as_of`.
    clock: crate::api::Clock,
}

impl PolicySource {
    pub fn new(service: Arc<LawExecutionService>, d: &SynthesisSource) -> Self {
        Self {
            service,
            regulation: d.regulation.clone().unwrap_or_default(),
            outputs: d.extra_fields.clone(),
            name: d.lexostatus.clone(),
            clock: Arc::new(|| chrono::Local::now().fixed_offset()),
        }
    }

    /// Read the date of a query without `as_of` from this clock (the
    /// runtime's), instead of the wall clock.
    pub fn with_clock(mut self, clock: crate::api::Clock) -> Self {
        self.clock = clock;
        self
    }

    /// A parameter from the query with the type the regulation gives it:
    /// the query knows only text (see [`path`]), the engine computes with a
    /// number, an amount or a boolean. A null input is not in the query at
    /// all, so the text "null" is that text.
    fn value(&self, name: &str, text: String) -> Value {
        use regelrecht_engine::ParameterType as T;
        let kind = self
            .service
            .resolver()
            .get_law(&self.regulation)
            .and_then(|law| {
                law.articles
                    .iter()
                    .filter_map(|a| a.get_execution_spec())
                    .flat_map(|e| e.parameters.iter().flatten())
                    .find(|p| p.name == name)
                    .map(|p| p.param_type)
            });
        match kind {
            Some(T::Number | T::Amount | T::Boolean | T::Array | T::Object) => {
                serde_json::from_str(&text).unwrap_or(Value::String(text))
            }
            _ => Value::String(text),
        }
    }

    fn response(&self, path: &str) -> Result<Value, TransportError> {
        let query = path.split_once('?').map_or("", |(_, q)| q);
        let pairs: Vec<(String, String)> = serde_urlencoded::from_str(query)
            .map_err(|e| TransportError::Json(format!("the query cannot be read: {e}")))?;
        let mut parameters: BTreeMap<String, Value> = BTreeMap::new();
        let mut date = crate::date::reference_date(&(self.clock)());
        for (k, v) in pairs {
            match k.as_str() {
                // A date or a moment: the engine reads the regulation on that day.
                "as_of" => {
                    date = match crate::date::date_of(&v) {
                        Some(d) => d.format("%Y-%m-%d").to_string(),
                        None => crate::date::reference_date_of(&v).map_err(TransportError::Json)?,
                    }
                }
                "known_at" => {}
                _ => {
                    let w = self.value(&k, v);
                    parameters.insert(k, w);
                }
            }
        }
        let outputs: Vec<&str> = self.outputs.iter().map(String::as_str).collect();
        let e = crate::assessment::evaluate(
            &self.service,
            &self.regulation,
            &outputs,
            &parameters,
            &date,
        );
        if let Some(f) = &e.error {
            return Err(TransportError::Response {
                status: 400,
                error: format!("{}: {f}", self.name),
            });
        }
        // An output without a value drops out (the synthesis then lacks that
        // field), but not silently.
        if !e.missing.is_empty() {
            tracing::warn!(source = %self.name, missing = %e.missing.join(", "), "policy source: outputs without a value");
        }
        let l = Lexostatus {
            extra_fields: e.values.into_iter().filter(|(_, v)| !v.is_null()).collect(),
            ..Lexostatus::empty(&self.name)
        };
        serde_json::to_value(l).map_err(|e| TransportError::Json(e.to_string()))
    }
}

impl Transport for PolicySource {
    fn kind(&self) -> &'static str {
        "policy"
    }
    fn fetch<'a>(&'a self, path: &'a str) -> crate::transport::Response<'a> {
        let out = self.response(path);
        Box::pin(async move { out })
    }
    fn send<'a>(&'a self, path: &'a str, _body: &'a Value) -> crate::transport::Response<'a> {
        Box::pin(async move {
            Err(TransportError::Json(format!(
                "{path}: a policy source records nothing"
            )))
        })
    }
}

/// Where a parameter came from.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "source", rename_all = "snake_case")]
pub enum Provenance {
    /// A lexostatus of the case, from the cell the process records in (for
    /// the assessment: the trial reduction of the draft).
    Own { lexostatus: String },
    /// A lexostatus of another cell.
    Cell {
        cell: String,
        lexostatus: String,
        transport: String,
    },
    /// Composed per row from a table field of an own lexostatus and the
    /// sources queried per row (see [`crate::rows`]).
    PerRow { lexostatus: String, field: String },
    /// The decision form: a verdict of the handler.
    Handler,
    /// The state at decision: a fact the procedure only asks for in a later
    /// stage (RFC-008), and that has not yet happened at the decision.
    StateAtDecision { stage: String },
    /// The window the applicant chose in the portal.
    Choice,
    /// The id of the decision the action acts on (the action's
    /// `decision_parameter`; note on source and gram id).
    Decision,
}

/// How the query to a source went.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Queried,
    /// No connection or no response within the time limit.
    Unreachable,
    /// A response, but not a lexostatus.
    Error,
    /// Not queried: an input was absent from the own lexostatus.
    NotQueried,
}

/// The result per source.
#[derive(Debug, Clone, Serialize)]
pub struct SourceResult {
    pub cell: String,
    pub lexostatus: String,
    pub transport: &'static str,
    pub status: Status,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub input: Map<String, Value>,
    /// The expected parameters the source did not deliver.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub not_delivered: Vec<String>,
    /// The extra fields this source passed on to a later source.
    #[serde(skip_serializing_if = "Map::is_empty")]
    pub extra_fields: Map<String, Value>,
    /// Along which route the source reduced, if it says so (a runtime with
    /// the engine route, experiment A).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reduction: Option<ReductionRoute>,
}

/// The combined parameters, with their provenance.
#[derive(Debug, Clone, Serialize)]
pub struct Combination {
    pub parameters: BTreeMap<String, Value>,
    pub provenance: BTreeMap<String, Provenance>,
    pub sources: Vec<SourceResult>,
}

impl Combination {
    /// Why the assessment cannot be judged when a source delivered nothing,
    /// in words; `None` if every source responded.
    pub fn reason(&self) -> Option<String> {
        self.sources
            .iter()
            .find_map(SourceResult::failure)
            .map(|r| format!("cannot be judged: {r}"))
    }

    /// Why a source that should deliver one of `missing` delivered nothing,
    /// without a prefix; `None` if no such source failed. A failed source
    /// whose parameters are all there is not the reason.
    pub fn reason_for(&self, sources: &[Source], missing: &[String]) -> Option<String> {
        self.sources
            .iter()
            .filter(|b| {
                sources.iter().any(|s| {
                    s.definition.cell == b.cell
                        && s.definition.lexostatus == b.lexostatus
                        && s.definition.parameters.iter().any(|p| missing.contains(p))
                })
            })
            .find_map(SourceResult::failure)
    }
}

impl SourceResult {
    /// What went wrong with this source, in words; `None` if it was queried.
    fn failure(&self) -> Option<String> {
        let error = self.error.as_deref().unwrap_or_default();
        match self.status {
            Status::Queried => None,
            Status::Unreachable => Some(format!("source {} unreachable", self.cell)),
            Status::Error => Some(format!(
                "source {} returned no lexostatus ({error})",
                self.cell
            )),
            Status::NotQueried => Some(format!("source {} not queried ({error})", self.cell)),
        }
    }
}

/// What a cell answers (with a 404) when a lexostatus picks a gram and the
/// chronicle has none for the query: the lexostatus is then empty, which is
/// not an error. Any other 404 is.
pub const NO_GRAM: &str = "no gram for this query";

/// The names of the query that are not an input: the as-of (see [`AsOf`]).
/// An input with such a name would be overwritten.
pub const RESERVED_INPUTS: [&str; 2] = ["as_of", "known_at"];

/// The path of a lexostatus on a runtime, with the input and the as-of (see
/// [`AsOf`]) as query. A null input is left out: in a query it would be
/// indistinguishable from the text "null".
pub fn path(cell: &str, lexostatus: &str, input: &Map<String, Value>, as_of: &AsOf) -> String {
    let mut pairs: BTreeMap<&str, String> = input
        .iter()
        .filter(|(_, v)| !v.is_null())
        .map(|(k, v)| {
            let text = match v {
                Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            (k.as_str(), text)
        })
        .collect();
    pairs.extend(as_of.query());
    let query = serde_urlencoded::to_string(&pairs).unwrap_or_default();
    // A lexostatus from the law is named after its article (`<regulation>#<article>`).
    let lexostatus = crate::cell_client::url_segment(lexostatus);
    if query.is_empty() {
        format!("/cells/{cell}/api/lexostatus/{lexostatus}")
    } else {
        format!("/cells/{cell}/api/lexostatus/{lexostatus}?{query}")
    }
}

/// The input for a source: from the own lexostatus (a parameter or an extra
/// field), from the extra fields an earlier source passed on (`earlier`:
/// lexostatus of that source to its extra fields), or a fixed value. An
/// error names what is absent.
fn input(
    source: &SynthesisSource,
    own: &Lexostatus,
    earlier: &BTreeMap<String, Map<String, Value>>,
) -> Result<Map<String, Value>, String> {
    let mut out = Map::new();
    for (name, i) in &source.input {
        let v = match i {
            SourceInput::Value { value } => {
                out.insert(name.clone(), value.clone());
                continue;
            }
            SourceInput::Field(v) => v,
        };
        let value = if v.lexostatus == own.name {
            own.field(&v.field)
        } else {
            earlier
                .get(&v.lexostatus)
                .and_then(|m| m.get(&v.field))
                .filter(|w| !w.is_null())
        };
        match value {
            Some(w) => {
                out.insert(name.clone(), w.clone());
            }
            None => {
                return Err(format!(
                    "input '{name}' is absent: {}.{} has no value",
                    v.lexostatus, v.field
                ))
            }
        }
    }
    Ok(out)
}

/// The lexostatuses of earlier sources a source waits for: every input from
/// a field that does not come from the own lexostatus.
fn wait_at<'b>(source: &'b SynthesisSource, own: &Lexostatus) -> Vec<&'b str> {
    source
        .input
        .values()
        .filter_map(SourceInput::field)
        .filter(|v| v.lexostatus != own.name)
        .map(|v| v.lexostatus.as_str())
        .collect()
}

/// Combines the own lexostatus with those of the sources, in rounds. Each
/// round queries at once the sources whose every input is already there:
/// from the own lexostatus, or an extra field of a source from an earlier
/// round (for example a name for a registration number, and then the
/// designation for that name). A source nobody can wait for any longer goes
/// in the last round and reports what is absent.
///
/// Every source reduces at `as_of`: the moment at which the process asks for
/// the state (the reference date of a decision, the start of a window).
pub async fn combine(own: &Lexostatus, sources: &[Source], as_of: &AsOf) -> Combination {
    let mut earlier: BTreeMap<String, Map<String, Value>> = BTreeMap::new();
    let mut done: BTreeSet<String> = BTreeSet::new();
    let mut open: Vec<&Source> = sources.iter().collect();
    let mut s = Combination {
        parameters: own.parameters.clone(),
        provenance: own
            .parameters
            .keys()
            .map(|k| {
                (
                    k.clone(),
                    Provenance::Own {
                        lexostatus: own.name.clone(),
                    },
                )
            })
            .collect(),
        sources: Vec::new(),
    };
    while !open.is_empty() {
        let (ready, wait): (Vec<&Source>, Vec<&Source>) = open.into_iter().partition(|b| {
            wait_at(&b.definition, own)
                .iter()
                .all(|l| done.contains(*l))
        });
        // Nobody is ready: what still waits, waits for a source that does
        // not exist or passed on nothing. Query them now; the input reports
        // what is absent.
        let (round, rest) = if ready.is_empty() {
            (wait, Vec::new())
        } else {
            (ready, wait)
        };
        let t = request(own, &round, &earlier, as_of).await;
        for (result, source) in t.sources.iter().zip(&round) {
            done.insert(source.definition.lexostatus.clone());
            if !source.definition.extra_fields.is_empty() {
                earlier.insert(result.lexostatus.clone(), result.extra_fields.clone());
            }
        }
        for (k, v) in t.parameters {
            if !own.parameters.contains_key(&k) {
                s.parameters.insert(k, v);
            }
        }
        for (k, h) in t.provenance {
            if !own.parameters.contains_key(&k) {
                s.provenance.insert(k, h);
            }
        }
        s.sources.extend(t.sources);
        open = rest;
    }
    s
}

/// Queries these sources at once and combines their parameters with the own
/// lexostatus.
async fn request(
    own: &Lexostatus,
    sources: &[&Source],
    earlier: &BTreeMap<String, Map<String, Value>>,
    as_of: &AsOf,
) -> Combination {
    let mut parameters = BTreeMap::new();
    let mut provenance: BTreeMap<String, Provenance> = BTreeMap::new();

    let ask = sources.iter().map(|source| async move {
        let d = &source.definition;
        let mut result = SourceResult {
            cell: d.cell.clone(),
            lexostatus: d.lexostatus.clone(),
            transport: source.transport.kind(),
            status: Status::NotQueried,
            error: None,
            input: Map::new(),
            not_delivered: Vec::new(),
            extra_fields: Map::new(),
            reduction: None,
        };
        let input = match input(d, own, earlier) {
            Ok(i) => i,
            Err(f) => {
                result.error = Some(f);
                return (result, None);
            }
        };
        let response = source.request(&input, as_of).await;
        result.input = input;
        match response {
            Ok(l) => {
                result.status = Status::Queried;
                result.reduction = l.reduction.clone();
                for field in &d.extra_fields {
                    if let Some(w) = l.extra_fields.get(field) {
                        result.extra_fields.insert(field.clone(), w.clone());
                    }
                }
                (result, Some(delivered(l)))
            }
            Err(TransportError::Unreachable(r)) => {
                result.status = Status::Unreachable;
                result.error = Some(r);
                (result, None)
            }
            Err(f @ (TransportError::Response { .. } | TransportError::Json(_))) => {
                result.status = Status::Error;
                result.error = Some(f.to_string());
                (result, None)
            }
        }
    });
    let responses = futures_util::future::join_all(ask).await;

    let mut results = Vec::new();
    for ((mut result, delivered), source) in responses.into_iter().zip(sources.iter()) {
        if let Some(delivered) = delivered {
            // The source delivers under its own name; the consumer asks for
            // it under its own.
            for (at_source, p) in source.definition.parameters.pairs() {
                match delivered.get(at_source) {
                    Some(w) => {
                        parameters.insert(p.to_string(), w.clone());
                        provenance.insert(
                            p.to_string(),
                            Provenance::Cell {
                                cell: result.cell.clone(),
                                lexostatus: result.lexostatus.clone(),
                                transport: result.transport.to_string(),
                            },
                        );
                    }
                    None => result.not_delivered.push(at_source.to_string()),
                }
            }
        }
        results.push(result);
    }
    Combination {
        parameters,
        provenance,
        sources: results,
    }
}

/// What a source delivered: what the consumer asks for as a parameter, the
/// source may deliver as a parameter or as an extra field (the consumer's
/// law says which facts are parameters, not the source). A parameter wins
/// over an extra field of the same name. Shared with the per-row synthesis
/// ([`crate::rows`]).
pub(crate) fn delivered(l: Lexostatus) -> BTreeMap<String, Value> {
    let mut out = l.parameters;
    for (k, v) in l.extra_fields {
        out.entry(k).or_insert(v);
    }
    out
}

/// The checks on sources that pass on an extra field, with and without a
/// portal:
///
/// - a source delivers a parameter or passes on an extra field;
/// - a passing source is named differently from every own lexostatus and
///   from every other passing source, so an input can point at only one
///   thing;
/// - an input from a passing source comes from an earlier source in the list
///   that passes on that field (which may itself wait for an earlier source:
///   the synthesis queries in rounds).
fn pass_on(process: &Process) -> Vec<String> {
    let mut errors = Vec::new();
    let sources: Vec<&SynthesisSource> = process.definition.other_sources().collect();
    let own: Vec<&str> = process
        .cell
        .lexostatuses
        .lexostatus_definitions
        .iter()
        .map(|d| d.name.as_str())
        .collect();
    for (i, source) in sources.iter().enumerate() {
        let who = format!("synthesis source {}/{}", source.cell, source.lexostatus);
        if source.parameters.is_empty() && source.extra_fields.is_empty() {
            errors.push(format!(
                "{who}: delivers no parameter and passes on no extra field"
            ));
        }
        if !source.extra_fields.is_empty() {
            if own.contains(&source.lexostatus.as_str()) {
                errors.push(format!(
                    "{who}: passes on extra fields, but has the name of an own lexostatus; an input could then point at two things"
                ));
            }
            if sources[..i]
                .iter()
                .any(|b| !b.extra_fields.is_empty() && b.lexostatus == source.lexostatus)
            {
                errors.push(format!(
                    "{who}: another source that passes on extra fields has the same name"
                ));
            }
        }
        for (name, v) in source
            .input
            .iter()
            .filter_map(|(n, i)| Some((n, i.field()?)))
        {
            let passer = sources
                .iter()
                .enumerate()
                .find(|(_, b)| !b.extra_fields.is_empty() && b.lexostatus == v.lexostatus);
            let Some((j, e)) = passer else { continue };
            if j >= i {
                errors.push(format!(
                    "{who}, input '{name}': source {}/{} does not come earlier in the list",
                    e.cell, e.lexostatus
                ));
            } else if !e.extra_fields.contains(&v.field) {
                errors.push(format!(
                    "{who}, input '{name}': source {}/{} does not pass on extra field '{}'",
                    e.cell, e.lexostatus, v.field
                ));
            }
        }
    }
    errors
}

/// The legal basis of the translations in the synthesis, at startup: every
/// legal basis of a synthesis source or of a per-row source points at a
/// loaded article, with the paragraph it names, and every source that
/// translates (a name at the consumer that differs from
/// the one at the source, or a fixed value in the input) carries a legal
/// basis: the translation is a reading of the law, just like a derivation in
/// a cell.
pub fn legal_bases(
    d: &crate::config::ProcessDefinition,
    service: &regelrecht_engine::LawExecutionService,
) -> Vec<String> {
    let synthesis = d.other_sources().map(|b| {
        (
            format!("synthesis source {}/{}", b.cell, b.lexostatus),
            &b.legal_basis,
            b.translates(),
        )
    });
    let rows = d
        .portal
        .iter()
        .flat_map(|p| {
            p.assessment
                .rows
                .iter()
                .map(|r| ("assessment".to_string(), r))
        })
        .chain(d.handling.iter().flat_map(|b| &b.actions).flat_map(|h| {
            h.rows
                .iter()
                .map(move |r| (format!("action '{}'", h.name), r))
        }))
        .flat_map(|(where_, r)| {
            r.sources.iter().map(move |b| {
                (
                    format!(
                        "{where_}, rows '{}', source {}/{}",
                        r.parameter, b.cell, b.lexostatus
                    ),
                    &b.legal_basis,
                    b.translates(),
                )
            })
        });
    let mut errors = BTreeSet::new();
    for (who, legal_basis, translates) in synthesis.chain(rows) {
        for g in legal_basis {
            if let Err(f) = regulations::valid(service, g) {
                errors.insert(format!("{who}: {f}"));
            }
        }
        if legal_basis.is_empty() && !translates.is_empty() {
            errors.insert(format!(
                "{who}: translates ({}) without legal basis; the consumer says which article a translation rests on",
                translates.join("; ")
            ));
        }
    }
    errors.into_iter().collect()
}

/// The checks on the synthesis of a process at startup. An error here stops
/// the runtime:
///
/// - synthesis requires a portal or a decision, because only the assessment
///   and the trial decision use it;
/// - a source is a cell other than the one the process records in, unless it
///   is a source of the case (`case: true`);
/// - every input comes from a field of the assessment lexostatus (with a
///   portal), or from an earlier source that passes it on (see [`pass_on`]);
/// - every parameter is a parameter of the article of the assessment, of the
///   decision or of the offer (`portal.offer`), or of an article that
///   transitively calls one of those;
/// - a parameter comes from only one source: the own reduction or a source
///   (with and without a portal);
/// - no input is named after a reserved query name ([`RESERVED_INPUTS`]).
///
/// What the decision requires further is in [`crate::action::check`].
pub fn check(process: &Process) -> Vec<String> {
    let mut errors = Vec::new();
    if process.definition.synthesis.is_empty() {
        return errors;
    }
    let sources: Vec<&SynthesisSource> = process.definition.other_sources().collect();
    let cell = &process.cell;
    let service = process.service.as_ref();
    errors.extend(pass_on(process));
    for source in &sources {
        for name in source
            .input
            .keys()
            .filter(|n| RESERVED_INPUTS.contains(&n.as_str()))
        {
            errors.push(format!(
                "synthesis source {}/{}, input '{name}': a reserved name of the query (the as-of); name the input differently",
                source.cell, source.lexostatus
            ));
        }
    }
    let passers: Vec<&str> = sources
        .iter()
        .filter(|b| !b.extra_fields.is_empty())
        .map(|b| b.lexostatus.as_str())
        .collect();
    // What the actions require: the parameters of their articles, together.
    let actions = process.actions();
    let under_decision: BTreeSet<String> = actions
        .iter()
        .filter_map(|h| {
            let a = regulations::article(service, &h.article).ok()?;
            Some(regulations::transitive_parameters(
                service,
                &h.regulation,
                a,
            ))
        })
        .flatten()
        .collect();
    let Some(portal) = process.portal() else {
        if actions.is_empty() {
            errors.push(
                "synthesis without portal and without actions: only the assessment of a portal and the actions in a case use it"
                    .into(),
            );
            return errors;
        }
        for source in &sources {
            let who = format!("synthesis source {}/{}", source.cell, source.lexostatus);
            if source.cell == cell.id() {
                errors.push(format!(
                    "{who}: a source from the cell the process records in is a source of the case (case: true)"
                ));
            }
            for p in source
                .parameters
                .iter()
                .filter(|p| !under_decision.contains(*p))
            {
                errors.push(format!(
                    "{who}: '{p}' is not a parameter of an action or of an article it calls"
                ));
            }
        }
        let mut per: BTreeMap<&str, Vec<String>> = BTreeMap::new();
        for source in &sources {
            for p in &source.parameters {
                per.entry(p).or_default().push(format!(
                    "synthesis source {}/{}",
                    source.cell, source.lexostatus
                ));
            }
        }
        errors.extend(more_than_one_source(per));
        return errors;
    };
    let own = cell.lexostatuses.lexostatus(&portal.assessment.lexostatus);
    let under_assessment = service
        .resolver()
        .get_article_by_output(
            &portal.assessment.regulation,
            &portal.assessment.output,
            None,
        )
        .map(|a| regulations::transitive_parameters(service, &portal.assessment.regulation, a))
        .unwrap_or_default();
    // What the offer requires: the parameters of the offer's article.
    let under_offer = portal
        .offer
        .as_ref()
        .and_then(|a| {
            let art = service
                .resolver()
                .get_article_by_output(&a.regulation, &a.output, None)?;
            Some(regulations::transitive_parameters(
                service,
                &a.regulation,
                art,
            ))
        })
        .unwrap_or_default();
    // parameter -> sources that deliver it
    let mut per: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    if let Some(def) = own {
        for p in def.reduction.derivations.keys() {
            per.entry(p)
                .or_default()
                .push(format!("the own lexostatus '{}'", def.name));
        }
    }
    for source in &sources {
        let who = format!("synthesis source {}/{}", source.cell, source.lexostatus);
        if source.cell == cell.id() {
            errors.push(format!(
                "{who}: a source from the cell the process records in is a source of the case (case: true)"
            ));
        }
        for (name, v) in source
            .input
            .iter()
            .filter_map(|(n, i)| Some((n, i.field()?)))
        {
            if passers.contains(&v.lexostatus.as_str()) {
                // Passed on by an earlier source: see `pass_on`.
            } else if v.lexostatus != portal.assessment.lexostatus {
                errors.push(format!(
                    "{who}, input '{name}': comes from lexostatus '{}', but the assessment reduces '{}'",
                    v.lexostatus, portal.assessment.lexostatus
                ));
            } else if !own.is_some_and(|d| d.delivers(&v.field)) {
                errors.push(format!(
                    "{who}, input '{name}': lexostatus '{}' does not deliver '{}' (no derivation and no extra field)",
                    v.lexostatus, v.field
                ));
            }
        }
        for p in &source.parameters {
            if !under_assessment.contains(p)
                && !under_decision.contains(p)
                && !under_offer.contains(p)
            {
                errors.push(format!(
                    "{who}: '{p}' is not a parameter of {}#{} (the assessment, '{}'), an action or the offer, or of an article that calls one of those",
                    portal.assessment.regulation,
                    service
                        .resolver()
                        .get_article_by_output(&portal.assessment.regulation, &portal.assessment.output, None)
                        .map(|a| a.number.clone())
                        .unwrap_or_default(),
                    portal.assessment.output
                ));
            }
            per.entry(p).or_default().push(who.clone());
        }
    }
    errors.extend(more_than_one_source(per));
    errors
}

/// A parameter comes from only one source.
fn more_than_one_source(per: BTreeMap<&str, Vec<String>>) -> Vec<String> {
    per.into_iter()
        .filter(|(_, who)| who.len() > 1)
        .map(|(p, who)| {
            format!(
                "parameter '{p}' comes from more than one source: {}",
                who.join(", ")
            )
        })
        .collect()
}

/// What a runtime says about its cells (`GET /api/cells`), as far as the
/// check needs it.
fn lexostatus_of<'v>(cells: &'v Value, cell: &str, lexostatus: &str) -> Result<&'v Value, String> {
    let c = cells
        .as_array()
        .and_then(|a| {
            a.iter()
                .find(|c| c.get("id").and_then(Value::as_str) == Some(cell))
        })
        .ok_or_else(|| format!("the source's runtime has no cell '{cell}'"))?;
    c.get("lexostatuses")
        .and_then(Value::as_array)
        .and_then(|a| {
            a.iter()
                .find(|l| l.get("name").and_then(Value::as_str) == Some(lexostatus))
        })
        .ok_or_else(|| format!("cell '{cell}' offers no lexostatus '{lexostatus}'"))
}

fn names(v: &Value, key: &str) -> BTreeSet<String> {
    v.get(key)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|x| {
            x.as_str()
                .or_else(|| x.get("name").and_then(Value::as_str))
                .map(str::to_string)
        })
        .collect()
}

/// The check on the sources themselves, at startup: is the source
/// reachable, does it offer the lexostatus with these parameters, and do the
/// inputs fit? Every problem is a warning, not a refusal: the source may come
/// later.
pub async fn warnings(process: &str, sources: &[Source]) -> Vec<String> {
    let mut out = Vec::new();
    // A policy source is the consumer's own regulation, computed by the
    // engine here: there is no runtime to ask, and a query without
    // parameters would only compute outputs without a value.
    for source in sources.iter().filter(|b| b.definition.regulation.is_none()) {
        let d = &source.definition;
        let who = format!(
            "process '{process}': synthesis source {}/{} ({})",
            d.cell,
            d.lexostatus,
            source.transport.kind()
        );
        let cells = match fetch(source.transport.as_ref(), "/api/cells", TIME_LIMIT).await {
            Ok(c) => c,
            Err(f) => {
                out.push(format!("{who}: cannot be checked now, {f}"));
                continue;
            }
        };
        let lexo = match lexostatus_of(&cells, &d.cell, &d.lexostatus) {
            Ok(l) => l,
            Err(f) => {
                out.push(format!("{who}: {f}"));
                continue;
            }
        };
        if lexo.get("list") == Some(&Value::Bool(true)) {
            out.push(format!(
                "{who}: the source is a list (group_by) and delivers no parameters"
            ));
        }
        let mut delivered = names(lexo, "parameters");
        delivered.extend(names(lexo, "extra_fields"));
        for (p, _) in d
            .parameters
            .pairs()
            .filter(|(p, _)| !delivered.contains(*p))
        {
            out.push(format!("{who}: the source delivers no parameter '{p}'"));
        }
        let inputs = names(lexo, "inputs");
        for i in inputs.iter().filter(|i| !d.input.contains_key(*i)) {
            out.push(format!(
                "{who}: the source requires input '{i}', and the synthesis does not give it"
            ));
        }
        for i in d.input.keys().filter(|i| !inputs.contains(*i)) {
            out.push(format!("{who}: input '{i}' is not an input of the source"));
        }
    }
    out
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::transport::trial::Fixed;
    use serde_json::json;

    fn source(response: Result<Value, TransportError>) -> (Source, Arc<Fixed>) {
        let t = Arc::new(Fixed::new(response));
        let definition: SynthesisSource = serde_json::from_value(json!({
            "cell": "register", "lexostatus": "status",
            "input": {"aanduiding": {"lexostatus": "eigen", "field": "aanduiding"}},
            "parameters": ["ingeschreven", "zetels"]
        }))
        .unwrap();
        (
            Source {
                definition,
                transport: t.clone(),
            },
            t,
        )
    }

    fn own(aanduiding: Option<&str>) -> Lexostatus {
        serde_json::from_value(json!({
            "name": "eigen",
            "parameters": {"bevat_aanduiding": aanduiding.is_some()},
            "extra_fields": aanduiding.map(|a| json!({"aanduiding": a})).unwrap_or(json!({})),
        }))
        .unwrap()
    }

    fn fixed(response: Value) -> Arc<Fixed> {
        Arc::new(Fixed::new(Ok(response)))
    }

    /// A source passes on an extra field to a later source: first a number
    /// to a name, then the name to a count. The passed-on field is not a
    /// parameter.
    #[tokio::test]
    async fn an_extra_field_goes_to_the_next_source() {
        let t1 = fixed(json!({"name": "bron", "parameters": {}, "extra_fields": {"naam": "EEN"}}));
        let t2 = fixed(json!({"name": "bron", "parameters": {"aantal": 3}}));
        let b1 = Source {
            definition: serde_json::from_value(json!({
                "cell": "register", "lexostatus": "op_nummer",
                "input": {"nummer": {"lexostatus": "eigen", "field": "nummer"}},
                "parameters": [], "extra_fields": ["naam"]
            }))
            .unwrap(),
            transport: t1.clone(),
        };
        let b2 = Source {
            definition: serde_json::from_value(json!({
                "cell": "register", "lexostatus": "op_naam",
                "input": {"naam": {"lexostatus": "op_nummer", "field": "naam"}},
                "parameters": ["aantal"]
            }))
            .unwrap(),
            transport: t2.clone(),
        };
        let own: Lexostatus = serde_json::from_value(json!({
            "name": "eigen", "parameters": {}, "extra_fields": {"nummer": "12345678"}
        }))
        .unwrap();
        // The waiting source goes in the second round; the startup check
        // requires the passing source to come earlier in the list.
        let s = combine(&own, &[b1, b2], &AsOf::default()).await;
        assert_eq!(
            t2.ask()[0],
            "/cells/register/api/lexostatus/op_naam?naam=EEN"
        );
        assert_eq!(s.parameters.get("aantal"), Some(&json!(3)));
        assert!(!s.parameters.contains_key("naam"));
        assert_eq!(s.sources.len(), 2);
    }

    /// A chain of three: a number to a name, the name to a designation, the
    /// designation to a fact. The synthesis queries in rounds; a source waits
    /// until its input is there. A fixed value goes along, and the consumer
    /// asks for the fact under its own name.
    #[tokio::test]
    async fn a_chain_of_sources_in_rounds_with_translation() {
        let t1 = fixed(json!({"name": "x", "parameters": {}, "extra_fields": {"naam": "EEN"}}));
        let t2 =
            fixed(json!({"name": "x", "parameters": {}, "extra_fields": {"aanduiding": "LIJST"}}));
        let t3 = fixed(
            json!({"name": "x", "parameters": {"is_ingeschreven_in_register": true},
                              "extra_fields": {"zetels_toegekend": 4}}),
        );
        let source = |t: &Arc<Fixed>, d: Value| Source {
            definition: serde_json::from_value(d).unwrap(),
            transport: t.clone(),
        };
        // In reverse order: the rounds follow from the input.
        let sources = [
            source(
                &t3,
                json!({
                    "cell": "register", "lexostatus": "register",
                    "input": {"aanduiding": {"lexostatus": "op_naam", "field": "aanduiding"}, "orgaan": {"value": "raad"}},
                    "parameters": {"is_ingeschreven_in_register": "is_ingeschreven_raad", "zetels_toegekend": "zetels_op_lijst"}
                }),
            ),
            source(
                &t2,
                json!({
                    "cell": "register", "lexostatus": "op_naam",
                    "input": {"naam": {"lexostatus": "op_nummer", "field": "naam"}},
                    "parameters": [], "extra_fields": ["aanduiding"]
                }),
            ),
            source(
                &t1,
                json!({
                    "cell": "handelsregister", "lexostatus": "op_nummer",
                    "input": {"nummer": {"lexostatus": "eigen", "field": "nummer"}},
                    "parameters": [], "extra_fields": ["naam"]
                }),
            ),
        ];
        let own: Lexostatus = serde_json::from_value(json!({
            "name": "eigen", "parameters": {}, "extra_fields": {"nummer": "12345678"}
        }))
        .unwrap();
        let s = combine(&own, &sources, &AsOf::default()).await;
        assert_eq!(
            t3.ask()[0],
            "/cells/register/api/lexostatus/register?aanduiding=LIJST&orgaan=raad"
        );
        assert_eq!(s.parameters.get("is_ingeschreven_raad"), Some(&json!(true)));
        assert!(!s.parameters.contains_key("is_ingeschreven_in_register"));
        // An extra field of the source is a parameter for the consumer if
        // its law asks for it.
        assert_eq!(s.parameters.get("zetels_op_lijst"), Some(&json!(4)));
        assert_eq!(
            s.provenance["is_ingeschreven_raad"],
            Provenance::Cell {
                cell: "register".into(),
                lexostatus: "register".into(),
                transport: t3.kind().into()
            }
        );
        let order: Vec<&str> = s.sources.iter().map(|b| b.lexostatus.as_str()).collect();
        assert_eq!(order, ["op_nummer", "op_naam", "register"]);
    }

    /// A source that waits for a source that does not exist is still
    /// queried, and reports what is absent; nothing is filled in.
    #[tokio::test]
    async fn a_source_that_can_wait_for_nothing() {
        let t = fixed(json!({"name": "x", "parameters": {"a": 1}}));
        let b = Source {
            definition: serde_json::from_value(json!({
                "cell": "register", "lexostatus": "l",
                "input": {"naam": {"lexostatus": "bestaat_niet", "field": "naam"}},
                "parameters": ["a"]
            }))
            .unwrap(),
            transport: t.clone(),
        };
        let own = Lexostatus::empty("eigen");
        let s = combine(&own, &[b], &AsOf::default()).await;
        assert!(t.ask().is_empty());
        assert_eq!(s.sources[0].status, Status::NotQueried);
        assert!(s.sources[0]
            .error
            .as_deref()
            .unwrap()
            .contains("input 'naam' is absent"));
        assert!(!s.parameters.contains_key("a"));
    }

    /// If the first source passes on nothing, the next one is not queried,
    /// and nothing is filled in.
    #[tokio::test]
    async fn no_passed_on_field_no_query() {
        let t1 = fixed(json!({"name": "bron", "parameters": {}}));
        let t2 = fixed(json!({"name": "bron", "parameters": {"aantal": 3}}));
        let b1 = Source {
            definition: serde_json::from_value(json!({
                "cell": "register", "lexostatus": "op_nummer",
                "input": {"nummer": {"lexostatus": "eigen", "field": "nummer"}},
                "parameters": [], "extra_fields": ["naam"]
            }))
            .unwrap(),
            transport: t1,
        };
        let b2 = Source {
            definition: serde_json::from_value(json!({
                "cell": "register", "lexostatus": "op_naam",
                "input": {"naam": {"lexostatus": "op_nummer", "field": "naam"}},
                "parameters": ["aantal"]
            }))
            .unwrap(),
            transport: t2.clone(),
        };
        let own: Lexostatus = serde_json::from_value(json!({
            "name": "eigen", "parameters": {}, "extra_fields": {"nummer": "12345678"}
        }))
        .unwrap();
        let s = combine(&own, &[b1, b2], &AsOf::default()).await;
        assert!(t2.ask().is_empty());
        assert!(!s.parameters.contains_key("aantal"));
        assert_eq!(s.sources[1].status, Status::NotQueried);
    }

    #[tokio::test]
    async fn combine_with_provenance() {
        let (b, t) = source(Ok(
            json!({"name": "bron", "parameters": {"ingeschreven": true, "zetels": 6, "anders": 1}}),
        ));
        let s = combine(&own(Some("EEN & ANDER")), &[b], &AsOf::default()).await;
        assert_eq!(
            t.ask()[0],
            "/cells/register/api/lexostatus/status?aanduiding=EEN+%26+ANDER"
        );
        assert_eq!(s.parameters["zetels"], json!(6));
        // Only the expected parameters, no wildcard.
        assert!(!s.parameters.contains_key("anders"));
        // The designation is an extra field and does not go to the engine.
        assert!(!s.parameters.contains_key("aanduiding"));
        assert_eq!(
            s.provenance["bevat_aanduiding"],
            Provenance::Own {
                lexostatus: "eigen".into()
            }
        );
        assert_eq!(
            serde_json::to_value(&s.provenance["ingeschreven"]).unwrap(),
            json!({"source": "cell", "cell": "register", "lexostatus": "status", "transport": "internal"})
        );
        assert_eq!(s.sources[0].status, Status::Queried);
        assert_eq!(s.reason(), None);
    }

    #[tokio::test]
    async fn unreachable_source_fills_in_nothing() {
        let (b, _) = source(Err(TransportError::Unreachable("weg".into())));
        let s = combine(&own(Some("X")), &[b], &AsOf::default()).await;
        assert_eq!(
            s.parameters.keys().collect::<Vec<_>>(),
            ["bevat_aanduiding"]
        );
        assert_eq!(s.sources[0].status, Status::Unreachable);
        assert_eq!(
            s.reason().as_deref(),
            Some("cannot be judged: source register unreachable")
        );
    }

    #[tokio::test]
    async fn absent_input_does_not_query_the_source() {
        let (b, t) = source(Ok(json!({"name": "bron", "parameters": {}})));
        let s = combine(&own(None), &[b], &AsOf::default()).await;
        assert!(t.ask().is_empty());
        assert_eq!(s.sources[0].status, Status::NotQueried);
        assert!(s.reason().unwrap().contains("input 'aanduiding' is absent"));
    }

    #[tokio::test]
    async fn undelivered_parameter_is_named() {
        let (b, _) = source(Ok(
            json!({"name": "bron", "parameters": {"ingeschreven": false}}),
        ));
        let s = combine(&own(Some("X")), &[b], &AsOf::default()).await;
        assert_eq!(s.sources[0].not_delivered, ["zetels"]);
        assert!(!s.parameters.contains_key("zetels"));
    }

    #[test]
    fn path_without_input() {
        let now = AsOf::default();
        assert_eq!(
            path("a", "b", &Map::new(), &now),
            "/cells/a/api/lexostatus/b"
        );
        let i = json!({"jaar": 2025}).as_object().unwrap().clone();
        assert_eq!(
            path("a", "b", &i, &now),
            "/cells/a/api/lexostatus/b?jaar=2025"
        );
    }

    /// A null input is not in the query: the text "null" is a text.
    #[test]
    fn a_null_input_is_left_out_of_the_query() {
        let i = json!({"jaar": null, "naam": "null"})
            .as_object()
            .unwrap()
            .clone();
        assert_eq!(
            path("a", "b", &i, &AsOf::default()),
            "/cells/a/api/lexostatus/b?naam=null"
        );
    }

    /// The startup check does not query a policy source: it is computed
    /// here, and a query without parameters only gives bogus warnings.
    #[tokio::test]
    async fn the_startup_check_skips_a_policy_source() {
        let t = fixed(json!([]));
        let b = Source {
            definition: serde_json::from_value(json!({
                "regulation": "beleid", "lexostatus": "beleid#1",
                "parameters": [], "extra_fields": ["x"]
            }))
            .unwrap(),
            transport: t.clone(),
        };
        assert!(warnings("p", &[b]).await.is_empty());
        assert!(t.ask().is_empty());
    }

    #[test]
    fn path_with_as_of() {
        let as_of = AsOf {
            as_of: Some(crate::date::TimePoint::read("p", "2027-01-01").unwrap()),
            known_at: Some(crate::date::TimePoint::read("b", "2026-09-25T10:00:00+02:00").unwrap()),
        };
        let i = json!({"jaar": 2025}).as_object().unwrap().clone();
        assert_eq!(
            path("a", "b", &i, &as_of),
            "/cells/a/api/lexostatus/b?as_of=2027-01-01&jaar=2025&known_at=2026-09-25T10%3A00%3A00%2B02%3A00"
        );
    }
}
