//! The map of a process (`GET /api/map`, spec "Opbouw"): which
//! configuration, events, lexostatuses and articles the runtime uses, and how
//! they hang together. Built from what the runtime loaded; the edges between
//! an event and the law come from the explanation of the event
//! ([`crate::law::Explanation`]), so the map and the "why" of the form show
//! the same chain. Every node names where it is written as a
//! [`SourceRef`], which the fragment routes resolve.

use std::collections::{BTreeMap, BTreeSet};

use chrono::NaiveDate;
use serde::Serialize;

use crate::config::RowsDefinition;
use crate::law::{SourceRef, StepKind};
use crate::process::Process;
use crate::reduction::{Filter, LexostatusDefinition};
use crate::register::RegisterLink;
use crate::stream::Stream;

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
        });
        id
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

/// A key of `process.yaml`, as the source of a node.
fn in_process(anchor: &str) -> SourceRef {
    SourceRef::config("process", anchor)
}

/// Where a part of the process is written: the policy article for a process
/// from policy (RFC-047), otherwise the key in `process.yaml`.
fn written(policy: Option<&String>, anchor: &str) -> SourceRef {
    match policy {
        Some(article) => SourceRef::law(article),
        None => in_process(anchor),
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
        written(d.declared_by.as_ref(), "id"),
    );
    for name in d.channels.keys() {
        let c = b.node(
            format!("channel:{name}"),
            NodeKind::Channel,
            name,
            written(d.channels[name].declared_by.as_ref(), name),
        );
        b.edge(&proc, &c, EdgeKind::Channel);
    }
    for (name, role) in &d.roles {
        let r = b.node(
            format!("role:{name}"),
            NodeKind::Role,
            name,
            written(
                d.channels
                    .get(&role.channel)
                    .and_then(|k| k.declared_by.as_ref()),
                name,
            ),
        );
        b.edge(&proc, &r, EdgeKind::Role);
        b.edge(&r, &format!("channel:{}", role.channel), EdgeKind::Channel);
    }
    for m in &d.mandates {
        let a = b.article(&m.legal_basis);
        b.edge(&proc, &a, EdgeKind::LegalBasis);
    }
    cell_part(&mut b, input);
    lexostatus_part(&mut b, input);
    if let Some(portal) = p.portal() {
        b.edge(
            &proc,
            &event_id(&portal.stream, &portal.event),
            EdgeKind::Portal,
        );
    }
    handling_part(&mut b, input, &proc);
    synthesis_part(&mut b, input, &proc);
    // Before the sources, so that an executed article gets its own.
    executes_part(&mut b, input);
    source_part(&mut b, input);
    register_part(&mut b, input);
    b.finish(id)
}

fn event_id(stream: &str, event: &str) -> String {
    format!("event:{stream}/{event}")
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

/// The cell of the process, its streams and events, and per event the
/// articles of its chain ([`crate::law::Explanation`]), the decisions on it
/// and the policies that fill in a field beforehand.
fn cell_part(b: &mut Builder, input: &MapInput) {
    let p = input.process;
    let cell = &p.cell;
    let c = b.node(
        format!("cell:{}", cell.id()),
        NodeKind::Cell,
        cell.id(),
        SourceRef::config("cell", "id"),
    );
    for stream in &cell.streams {
        let s = b.node(
            format!("stream:{}", stream.id),
            NodeKind::Stream,
            &stream.id,
            SourceRef::config(&format!("stream/{}", stream.id), "events"),
        );
        b.edge(&c, &s, EdgeKind::Records);
        for event in &stream.events {
            let e = b.node(
                event_id(&stream.id, &event.name),
                NodeKind::Event,
                &event.name,
                SourceRef::stream(&stream.id, &event.name),
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
fn read_events(def: &LexostatusDefinition, streams: &[Stream]) -> BTreeSet<String> {
    let filters: Vec<Option<&Filter>> = def.all_derivations().map(|(_, d)| d.filter()).collect();
    // A derivation on the chosen gram reads what the lexostatus selects.
    let chosen = filters.is_empty() || filters.iter().any(Option::is_none);
    let mut out = BTreeSet::new();
    for f in std::iter::once(None)
        .filter(|_| chosen)
        .chain(filters.into_iter().flatten().map(Some))
    {
        for (s, e) in crate::check::events_for(def, f, streams) {
            out.insert(event_id(&s.id, &e.name));
        }
    }
    out
}

/// The lexostatuses of the cell, the events they read and, for one the law
/// reads, the reading article.
fn lexostatus_part(b: &mut Builder, input: &MapInput) {
    let cell = &input.process.cell;
    for d in &cell.lexostatuses.lexostatus_definitions {
        let source = match &d.law {
            Some(law) => SourceRef::law(&law.article),
            // The runtime offers the worklist (RFC-047); no file defines
            // it, so it opens to the submission it lists (its filter names
            // the one the portal submits).
            None if d.name == crate::reduction::WORKLIST => {
                crate::check::events_for(d, None, &cell.streams)
                    .first()
                    .map(|(s, e)| SourceRef::stream(&s.id, &e.name))
                    .unwrap_or_else(|| SourceRef::config("lexostatuses", &d.name))
            }
            None => SourceRef::config("lexostatuses", &d.name),
        };
        let l = b.node(
            lexostatus_id(cell.id(), &d.name),
            NodeKind::Lexostatus,
            &d.name,
            source,
        );
        for event in read_events(d, &cell.streams) {
            b.edge(&l, &event, EdgeKind::Reads);
        }
        if let Some(law) = &d.law {
            let a = b.article(&law.article);
            b.edge(&l, &a, EdgeKind::Executes);
        }
    }
}

/// A cell the process queries that is not its own (synthesis, rows). For a
/// process from policy (RFC-047) that is written in the synthesis of the
/// deployment, under the id of the process's cell.
fn source_cell(b: &mut Builder, input: &MapInput, cell: &str, anchor: &str) -> String {
    let p = input.process;
    let source = match p.definition.declared_by {
        Some(_) => SourceRef::config("synthesis", p.cell.id()),
        None => in_process(anchor),
    };
    b.node(
        format!("source_cell:{cell}"),
        NodeKind::SourceCell,
        cell,
        source,
    )
}

/// The cells a synthesis per row queries, from `from`.
fn rows_part(b: &mut Builder, input: &MapInput, from: &str, rows: &[RowsDefinition]) {
    for r in rows {
        for src in &r.sources {
            let c = source_cell(b, input, &src.cell, "rows");
            b.edge(from, &c, EdgeKind::Rows);
        }
    }
}

/// The worklist and the actions of the handling: the article each action
/// executes, the hooks on its stage and the event it records.
fn handling_part(b: &mut Builder, input: &MapInput, proc: &str) {
    let Some(h) = &input.process.definition.handling else {
        return;
    };
    b.edge(
        proc,
        &lexostatus_id(&h.worklist.cell, &h.worklist.lexostatus),
        EdgeKind::Synthesis,
    );
    for action in &h.actions {
        let a = b.node(
            format!("action:{}", action.name),
            NodeKind::Action,
            action.label(),
            // A process from policy (RFC-047) derives the action from the
            // event it records.
            match input.process.definition.declared_by {
                Some(_) => SourceRef::stream(&action.record.stream, &action.record.event),
                None => in_process(&action.name),
            },
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
            &event_id(&action.record.stream, &action.record.event),
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
        let target = if s.cell == p.cell.id() {
            lexostatus_id(&s.cell, &s.lexostatus)
        } else {
            source_cell(b, input, &s.cell, "synthesis")
        };
        b.edge(proc, &target, EdgeKind::Synthesis);
    }
    rows_part(b, input, proc, p.assessment_rows());
}

/// What the articles on the map execute (RFC-047): an edge from the policy
/// article to the executed article, one level.
fn executes_part(b: &mut Builder, input: &MapInput) {
    let present: Vec<(String, String, String)> = b
        .nodes
        .values()
        .filter_map(|n| Some((n.id.clone(), n.regulation.clone()?, n.article.clone()?)))
        .collect();
    for (node, regulation, number) in present {
        for e in input
            .process
            .service
            .resolver()
            .executes_of(&regulation, &number)
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
    let present: Vec<(String, String, String)> = b
        .nodes
        .values()
        .filter_map(|n| Some((n.id.clone(), n.regulation.clone()?, n.article.clone()?)))
        .collect();
    for (node, regulation, number) in present {
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
/// into its chronicle if that is the cell of the process, otherwise the
/// cell elsewhere.
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
        let cell = &input.process.cell;
        if link.cell == cell.id() {
            for stream in cell
                .streams
                .iter()
                .filter(|s| s.chronicle == link.chronicle)
            {
                b.edge(&r, &format!("stream:{}", stream.id), EdgeKind::Register);
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
