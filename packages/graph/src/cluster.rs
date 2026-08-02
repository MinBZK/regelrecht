//! Community detection on the weighted law-level graph.
//!
//! The design asks for Leiden. This is Louvain: multi-level greedy modularity,
//! with every source of randomness removed. Nodes are visited in canonical
//! index order, a move is only made on a strict improvement, and ties go to the
//! lowest community index, so there is no seed to fix because there is nothing
//! to seed. That is a deliberate downgrade and it costs Leiden's guarantee that
//! a community is internally connected; the mitigation is that framework laws,
//! which are exactly what produces disconnected communities in Louvain, are
//! taken out before the run and put in their own pseudo-cluster.
//!
//! The weights are the typed layout weights, not the raw citation count. With
//! raw counts the detector finds one community: everything that falls under the
//! Awb.

use std::collections::HashMap;

use crate::graph::{CorpusGraph, NodeIx};

/// Cluster index reserved for the framework layer. Framework laws belong to no
/// community; Leiden or Louvain would assign them one arbitrarily.
pub const FRAMEWORK_CLUSTER: u16 = u16::MAX;

/// Assign a community to every law-level node and return the number of
/// communities found (excluding the framework layer).
pub fn assign(graph: &mut CorpusGraph) -> usize {
    let total = graph.law_node_count;
    let free: Vec<NodeIx> = (0..total as NodeIx)
        .filter(|&ix| !graph.nodes[ix as usize].framework)
        .collect();
    let slot: HashMap<NodeIx, usize> = free.iter().enumerate().map(|(i, &ix)| (ix, i)).collect();

    let n = free.len();
    let mut edges: Vec<(u32, u32, f64)> = Vec::new();
    for edge in &graph.edges[..graph.law_edge_count] {
        let (Some(&a), Some(&b)) = (slot.get(&edge.source), slot.get(&edge.target)) else {
            continue;
        };
        if a == b {
            continue;
        }
        let w = (edge.edge_type.layout_weight() as f64) * (1.0 + (edge.count as f64).ln());
        edges.push((a.min(b) as u32, a.max(b) as u32, w));
    }
    edges.sort_by(|x, y| x.0.cmp(&y.0).then_with(|| x.1.cmp(&y.1)));
    let mut merged: Vec<(u32, u32, f64)> = Vec::with_capacity(edges.len());
    for e in edges {
        match merged.last_mut() {
            Some(last) if last.0 == e.0 && last.1 == e.1 => last.2 += e.2,
            _ => merged.push(e),
        }
    }

    let labels = louvain(n, &merged);
    let mut relabel: HashMap<usize, u16> = HashMap::new();
    for (&ix, &label) in free.iter().zip(labels.iter()) {
        let next = relabel.len() as u16;
        let cluster = *relabel.entry(label).or_insert(next);
        graph.nodes[ix as usize].cluster = cluster;
    }
    for node in graph.nodes[..total].iter_mut() {
        if node.framework {
            node.cluster = FRAMEWORK_CLUSTER;
        }
    }
    // An article belongs to whatever its law belongs to; the containment tree
    // has no say of its own here.
    for ix in total..graph.nodes.len() {
        if let Some(parent) = graph.nodes[ix].parent {
            graph.nodes[ix].cluster = graph.nodes[parent as usize].cluster;
        }
    }
    let count = relabel.len();
    graph.stats.clusters = count;
    count
}

/// Multi-level Louvain. Returns a community label per node.
fn louvain(n: usize, edges: &[(u32, u32, f64)]) -> Vec<usize> {
    let mut mapping: Vec<usize> = (0..n).collect();
    let mut level_n = n;
    let mut level_edges = edges.to_vec();

    for _ in 0..10 {
        let labels = local_moving(level_n, &level_edges);
        let mut relabel: HashMap<usize, usize> = HashMap::new();
        let mut compact = vec![0usize; level_n];
        for (i, &label) in labels.iter().enumerate() {
            let next = relabel.len();
            compact[i] = *relabel.entry(label).or_insert(next);
        }
        let communities = relabel.len();
        if communities == level_n {
            break;
        }
        for m in mapping.iter_mut() {
            *m = compact[*m];
        }
        // Collapse each community into one node and keep going one level up.
        // The internal weight of a community becomes a self-loop on the
        // collapsed node. Dropping it instead is a subtle and fatal mistake:
        // the collapsed node then looks weightless, joining a neighbour always
        // looks profitable, and the next level merges everything back into one
        // community.
        let mut agg: HashMap<(usize, usize), f64> = HashMap::new();
        for &(a, b, w) in &level_edges {
            let (ca, cb) = (compact[a as usize], compact[b as usize]);
            *agg.entry((ca.min(cb), ca.max(cb))).or_insert(0.0) += w;
        }
        let mut next_edges: Vec<(u32, u32, f64)> = agg
            .into_iter()
            .map(|((a, b), w)| (a as u32, b as u32, w))
            .collect();
        next_edges.sort_by(|x, y| x.0.cmp(&y.0).then_with(|| x.1.cmp(&y.1)));
        level_edges = next_edges;
        level_n = communities;
    }
    mapping
}

/// One level of local moving: repeatedly offer every node the chance to join a
/// neighbour's community, take the strictly best improvement, stop when a pass
/// changes nothing.
fn local_moving(n: usize, edges: &[(u32, u32, f64)]) -> Vec<usize> {
    if n == 0 {
        return Vec::new();
    }
    let mut adj: Vec<Vec<(u32, f64)>> = vec![Vec::new(); n];
    let mut degree = vec![0.0f64; n];
    let mut total = 0.0f64;
    for &(a, b, w) in edges {
        if a == b {
            // A self-loop is weight the node already holds inside itself. It
            // counts twice towards the degree and once towards the total, and
            // it is not a link to anywhere, so it never enters `adj`.
            degree[a as usize] += 2.0 * w;
            total += w;
            continue;
        }
        adj[a as usize].push((b, w));
        adj[b as usize].push((a, w));
        degree[a as usize] += w;
        degree[b as usize] += w;
        total += w;
    }
    for list in adj.iter_mut() {
        list.sort_by_key(|x| x.0);
    }
    if total <= 0.0 {
        return (0..n).collect();
    }
    let m2 = 2.0 * total;

    let mut community: Vec<usize> = (0..n).collect();
    let mut community_degree = degree.clone();

    for _ in 0..30 {
        let mut moved = false;
        for node in 0..n {
            let own = community[node];
            let mut links: HashMap<usize, f64> = HashMap::new();
            for &(other, w) in &adj[node] {
                *links.entry(community[other as usize]).or_insert(0.0) += w;
            }
            community_degree[own] -= degree[node];
            let own_link = links.get(&own).copied().unwrap_or(0.0);
            let mut best = own;
            let mut best_gain = own_link - community_degree[own] * degree[node] / m2;
            // Candidates in ascending community index: the tie-break is the
            // whole reason this is reproducible.
            let mut candidates: Vec<usize> = links.keys().copied().collect();
            candidates.sort_unstable();
            for candidate in candidates {
                if candidate == own {
                    continue;
                }
                let gain = links[&candidate] - community_degree[candidate] * degree[node] / m2;
                if gain > best_gain + 1e-12 {
                    best_gain = gain;
                    best = candidate;
                }
            }
            community_degree[best] += degree[node];
            if best != own {
                community[node] = best;
                moved = true;
            }
        }
        if !moved {
            break;
        }
    }
    community
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testkit::three_communities;

    #[test]
    fn finds_the_planted_communities() {
        let mut graph = three_communities(12);
        crate::metrics::compute(&mut graph, crate::metrics::FrameworkRule::default());
        let count = assign(&mut graph);
        assert!(
            (2..=5).contains(&count),
            "verwacht ongeveer drie gemeenschappen, kreeg {count}"
        );
        // Members of one planted community must land together.
        let first = graph
            .lookup("c0_l0")
            .map(|ix| graph.node(ix).cluster)
            .expect("c0_l0");
        for i in 1..12 {
            let ix = graph.lookup(&format!("c0_l{i}")).expect("lid");
            assert_eq!(graph.node(ix).cluster, first);
        }
    }

    #[test]
    fn clustering_is_reproducible() {
        let mut a = three_communities(12);
        let mut b = three_communities(12);
        crate::metrics::compute(&mut a, crate::metrics::FrameworkRule::default());
        crate::metrics::compute(&mut b, crate::metrics::FrameworkRule::default());
        assign(&mut a);
        assign(&mut b);
        let ca: Vec<u16> = a.nodes.iter().map(|n| n.cluster).collect();
        let cb: Vec<u16> = b.nodes.iter().map(|n| n.cluster).collect();
        assert_eq!(ca, cb);
    }
}
