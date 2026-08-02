//! Synthetic graphs and synthetic corpora for the tests.
//!
//! Available outside `cfg(test)` so the integration tests can write a corpus to
//! a temp directory and run the real builder over it, rather than testing a
//! second implementation of the builder.

use std::path::Path;

use crate::graph::{CorpusGraph, Edge, EdgeType, Node, NodeKind, RegulatoryLayer};

fn law(id: &str) -> Node {
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
        rank: 0.0,
        cluster: 0,
        framework: false,
    }
}

fn finish(graph: &mut CorpusGraph) {
    graph.canonicalise();
    for edge in &graph.edges[..graph.law_edge_count] {
        graph.nodes[edge.source as usize].out_refs += edge.count;
        graph.nodes[edge.target as usize].in_refs += edge.count;
    }
    graph.stats.laws = graph.law_node_count;
    graph.stats.aggregated_edges = graph.edges.len();
}

/// One hub cited by `arms` laws, which are additionally wired into a chain of
/// `chain_span` neighbours each so there is some structure to lose.
pub fn star_graph(arms: usize, chain_span: usize) -> CorpusGraph {
    let mut graph = CorpusGraph::default();
    let hub = graph.intern(law("hub"));
    let mut arm_ix = Vec::with_capacity(arms);
    for i in 0..arms {
        arm_ix.push(graph.intern(law(&format!("arm{i:04}"))));
    }
    for (i, &ix) in arm_ix.iter().enumerate() {
        graph.edges.push(Edge {
            source: ix,
            target: hub,
            edge_type: EdgeType::Citation,
            count: 1,
        });
        for step in 1..=chain_span {
            let j = (i + step) % arms;
            graph.edges.push(Edge {
                source: ix,
                target: arm_ix[j],
                edge_type: EdgeType::Citation,
                count: 1,
            });
        }
    }
    finish(&mut graph);
    graph
}

/// Three dense communities plus a framework law that every single member cites.
///
/// This is the shape the design worries about: without damping the framework
/// star, the force layout answers with a ball and the three communities are
/// gone. `size` is the number of laws per community.
pub fn three_communities(size: usize) -> CorpusGraph {
    let mut graph = CorpusGraph::default();
    let frame = graph.intern(law("kaderwet"));
    let mut members: Vec<Vec<u32>> = Vec::new();
    for c in 0..3 {
        let mut group = Vec::with_capacity(size);
        for i in 0..size {
            group.push(graph.intern(law(&format!("c{c}_l{i}"))));
        }
        members.push(group);
    }
    for group in &members {
        for (i, &a) in group.iter().enumerate() {
            // Everyone cites the framework law: the star.
            graph.edges.push(Edge {
                source: a,
                target: frame,
                edge_type: EdgeType::Citation,
                count: 3,
            });
            // Dense inside the community, and these are real dependencies.
            for (j, &b) in group.iter().enumerate() {
                if i == j {
                    continue;
                }
                graph.edges.push(Edge {
                    source: a,
                    target: b,
                    edge_type: EdgeType::Source,
                    count: 1,
                });
            }
        }
    }
    // One thin bridge between consecutive communities, so the graph is
    // connected without the communities being arbitrary.
    for c in 0..3 {
        graph.edges.push(Edge {
            source: members[c][0],
            target: members[(c + 1) % 3][0],
            edge_type: EdgeType::Citation,
            count: 1,
        });
    }
    finish(&mut graph);
    graph
}

/// The shape the design actually worries about: communities that are sparse
/// and unequal, and a framework law that every member cites several times.
///
/// This is harder than [`three_communities`] on purpose. There the communities
/// are cliques and the star is perfectly symmetric across three equal groups,
/// so it does no damage. Here each community is a ring, so a member holds two
/// weak links to its own group against one strong link to the framework law,
/// and the groups differ in size so the star has no symmetry to hide behind.
/// Every edge is a citation, so the type weights cannot help either: what is
/// left is hub damping and pulling the framework law out of the computation.
pub fn star_dominated(sizes: &[usize]) -> CorpusGraph {
    let mut graph = CorpusGraph::default();
    let frame = graph.intern(law("kaderwet"));
    for (c, &size) in sizes.iter().enumerate() {
        let group: Vec<u32> = (0..size)
            .map(|i| graph.intern(law(&format!("c{c}_l{i:03}"))))
            .collect();
        for (i, &a) in group.iter().enumerate() {
            graph.edges.push(Edge {
                source: a,
                target: frame,
                edge_type: EdgeType::Citation,
                count: 5,
            });
            graph.edges.push(Edge {
                source: a,
                target: group[(i + 1) % size],
                edge_type: EdgeType::Citation,
                count: 1,
            });
        }
    }
    finish(&mut graph);
    graph
}

/// Write a tiny but complete corpus to `root`, in the real directory shape, so
/// the discovery and version-selection paths are exercised for real.
///
/// Three laws with a citation web, one `implements`, one `source.regulation`,
/// one `open_term` pointing at a regulation nobody harvested, one reference to
/// a BWB identifier outside the corpus, and two versions of one law so the
/// peildatum has something to choose between.
pub fn write_sample_corpus(root: &Path) -> std::io::Result<()> {
    let wet = root.join("regulation/nl/wet/wet_op_de_zorgtoeslag");
    let awb = root.join("regulation/nl/wet/algemene_wet_bestuursrecht");
    let regeling = root.join("regulation/nl/ministeriele_regeling/regeling_standaardpremie");
    for dir in [&wet, &awb, &regeling] {
        std::fs::create_dir_all(dir)?;
    }

    std::fs::write(
        awb.join("1994-01-01.yaml"),
        r#"---
$id: algemene_wet_bestuursrecht
bwb_id: BWBR0005537
name: Algemene wet bestuursrecht
regulatory_layer: WET
publication_date: '1992-06-04'
valid_from: '1994-01-01'
articles:
  - number: '1:3'
    text: Een besluit is een schriftelijke beslissing.
"#,
    )?;

    // Two versions: the peildatum decides which one is on the map.
    std::fs::write(
        wet.join("2024-01-01.yaml"),
        r#"---
$id: wet_op_de_zorgtoeslag
bwb_id: BWBR0018451
name: Wet op de zorgtoeslag
regulatory_layer: WET
publication_date: '2005-06-16'
valid_from: '2024-01-01'
articles:
  - number: '1'
    text: Oude versie met [een verwijzing][ref1].
    references:
      - id: ref1
        bwb_id: BWBR0005537
        artikel: '1:3'
"#,
    )?;
    std::fs::write(
        wet.join("2026-01-01.yaml"),
        r#"---
$id: wet_op_de_zorgtoeslag
bwb_id: BWBR0018451
name: Wet op de zorgtoeslag
regulatory_layer: WET
publication_date: '2024-10-16'
valid_from: '2026-01-01'
articles:
  - number: '1'
    text: >-
      Verwijzingen naar [de Awb][ref1] en naar [een niet-geoogste wet][ref2].
    references:
      - id: ref1
        bwb_id: BWBR0005537
        artikel: '1:3'
      - id: ref2
        bwb_id: BWBR0009999
        artikel: '2'
  - number: '4'
    text: Bij ministeriele regeling wordt de standaardpremie vastgesteld.
    references:
      - id: ref1
        bwb_id: BWBR0005537
        artikel: '1:3'
    machine_readable:
      open_terms:
        - id: standaardpremie
          type: amount
          required: true
          delegated_to: minister
          delegation_type: MINISTERIELE_REGELING
          expected_source: Regeling zorgverzekering
"#,
    )?;

    std::fs::write(
        regeling.join("2025-01-01.yaml"),
        r#"---
$id: regeling_standaardpremie
bwb_id: BWBR0037777
name: Regeling standaardpremie
regulatory_layer: MINISTERIELE_REGELING
publication_date: '2024-11-01'
valid_from: '2025-01-01'
articles:
  - number: '1'
    text: De standaardpremie bedraagt een bedrag.
    references:
      - id: ref1
        bwb_id: BWBR0018451
        artikel: '4'
    machine_readable:
      implements:
        - law: wet_op_de_zorgtoeslag
          article: '4'
          open_term: standaardpremie
      execution:
        input:
          - name: besluitbegrip
            source:
              regulation: algemene_wet_bestuursrecht
              output: is_besluit
"#,
    )?;

    // Bookkeeping files that must not be mistaken for a version.
    std::fs::write(
        wet.join("status.yaml"),
        "law_id: BWBR0018451\nstatus: harvested\n",
    )?;
    Ok(())
}
