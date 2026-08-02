//! Centrality, and the two things a degree count is and is not allowed to
//! decide.
//!
//! Three numbers per law-level node. `in_refs` is how often the law is pointed
//! at and `citers` is how many distinct laws do the pointing; both are honest,
//! explainable and go into the payload. `rank` is PageRank over the citation
//! direction, so mass flows towards the law that is cited, which is the "waar
//! hangt veel van af"-measure the design asks for.
//!
//! What these numbers do **not** decide is whether a law is a framework law.
//! That is a legal qualification and it comes from [`crate::kaderwet`], where
//! someone designates it. The numbers are what that someone looks at while
//! deciding, and [`top_by_citers`] exists to put them in front of them.
//!
//! They do decide something else, and it is worth keeping the two apart by
//! name: a law with a very high in-degree is damped in the layout, because a
//! node with 867 incoming edges pushes every other structure out of the
//! picture. That is [`crate::layout`]'s business, it has a graphical reason,
//! and it deliberately does not have to coincide with the designation list.

use crate::graph::{CorpusGraph, EdgeType, NodeIx};
use crate::kaderwet::Kaderwetten;

/// Fill in `citers`, `rank` and `framework` for every law-level node.
///
/// Returns the distinct-citer count per law-level node. That is the number the
/// report shows next to a law, and it is material for whoever fills the
/// designation list; it is not itself a verdict.
pub fn compute(graph: &mut CorpusGraph, kaderwetten: &Kaderwetten) -> Vec<u32> {
    let n = graph.law_node_count;
    let mut citers = vec![0u32; n];
    {
        // Distinct citers, not references: a law that cites the Awb forty
        // times still only counts once towards the star.
        let mut last_source = vec![u32::MAX; n];
        for edge in &graph.edges[..graph.law_edge_count] {
            let t = edge.target as usize;
            if last_source[t] != edge.source {
                last_source[t] = edge.source;
                citers[t] += 1;
            }
        }
    }

    // A law that declares itself applicable to another is a framework law by
    // the nature of that relation, however few or many times it does it. The
    // harvested reference block cannot express this, so today this finds
    // nothing; the mechanism is here so that an enriched corpus does not need
    // the designation list to state the obvious.
    let mut declares_applicability = vec![false; n];
    for edge in &graph.edges[..graph.law_edge_count] {
        if edge.edge_type == EdgeType::Applicability && edge.source != edge.target {
            declares_applicability[edge.source as usize] = true;
        }
    }

    let rank = pagerank(graph);
    let max = rank
        .iter()
        .copied()
        .fold(0.0f32, f32::max)
        .max(f32::EPSILON);

    let mut framework = 0;
    let mut designated = 0;
    for (ix, node) in graph.nodes[..n].iter_mut().enumerate() {
        node.rank = rank[ix] / max;
        node.citers = citers[ix];
        let on_the_list = kaderwetten.designates(node.bwb_id.as_deref(), &node.id);
        if on_the_list {
            designated += 1;
        }
        node.framework = on_the_list || declares_applicability[ix];
        if node.framework {
            framework += 1;
        }
    }
    graph.stats.framework_laws = framework;
    graph.stats.designated_framework_laws = designated;
    graph.stats.derived_framework_laws = framework - designated;
    citers
}

/// PageRank over the law-level edge set, weighted by how often the reference is
/// actually made.
///
/// Fifty iterations at damping 0.85, which is well past convergence on a graph
/// this size, and the iteration order is the node order, so the result is
/// reproducible to the bit.
fn pagerank(graph: &CorpusGraph) -> Vec<f32> {
    let n = graph.law_node_count;
    if n == 0 {
        return Vec::new();
    }
    let damping = 0.85f32;
    let mut out_weight = vec![0.0f32; n];
    for edge in &graph.edges[..graph.law_edge_count] {
        out_weight[edge.source as usize] += edge.count as f32;
    }

    let mut rank = vec![1.0f32 / n as f32; n];
    let mut next = vec![0.0f32; n];
    for _ in 0..50 {
        next.fill(0.0);
        let mut dangling = 0.0f32;
        for ix in 0..n {
            if out_weight[ix] == 0.0 {
                dangling += rank[ix];
            }
        }
        for edge in &graph.edges[..graph.law_edge_count] {
            let src = edge.source as usize;
            if out_weight[src] == 0.0 {
                continue;
            }
            next[edge.target as usize] += rank[src] * (edge.count as f32) / out_weight[src];
        }
        let base = (1.0 - damping) / n as f32 + damping * dangling / n as f32;
        for value in next.iter_mut() {
            *value = base + damping * *value;
        }
        std::mem::swap(&mut rank, &mut next);
    }
    rank
}

/// The heaviest stars, most-cited first.
///
/// This is the working material for whoever fills the kaderwetlijst: a ranked
/// list of the laws the corpus leans on hardest, with the number that makes
/// them stand out. Reading a qualification straight off it is the mistake this
/// module exists to avoid, and the builder prints it under a heading that says
/// so.
pub fn top_by_citers(graph: &CorpusGraph, citers: &[u32], limit: usize) -> Vec<(NodeIx, u32)> {
    let mut ranked: Vec<(NodeIx, u32)> = citers
        .iter()
        .enumerate()
        .map(|(ix, &c)| (ix as NodeIx, c))
        .collect();
    ranked.sort_by(|a, b| {
        b.1.cmp(&a.1).then_with(|| {
            graph.nodes[a.0 as usize]
                .id
                .cmp(&graph.nodes[b.0 as usize].id)
        })
    });
    ranked.truncate(limit);
    ranked
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kaderwet::{Kaderwetkaart, Kaderwetten};
    use crate::testkit::star_graph;

    fn designating(law_id: &str) -> Kaderwetten {
        Kaderwetten {
            version: 1,
            beheerder: Some("test".to_string()),
            kaderwetten: vec![Kaderwetkaart {
                law_id: Some(law_id.to_string()),
                ..Kaderwetkaart::default()
            }],
        }
    }

    #[test]
    fn the_star_centre_outranks_its_arms() {
        let mut graph = star_graph(60, 3);
        let citers = compute(&mut graph, &Kaderwetten::default());
        let hub = graph.lookup("hub").expect("hub");
        assert_eq!(citers[hub as usize], 60);
        for node in &graph.nodes {
            if node.id != "hub" {
                assert!(node.rank < graph.node(hub).rank);
            }
        }
    }

    /// The point of the whole module: being cited by everyone is not a
    /// qualification, however many everyone is.
    #[test]
    fn the_heaviest_star_is_not_a_framework_law_by_itself() {
        let mut graph = star_graph(600, 0);
        let citers = compute(&mut graph, &Kaderwetten::default());
        let hub = graph.lookup("hub").expect("hub");
        assert_eq!(citers[hub as usize], 600);
        assert!(
            !graph.node(hub).framework,
            "een ingraad van 600 is geen juridische kwalificatie"
        );
        assert_eq!(graph.stats.framework_laws, 0);
    }

    #[test]
    fn designation_makes_a_framework_law_whatever_its_degree() {
        // The designated law here is an arm with exactly one citation, not the
        // hub: the list decides, the degree does not.
        let mut graph = star_graph(60, 0);
        compute(&mut graph, &designating("arm0000"));
        let arm = graph.lookup("arm0000").expect("arm");
        let hub = graph.lookup("hub").expect("hub");
        assert!(graph.node(arm).framework);
        assert!(!graph.node(hub).framework);
        assert_eq!(graph.stats.designated_framework_laws, 1);
        assert_eq!(graph.stats.derived_framework_laws, 0);
    }

    /// The second route, which the corpus cannot take yet.
    #[test]
    fn an_applicability_edge_qualifies_its_source() {
        let mut graph = star_graph(60, 0);
        let hub = graph.lookup("hub").expect("hub");
        let arm = graph.lookup("arm0000").expect("arm");
        graph.edges.push(crate::graph::Edge {
            source: hub,
            target: arm,
            edge_type: EdgeType::Applicability,
            count: 1,
        });
        graph.law_edge_count = graph.edges.len();
        compute(&mut graph, &Kaderwetten::default());
        assert!(graph.node(hub).framework);
        assert_eq!(graph.stats.designated_framework_laws, 0);
        assert_eq!(graph.stats.derived_framework_laws, 1);
    }

    #[test]
    fn distinct_citers_ignore_repeat_references() {
        let mut graph = star_graph(60, 0);
        // star_graph gives each arm a single reference; multiply one of them.
        let hub = graph.lookup("hub").expect("hub");
        for edge in &mut graph.edges {
            if edge.target == hub {
                edge.count = 40;
            }
        }
        let citers = compute(&mut graph, &Kaderwetten::default());
        assert_eq!(citers[hub as usize], 60);
    }
}
