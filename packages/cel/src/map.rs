//! The map of a process (`GET /api/map`, spec "Opbouw"): which
//! configuration, events, lexostatuses and articles the runtime uses, and how
//! they hang together. Built from what the runtime loaded; the edges between
//! an event and the law come from the explanation of the event
//! ([`crate::law::Explanation`]), so the map and the "why" of the form show
//! the same chain. Every node names where it is written as a
//! [`SourceRef`], which the fragment routes resolve. Every configuration
//! file the process loaded has a node, and so has every cell of this runtime
//! the process queries, with its streams, events and lexostatuses: all that
//! the application rests on is on the map.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::Arc;

use chrono::NaiveDate;
use serde::Serialize;

use crate::cell::Cell;
use crate::code_ref::CodeRef;
use crate::config::RowsDefinition;
use crate::law::{SourceRef, StepKind};
use crate::process::Process;
use crate::reduction::{Filter, LexostatusDefinition};
use crate::register::RegisterLink;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeKind {
    Process,
    Channel,
    Role,
    Action,
    Cell,
    Stream,
    Event,
    Lexostatus,
    Register,
    SourceCell,
    Article,
    /// A configuration file: of the process (form, channels, synthesis,
    /// examples, registers), or of a cell: its initial state, and its
    /// lexostatuses file when that supplements a lexostatus the law reads.
    Config,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeKind {
    Channel,
    Role,
    Portal,
    Action,
    LegalBasis,
    Records,
    Establishes,
    Hook,
    Extends,
    Origin,
    DecidesOn,
    Prefill,
    Source,
    Reads,
    /// Two meanings: a lexostatus to the article that reads it, and a
    /// policy article to the article of law it executes (RFC-047).
    Executes,
    Synthesis,
    Rows,
    Register,
    /// The process to a configuration file it loaded.
    Configures,
    /// A cell to the file with the grams of its empty chronicle.
    InitialState,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Node {
    /// `<kind>:<name>`; an event `event:<stream>/<event>`, an article
    /// `article:<regulation>#<article>`.
    pub id: String,
    pub kind: NodeKind,
    pub label: String,
    /// Where it is written; the fragment routes resolve it.
    pub source: SourceRef,
    /// Only on an article: the regulation, on which the frontend collapses.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub regulation: Option<String>,
    /// Only on an article: its number in the regulation (for the map itself;
    /// the frontend reads the label).
    #[serde(skip)]
    pub article: Option<String>,
    /// Where the code decides what the node is, with knowledge that belongs
    /// in the law or the policy. A node the code makes on its own has an
    /// empty `source`: it is written nowhere else.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub code: Vec<CodeRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct Edge {
    pub from: String,
    pub to: String,
    pub kind: EdgeKind,
}

#[derive(Debug, Clone, Serialize)]
pub struct Map {
    pub process: String,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
}

/// What the map is built from.
pub struct MapInput<'a> {
    pub process: &'a Process,
    /// Which policy queries which register.
    pub register_links: &'a [RegisterLink],
    /// Every cell of the runtime, per id.
    pub cells: &'a BTreeMap<String, Arc<Cell>>,
    /// The configuration files of the process, under the key the fragment
    /// route knows them by.
    pub files: &'a [(&'static str, PathBuf)],
    /// The day whose version of each regulation the map reads (as the
    /// fragment route of an article does).
    pub date: NaiveDate,
}

/// Nodes and edges, deduplicated and in a stable order.
#[derive(Default)]
struct Builder {
    nodes: BTreeMap<String, Node>,
    edges: BTreeSet<Edge>,
}

impl Builder {
    /// A node (the first one with this id wins); returns its id.
    fn node(&mut self, id: String, kind: NodeKind, label: &str, source: SourceRef) -> String {
        self.nodes.entry(id.clone()).or_insert_with(|| Node {
            id: id.clone(),
            kind,
            label: label.to_string(),
            source,
            regulation: None,
            article: None,
            code: Vec::new(),
        });
        id
    }

    /// Mark a node as decided (also) by the code.
    fn code(&mut self, id: &str, refs: &[CodeRef]) {
        if let Some(n) = self.nodes.get_mut(id) {
            for r in refs {
                if !n.code.contains(r) {
                    n.code.push(*r);
                }
            }
        }
    }

    /// The node of an article, from `<regulation>#<article>` (a paragraph is
    /// dropped, as in the explanation); returns its id.
    fn article(&mut self, reference: &str) -> String {
        let source = SourceRef::law(reference);
        let reference = source.law.clone().unwrap_or_default();
        let (regulation, article) = reference.split_once('#').unwrap_or((&reference, ""));
        let id = format!("article:{reference}");
        self.nodes.entry(id.clone()).or_insert_with(|| Node {
            id: id.clone(),
            kind: NodeKind::Article,
            label: article.to_string(),
            source,
            regulation: Some(regulation.to_string()),
            article: Some(article.to_string()),
            code: Vec::new(),
        });
        id
    }

    fn edge(&mut self, from: &str, to: &str, kind: EdgeKind) {
        self.edges.insert(Edge {
            from: from.into(),
            to: to.into(),
            kind,
        });
    }

    fn finish(self, process: &str) -> Map {
        Map {
            process: process.to_string(),
            nodes: self.nodes.into_values().collect(),
            edges: self.edges.into_iter().collect(),
        }
    }
}

/// The map of a process.
pub fn build(input: &MapInput) -> Map {
    let p = input.process;
    let id = p.id();
    let d = &p.definition;
    let mut b = Builder::default();
    let proc = b.node(
        format!("process:{id}"),
        NodeKind::Process,
        id,
        SourceRef::law(&d.declared_by),
    );
    for name in d.channels.keys() {
        let c = b.node(
            format!("channel:{name}"),
            NodeKind::Channel,
            name,
            SourceRef::law(&d.channels[name].declared_by),
        );
        b.edge(&proc, &c, EdgeKind::Channel);
    }
    for (name, role) in &d.roles {
        let r = b.node(
            format!("role:{name}"),
            NodeKind::Role,
            name,
            SourceRef::law(
                d.channels
                    .get(&role.channel)
                    .map_or(&d.declared_by, |k| &k.declared_by),
            ),
        );
        b.edge(&proc, &r, EdgeKind::Role);
        b.edge(&r, &format!("channel:{}", role.channel), EdgeKind::Channel);
    }
    for m in &d.mandates {
        let a = b.article(&m.legal_basis);
        b.edge(&proc, &a, EdgeKind::LegalBasis);
    }
    config_part(&mut b, input, &proc);
    for cell in shown_cells(input) {
        cell_part(&mut b, input, cell);
        lexostatus_part(&mut b, cell);
    }
    if let Some(portal) = p.portal() {
        let event = event_id(p.cell.id(), &portal.stream, &portal.event);
        b.edge(&proc, &event, EdgeKind::Portal);
        b.code(&event, &[crate::law::APPLICATION_CONTENT_CODE]);
    }
    handling_part(&mut b, input, &proc);
    synthesis_part(&mut b, input, &proc);
    // Before the sources, so that an executed article gets its own.
    executes_part(&mut b, input);
    source_part(&mut b, input);
    register_part(&mut b, input);
    b.finish(id)
}

/// Stream and event ids carry their cell: two cells may name a stream the
/// same (the startup check makes them unique per cell only).
fn stream_id(cell: &str, stream: &str) -> String {
    format!("stream:{cell}/{stream}")
}

fn event_id(cell: &str, stream: &str, event: &str) -> String {
    format!("event:{cell}/{stream}/{event}")
}

/// The edge of a step of the chain of an event to its article; `None` for a
/// step that is not one. A submission is how an application is established
/// (RFC-046). The decision on it is an edge between articles (see
/// [`cell_part`]), not one from the event.
fn step_edge(kind: StepKind) -> Option<EdgeKind> {
    Some(match kind {
        StepKind::Submission | StepKind::Establishes => EdgeKind::Establishes,
        StepKind::Hook => EdgeKind::Hook,
        StepKind::Extends => EdgeKind::Extends,
        StepKind::Origin => EdgeKind::Origin,
        _ => return None,
    })
}

/// The article of a regulation with this output, in the version of `date`.
fn output_article(p: &Process, regulation: &str, output: &str, date: NaiveDate) -> Option<String> {
    p.service
        .resolver()
        .get_article_by_output(regulation, output, Some(date))
        .map(|a| format!("{regulation}#{}", a.number))
}

/// A configuration file of a cell as the fragment route names it:
/// `cells/<id>/<config>`, for the cell of the process as for any other.
fn cell_config(cell: &Cell, config: &str) -> String {
    format!("cells/{}/{config}", cell.id())
}

/// The cell of the process and the cells of this runtime it queries
/// (synthesis, rows, registers, worklist), in a stable order: the cells on
/// the map, whose files the fragment route serves. A cell reached along a
/// url runs elsewhere and is not here: it stays a source cell.
pub fn queried_cells<'a>(
    p: &'a Process,
    register_links: &[RegisterLink],
    cells: &'a BTreeMap<String, Arc<Cell>>,
) -> Vec<&'a Cell> {
    let d = &p.definition;
    let mut ids: BTreeSet<&str> = BTreeSet::new();
    ids.extend(
        d.synthesis
            .iter()
            .filter(|s| s.regulation.is_none() && s.url.is_none())
            .map(|s| s.cell.as_str()),
    );
    let rows = p.assessment_rows().iter().chain(
        d.handling
            .iter()
            .flat_map(|h| h.actions.iter().flat_map(|a| &a.rows)),
    );
    for r in rows {
        ids.extend(
            r.sources
                .iter()
                .filter(|s| s.url.is_none())
                .map(|s| s.cell.as_str()),
        );
    }
    let registers: Vec<&str> = register_links.iter().map(|l| l.cell.as_str()).collect();
    if let Some(h) = &d.handling {
        ids.extend([h.worklist.cell.as_str(), h.cases.cell.as_str()]);
    }
    ids.remove(p.cell.id());
    let mut out = vec![&*p.cell];
    for id in ids.into_iter().chain(registers) {
        if let Some(c) = cells.get(id).filter(|_| out.iter().all(|o| o.id() != id)) {
            out.push(c);
        }
    }
    out
}

fn shown_cells<'a>(input: &MapInput<'a>) -> Vec<&'a Cell> {
    queried_cells(input.process, input.register_links, input.cells)
}

/// The cell on the map with this id, unless it is reached along a url.
fn local<'a>(input: &MapInput<'a>, cell: &str, url: Option<&String>) -> Option<&'a Cell> {
    url.is_none()
        .then(|| shown_cells(input).into_iter().find(|c| c.id() == cell))
        .flatten()
}

/// The name of a file as the map shows it.
fn file_name(file: &std::path::Path) -> String {
    file.file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default()
}

/// A configuration file node, labelled `label`; returns its id.
fn config_node(b: &mut Builder, config: &str, label: &str) -> String {
    b.node(
        format!("config:{config}"),
        NodeKind::Config,
        label,
        SourceRef::file(config),
    )
}

/// The node of a file of a cell, labelled with the directory of the cell,
/// as every cell has a file of the same name; returns its id.
fn cell_file_node(b: &mut Builder, cell: &Cell, config: &str, file: &str) -> String {
    let label = format!(
        "{}/{}",
        file_name(&cell.dir),
        file_name(&cell.dir.join(file))
    );
    config_node(b, &cell_config(cell, config), &label)
}

/// The configuration files of the process: form, channels, synthesis,
/// examples and registers, each as a whole file.
fn config_part(b: &mut Builder, input: &MapInput, proc: &str) {
    for (key, file) in input.files {
        let c = config_node(b, key, &file_name(file));
        b.edge(proc, &c, EdgeKind::Configures);
    }
}

/// A cell, its streams and events, and per event the articles of its chain
/// ([`crate::law::Explanation`]), the decisions on it and the policies that
/// fill in a field beforehand; and the file of its initial state.
fn cell_part(b: &mut Builder, input: &MapInput, cell: &Cell) {
    let p = input.process;
    let c = b.node(
        format!("cell:{}", cell.id()),
        NodeKind::Cell,
        cell.id(),
        SourceRef::config(&cell_config(cell, "cell"), "id"),
    );
    if let Some(file) = &cell.definition.initial_state {
        let i = cell_file_node(b, cell, "initial_state", file);
        b.edge(&c, &i, EdgeKind::InitialState);
    }
    for stream in &cell.streams {
        let config = cell_config(cell, &format!("stream/{}", stream.id));
        let s = b.node(
            stream_id(cell.id(), &stream.id),
            NodeKind::Stream,
            &stream.id,
            SourceRef::config(&config, "events"),
        );
        b.edge(&c, &s, EdgeKind::Records);
        for event in &stream.events {
            let e = b.node(
                event_id(cell.id(), &stream.id, &event.name),
                NodeKind::Event,
                &event.name,
                SourceRef::config(&config, &event.name),
            );
            b.edge(&s, &e, EdgeKind::Records);
            for step in &event.explanation.event {
                let (Some(kind), Some(law)) = (step_edge(step.kind), &step.source.law) else {
                    continue;
                };
                let a = b.article(law);
                b.edge(&e, &a, kind);
            }
            // The first article that takes part is the one that establishes
            // the event (see `crate::law::establish`).
            let Some(establishing) = event.establishes.first().map(|a| b.article(a)) else {
                continue;
            };
            for decider in &event.decided_by {
                let d = b.article(decider);
                b.edge(&d, &establishing, EdgeKind::DecidesOn);
            }
            for prefill in event.prefill.values() {
                if let Some(policy) =
                    output_article(p, &prefill.regulation, &prefill.output, input.date)
                {
                    let policy = b.article(&policy);
                    b.edge(&establishing, &policy, EdgeKind::Prefill);
                }
            }
        }
    }
}

fn lexostatus_id(cell: &str, name: &str) -> String {
    format!("lexostatus:{cell}/{name}")
}

/// The events a lexostatus reads: those its reduction can select, through
/// the filter of the lexostatus and, per derivation over a collection, its
/// own filter (the same selection as the startup check,
/// [`crate::check::events_for`]).
fn read_events(def: &LexostatusDefinition, cell: &Cell) -> BTreeSet<String> {
    let filters: Vec<Option<&Filter>> = def.all_derivations().map(|(_, d)| d.filter()).collect();
    // A derivation on the chosen gram reads what the lexostatus selects.
    let chosen = filters.is_empty() || filters.iter().any(Option::is_none);
    let mut out = BTreeSet::new();
    for f in std::iter::once(None)
        .filter(|_| chosen)
        .chain(filters.into_iter().flatten().map(Some))
    {
        for (s, e) in crate::check::events_for(def, f, &cell.streams) {
            out.insert(event_id(cell.id(), &s.id, &e.name));
        }
    }
    out
}

/// The lexostatuses of a cell, the events they read and, for one the law
/// reads, the reading article.
fn lexostatus_part(b: &mut Builder, cell: &Cell) {
    for d in &cell.lexostatuses.lexostatus_definitions {
        // The runtime makes the worklist and the list of all cases itself
        // (RFC-047): no file defines them, so they point to the code.
        let code = match d.name.as_str() {
            crate::reduction::WORKLIST => vec![crate::reduction::worklist::WORKLIST_CODE],
            crate::reduction::CASES => vec![crate::reduction::worklist::CASES_CODE],
            _ => Vec::new(),
        };
        let source = match &d.law {
            Some(law) => SourceRef::law(&law.article),
            None if !code.is_empty() => SourceRef::default(),
            None => SourceRef::config(&cell_config(cell, "lexostatuses"), &d.name),
        };
        let l = b.node(
            lexostatus_id(cell.id(), &d.name),
            NodeKind::Lexostatus,
            &d.name,
            source,
        );
        if !code.is_empty() {
            b.code(&l, &code);
            b.code(&l, &[crate::stream::DECISION_CODE]);
        }
        for event in read_events(d, cell) {
            b.edge(&l, &event, EdgeKind::Reads);
        }
        if let Some(law) = &d.law {
            let a = b.article(&law.article);
            b.edge(&l, &a, EdgeKind::Executes);
            // What the cell adds to a lexostatus the law reads: extra fields
            // in its own file, which no other node opens.
            if cell
                .lexostatuses
                .law
                .iter()
                .any(|s| s.article == law.article)
            {
                let c = cell_file_node(b, cell, "lexostatuses", &cell.definition.lexostatuses);
                b.edge(&c, &l, EdgeKind::Extends);
            }
        }
    }
}

/// A cell the process queries that runs elsewhere (synthesis, rows): written
/// in the synthesis of the deployment, under the id of the process's cell
/// (RFC-047). A cell of this runtime is on the map with its lexostatuses.
fn source_cell(b: &mut Builder, input: &MapInput, cell: &str) -> String {
    let p = input.process;
    b.node(
        format!("source_cell:{cell}"),
        NodeKind::SourceCell,
        cell,
        SourceRef::config("synthesis", p.cell.id()),
    )
}

/// What a query of `cell` points to: its lexostatus if the cell is on the
/// map and defines it, the cell itself if it is on the map but the runtime
/// offers the lexostatus (such as the case state), otherwise the source cell.
fn queried(
    b: &mut Builder,
    input: &MapInput,
    cell: &str,
    lexostatus: &str,
    url: Option<&String>,
) -> String {
    match local(input, cell, url) {
        Some(c)
            if c.lexostatuses
                .lexostatus_definitions
                .iter()
                .any(|d| d.name == lexostatus) =>
        {
            lexostatus_id(cell, lexostatus)
        }
        Some(c) => format!("cell:{}", c.id()),
        None => source_cell(b, input, cell),
    }
}

/// The lexostatuses (or cells) a synthesis per row queries, from `from`.
fn rows_part(b: &mut Builder, input: &MapInput, from: &str, rows: &[RowsDefinition]) {
    for r in rows {
        for src in &r.sources {
            let target = queried(b, input, &src.cell, &src.lexostatus, src.url.as_ref());
            b.edge(from, &target, EdgeKind::Rows);
        }
    }
}

/// The worklist, the list of all cases and the actions of the handling:
/// the article each action executes, the hooks on its stage and the event
/// it records.
fn handling_part(b: &mut Builder, input: &MapInput, proc: &str) {
    let Some(h) = &input.process.definition.handling else {
        return;
    };
    for list in [&h.worklist, &h.cases] {
        b.edge(
            proc,
            &lexostatus_id(&list.cell, &list.lexostatus),
            EdgeKind::Synthesis,
        );
    }
    for action in &h.actions {
        let a = b.node(
            format!("action:{}", action.name),
            NodeKind::Action,
            action.label(),
            // The runtime derives the action from the stages of the events
            // (RFC-047): no file names it, so it points to the code.
            SourceRef::default(),
        );
        b.code(
            &a,
            &[
                crate::derive::handling::ACTIONS_CODE,
                crate::stream::DECISION_CODE,
            ],
        );
        b.edge(proc, &a, EdgeKind::Action);
        let article = b.article(&action.article);
        b.edge(&a, &article, EdgeKind::LegalBasis);
        for hook in &action.hooks {
            let hook = b.article(hook);
            b.edge(&a, &hook, EdgeKind::Hook);
        }
        b.edge(
            &a,
            &event_id(
                input.process.cell.id(),
                &action.record.stream,
                &action.record.event,
            ),
            EdgeKind::Records,
        );
        rows_part(b, input, &a, &action.rows);
    }
}

/// What the synthesis of the process combines: a lexostatus of its own
/// cell, a cell elsewhere, or the articles of its own policy; and the cells
/// of the rows of the assessment.
fn synthesis_part(b: &mut Builder, input: &MapInput, proc: &str) {
    let p = input.process;
    for s in &p.definition.synthesis {
        if let Some(regulation) = &s.regulation {
            // The own policy, computed by the engine: its outputs are the
            // extra fields; the articles that have them.
            for output in &s.extra_fields {
                if let Some(article) = output_article(p, regulation, output, input.date) {
                    let a = b.article(&article);
                    b.edge(proc, &a, EdgeKind::Synthesis);
                }
            }
            continue;
        }
        let target = queried(b, input, &s.cell, &s.lexostatus, s.url.as_ref());
        b.edge(proc, &target, EdgeKind::Synthesis);
    }
    rows_part(b, input, proc, p.assessment_rows());
}

/// The article nodes on the map so far: id, regulation and number.
fn article_nodes(b: &Builder) -> Vec<(String, String, String)> {
    b.nodes
        .values()
        .filter_map(|n| Some((n.id.clone(), n.regulation.clone()?, n.article.clone()?)))
        .collect()
}

/// What the articles on the map execute (RFC-047), in the version of the
/// map's date: an edge from the policy article to the executed article, one
/// level.
fn executes_part(b: &mut Builder, input: &MapInput) {
    for (node, regulation, number) in article_nodes(b) {
        for e in
            input
                .process
                .service
                .resolver()
                .executes_of_on(&regulation, &number, Some(input.date))
        {
            let target = b.article(&e.target);
            b.edge(&node, &target, EdgeKind::Executes);
        }
    }
}

/// One level of `source` between articles: an article on the map that takes
/// an input from the output of another regulation points to the article of
/// that output. The new articles get no `source` edges of their own, or the
/// map would pull in the whole chain.
fn source_part(b: &mut Builder, input: &MapInput) {
    let p = input.process;
    for (node, regulation, number) in article_nodes(b) {
        let Some(spec) = p
            .service
            .resolver()
            .get_law_for_date(&regulation, Some(input.date))
            .and_then(|l| l.find_article_by_number(&number))
            .and_then(|a| a.get_execution_spec())
        else {
            continue;
        };
        for i in spec.input.iter().flatten() {
            let Some(src) = &i.source else { continue };
            let (Some(reg), Some(out)) = (&src.regulation, &src.output) else {
                continue;
            };
            if let Some(target) = output_article(p, reg, out, input.date) {
                let target = b.article(&target);
                b.edge(&node, &target, EdgeKind::Source);
            }
        }
    }
}

/// The registers the policies on the map query: per register an edge from
/// every article on the map that asks its policy's register (an input
/// without a source), and one to who supplies it: the streams that record
/// into its chronicle if that cell is on the map, otherwise the cell
/// elsewhere.
fn register_part(b: &mut Builder, input: &MapInput) {
    let resolver = input.process.service.resolver();
    for link in input.register_links {
        let Some(law) = resolver.get_law_for_date(&link.policy, Some(input.date)) else {
            continue;
        };
        let articles: Vec<String> = b
            .nodes
            .values()
            .filter(|n| n.regulation.as_deref() == Some(link.policy.as_str()))
            .filter(|n| {
                n.article
                    .as_deref()
                    .and_then(|number| law.find_article_by_number(number))
                    .is_some_and(|a| crate::register::source_less_inputs(a).next().is_some())
            })
            .map(|n| n.id.clone())
            .collect();
        if articles.is_empty() {
            continue;
        }
        // The key of the binding file, which is also the anchor there.
        let key = format!("{}#{}", link.policy, link.name);
        let r = b.node(
            format!("register:{key}"),
            NodeKind::Register,
            &link.name,
            SourceRef::config("registers", &key),
        );
        for a in articles {
            b.edge(&a, &r, EdgeKind::Register);
        }
        if let Some(cell) = local(input, &link.cell, None) {
            for stream in cell
                .streams
                .iter()
                .filter(|s| s.chronicle == link.chronicle)
            {
                b.edge(&r, &stream_id(cell.id(), &stream.id), EdgeKind::Register);
            }
        } else {
            let c = b.node(
                format!("source_cell:{}", link.cell),
                NodeKind::SourceCell,
                &link.cell,
                SourceRef::config("registers", &key),
            );
            b.edge(&r, &c, EdgeKind::Register);
        }
    }
}
