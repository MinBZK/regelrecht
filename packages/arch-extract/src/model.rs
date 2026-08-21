//! The architecture model: the single, language-agnostic artifact this tool
//! emits. Containment is expressed with `parent`; relationships with `edges`.
//!
//! The shape mirrors a C4-style hierarchy so the architecture explorer can
//! render both the high-level view and the interactive deep-zoom from one file.
//! Node ids are stable paths (`crate:engine`,
//! `mod:engine::service`, `type:engine::service::LawExecutionService`,
//! `fn:engine::service::LawExecutionService::execute`) so diffs stay meaningful
//! across regenerations. Deliberately **no timestamp** lives in the generator
//! output, so a node keeps a stable identity between runs — the architecture
//! explorer relies on that.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Bumped when the *shape* of the model changes (not its contents), so the JSON
/// schema and any consumers can guard against a mismatch.
pub const SCHEMA_VERSION: &str = "1";

/// C4-style coarse tier. `system` is reserved for the explorer (which
/// synthesizes the system boundary that wraps the containers); the generator
/// emits `container`, `component` and `code` nodes in v1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    System,
    Container,
    Component,
    Code,
}

/// The concrete code construct a node represents. The first block is the
/// Rust-workspace vocabulary; the second is the JavaScript/TypeScript/Vue
/// vocabulary for the frontends (see `js_pass.rs`). Both share one model — the
/// `kind` just records which language construct a node stands for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    // Rust workspace (crate graph + `syn` pass).
    Crate,
    Binary,
    Module,
    Struct,
    Enum,
    Trait,
    Method,
    Fn,
    // JS/TS/Vue frontends (`js_pass`).
    /// An npm-workspace frontend (a Vue app or the shared package) — the JS-side
    /// analogue of a `crate`.
    App,
    /// A source directory that groups files — the JS-side analogue of a Rust
    /// module directory.
    Dir,
    /// A Vue single-file component (`.vue`).
    Component,
    /// A `useXxx` composable module.
    Composable,
}

/// A relationship between two nodes. `depends-on` is crate→crate (from cargo
/// metadata); `impl` is type→trait; `uses` is a type→type source reference.
/// `calls` (full call-graph) is intentionally out of scope for v1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EdgeKind {
    DependsOn,
    Impl,
    Uses,
    Calls,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub id: String,
    pub level: Level,
    pub kind: Kind,
    /// Source language of the node: `"rust"` for the workspace crates,
    /// `"vue"`/`"ts"`/`"js"` for the frontends. The model is deliberately
    /// language-agnostic — this field is how a consumer tells the tiers apart.
    pub lang: String,
    pub name: String,
    /// Repo-relative source location (a directory for crates, a file for the
    /// rest). Empty only for synthetic nodes.
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    /// First line of the item's doc-comment, feeding the prose layer.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub doc: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Edge {
    pub from: String,
    pub to: String,
    pub kind: EdgeKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Model {
    /// Points editors at the schema; ignored by consumers of the model.
    #[serde(rename = "$schema")]
    pub schema: String,
    #[serde(rename = "schemaVersion")]
    pub schema_version: String,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
}

impl Model {
    /// Assembles a model from collected nodes/edges, then canonicalizes it:
    /// nodes sorted by id, edges sorted and de-duplicated, dangling edges
    /// dropped. This determinism gives every node a stable identity between
    /// runs, which the architecture explorer relies on.
    pub fn new(mut nodes: Vec<Node>, edges: Vec<Edge>) -> Self {
        nodes.sort_by(|a, b| a.id.cmp(&b.id));
        nodes.dedup_by(|a, b| a.id == b.id);

        let ids: BTreeSet<&str> = nodes.iter().map(|n| n.id.as_str()).collect();
        let mut edges: Vec<Edge> = edges
            .into_iter()
            .filter(|e| {
                e.from != e.to && ids.contains(e.from.as_str()) && ids.contains(e.to.as_str())
            })
            .collect();
        edges.sort();
        edges.dedup();

        Self {
            schema: "./model.schema.json".to_string(),
            schema_version: SCHEMA_VERSION.to_string(),
            nodes,
            edges,
        }
    }

    /// Canonical JSON: pretty-printed, 2-space indent, trailing newline.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        let mut s = serde_json::to_string_pretty(self)?;
        s.push('\n');
        Ok(s)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use super::*;

    fn node(id: &str, parent: Option<&str>) -> Node {
        Node {
            id: id.to_string(),
            level: Level::Container,
            kind: Kind::Crate,
            lang: "rust".to_string(),
            name: id.to_string(),
            path: String::new(),
            parent: parent.map(str::to_string),
            doc: None,
        }
    }

    fn edge(from: &str, to: &str) -> Edge {
        Edge {
            from: from.to_string(),
            to: to.to_string(),
            kind: EdgeKind::DependsOn,
        }
    }

    #[test]
    fn nodes_are_sorted_and_deduped() {
        let m = Model::new(
            vec![
                node("crate:b", None),
                node("crate:a", None),
                node("crate:a", None),
            ],
            vec![],
        );
        let ids: Vec<&str> = m.nodes.iter().map(|n| n.id.as_str()).collect();
        assert_eq!(ids, vec!["crate:a", "crate:b"]);
    }

    #[test]
    fn dangling_and_self_edges_are_dropped() {
        let m = Model::new(
            vec![node("crate:a", None), node("crate:b", None)],
            vec![
                edge("crate:a", "crate:b"),       // kept
                edge("crate:a", "crate:missing"), // dangling target
                edge("crate:a", "crate:a"),       // self-edge
            ],
        );
        assert_eq!(m.edges, vec![edge("crate:a", "crate:b")]);
    }

    #[test]
    fn output_is_deterministic_and_newline_terminated() {
        let a = Model::new(vec![node("crate:a", None)], vec![])
            .to_json()
            .expect("json");
        let b = Model::new(vec![node("crate:a", None)], vec![])
            .to_json()
            .expect("json");
        assert_eq!(a, b);
        assert!(a.ends_with("\n"));
    }
}
