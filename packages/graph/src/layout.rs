//! The layout: spectral initialisation, then ForceAtlas2 with Barnes-Hut.
//!
//! The hard requirement is that the same law is in the same place tomorrow. A
//! force layout started from random positions cannot meet that, so the starting
//! positions come from the graph itself: the three eigenvectors belonging to
//! the smallest non-trivial eigenvalues of the graph Laplacian. That is
//! determined by the graph up to the sign of each eigenvector, and the sign is
//! pinned by a canonical rule (the component with the largest absolute value
//! is made positive; ties go to the lowest node index, and the node indices are
//! themselves canonical because [`crate::graph::CorpusGraph::canonicalise`]
//! sorted them by their stable id).
//!
//! Everything after that is deterministic arithmetic in a fixed order: no RNG,
//! no hash-map iteration, no threads. Two runs over the same corpus produce
//! bit-identical coordinates, and that is asserted in the tests.
//!
//! The one place the design's recipe had to be extended is
//! [`initial_positions`]. Spectral-then-ForceAtlas2 over the whole graph in one
//! go was measured on the harvested corpus and it does not separate the
//! communities: three eigenvectors cannot hold fifteen groups apart, and the
//! communities came out completely interleaved even though 71% of all citations
//! stay inside one. The initialisation therefore runs the same two steps twice,
//! once on the community graph and once inside each community, and composes
//! them. That is the design's own navigation hierarchy (rechtsgebied → wet)
//! turned into a layout order.
//!
//! The pipeline for one connected component:
//!
//! 1. [`equilibrium_scale`] — where the forces want this graph to settle.
//! 2. [`initial_positions`] — communities placed, then laws inside them.
//! 3. [`force_atlas2`] — refinement, with hub damping and community attraction.
//! 4. [`recentre`] — local frame, so components can be placed side by side.
//!
//! Framework laws never enter any of this; they are parked on a ring above the
//! field by [`place_framework_ring`].

// Three-component vector arithmetic reads better with an explicit axis index
// than with zipped iterators: `for k in 0..3 { a[k] += b[k] }` is the standard
// notation and the alternative obscures which axis is which.
#![allow(clippy::needless_range_loop)]

use std::collections::HashMap;

use crate::graph::{CorpusGraph, NodeIx, NodeKind};

#[derive(Debug, Clone)]
pub struct LayoutOptions {
    /// ForceAtlas2 iterations on the law-level graph.
    pub iterations: usize,
    /// Barnes-Hut opening angle. Higher is faster and coarser.
    pub theta: f32,
    /// Repulsion strength.
    pub repulsion: f32,
    /// Pull towards the origin, keeps disconnected material from drifting off.
    pub gravity: f32,
    /// Divide attraction by the degree of the pulling end. This is the
    /// `dissuade hubs` mode: without it a law with a thousand citations pulls a
    /// thousand times as hard and the picture becomes a ball around it.
    pub dissuade_hubs: bool,
    /// Power-iteration steps per spectral eigenvector.
    pub spectral_steps: usize,
    /// Radius multiplier for the ring the framework laws are parked on.
    pub framework_ring: f32,
    /// Logarithmic attraction (`w·ln(1+d)`) instead of linear (`w·d`).
    pub linlog: bool,
    /// Damp the repulsion mass logarithmically instead of using the raw degree.
    pub log_mass: bool,
    /// Spring stiffness pulling a law towards the centre of its community.
    ///
    /// The design asks for this in so many words: the community partition goes
    /// into the force step as an extra attraction, so laws from one community
    /// end up spatially together without being put in a hard box. Without it
    /// the communities are in the edges and not in the coordinates: measured on
    /// the harvested corpus, the partition has modularity 0.61 and the layout
    /// still interleaves it completely.
    pub cluster_pull: f32,
}

impl Default for LayoutOptions {
    fn default() -> Self {
        Self {
            iterations: 300,
            theta: 1.2,
            repulsion: 1.0,
            gravity: 1.0,
            dissuade_hubs: true,
            spectral_steps: 240,
            framework_ring: 1.15,
            linlog: true,
            log_mass: true,
            cluster_pull: 0.02,
        }
    }
}

/// The undirected weighted graph the layout actually sees.
///
/// Direction is dropped (force layouts do not use it; the renderer draws it
/// back), parallel edges of different types are merged into one weight, and the
/// weight of a type is what decides how hard it pulls.
struct Mechanical {
    /// `adj[i]` holds `(neighbour, weight)` pairs, sorted by neighbour.
    adj: Vec<Vec<(u32, f32)>>,
    /// Node mass, used for both repulsion and the adaptive speed.
    mass: Vec<f32>,
    /// Community index per node, compacted to `0..cluster_count`.
    cluster: Vec<u16>,
    cluster_count: usize,
}

impl Mechanical {
    fn len(&self) -> usize {
        self.adj.len()
    }
}

/// Compute positions for every node and write them into the graph.
///
/// Law-level nodes get global coordinates. Article nodes get coordinates local
/// to their parent law, so the inside of a law looks the same wherever the law
/// ends up on the map.
pub fn apply(graph: &mut CorpusGraph, opts: &LayoutOptions) {
    layout_law_level(graph, opts);
    layout_articles(graph);
}

fn layout_law_level(graph: &mut CorpusGraph, opts: &LayoutOptions) {
    let total = graph.law_node_count;
    if total == 0 {
        return;
    }

    // Framework laws are taken out of the force computation entirely. Damped
    // edge weights and dissuade-hubs both help and neither is enough: at a
    // thousand incoming citations the Awb still flattens the picture. They come
    // back on their own ring afterwards.
    let free: Vec<NodeIx> = (0..total as NodeIx)
        .filter(|&ix| !graph.nodes[ix as usize].framework)
        .collect();
    let slot: HashMap<NodeIx, usize> = free.iter().enumerate().map(|(i, &ix)| (ix, i)).collect();

    let mech = mechanical(graph, &free, &slot, opts);
    let components = components(&mech);

    // One component at a time, largest first. A component is laid out in its
    // own local frame and then placed; that keeps a small disconnected law from
    // being flung to the far side of the universe by gravity alone.
    let mut positions = vec![[0.0f32; 3]; mech.len()];
    let mut placed: Vec<(Vec<usize>, f32)> = Vec::new();
    for members in &components {
        let sub = subgraph(&mech, members);
        let scale = equilibrium_scale(&sub, opts);
        let mut pos = initial_positions(&sub, opts, scale);
        force_atlas2(&sub, &mut pos, opts, scale);
        let radius = recentre(&mut pos);
        for (local, &global) in members.iter().enumerate() {
            positions[global] = pos[local];
        }
        placed.push((members.clone(), radius));
    }

    // Components are arranged on a Fibonacci sphere around the largest one,
    // which sits at the origin. Deterministic, and it does not pretend there is
    // a relation between components where there is none.
    let main_radius = placed.first().map(|(_, r)| *r).unwrap_or(1.0).max(1.0);
    let mut shell = main_radius * 1.6;
    for (rank, (members, radius)) in placed.iter().enumerate().skip(1) {
        let dir = fibonacci_point(rank, placed.len());
        shell += radius * 0.4;
        let centre = [dir[0] * shell, dir[1] * shell, dir[2] * shell];
        for &global in members {
            positions[global][0] += centre[0];
            positions[global][1] += centre[1];
            positions[global][2] += centre[2];
        }
        shell += radius * 0.4;
    }

    for (&ix, &pos) in free.iter().zip(positions.iter()) {
        let node = &mut graph.nodes[ix as usize];
        node.x = pos[0];
        node.y = pos[1];
        node.z = pos[2];
    }

    place_framework_ring(graph, opts);
}

/// Framework laws go on a ring above the plane, ordered by how heavily they are
/// cited.
///
/// This is not only a graphical dodge. A framework law sits above the material
/// laws in the legal hierarchy, and a ring above the field is what that
/// relation looks like. Whoever selects the Awb sees its edges fan downwards
/// into the clusters underneath, which is the picture someone should carry of
/// the Awb.
fn place_framework_ring(graph: &mut CorpusGraph, opts: &LayoutOptions) {
    let mut ring: Vec<NodeIx> = (0..graph.law_node_count as NodeIx)
        .filter(|&ix| graph.nodes[ix as usize].framework)
        .collect();
    if ring.is_empty() {
        return;
    }
    // The extent of everything already placed, so the ring clears the whole
    // field and not just the component that happened to be laid out first.
    let main_radius = graph.nodes[..graph.law_node_count]
        .iter()
        .filter(|n| !n.framework)
        .map(|n| (n.x * n.x + n.y * n.y + n.z * n.z).sqrt())
        .fold(1.0f32, f32::max);
    ring.sort_by(|&a, &b| {
        let (na, nb) = (&graph.nodes[a as usize], &graph.nodes[b as usize]);
        nb.in_refs.cmp(&na.in_refs).then_with(|| na.id.cmp(&nb.id))
    });
    let radius = main_radius * opts.framework_ring;
    // Clear of the field, not merely offset from it: the largest distance from
    // the centroid bounds how high a law in the field can sit, so the ring goes
    // above that.
    let height = main_radius * 1.25;
    let count = ring.len() as f32;
    for (i, &ix) in ring.iter().enumerate() {
        let angle = std::f32::consts::TAU * (i as f32) / count;
        let node = &mut graph.nodes[ix as usize];
        node.x = radius * angle.cos();
        node.y = height;
        node.z = radius * angle.sin();
    }
}

/// Articles are laid out inside their own law, in local coordinates.
///
/// Until the schema carries chapter and section containers there is no real
/// structure to follow, so this is a Fibonacci sphere in article order: stable,
/// evenly spread, and the same for a law wherever the law sits. The radius
/// grows as the cube root of the article count so the density stays constant.
fn layout_articles(graph: &mut CorpusGraph) {
    let mut per_law: HashMap<NodeIx, Vec<NodeIx>> = HashMap::new();
    for ix in graph.law_node_count..graph.nodes.len() {
        if let Some(parent) = graph.nodes[ix].parent {
            per_law.entry(parent).or_default().push(ix as NodeIx);
        }
    }
    for (_, members) in per_law.iter() {
        let n = members.len();
        let radius = 1.2 * (n as f32).cbrt();
        for (i, &ix) in members.iter().enumerate() {
            let dir = fibonacci_point(i, n);
            let node = &mut graph.nodes[ix as usize];
            node.x = dir[0] * radius;
            node.y = dir[1] * radius;
            node.z = dir[2] * radius;
        }
    }
}

/// The distance two connected nodes settle at, derived from the forces rather
/// than guessed.
///
/// Attraction on an edge is `w·d/m`, repulsion between two nodes is `m²/d`, so
/// a connected pair balances at `d = m^1.5 / sqrt(w)`. Getting this right
/// matters more than it looks: ForceAtlas2 started far below its own
/// equilibrium spends its whole budget inflating, overshoots, and ends up in an
/// oscillation that never settles. Starting the spectral embedding at the right
/// scale turns the force step into what it should be, a refinement.
fn equilibrium_scale(mech: &Mechanical, opts: &LayoutOptions) -> f32 {
    let n = mech.len().max(1);
    let mean_mass = mech.mass.iter().sum::<f32>() / n as f32;
    let edges: usize = mech.adj.iter().map(|list| list.len()).sum();
    let mean_weight = if edges == 0 {
        1.0
    } else {
        mech.adj
            .iter()
            .flat_map(|list| list.iter().map(|&(_, w)| w))
            .sum::<f32>()
            / edges as f32
    };
    let mass = if opts.dissuade_hubs { mean_mass } else { 1.0 };
    let repel = mean_mass * mean_mass * mass / mean_weight.max(1e-3);
    // Linear attraction balances repulsion at `d = sqrt(repel)`. Logarithmic
    // attraction balances much further out, because `ln(1+d)` barely grows: the
    // equation is `d·ln(1+d) = repel`, and the fixed point below is the two-step
    // Newton solution of it. Getting this within a factor of two is enough; it
    // only has to put the spectral embedding in the right neighbourhood.
    let pair = if opts.linlog {
        let mut d = repel.max(1.0);
        for _ in 0..40 {
            d = repel / (1.0 + d).ln().max(0.5);
        }
        d
    } else {
        repel.sqrt()
    };
    (pair * (n as f32).cbrt() * 0.5).max(1.0)
}

/// Evenly spread points on a sphere, index `i` of `n`. Pure function of the
/// two integers, so it never introduces a source of variation.
fn fibonacci_point(i: usize, n: usize) -> [f32; 3] {
    let n = n.max(1) as f32;
    let i = i as f32;
    let y = 1.0 - 2.0 * (i + 0.5) / n;
    let r = (1.0 - y * y).max(0.0).sqrt();
    let golden = std::f32::consts::PI * (3.0 - 5.0f32.sqrt());
    let theta = golden * i;
    [r * theta.cos(), y, r * theta.sin()]
}

/// Turn the typed, directed, aggregated edge set into one undirected weighted
/// graph.
///
/// Two dampings apply here and they answer different problems. The type weight
/// says a computed dependency means more than a mention. The logarithm on the
/// count says that citing a law forty times is more than citing it once but not
/// forty times more, which stops a definition article that repeats one
/// reference in every lid from outweighing a real structural relation.
fn mechanical(
    graph: &CorpusGraph,
    free: &[NodeIx],
    slot: &HashMap<NodeIx, usize>,
    opts: &LayoutOptions,
) -> Mechanical {
    let n = free.len();
    let mut weights: Vec<HashMap<u32, f32>> = vec![HashMap::new(); n];
    for edge in &graph.edges[..graph.law_edge_count] {
        let (Some(&a), Some(&b)) = (slot.get(&edge.source), slot.get(&edge.target)) else {
            continue;
        };
        if a == b {
            continue;
        }
        let w = edge.edge_type.layout_weight() * (1.0 + (edge.count as f32).ln());
        *weights[a].entry(b as u32).or_insert(0.0) += w;
        *weights[b].entry(a as u32).or_insert(0.0) += w;
    }

    let mut clusters: HashMap<u16, u16> = HashMap::new();
    let mut cluster = Vec::with_capacity(n);
    for &ix in free {
        let raw = graph.nodes[ix as usize].cluster;
        let next = clusters.len() as u16;
        cluster.push(*clusters.entry(raw).or_insert(next));
    }
    let cluster_count = clusters.len();

    let mut adj: Vec<Vec<(u32, f32)>> = Vec::with_capacity(n);
    let mut mass = vec![1.0f32; n];
    for (i, map) in weights.into_iter().enumerate() {
        let mut list: Vec<(u32, f32)> = map.into_iter().collect();
        list.sort_by_key(|&(j, _)| j);
        mass[i] = if opts.log_mass {
            1.0 + (1.0 + list.len() as f32).ln()
        } else {
            1.0 + list.len() as f32
        };
        adj.push(list);
    }
    Mechanical {
        adj,
        mass,
        cluster,
        cluster_count,
    }
}

/// Connected components, largest first, ties by lowest member index.
fn components(mech: &Mechanical) -> Vec<Vec<usize>> {
    let n = mech.len();
    let mut seen = vec![false; n];
    let mut out: Vec<Vec<usize>> = Vec::new();
    for start in 0..n {
        if seen[start] {
            continue;
        }
        let mut stack = vec![start];
        seen[start] = true;
        let mut members = Vec::new();
        while let Some(cur) = stack.pop() {
            members.push(cur);
            for &(next, _) in &mech.adj[cur] {
                if !seen[next as usize] {
                    seen[next as usize] = true;
                    stack.push(next as usize);
                }
            }
        }
        members.sort_unstable();
        out.push(members);
    }
    out.sort_by(|a, b| b.len().cmp(&a.len()).then_with(|| a[0].cmp(&b[0])));
    out
}

fn subgraph(mech: &Mechanical, members: &[usize]) -> Mechanical {
    let mut local: HashMap<usize, u32> = HashMap::with_capacity(members.len());
    for (i, &m) in members.iter().enumerate() {
        local.insert(m, i as u32);
    }
    let mut adj = Vec::with_capacity(members.len());
    let mut mass = Vec::with_capacity(members.len());
    let mut cluster = Vec::with_capacity(members.len());
    for &m in members {
        cluster.push(mech.cluster[m]);
        let mut list: Vec<(u32, f32)> = mech.adj[m]
            .iter()
            .filter_map(|&(j, w)| local.get(&(j as usize)).map(|&lj| (lj, w)))
            .collect();
        list.sort_by_key(|&(j, _)| j);
        adj.push(list);
        mass.push(mech.mass[m]);
    }
    Mechanical {
        adj,
        mass,
        cluster,
        cluster_count: mech.cluster_count,
    }
}

/// Starting positions for one component: communities first, laws inside them.
///
/// A plain spectral embedding of the whole component does not work here, and
/// the harvested corpus says so with a number. Its partition has modularity
/// 0.61 and 71% of all citations stay inside a community, yet embedding the
/// component in three dimensions in one go interleaves the communities
/// completely: the community centroids end up 53 units apart while each
/// community spreads over 239. Three eigenvectors simply cannot keep fifteen
/// groups apart, and no amount of extra attraction repairs it afterwards,
/// because pulling every law towards its own centroid does not move the
/// centroids apart.
///
/// So the layout is built the way the design describes the navigation: the
/// community graph is laid out first, each community is laid out inside itself,
/// and the two are composed. The force step afterwards refines that instead of
/// having to discover it.
fn initial_positions(mech: &Mechanical, opts: &LayoutOptions, scale: f32) -> Vec<[f32; 3]> {
    let n = mech.len();
    let mut groups: Vec<(u16, Vec<usize>)> = {
        let mut by_cluster: HashMap<u16, Vec<usize>> = HashMap::new();
        for (i, &c) in mech.cluster.iter().enumerate() {
            by_cluster.entry(c).or_default().push(i);
        }
        by_cluster.into_iter().collect()
    };
    groups.sort_by_key(|(c, _)| *c);
    if groups.len() < 2 || n < 8 {
        return spectral_init(mech, opts.spectral_steps, scale);
    }

    // The community graph: one node per community, edge weights summed.
    let index: HashMap<u16, usize> = groups
        .iter()
        .enumerate()
        .map(|(i, (c, _))| (*c, i))
        .collect();
    let mut coarse_weights: Vec<HashMap<u32, f32>> = vec![HashMap::new(); groups.len()];
    for i in 0..n {
        let a = index[&mech.cluster[i]];
        for &(j, w) in &mech.adj[i] {
            let b = index[&mech.cluster[j as usize]];
            if a == b {
                continue;
            }
            *coarse_weights[a].entry(b as u32).or_insert(0.0) += w;
        }
    }
    let mut coarse_adj = Vec::with_capacity(groups.len());
    let mut coarse_mass = Vec::with_capacity(groups.len());
    for (i, map) in coarse_weights.into_iter().enumerate() {
        let mut list: Vec<(u32, f32)> = map.into_iter().collect();
        list.sort_by_key(|&(j, _)| j);
        // Same logarithmic damping as at law level: a community of 577 laws
        // must not repel 577 times as hard as one of five.
        coarse_mass.push(1.0 + (1.0 + groups[i].1.len() as f32).ln());
        coarse_adj.push(list);
    }
    let coarse = Mechanical {
        adj: coarse_adj,
        mass: coarse_mass,
        cluster: (0..groups.len() as u16).collect(),
        cluster_count: groups.len(),
    };

    // The community graph is laid out with the community attraction off: every
    // node there is its own community and the term would be a no-op that only
    // costs time.
    let coarse_opts = LayoutOptions {
        cluster_pull: 0.0,
        ..opts.clone()
    };
    let coarse_scale = equilibrium_scale(&coarse, &coarse_opts);
    let mut centres = spectral_init(&coarse, opts.spectral_steps, coarse_scale);
    force_atlas2(&coarse, &mut centres, &coarse_opts, coarse_scale);
    recentre(&mut centres);

    // Each community inside itself, in its own frame.
    let mut local: Vec<Vec<[f32; 3]>> = Vec::with_capacity(groups.len());
    let mut radius: Vec<f32> = Vec::with_capacity(groups.len());
    for (_, members) in &groups {
        let sub = subgraph(mech, members);
        let sub_scale = equilibrium_scale(&sub, opts);
        let mut pos = spectral_init(&sub, opts.spectral_steps, sub_scale);
        force_atlas2(&sub, &mut pos, &coarse_opts, sub_scale);
        radius.push(recentre(&mut pos));
        local.push(pos);
    }

    // Make room: the community layout has the right shape at the wrong size, so
    // the communities still sit inside one another. Scaling everything by the
    // worst overlap would work and is a bad idea, because two centres that
    // happen to land close together would blow the whole map up by their
    // ratio. Instead the community spheres are pushed apart pairwise until they
    // no longer overlap, which is a local repair and leaves the shape alone.
    separate_spheres(&mut centres, &radius);

    let mut out = vec![[0.0f32; 3]; n];
    for (g, (_, members)) in groups.iter().enumerate() {
        for (local_ix, &member) in members.iter().enumerate() {
            for k in 0..3 {
                out[member][k] = centres[g][k] + local[g][local_ix][k];
            }
        }
    }
    out
}

/// Push overlapping spheres apart until they touch, in a fixed pair order.
///
/// Two centres that start on top of each other are separated along a fixed
/// axis derived from their indices, so even that degenerate case has one
/// answer rather than an arbitrary one.
fn separate_spheres(centres: &mut [[f32; 3]], radius: &[f32]) {
    let n = centres.len();
    let mean_radius = radius.iter().sum::<f32>() / n.max(1) as f32;
    // Normalise the coarse frame first. The community layout is computed in its
    // own units and those units mean nothing here, so the mean centre distance
    // is simply set to a small multiple of the mean community radius. Doing
    // this in both directions matters: too wide is as unreadable as too narrow,
    // and the pairwise repair below can only ever push things further apart.
    let mean_distance = {
        let mut sum = 0.0f32;
        let mut count = 0u32;
        for a in 0..n {
            for b in (a + 1)..n {
                sum += distance(centres[a], centres[b]);
                count += 1;
            }
        }
        (sum / count.max(1) as f32).max(1e-3)
    };
    let expand = 2.4 * mean_radius.max(1e-3) / mean_distance;
    for c in centres.iter_mut() {
        for k in 0..3 {
            c[k] *= expand;
        }
    }

    for _ in 0..400 {
        let mut moved = false;
        for a in 0..n {
            for b in (a + 1)..n {
                let want = 1.15 * (radius[a] + radius[b]);
                let d = distance(centres[a], centres[b]);
                if d >= want {
                    continue;
                }
                let axis = if d > 1e-4 {
                    let mut axis = [0.0f32; 3];
                    for k in 0..3 {
                        axis[k] = (centres[a][k] - centres[b][k]) / d;
                    }
                    axis
                } else {
                    fibonacci_point(a * n + b, n * n)
                };
                let push = 0.5 * (want - d);
                for k in 0..3 {
                    centres[a][k] += axis[k] * push;
                    centres[b][k] -= axis[k] * push;
                }
                moved = true;
            }
        }
        if !moved {
            break;
        }
    }
}

fn distance(a: [f32; 3], b: [f32; 3]) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

/// Spectral embedding: the three eigenvectors belonging to the smallest
/// non-trivial eigenvalues of `L = D - W`.
///
/// Found by power iteration on `M = cI - L`, whose largest eigenvalues are `L`'s
/// smallest. The constant vector is `L`'s null vector on a connected component
/// and is deflated away first; each further vector is re-orthogonalised against
/// everything already found at every step, which is what keeps the three
/// directions from collapsing onto each other.
///
/// The starting vector is a fixed function of the node index (splitmix64), not
/// a random draw. That is the difference between "deterministic" and "usually
/// the same".
fn spectral_init(mech: &Mechanical, steps: usize, scale: f32) -> Vec<[f32; 3]> {
    let n = mech.len();
    let mut out = vec![[0.0f32; 3]; n];
    if n <= 3 {
        for (i, p) in out.iter_mut().enumerate() {
            *p = fibonacci_point(i, n);
        }
        return out;
    }

    let degree: Vec<f32> = mech
        .adj
        .iter()
        .map(|list| list.iter().map(|&(_, w)| w).sum())
        .collect();
    let shift = degree.iter().copied().fold(0.0f32, f32::max) * 2.0 + 1.0;

    let constant = vec![1.0f32 / (n as f32).sqrt(); n];
    let mut found: Vec<Vec<f32>> = vec![constant];

    for k in 0..3 {
        let mut v: Vec<f32> = (0..n).map(|i| seeded_unit(i as u64, k as u64)).collect();
        orthogonalise(&mut v, &found);
        normalise(&mut v);
        let mut next = vec![0.0f32; n];
        for _ in 0..steps {
            // next = (shift * I - L) v = shift*v - D v + W v
            for i in 0..n {
                let mut acc = (shift - degree[i]) * v[i];
                for &(j, w) in &mech.adj[i] {
                    acc += w * v[j as usize];
                }
                next[i] = acc;
            }
            std::mem::swap(&mut v, &mut next);
            orthogonalise(&mut v, &found);
            if !normalise(&mut v) {
                // Degenerate: fewer than k+1 informative directions exist.
                // Fall back on a fixed spread rather than on noise.
                for (i, slot) in v.iter_mut().enumerate() {
                    *slot = seeded_unit(i as u64, 97 + k as u64);
                }
                orthogonalise(&mut v, &found);
                if !normalise(&mut v) {
                    break;
                }
            }
        }
        canonical_sign(&mut v);
        found.push(v);
    }

    for i in 0..n {
        for k in 0..3 {
            out[i][k] = found.get(k + 1).map(|v| v[i]).unwrap_or(0.0);
        }
    }
    // The eigenvectors are unit-norm, so their spread is 1/sqrt(n) whatever the
    // graph looks like. Scaling by sqrt(n) turns that into a spread of order 1
    // and the caller's `scale` puts it at the size the forces want.
    let scale = scale * (n as f32).sqrt();
    for p in out.iter_mut() {
        p[0] *= scale;
        p[1] *= scale;
        p[2] *= scale;
    }
    out
}

/// A fixed pseudo-random value in `[-1, 1)` from two integers. splitmix64, used
/// as a hash and never advanced as a stream, so nothing depends on call order.
fn seeded_unit(index: u64, salt: u64) -> f32 {
    let mut z = index
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        .wrapping_add(salt.wrapping_mul(0xBF58_476D_1CE4_E5B9))
        .wrapping_add(0x94D0_49BB_1331_11EB);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    ((z >> 40) as f32 / 8_388_608.0) - 1.0
}

fn orthogonalise(v: &mut [f32], basis: &[Vec<f32>]) {
    for b in basis {
        let dot: f32 = v.iter().zip(b).map(|(a, c)| a * c).sum();
        for (a, c) in v.iter_mut().zip(b) {
            *a -= dot * c;
        }
    }
}

fn normalise(v: &mut [f32]) -> bool {
    let norm: f32 = v.iter().map(|a| a * a).sum::<f32>().sqrt();
    if norm < 1e-12 {
        return false;
    }
    for a in v.iter_mut() {
        *a /= norm;
    }
    true
}

/// Pin the sign of an eigenvector: the largest absolute component is made
/// positive, ties going to the lowest index.
///
/// An eigenvector is only determined up to its sign, so without this rule the
/// map is mirrored roughly half the time and every law is somewhere else
/// tomorrow. This one line is what the stability claim rests on.
fn canonical_sign(v: &mut [f32]) {
    let mut best = 0usize;
    let mut best_abs = 0.0f32;
    for (i, &x) in v.iter().enumerate() {
        if x.abs() > best_abs + 1e-9 {
            best_abs = x.abs();
            best = i;
        }
    }
    if v[best] < 0.0 {
        for x in v.iter_mut() {
            *x = -*x;
        }
    }
}

/// Recentre on the centroid and report the radius. Returned so components can
/// be placed without overlapping.
fn recentre(pos: &mut [[f32; 3]]) -> f32 {
    if pos.is_empty() {
        return 0.0;
    }
    let mut centre = [0.0f32; 3];
    for p in pos.iter() {
        for k in 0..3 {
            centre[k] += p[k];
        }
    }
    for c in centre.iter_mut() {
        *c /= pos.len() as f32;
    }
    let mut radius = 0.0f32;
    for p in pos.iter_mut() {
        for k in 0..3 {
            p[k] -= centre[k];
        }
        radius = radius.max((p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt());
    }
    radius
}

/// ForceAtlas2 in three dimensions with Barnes-Hut repulsion and the adaptive
/// global speed from the Gephi implementation.
fn force_atlas2(mech: &Mechanical, pos: &mut [[f32; 3]], opts: &LayoutOptions, scale: f32) {
    let n = mech.len();
    if n < 2 {
        return;
    }
    let mut force = vec![[0.0f32; 3]; n];
    let mut previous = vec![[0.0f32; 3]; n];
    let mut speed = 1.0f32;
    let jitter = 0.05f32;

    for _ in 0..opts.iterations {
        std::mem::swap(&mut force, &mut previous);
        for f in force.iter_mut() {
            *f = [0.0; 3];
        }

        let tree = Octree::build(pos, &mech.mass);
        for i in 0..n {
            tree.repulsion(
                pos[i],
                mech.mass[i],
                opts.theta,
                opts.repulsion,
                &mut force[i],
            );
        }

        for i in 0..n {
            for &(j, w) in &mech.adj[i] {
                let j = j as usize;
                if j <= i {
                    // Every undirected pair appears twice in `adj`; do the work
                    // once and apply it to both ends.
                    continue;
                }
                let d = [
                    pos[j][0] - pos[i][0],
                    pos[j][1] - pos[i][1],
                    pos[j][2] - pos[i][2],
                ];
                let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt().max(0.01);
                let unit = [d[0] / len, d[1] / len, d[2] / len];
                let reach = if opts.linlog { (1.0 + len).ln() } else { len };
                let pull_i = if opts.dissuade_hubs {
                    w * reach / mech.mass[i]
                } else {
                    w * reach
                };
                let pull_j = if opts.dissuade_hubs {
                    w * reach / mech.mass[j]
                } else {
                    w * reach
                };
                for k in 0..3 {
                    force[i][k] += unit[k] * pull_i;
                    force[j][k] -= unit[k] * pull_j;
                }
            }
        }

        for i in 0..n {
            let len = (pos[i][0] * pos[i][0] + pos[i][1] * pos[i][1] + pos[i][2] * pos[i][2])
                .sqrt()
                .max(0.01);
            let g = opts.gravity * mech.mass[i] / len;
            for k in 0..3 {
                force[i][k] -= pos[i][k] * g;
            }
        }

        // Community attraction: a linear spring towards the centre of one's own
        // community. The centroid is recomputed every step from the current
        // positions, in node order, so it adds no new source of variation.
        if opts.cluster_pull > 0.0 && mech.cluster_count > 1 {
            let mut centre = vec![[0.0f32; 3]; mech.cluster_count];
            let mut count = vec![0u32; mech.cluster_count];
            for i in 0..n {
                let c = mech.cluster[i] as usize;
                for k in 0..3 {
                    centre[c][k] += pos[i][k];
                }
                count[c] += 1;
            }
            for (c, slot) in centre.iter_mut().enumerate() {
                if count[c] > 0 {
                    for k in 0..3 {
                        slot[k] /= count[c] as f32;
                    }
                }
            }
            for i in 0..n {
                let c = mech.cluster[i] as usize;
                if count[c] < 2 {
                    continue;
                }
                for k in 0..3 {
                    force[i][k] += opts.cluster_pull * (centre[c][k] - pos[i][k]);
                }
            }
        }

        // Adaptive global speed: swinging is how much a node changed its mind
        // since the previous step, traction is how much it kept going. A graph
        // that swings gets slowed down, which is what stops the oscillation a
        // fixed step size produces around hubs.
        let mut total_swing = 0.0f32;
        let mut total_traction = 0.0f32;
        let mut swing = vec![0.0f32; n];
        for i in 0..n {
            let d = [
                force[i][0] - previous[i][0],
                force[i][1] - previous[i][1],
                force[i][2] - previous[i][2],
            ];
            let s = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
            let t = [
                force[i][0] + previous[i][0],
                force[i][1] + previous[i][1],
                force[i][2] + previous[i][2],
            ];
            let tr = 0.5 * (t[0] * t[0] + t[1] * t[1] + t[2] * t[2]).sqrt();
            swing[i] = s;
            total_swing += mech.mass[i] * s;
            total_traction += mech.mass[i] * tr;
        }
        if total_swing > 0.0 {
            let target = jitter * jitter * total_traction / total_swing;
            let max_rise = 1.5 * speed;
            speed = speed + (target - speed).min(max_rise).max(-0.5 * speed);
            speed = speed.clamp(1e-4, 1.0);
        }

        // A hard cap on how far a node may travel in one step. The swing
        // damping alone lets a badly conditioned graph take a step far larger
        // than the structure it is trying to find, and once that happens the
        // layout oscillates outwards instead of settling.
        let max_step = 0.1 * scale;
        for i in 0..n {
            let factor = speed / (1.0 + (speed * swing[i]).sqrt());
            let step = [
                force[i][0] * factor,
                force[i][1] * factor,
                force[i][2] * factor,
            ];
            let len = (step[0] * step[0] + step[1] * step[1] + step[2] * step[2]).sqrt();
            let clamp = if len > max_step { max_step / len } else { 1.0 };
            for k in 0..3 {
                pos[i][k] += step[k] * clamp;
            }
        }
    }
}

/// Barnes-Hut octree over the current positions.
///
/// Flat arrays, children by index, no pointers and no hashing: the traversal
/// order is fixed, so the repulsion sum is summed in the same order every run
/// and floating point rounding cannot make two runs differ.
struct Octree {
    centre: Vec<[f32; 3]>,
    mass: Vec<f32>,
    size: Vec<f32>,
    children: Vec<[i32; 8]>,
    /// Index of the single body in a leaf, `-1` for an internal or empty cell.
    body: Vec<i32>,
}

impl Octree {
    fn build(pos: &[[f32; 3]], mass: &[f32]) -> Octree {
        let mut lo = [f32::MAX; 3];
        let mut hi = [f32::MIN; 3];
        for p in pos {
            for k in 0..3 {
                lo[k] = lo[k].min(p[k]);
                hi[k] = hi[k].max(p[k]);
            }
        }
        let mut size = 0.0f32;
        let mut root = [0.0f32; 3];
        for k in 0..3 {
            size = size.max(hi[k] - lo[k]);
            root[k] = 0.5 * (lo[k] + hi[k]);
        }
        let size = (size * 1.05).max(1.0);

        let mut tree = Octree {
            centre: vec![root],
            mass: vec![0.0],
            size: vec![size],
            children: vec![[-1; 8]],
            body: vec![-1],
        };
        // `centre` starts as the geometric centre of a cell and is turned into
        // the centre of mass as bodies are inserted.
        let mut geom = vec![root];
        for i in 0..pos.len() {
            tree.insert(0, i, pos, mass, &mut geom, 0);
        }
        for i in 0..tree.mass.len() {
            if tree.mass[i] > 0.0 {
                for k in 0..3 {
                    tree.centre[i][k] /= tree.mass[i];
                }
            }
        }
        tree
    }

    fn insert(
        &mut self,
        cell: usize,
        body: usize,
        pos: &[[f32; 3]],
        mass: &[f32],
        geom: &mut Vec<[f32; 3]>,
        depth: u32,
    ) {
        // 24 levels is a metre of resolution on a kilometre of graph; deeper
        // than that means coincident points and splitting further never ends.
        if depth > 24 {
            self.mass[cell] += mass[body];
            for k in 0..3 {
                self.centre[cell][k] += pos[body][k] * mass[body];
            }
            return;
        }
        if self.mass[cell] == 0.0 && self.body[cell] == -1 && self.children[cell][0] == -1 {
            self.body[cell] = body as i32;
            self.mass[cell] = mass[body];
            for k in 0..3 {
                self.centre[cell][k] = pos[body][k] * mass[body];
            }
            return;
        }
        if self.body[cell] >= 0 {
            let existing = self.body[cell] as usize;
            self.body[cell] = -1;
            self.push_down(cell, existing, pos, mass, geom, depth);
        }
        self.mass[cell] += mass[body];
        for k in 0..3 {
            self.centre[cell][k] += pos[body][k] * mass[body];
        }
        self.push_down(cell, body, pos, mass, geom, depth);
    }

    fn push_down(
        &mut self,
        cell: usize,
        body: usize,
        pos: &[[f32; 3]],
        mass: &[f32],
        geom: &mut Vec<[f32; 3]>,
        depth: u32,
    ) {
        let centre = geom[cell];
        let half = self.size[cell] * 0.5;
        let mut octant = 0usize;
        let mut child_centre = [0.0f32; 3];
        for k in 0..3 {
            if pos[body][k] >= centre[k] {
                octant |= 1 << k;
                child_centre[k] = centre[k] + half * 0.5;
            } else {
                child_centre[k] = centre[k] - half * 0.5;
            }
        }
        let child = if self.children[cell][octant] >= 0 {
            self.children[cell][octant] as usize
        } else {
            let ix = self.centre.len();
            self.centre.push([0.0; 3]);
            geom.push(child_centre);
            self.mass.push(0.0);
            self.size.push(half);
            self.children.push([-1; 8]);
            self.body.push(-1);
            self.children[cell][octant] = ix as i32;
            ix
        };
        self.insert(child, body, pos, mass, geom, depth + 1);
    }

    fn repulsion(&self, at: [f32; 3], mass: f32, theta: f32, strength: f32, out: &mut [f32; 3]) {
        let mut stack = vec![0usize];
        while let Some(cell) = stack.pop() {
            if self.mass[cell] == 0.0 {
                continue;
            }
            let d = [
                at[0] - self.centre[cell][0],
                at[1] - self.centre[cell][1],
                at[2] - self.centre[cell][2],
            ];
            let dist2 = d[0] * d[0] + d[1] * d[1] + d[2] * d[2];
            let dist = dist2.sqrt();
            if self.body[cell] >= 0 || self.size[cell] < theta * dist {
                if dist < 1e-6 {
                    continue;
                }
                // FA2 repulsion falls off as 1/d, not 1/d^2: that is what keeps
                // a large graph from collapsing into its own centre.
                let f = strength * mass * self.mass[cell] / dist2;
                out[0] += d[0] * f;
                out[1] += d[1] * f;
                out[2] += d[2] * f;
                continue;
            }
            for &child in &self.children[cell] {
                if child >= 0 {
                    stack.push(child as usize);
                }
            }
        }
    }
}

/// How spread out the law-level layout is: the mean distance to the centroid
/// divided by the largest distance.
///
/// A ball with everything piled around one hub scores low; a layout with
/// separated communities scores high. Used in the tests to assert that damping
/// the framework star actually produces structure instead of a blob.
pub fn dispersion(graph: &CorpusGraph) -> f32 {
    let nodes: Vec<&crate::graph::Node> = graph.nodes[..graph.law_node_count]
        .iter()
        .filter(|n| !n.framework && n.kind != NodeKind::Article)
        .collect();
    if nodes.len() < 2 {
        return 0.0;
    }
    let mut centre = [0.0f32; 3];
    for n in &nodes {
        centre[0] += n.x;
        centre[1] += n.y;
        centre[2] += n.z;
    }
    for c in centre.iter_mut() {
        *c /= nodes.len() as f32;
    }
    let mut sum = 0.0f32;
    let mut max = 0.0f32;
    for n in &nodes {
        let d = ((n.x - centre[0]).powi(2) + (n.y - centre[1]).powi(2) + (n.z - centre[2]).powi(2))
            .sqrt();
        sum += d;
        max = max.max(d);
    }
    if max <= 0.0 {
        return 0.0;
    }
    (sum / nodes.len() as f32) / max
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_sign_flips_towards_the_largest_component() {
        let mut v = vec![0.1, -0.9, 0.3];
        canonical_sign(&mut v);
        assert_eq!(v, vec![-0.1, 0.9, -0.3]);
        let mut w = vec![0.1, 0.9, 0.3];
        canonical_sign(&mut w);
        assert_eq!(w, vec![0.1, 0.9, 0.3]);
    }

    #[test]
    fn seeded_unit_is_a_pure_function() {
        assert_eq!(seeded_unit(7, 1), seeded_unit(7, 1));
        assert_ne!(seeded_unit(7, 1), seeded_unit(8, 1));
        assert!((-1.0..1.0).contains(&seeded_unit(12345, 2)));
    }

    #[test]
    fn spectral_separates_two_cliques() {
        // Two four-cliques joined by one edge. The first non-trivial
        // eigenvector is the classic separator: it must have opposite signs on
        // the two halves.
        let mut adj: Vec<Vec<(u32, f32)>> = vec![Vec::new(); 8];
        let link = |a: usize, b: usize, adj: &mut Vec<Vec<(u32, f32)>>| {
            adj[a].push((b as u32, 1.0));
            adj[b].push((a as u32, 1.0));
        };
        for a in 0..4 {
            for b in (a + 1)..4 {
                link(a, b, &mut adj);
                link(a + 4, b + 4, &mut adj);
            }
        }
        link(0, 4, &mut adj);
        for list in adj.iter_mut() {
            list.sort_by_key(|&(j, _)| j);
        }
        let mass = vec![1.0; 8];
        let mech = Mechanical {
            adj,
            mass,
            cluster: vec![0; 8],
            cluster_count: 1,
        };
        let pos = spectral_init(&mech, 400, 1.0);
        let left = pos[1][0] + pos[2][0] + pos[3][0];
        let right = pos[5][0] + pos[6][0] + pos[7][0];
        assert!(
            left * right < 0.0,
            "de twee kliekjes horen aan weerszijden te liggen, kreeg {left} en {right}"
        );
    }

    #[test]
    fn fibonacci_points_are_on_the_unit_sphere() {
        for i in 0..50 {
            let p = fibonacci_point(i, 50);
            let r = (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt();
            assert!((r - 1.0).abs() < 1e-4, "straal {r}");
        }
    }
}
