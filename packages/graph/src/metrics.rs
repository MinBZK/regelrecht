//! Centrality and the framework-law verdict.
//!
//! Two numbers per law-level node. `in_refs` is the honest simple one and is
//! filled in during the build: how often is this law pointed at. `rank` is
//! PageRank over the citation direction, so mass flows towards the law that is
//! cited, which is the "waar hangt veel van af"-measure the design asks for.

use crate::graph::{CorpusGraph, NodeIx};

/// When a law stops being a law on the map and becomes background.
#[derive(Debug, Clone, Copy)]
pub struct FrameworkRule {
    /// Fraction of all law-level nodes that must cite a law before it counts as
    /// a framework law.
    pub fraction: f32,
    /// Absolute floor, so the rule does not fire on a corpus of twenty laws
    /// where everything cites everything.
    pub min_citers: usize,
}

impl Default for FrameworkRule {
    fn default() -> Self {
        // The design proposes 20% and calls it a guess that has to be adjusted
        // against real data. It has now been measured, and 20% is too high: on
        // the 4.132-law harvested corpus exactly one law clears it (the Awb,
        // cited by 867 laws, 21%), while the next six behave like stars too and
        // sit between 5% and 8% (BW 2, Wetboek van Strafrecht, Wet milieubeheer,
        // Wetboek van Strafvordering, BW 7, Wet IB 2001). 5% catches those and
        // stops well before the material laws, which is the band the design says
        // it wants. The floor keeps the rule quiet on a corpus of twenty laws
        // where everything cites everything.
        Self {
            fraction: 0.05,
            min_citers: 25,
        }
    }
}

/// Fill in `rank` and `framework` for every law-level node.
///
/// Returns the distinct-citer count per law-level node, which is the number the
/// framework verdict is made on and the number worth showing next to the
/// verdict in the UI.
pub fn compute(graph: &mut CorpusGraph, rule: FrameworkRule) -> Vec<u32> {
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

    let rank = pagerank(graph);
    let max = rank
        .iter()
        .copied()
        .fold(0.0f32, f32::max)
        .max(f32::EPSILON);
    for (ix, node) in graph.nodes[..n].iter_mut().enumerate() {
        node.rank = rank[ix] / max;
    }

    let threshold = ((n as f32 * rule.fraction) as usize).max(rule.min_citers);
    let mut framework = 0;
    for (ix, node) in graph.nodes[..n].iter_mut().enumerate() {
        node.framework = citers[ix] as usize >= threshold;
        if node.framework {
            framework += 1;
        }
    }
    graph.stats.framework_laws = framework;
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

/// The heaviest stars, most-cited first. Reporting material, and the input to
/// any manual override of the framework list.
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
    use crate::testkit::star_graph;

    #[test]
    fn the_star_centre_outranks_its_arms() {
        let mut graph = star_graph(60, 3);
        let citers = compute(&mut graph, FrameworkRule::default());
        let hub = graph.lookup("hub").expect("hub");
        assert_eq!(citers[hub as usize], 60);
        for node in &graph.nodes {
            if node.id != "hub" {
                assert!(node.rank < graph.node(hub).rank);
            }
        }
    }

    #[test]
    fn framework_verdict_needs_both_fraction_and_floor() {
        // Ten laws all citing one hub: the fraction is met, the floor is not,
        // so nothing is a framework law yet.
        let mut small = star_graph(10, 0);
        compute(&mut small, FrameworkRule::default());
        assert_eq!(small.stats.framework_laws, 0);

        let mut large = star_graph(60, 0);
        compute(&mut large, FrameworkRule::default());
        assert_eq!(large.stats.framework_laws, 1);
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
        let citers = compute(&mut graph, FrameworkRule::default());
        assert_eq!(citers[hub as usize], 60);
    }
}
