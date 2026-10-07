//! Reading order: nodes grouped into layers by what they call.
//!
//! Layer 0 calls nothing else in the graph; a node in layer N calls something
//! in layer N-1 and nothing above it. Nodes that call each other (directly or
//! round a longer loop) form one component and share a layer, so the order is
//! defined even where the calls are not a hierarchy.

use std::collections::{BTreeMap, BTreeSet};

/// Strongly connected components of `deps` (node -> what it calls), each sorted,
/// the list sorted by first member. Iterative Tarjan, so a deep call chain
/// cannot overflow the stack.
pub fn components(deps: &BTreeMap<String, BTreeSet<String>>) -> Vec<Vec<String>> {
    let nodes: Vec<&String> = deps.keys().collect();
    let pos: BTreeMap<&String, usize> = nodes.iter().enumerate().map(|(i, n)| (*n, i)).collect();
    let succ: Vec<Vec<usize>> = nodes
        .iter()
        .map(|n| {
            deps[*n]
                .iter()
                .filter_map(|t| pos.get(t).copied())
                .collect()
        })
        .collect();

    let n = nodes.len();
    let mut index = vec![usize::MAX; n];
    let mut low = vec![0; n];
    let mut on_stack = vec![false; n];
    let mut stack = Vec::new();
    let mut next = 0;
    let mut out: Vec<Vec<String>> = Vec::new();

    for root in 0..n {
        if index[root] != usize::MAX {
            continue;
        }
        // Each frame: the node and how far through its successors it is.
        let mut call: Vec<(usize, usize)> = vec![(root, 0)];
        index[root] = next;
        low[root] = next;
        next += 1;
        stack.push(root);
        on_stack[root] = true;
        while let Some(&mut (v, ref mut i)) = call.last_mut() {
            if let Some(&w) = succ[v].get(*i) {
                *i += 1;
                if index[w] == usize::MAX {
                    index[w] = next;
                    low[w] = next;
                    next += 1;
                    stack.push(w);
                    on_stack[w] = true;
                    call.push((w, 0));
                } else if on_stack[w] {
                    low[v] = low[v].min(index[w]);
                }
            } else {
                call.pop();
                if let Some(&(parent, _)) = call.last() {
                    low[parent] = low[parent].min(low[v]);
                }
                if low[v] == index[v] {
                    let mut comp = Vec::new();
                    while let Some(w) = stack.pop() {
                        on_stack[w] = false;
                        comp.push(nodes[w].clone());
                        if w == v {
                            break;
                        }
                    }
                    comp.sort();
                    out.push(comp);
                }
            }
        }
    }
    out.sort();
    out
}

/// The components of `deps` in layers; see the module docs.
pub fn layers(deps: &BTreeMap<String, BTreeSet<String>>) -> Vec<Vec<Vec<String>>> {
    let comps = components(deps);
    let comp_of: BTreeMap<&str, usize> = comps
        .iter()
        .enumerate()
        .flat_map(|(i, c)| c.iter().map(move |m| (m.as_str(), i)))
        .collect();
    // The condensation is a DAG; give each component 1 + the highest layer it
    // calls into, resolving in an order where callees come first.
    let succ: Vec<BTreeSet<usize>> = comps
        .iter()
        .enumerate()
        .map(|(i, c)| {
            c.iter()
                .flat_map(|m| deps[m].iter())
                .filter_map(|t| comp_of.get(t.as_str()).copied())
                .filter(|&j| j != i)
                .collect()
        })
        .collect();
    let mut level: Vec<Option<usize>> = vec![None; comps.len()];
    for start in 0..comps.len() {
        let mut work = vec![start];
        while let Some(&c) = work.last() {
            if level[c].is_some() {
                work.pop();
                continue;
            }
            let pending: Vec<usize> = succ[c]
                .iter()
                .copied()
                .filter(|&j| level[j].is_none())
                .collect();
            if pending.is_empty() {
                let l = succ[c]
                    .iter()
                    .filter_map(|&j| level[j])
                    .max()
                    .map_or(0, |m| m + 1);
                level[c] = Some(l);
                work.pop();
            } else {
                work.extend(pending);
            }
        }
    }
    let max = level.iter().flatten().copied().max().unwrap_or(0);
    let mut out: Vec<Vec<Vec<String>>> =
        vec![Vec::new(); if comps.is_empty() { 0 } else { max + 1 }];
    for (i, c) in comps.into_iter().enumerate() {
        out[level[i].unwrap_or(0)].push(c);
    }
    out
}

/// Layer number per node, from [`layers`].
pub fn layer_of(layers: &[Vec<Vec<String>>]) -> BTreeMap<String, usize> {
    layers
        .iter()
        .enumerate()
        .flat_map(|(i, l)| l.iter().flatten().map(move |n| (n.clone(), i)))
        .collect()
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    fn deps(edges: &[(&str, &str)], nodes: &[&str]) -> BTreeMap<String, BTreeSet<String>> {
        let mut d: BTreeMap<String, BTreeSet<String>> = nodes
            .iter()
            .map(|n| (n.to_string(), BTreeSet::new()))
            .collect();
        for (a, b) in edges {
            d.entry(a.to_string()).or_default().insert(b.to_string());
            d.entry(b.to_string()).or_default();
        }
        d
    }

    #[test]
    fn callees_come_before_their_callers() {
        let l = layer_of(&layers(&deps(
            &[("app", "engine"), ("engine", "model")],
            &[],
        )));
        assert_eq!((l["model"], l["engine"], l["app"]), (0, 1, 2));
    }

    #[test]
    fn a_node_sits_one_above_its_highest_callee() {
        let l = layer_of(&layers(&deps(&[("a", "b"), ("b", "c"), ("a", "c")], &[])));
        assert_eq!((l["c"], l["b"], l["a"]), (0, 1, 2));
    }

    #[test]
    fn nodes_that_call_each_other_share_a_layer() {
        let ls = layers(&deps(
            &[("a", "b"), ("b", "a"), ("top", "a"), ("b", "base")],
            &[],
        ));
        assert!(ls[1].contains(&vec!["a".to_string(), "b".to_string()]));
        assert_eq!(layer_of(&ls)["top"], 2);
    }

    #[test]
    fn isolated_nodes_are_in_layer_0_and_empty_input_gives_no_layers() {
        assert_eq!(layer_of(&layers(&deps(&[], &["alone"])))["alone"], 0);
        assert!(layers(&BTreeMap::new()).is_empty());
    }

    #[test]
    fn a_very_long_chain_does_not_overflow_the_stack() {
        let names: Vec<String> = (0..20_000).map(|i| format!("f{i:05}")).collect();
        let edges: Vec<(&str, &str)> = names
            .windows(2)
            .map(|w| (w[0].as_str(), w[1].as_str()))
            .collect();
        let l = layer_of(&layers(&deps(&edges, &[])));
        assert_eq!(l["f00000"], 19_999);
        assert_eq!(l["f19999"], 0);
    }

    #[test]
    fn the_result_does_not_depend_on_insertion_order() {
        let a = layers(&deps(&[("x", "y"), ("y", "z"), ("w", "y")], &[]));
        let b = layers(&deps(&[("w", "y"), ("y", "z"), ("x", "y")], &[]));
        assert_eq!(a, b);
    }
}
