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

use crate::law::{SourceRef, StepKind};
use crate::process::Process;
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
    pub registers: &'a [RegisterLink],
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
        in_process("id"),
    );
    for name in d.channels.keys() {
        let c = b.node(
            format!("channel:{name}"),
            NodeKind::Channel,
            name,
            in_process(name),
        );
        b.edge(&proc, &c, EdgeKind::Channel);
    }
    for (name, role) in &d.roles {
        let r = b.node(
            format!("role:{name}"),
            NodeKind::Role,
            name,
            in_process(name),
        );
        b.edge(&proc, &r, EdgeKind::Role);
        b.edge(&r, &format!("channel:{}", role.channel), EdgeKind::Channel);
    }
    for m in &d.mandates {
        let a = b.article(&m.legal_basis);
        b.edge(&proc, &a, EdgeKind::LegalBasis);
    }
    cell_part(&mut b, input);
    if let Some(portal) = p.portal() {
        b.edge(
            &proc,
            &event_id(&portal.stream, &portal.event),
            EdgeKind::Portal,
        );
    }
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
