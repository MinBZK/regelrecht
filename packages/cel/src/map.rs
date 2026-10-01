//! The map of a process (`GET /api/map`, spec "Opbouw"): which
//! configuration, events, lexostatuses and articles the runtime uses, and how
//! they hang together. Built from what the runtime loaded; the edges between
//! an event and the law come from the explanation of the event
//! ([`crate::law::Explanation`]), so the map and the "why" of the form show
//! the same chain. Every node names where it is written as a
//! [`SourceRef`], which the fragment routes resolve.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

use crate::law::SourceRef;
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
    pub date: chrono::NaiveDate,
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
    b.finish(id)
}
