//! The graph itself: node and edge types, and the container the rest of the
//! pipeline (metrics, clustering, layout, payload) operates on.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// What a node stands for.
///
/// The distinction that matters for the renderer is between a node that is a
/// document we hold and a node that only exists because something else points
/// at it. Both `External` and `Expected` are the second kind; they differ in
/// how we learned about them, which decides what a user can do with them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeKind {
    /// A regulation the corpus holds, at one version.
    Law,
    /// An article inside a law. Carries local coordinates around its parent.
    Article,
    /// A BWB identifier that harvested references point at, of which the corpus
    /// holds no document. This is the corpus boundary made countable.
    External,
    /// A regulation a law says should exist (`open_terms.delegated_to` /
    /// `expected_source`) and which has not been harvested. RFC-026's second
    /// work queue.
    Expected,
}

impl NodeKind {
    pub const ALL: [NodeKind; 4] = [
        NodeKind::Law,
        NodeKind::Article,
        NodeKind::External,
        NodeKind::Expected,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            NodeKind::Law => "law",
            NodeKind::Article => "article",
            NodeKind::External => "external",
            NodeKind::Expected => "expected",
        }
    }
}

/// What a relation means, and therefore how hard it should pull in the layout.
///
/// Only `Citation` exists in bulk today. The others are what the corpus says
/// once it is enriched; they are built and weighted now so that the layout does
/// not have to change shape when enrichment arrives.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeType {
    /// A pointer in the wettekst, from the harvested `references` block. Says
    /// "this article mentions that one" and nothing stronger.
    Citation,
    /// `source.regulation` on an input: this article cannot be computed without
    /// that law's output. The strongest statement the corpus makes.
    Source,
    /// `implements`: a lower regulation filling in a higher law's open term,
    /// drawn from the implementer to the law it implements.
    Delegation,
    /// An `open_term` that names an invuller which the corpus does not hold.
    /// Runs from the delegating article to an [`NodeKind::Expected`] node.
    ExpectedDelegation,
    /// A citation that a heuristic reads as a scope statement ("is van
    /// overeenkomstige toepassing"). Reserved: the harvested reference block
    /// carries no such marker today, so nothing produces this yet.
    Applicability,
    /// A citation from an amending instrument to the law it amends. Reserved
    /// for the same reason.
    Amendment,
}

impl EdgeType {
    pub const ALL: [EdgeType; 6] = [
        EdgeType::Citation,
        EdgeType::Source,
        EdgeType::Delegation,
        EdgeType::ExpectedDelegation,
        EdgeType::Applicability,
        EdgeType::Amendment,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            EdgeType::Citation => "citation",
            EdgeType::Source => "source",
            EdgeType::Delegation => "delegation",
            EdgeType::ExpectedDelegation => "expected_delegation",
            EdgeType::Applicability => "applicability",
            EdgeType::Amendment => "amendment",
        }
    }

    /// How hard this relation pulls in the layout, before hub damping.
    ///
    /// The numbers follow the design: a computed dependency and a delegation
    /// are structure, a citation is mostly background noise, and applicability
    /// is nearly free. Citation sits low on purpose. It is the only type the
    /// corpus has in bulk today, and at full strength the five million of them
    /// flatten every other signal.
    pub fn layout_weight(self) -> f32 {
        match self {
            EdgeType::Source => 1.0,
            EdgeType::Delegation => 0.6,
            EdgeType::ExpectedDelegation => 0.6,
            EdgeType::Citation => 0.25,
            EdgeType::Amendment => 0.2,
            EdgeType::Applicability => 0.1,
        }
    }
}

/// The layer of the regulatory hierarchy a document sits on. The renderer maps
/// this to node shape; the graph only carries it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RegulatoryLayer {
    Grondwet,
    Wet,
    Amvb,
    MinisterieleRegeling,
    Beleidsregel,
    GemeentelijkeVerordening,
    ProvincialeVerordening,
    WaterschapsVerordening,
    EuVerordening,
    Onbekend,
}

impl RegulatoryLayer {
    pub const ALL: [RegulatoryLayer; 10] = [
        RegulatoryLayer::Grondwet,
        RegulatoryLayer::Wet,
        RegulatoryLayer::Amvb,
        RegulatoryLayer::MinisterieleRegeling,
        RegulatoryLayer::Beleidsregel,
        RegulatoryLayer::GemeentelijkeVerordening,
        RegulatoryLayer::ProvincialeVerordening,
        RegulatoryLayer::WaterschapsVerordening,
        RegulatoryLayer::EuVerordening,
        RegulatoryLayer::Onbekend,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            RegulatoryLayer::Grondwet => "GRONDWET",
            RegulatoryLayer::Wet => "WET",
            RegulatoryLayer::Amvb => "AMVB",
            RegulatoryLayer::MinisterieleRegeling => "MINISTERIELE_REGELING",
            RegulatoryLayer::Beleidsregel => "BELEIDSREGEL",
            RegulatoryLayer::GemeentelijkeVerordening => "GEMEENTELIJKE_VERORDENING",
            RegulatoryLayer::ProvincialeVerordening => "PROVINCIALE_VERORDENING",
            RegulatoryLayer::WaterschapsVerordening => "WATERSCHAPS_VERORDENING",
            RegulatoryLayer::EuVerordening => "EU_VERORDENING",
            RegulatoryLayer::Onbekend => "ONBEKEND",
        }
    }

    pub fn parse(raw: &str) -> RegulatoryLayer {
        match raw.trim().to_ascii_uppercase().as_str() {
            "GRONDWET" => RegulatoryLayer::Grondwet,
            "WET" => RegulatoryLayer::Wet,
            "AMVB" | "AMVB_" | "ALGEMENE_MAATREGEL_VAN_BESTUUR" => RegulatoryLayer::Amvb,
            "MINISTERIELE_REGELING" => RegulatoryLayer::MinisterieleRegeling,
            "BELEIDSREGEL" => RegulatoryLayer::Beleidsregel,
            "GEMEENTELIJKE_VERORDENING" => RegulatoryLayer::GemeentelijkeVerordening,
            "PROVINCIALE_VERORDENING" => RegulatoryLayer::ProvincialeVerordening,
            "WATERSCHAPS_VERORDENING" | "WATERSCHAPSVERORDENING" => {
                RegulatoryLayer::WaterschapsVerordening
            }
            "EU_VERORDENING" => RegulatoryLayer::EuVerordening,
            _ => RegulatoryLayer::Onbekend,
        }
    }
}

/// How far the enrichment has got on this node.
///
/// Three values and not a flag bit, because on a law it is not a yes or no. A
/// law of forty articles with three modelled is neither enriched nor
/// un-enriched, and collapsing that to one bit throws away exactly the thing
/// the map is supposed to show: how much is done and where. On an article it
/// *is* binary, and there only [`Enrichment::None`] and [`Enrichment::Full`]
/// occur.
///
/// Whether the enricher is working on a law right now is deliberately not one
/// of these values. That is orthogonal (a half-enriched law can be under way or
/// idle) and it changes by the minute, so it lives in its own section; see
/// [`crate::payload`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Enrichment {
    /// No article carries a substantive `machine_readable` section. In the
    /// first version of the map this is nearly everything, and that is the
    /// point.
    None,
    /// Some articles do.
    Partial,
    /// Every article does.
    Full,
}

impl Enrichment {
    pub const ALL: [Enrichment; 3] = [Enrichment::None, Enrichment::Partial, Enrichment::Full];

    pub fn as_str(self) -> &'static str {
        match self {
            Enrichment::None => "none",
            Enrichment::Partial => "partial",
            Enrichment::Full => "full",
        }
    }

    /// The state of a law given how many of its articles are modelled.
    pub fn of(articles: u32, enriched: u32) -> Enrichment {
        match (articles, enriched) {
            (_, 0) => Enrichment::None,
            (a, e) if e >= a => Enrichment::Full,
            _ => Enrichment::Partial,
        }
    }
}

/// Index into [`CorpusGraph::nodes`]. Stable within one build only; the stable
/// cross-build identity is [`Node::id`].
pub type NodeIx = u32;

#[derive(Debug, Clone)]
pub struct Node {
    /// Stable identity across builds. Slug for a harvested law, `bwb:BWBR…`
    /// for a law without a slug and for an external, `expected:…` for an
    /// expected regulation, `<law>#<article>` for an article.
    pub id: String,
    /// What a human reads on the node.
    pub label: String,
    pub kind: NodeKind,
    pub layer: RegulatoryLayer,
    /// BWB identifier where we have one.
    pub bwb_id: Option<String>,
    /// The version date the node was built from (`valid_from`), for laws.
    pub valid_from: Option<String>,
    /// Containment parent: the law an article belongs to. `None` at law level.
    pub parent: Option<NodeIx>,
    /// Position. Global for law-level nodes, relative to the parent for
    /// articles.
    pub x: f32,
    pub y: f32,
    pub z: f32,
    /// Number of incoming underlying references. This is the honest simple
    /// measure and it is what the renderer scales node size with.
    pub in_refs: u32,
    /// Number of outgoing underlying references.
    pub out_refs: u32,
    /// How much of this node is modelled.
    pub enrichment: Enrichment,
    /// Articles in this law. Zero for a node that is not a held document, one
    /// for an article node.
    pub articles: u32,
    /// Of which carry a substantive `machine_readable` section.
    pub articles_enriched: u32,
    /// Number of distinct laws that cite this one. The honest simple measure of
    /// how much the corpus leans on it, and the working material for whoever
    /// fills the kaderwetlijst. Never a qualification by itself.
    pub citers: u32,
    /// Reverse PageRank, normalised so the maximum is 1.0.
    pub rank: f32,
    /// Community index from [`crate::cluster`]. Framework laws get their own
    /// pseudo-cluster.
    pub cluster: u16,
    /// A framework law: designated on the kaderwetlijst, or stating an
    /// applicability relation itself. A legal qualification, never derived from
    /// a degree. Placed on its own ring above the field and kept out of the
    /// communities, because it belongs to none of them.
    pub framework: bool,
}

/// One relation between two nodes, already aggregated per
/// `(source, target, type)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edge {
    pub source: NodeIx,
    pub target: NodeIx,
    pub edge_type: EdgeType,
    /// How many underlying references collapsed into this edge.
    pub count: u32,
}

/// Everything one build produced.
#[derive(Debug, Clone, Default)]
pub struct CorpusGraph {
    /// Law-level nodes first (law, external, expected), articles after. Both
    /// blocks are sorted by [`Node::id`], so the order does not depend on how
    /// the filesystem handed the files over.
    pub nodes: Vec<Node>,
    /// Law-level edges first, article-level edges after. Both blocks are
    /// sorted by `(source, target, type)`.
    pub edges: Vec<Edge>,
    /// Number of leading law-level nodes. A renderer that only draws the
    /// overview reads `nodes[..law_node_count]` and stops.
    pub law_node_count: usize,
    /// Number of leading law-level edges, same idea.
    pub law_edge_count: usize,
    /// Statistics worth reporting; also written into the payload header.
    pub stats: BuildStats,
    index: HashMap<String, NodeIx>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct BuildStats {
    pub files_scanned: usize,
    pub files_parsed: usize,
    pub files_failed: usize,
    /// The files that could not be read, with the reason. Named rather than
    /// only counted: an unparsed file is a law missing from the map.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub failures: Vec<crate::build::ParseFailure>,
    pub laws: usize,
    pub articles: usize,
    pub external_nodes: usize,
    pub expected_nodes: usize,
    /// Raw reference lines read, before aggregation.
    pub raw_references: u64,
    /// References that could not be attached to any node (no `bwb_id`).
    pub dangling_references: u64,
    pub aggregated_edges: usize,
    pub framework_laws: usize,
    /// Laws with at least one modelled article.
    pub laws_partly_enriched: usize,
    /// Laws with every article modelled.
    pub laws_fully_enriched: usize,
    /// Articles carrying a substantive `machine_readable` section.
    pub enriched_articles: usize,
    /// Of which designated on the kaderwetlijst.
    pub designated_framework_laws: usize,
    /// Of which derived from an applicability relation in the corpus.
    pub derived_framework_laws: usize,
    /// Where the kaderwetlijst came from, or why there is none.
    pub kaderwetlijst: String,
    pub clusters: usize,
    pub parse_ms: u128,
    pub metrics_ms: u128,
    pub cluster_ms: u128,
    pub layout_ms: u128,
    /// Net drift of the average node over the last tenth of the layout run, as
    /// a fraction of the radius of the cloud. Zero is settled; anything you can see is
    /// a picture that would still be moving if you kept going.
    pub layout_unsettled: f32,
    /// Rank correlation of all pairwise distances between the start and the end
    /// of that same window. One means the picture stopped changing, whatever
    /// the individual nodes are still doing.
    pub layout_stability: f32,
    pub peak_rss_kb: u64,
}

impl CorpusGraph {
    pub fn node(&self, ix: NodeIx) -> &Node {
        &self.nodes[ix as usize]
    }

    pub fn lookup(&self, id: &str) -> Option<NodeIx> {
        self.index.get(id).copied()
    }

    /// Insert a node, or return the existing index if the id is already taken.
    pub fn intern(&mut self, node: Node) -> NodeIx {
        if let Some(&ix) = self.index.get(&node.id) {
            return ix;
        }
        let ix = self.nodes.len() as NodeIx;
        self.index.insert(node.id.clone(), ix);
        self.nodes.push(node);
        ix
    }

    /// Rebuild the id index after nodes have been reordered.
    pub fn reindex(&mut self) {
        self.index.clear();
        for (ix, node) in self.nodes.iter().enumerate() {
            self.index.insert(node.id.clone(), ix as NodeIx);
        }
    }

    /// Indices of the law-level nodes: everything that is not an article.
    /// These are the nodes the global layout runs on.
    pub fn law_level(&self) -> Vec<NodeIx> {
        (0..self.law_node_count as NodeIx).collect()
    }

    /// How hard an edge pulls in the layout and how much it counts towards a
    /// community.
    ///
    /// Two factors, and it matters that neither of them looks at how often the
    /// *target* is cited. **The type weight** says a computed dependency means
    /// more than a mention, which is a statement about the relation. **The
    /// logarithm on the count** says that citing a law forty times is more than
    /// citing it once but not forty times more, which stops a definition
    /// article that repeats one reference in every lid from outweighing a real
    /// structural relation; that is a statement about one article's drafting
    /// habits, not about the law it points at.
    ///
    /// There is deliberately no third factor damping edges into heavily cited
    /// laws. If the Awb ends up in the middle because 867 laws hang off it,
    /// that is where it belongs, and a map that weakens those edges to look
    /// tidier is lying about the shape of Dutch law. The distance to that
    /// middle is itself the finding: private law and criminal law largely fall
    /// outside the bestuursrecht, and where they end up relative to the Awb is
    /// something a lawyer wants to see. Damping the pull would erase exactly
    /// that.
    pub fn mechanical_weight(&self, edge: &Edge) -> f32 {
        edge.edge_type.layout_weight() * (1.0 + (edge.count as f32).ln())
    }

    /// Put the nodes in canonical order (law level first, then articles, each
    /// block sorted by id) and rewrite every index that pointed into the old
    /// order.
    ///
    /// This is what makes the whole build independent of directory-walk order:
    /// the layout consumes node indices, so if the indices move, the layout
    /// moves. Sorting on the stable id pins them.
    pub fn canonicalise(&mut self) {
        let mut order: Vec<NodeIx> = (0..self.nodes.len() as NodeIx).collect();
        order.sort_by(|&a, &b| {
            let na = &self.nodes[a as usize];
            let nb = &self.nodes[b as usize];
            let article_a = na.kind == NodeKind::Article;
            let article_b = nb.kind == NodeKind::Article;
            article_a.cmp(&article_b).then_with(|| na.id.cmp(&nb.id))
        });

        let mut new_of = vec![0 as NodeIx; self.nodes.len()];
        for (new, &old) in order.iter().enumerate() {
            new_of[old as usize] = new as NodeIx;
        }

        let mut nodes: Vec<Node> = Vec::with_capacity(self.nodes.len());
        for &old in &order {
            let mut node = self.nodes[old as usize].clone();
            node.parent = node.parent.map(|p| new_of[p as usize]);
            nodes.push(node);
        }
        self.nodes = nodes;
        self.law_node_count = self
            .nodes
            .iter()
            .take_while(|n| n.kind != NodeKind::Article)
            .count();

        for edge in &mut self.edges {
            edge.source = new_of[edge.source as usize];
            edge.target = new_of[edge.target as usize];
        }
        let law_level = self.law_node_count as NodeIx;
        self.edges.sort_by_key(|e| {
            (
                e.source >= law_level || e.target >= law_level,
                e.source,
                e.target,
                e.edge_type,
            )
        });
        self.law_edge_count = self
            .edges
            .iter()
            .take_while(|e| e.source < law_level && e.target < law_level)
            .count();
        self.reindex();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intern_is_idempotent_on_id() {
        let mut g = CorpusGraph::default();
        let a = g.intern(test_node("x"));
        let b = g.intern(test_node("x"));
        assert_eq!(a, b);
        assert_eq!(g.nodes.len(), 1);
    }

    #[test]
    fn layout_weights_order_types_as_designed() {
        assert!(EdgeType::Source.layout_weight() > EdgeType::Delegation.layout_weight());
        assert!(EdgeType::Delegation.layout_weight() > EdgeType::Citation.layout_weight());
        assert!(EdgeType::Citation.layout_weight() > EdgeType::Applicability.layout_weight());
    }

    fn test_node(id: &str) -> Node {
        Node {
            id: id.to_string(),
            label: id.to_string(),
            kind: NodeKind::Law,
            layer: RegulatoryLayer::Wet,
            bwb_id: None,
            valid_from: None,
            parent: None,
            x: 0.0,
            y: 0.0,
            z: 0.0,
            in_refs: 0,
            out_refs: 0,
            citers: 0,
            enrichment: Enrichment::None,
            articles: 0,
            articles_enriched: 0,
            rank: 0.0,
            cluster: 0,
            framework: false,
        }
    }
}
