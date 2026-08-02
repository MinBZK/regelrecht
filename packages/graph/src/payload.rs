//! The wire format: what the renderer reads.
//!
//! # `.rrgraph` — corpusgraaf payload v1
//!
//! One self-describing file, or one `application/octet-stream` response body.
//! A JSON header names the sections and holds the string table; the sections
//! themselves are plain little-endian typed arrays, so the client turns each
//! one into a `Float32Array`/`Uint32Array` view over the same `ArrayBuffer`
//! without copying or parsing.
//!
//! ```text
//! offset  0   8 bytes   magic "RRGRAPH\x01"
//! offset  8   u32 LE    header_len
//! offset 12   bytes     JSON header, header_len bytes, UTF-8
//!             padding   zero bytes up to the next multiple of 8
//!             bytes     the sections, back to back, in header order
//! ```
//!
//! Every section offset in the header is relative to the start of the section
//! block (i.e. to `data_offset`), and is a multiple of 8, so every typed array
//! is naturally aligned.
//!
//! ## Header
//!
//! ```json
//! {
//!   "format": "rrgraph",
//!   "version": 1,
//!   "snapshot_id": "9f2c…",
//!   "layout_version": "spectral-fa2/1",
//!   "built_at": "2026-08-02T12:00:00Z",
//!   "data_offset": 4096,
//!   "node_count": 4138,
//!   "law_node_count": 4138,
//!   "edge_count": 250000,
//!   "law_edge_count": 250000,
//!   "kinds": ["law", "article", "external", "expected"],
//!   "edge_types": ["citation", "source", "delegation",
//!                  "expected_delegation", "applicability", "amendment"],
//!   "layers": ["GRONDWET", "WET", "..."],
//!   "clusters": 14,
//!   "enrichment_states": ["none", "partial", "full"],
//!   "status_built_at": "2026-08-02T12:00:00Z",
//!   "status_sections": ["node_enrichment", "node_activity",
//!                       "node_articles", "node_articles_enriched"],
//!   "strings": ["Wet op de zorgtoeslag", "..."],
//!   "sections": [{"name": "node_pos", "type": "f32", "len": 12414,
//!                 "offset": 0, "bytes": 49656}],
//!   "stats": { … }
//! }
//! ```
//!
//! ## Sections
//!
//! `N` is `node_count`, `E` is `edge_count`. All indices are 0-based.
//!
//! | section          | type | length | meaning |
//! |------------------|------|--------|---------|
//! | `node_pos`       | f32  | 3N     | x, y, z interleaved. Global for law-level nodes, **relative to the parent** for articles. |
//! | `node_id`        | u32  | N      | index into `strings`: the stable id |
//! | `node_label`     | u32  | N      | index into `strings`: what a human reads |
//! | `node_kind`      | u8   | N      | index into `kinds` |
//! | `node_layer`     | u8   | N      | index into `layers` |
//! | `node_weight`    | f32  | N      | incoming references; what node size scales with |
//! | `node_rank`      | f32  | N      | PageRank towards the cited law, normalised to 1.0 |
//! | `node_out`       | u32  | N      | outgoing references |
//! | `node_citers`    | u32  | N      | distinct laws citing this one |
//! | `node_cluster`   | u16  | N      | community; `framework_cluster` is the framework layer |
//! | `node_parent`    | u32  | N      | containing law, or `0xFFFFFFFF` at law level |
//! | `node_flags`     | u8   | N      | bit 0 framework law (designated, not measured), bit 1 node is not a held document |
//! | `node_enrichment`| u8   | N      | index into `enrichment_states` |
//! | `node_activity`  | u8   | N      | 0 idle, 1 the enricher is on it now (reserved) |
//! | `node_articles`  | u32  | N      | articles in this law |
//! | `node_articles_enriched` | u32 | N | of which modelled |
//! | `edge_src`       | u32  | E      | node index |
//! | `edge_dst`       | u32  | E      | node index |
//! | `edge_type`      | u8   | E      | index into `edge_types` |
//! | `edge_count`     | u32  | E      | underlying references, for line thickness |
//!
//! Nodes are ordered law level first (`nodes[..law_node_count]`), articles
//! after; edges likewise. A renderer that only draws the overview reads the
//! first block of each and stops. Within each block the order is canonical
//! (nodes by id, edges by `(src, dst, type)`), so a diff between two builds is
//! readable.
//!
//! ## The small readable example
//!
//! `--format json` writes the same graph as readable JSON. This is a real
//! excerpt, from `regelrecht-graph-build --corpus corpus --format json` over
//! the corpus in this repository (36 nodes, 28 edges, 13 communities):
//!
//! ```json
//! {
//!   "format": "rrgraph-json",
//!   "version": 1,
//!   "layout_version": "spectral-fa2/1",
//!   "law_node_count": 36,
//!   "law_edge_count": 28,
//!   "nodes": [
//!     {"id": "algemene_wet_bestuursrecht", "label": "Algemene wet bestuursrecht",
//!      "kind": "law", "layer": "WET",
//!      "x": 25.76, "y": 4.75, "z": -16.38,
//!      "weight": 1, "rank": 0.360, "out": 1, "citers": 1, "cluster": 1,
//!      "parent": null, "framework": true,
//!      "enrichment": "full", "articles": 3, "articles_enriched": 3},
//!     {"id": "expected:minister", "label": "minister", "kind": "expected",
//!      "layer": "MINISTERIELE_REGELING",
//!      "x": -14.17, "y": 21.42, "z": 10.98,
//!      "weight": 1, "rank": 0.068, "out": 0, "citers": 1, "cluster": 2,
//!      "parent": null, "framework": false,
//!      "enrichment": "none", "articles": 0, "articles_enriched": 0},
//!     {"id": "wet_op_de_zorgtoeslag", "label": "Wet op de zorgtoeslag",
//!      "kind": "law", "layer": "WET",
//!      "x": -11.09, "y": 17.53, "z": 10.11,
//!      "weight": 1, "rank": 0.100, "out": 6, "citers": 1, "cluster": 2,
//!      "parent": null, "framework": false,
//!      "enrichment": "partial", "articles": 10, "articles_enriched": 6}
//!   ],
//!   "edges": [
//!     {"source": 33, "target": 2,  "type": "source", "count": 3},
//!     {"source": 33, "target": 18, "type": "expected_delegation", "count": 1},
//!     {"source": 33, "target": 35, "type": "source", "count": 1}
//!   ]
//! }
//! ```
//!
//! (`source`/`target` are indices into the full 36-node array; the three nodes
//! shown are 1, 18 and 33.)
//!
//! Read it as: the Wet op de zorgtoeslag cannot be computed without the Awir
//! and the Zorgverzekeringswet, so there are `source` edges to both, the Awir
//! one three times over. It also names a minister who has to fill in a term and
//! for whom no regulation is held, so there is an `expected_delegation` edge to
//! a node that stands for a document nobody has harvested. Six of its ten
//! articles are modelled, so it draws as partial. The Awb carries
//! `"framework": true` because it is on the kaderwetlijst, and note what that
//! does to its coordinates: nothing. It sits where its citations put it.
//!
//! ## Enrichment status, and refreshing it on its own
//!
//! `node_enrichment` is what the map colours by: grey for `none`, partial for
//! `partial`, full colour for `full`. In the first version nearly every law is
//! `none`, and that is the message rather than a defect — the map shows in one
//! look how little is done and where it sits. `node_articles` and
//! `node_articles_enriched` are there so a law that is 3 out of 40 can be drawn
//! differently from one that is 39 out of 40 without a second request.
//!
//! A law is `full` only when every article carries a **substantive**
//! `machine_readable` section. An empty section does not count; see
//! [`crate::model::MachineReadableFile::is_substantive`].
//!
//! The four sections listed in `status_sections` are the last sections in the
//! blob and their length depends only on the node count. That is deliberate:
//! the status changes by the minute while the enricher runs and the layout
//! costs seconds to recompute, so a refresh has to be able to rewrite this tail
//! and nothing else. `snapshot_id` therefore covers the nodes, their positions
//! and the edges, and **not** the status, so a refreshed payload keeps the same
//! snapshot identity and the client knows its coordinates are still valid.
//! `status_built_at` says when the tail was last written.
//!
//! `node_activity` is reserved and always 0 today. An enrichment run writes
//! `.enrichment-progress.json` next to the law it is working on, so filling it
//! in later is a matter of reading that file during the status pass.
//!
//! ## Two things a renderer has to handle. A `source == target` edge occurs (a law
//! binding to its own output) and should become a counter on the node rather
//! than a loop. And a node whose `flags` bit 1 is set is not a document we
//! hold: it has a position and a weight like any other node, but there is no
//! text behind it.

use std::collections::HashMap;

use serde::Serialize;

use crate::graph::{CorpusGraph, EdgeType, Enrichment, NodeKind, RegulatoryLayer};

/// The sections that describe progress rather than structure, and that a status
/// refresh may rewrite on its own. They are written last, in this order.
pub const STATUS_SECTIONS: [&str; 4] = [
    "node_enrichment",
    "node_activity",
    "node_articles",
    "node_articles_enriched",
];

/// The version of the layout algorithm. Bump it whenever a change moves nodes;
/// the client uses it to decide whether to animate from the old positions.
pub const LAYOUT_VERSION: &str = "spectral-fa2/1";
pub const MAGIC: &[u8; 8] = b"RRGRAPH\x01";
/// `node_parent` value for a node that has no containing law.
pub const NO_PARENT: u32 = u32::MAX;

#[derive(Debug, Serialize)]
struct Section {
    name: &'static str,
    #[serde(rename = "type")]
    kind: &'static str,
    len: usize,
    offset: usize,
    bytes: usize,
}

#[derive(Debug, Serialize)]
struct Header {
    format: &'static str,
    version: u32,
    snapshot_id: String,
    layout_version: &'static str,
    built_at: String,
    data_offset: usize,
    node_count: usize,
    law_node_count: usize,
    edge_count: usize,
    law_edge_count: usize,
    kinds: Vec<&'static str>,
    edge_types: Vec<&'static str>,
    layers: Vec<&'static str>,
    clusters: usize,
    enrichment_states: Vec<&'static str>,
    status_built_at: String,
    status_sections: Vec<&'static str>,
    strings: Vec<String>,
    sections: Vec<Section>,
    stats: crate::graph::BuildStats,
}

/// Serialise the graph into the binary payload described at the top of this
/// module.
pub fn encode_binary(graph: &CorpusGraph, built_at: &str) -> Vec<u8> {
    let n = graph.nodes.len();
    let e = graph.edges.len();

    // Labels repeat across articles of the same law ("artikel 1" thousands of
    // times), so the table is interned rather than written out per node.
    let mut strings: Vec<String> = Vec::with_capacity(n);
    let mut string_ix: HashMap<String, u32> = HashMap::with_capacity(n * 2);
    let intern = |s: &str, strings: &mut Vec<String>, map: &mut HashMap<String, u32>| -> u32 {
        if let Some(&ix) = map.get(s) {
            return ix;
        }
        let ix = strings.len() as u32;
        strings.push(s.to_string());
        map.insert(s.to_string(), ix);
        ix
    };

    let mut node_pos: Vec<f32> = Vec::with_capacity(n * 3);
    let mut node_id: Vec<u32> = Vec::with_capacity(n);
    let mut node_label: Vec<u32> = Vec::with_capacity(n);
    let mut node_kind: Vec<u8> = Vec::with_capacity(n);
    let mut node_layer: Vec<u8> = Vec::with_capacity(n);
    let mut node_weight: Vec<f32> = Vec::with_capacity(n);
    let mut node_rank: Vec<f32> = Vec::with_capacity(n);
    let mut node_out: Vec<u32> = Vec::with_capacity(n);
    let mut node_citers: Vec<u32> = Vec::with_capacity(n);
    let mut node_cluster: Vec<u16> = Vec::with_capacity(n);
    let mut node_parent: Vec<u32> = Vec::with_capacity(n);
    let mut node_flags: Vec<u8> = Vec::with_capacity(n);
    let mut node_enrichment: Vec<u8> = Vec::with_capacity(n);
    let mut node_activity: Vec<u8> = Vec::with_capacity(n);
    let mut node_articles: Vec<u32> = Vec::with_capacity(n);
    let mut node_articles_enriched: Vec<u32> = Vec::with_capacity(n);

    for node in &graph.nodes {
        node_pos.extend_from_slice(&[node.x, node.y, node.z]);
        node_id.push(intern(&node.id, &mut strings, &mut string_ix));
        node_label.push(intern(&node.label, &mut strings, &mut string_ix));
        node_kind.push(kind_index(node.kind));
        node_layer.push(layer_index(node.layer));
        node_weight.push(node.in_refs as f32);
        node_rank.push(node.rank);
        node_out.push(node.out_refs);
        node_citers.push(node.citers);
        node_cluster.push(node.cluster);
        node_parent.push(node.parent.unwrap_or(NO_PARENT));
        let mut flags = 0u8;
        if node.framework {
            flags |= 1;
        }
        if matches!(node.kind, NodeKind::External | NodeKind::Expected) {
            flags |= 2;
        }
        node_flags.push(flags);
        node_enrichment.push(enrichment_index(node.enrichment));
        // Reserved: nothing reads `.enrichment-progress.json` yet.
        node_activity.push(0);
        node_articles.push(node.articles);
        node_articles_enriched.push(node.articles_enriched);
    }

    let mut edge_src: Vec<u32> = Vec::with_capacity(e);
    let mut edge_dst: Vec<u32> = Vec::with_capacity(e);
    let mut edge_type: Vec<u8> = Vec::with_capacity(e);
    let mut edge_count: Vec<u32> = Vec::with_capacity(e);
    for edge in &graph.edges {
        edge_src.push(edge.source);
        edge_dst.push(edge.target);
        edge_type.push(type_index(edge.edge_type));
        edge_count.push(edge.count);
    }

    let mut blob: Vec<u8> = Vec::new();
    let mut sections: Vec<Section> = Vec::new();
    push_f32(&mut blob, &mut sections, "node_pos", &node_pos);
    push_u32(&mut blob, &mut sections, "node_id", &node_id);
    push_u32(&mut blob, &mut sections, "node_label", &node_label);
    push_u8(&mut blob, &mut sections, "node_kind", &node_kind);
    push_u8(&mut blob, &mut sections, "node_layer", &node_layer);
    push_f32(&mut blob, &mut sections, "node_weight", &node_weight);
    push_f32(&mut blob, &mut sections, "node_rank", &node_rank);
    push_u32(&mut blob, &mut sections, "node_out", &node_out);
    push_u32(&mut blob, &mut sections, "node_citers", &node_citers);
    push_u16(&mut blob, &mut sections, "node_cluster", &node_cluster);
    push_u32(&mut blob, &mut sections, "node_parent", &node_parent);
    push_u8(&mut blob, &mut sections, "node_flags", &node_flags);
    push_u32(&mut blob, &mut sections, "edge_src", &edge_src);
    push_u32(&mut blob, &mut sections, "edge_dst", &edge_dst);
    push_u8(&mut blob, &mut sections, "edge_type", &edge_type);
    push_u32(&mut blob, &mut sections, "edge_count", &edge_count);
    // The refreshable tail. Everything above describes the graph and its
    // layout; everything from here on describes how far the work has got, and
    // that is the part that moves while the enricher runs.
    push_u8(
        &mut blob,
        &mut sections,
        "node_enrichment",
        &node_enrichment,
    );
    push_u8(&mut blob, &mut sections, "node_activity", &node_activity);
    push_u32(&mut blob, &mut sections, "node_articles", &node_articles);
    push_u32(
        &mut blob,
        &mut sections,
        "node_articles_enriched",
        &node_articles_enriched,
    );

    let header = Header {
        format: "rrgraph",
        version: 1,
        snapshot_id: snapshot_id(graph),
        layout_version: LAYOUT_VERSION,
        built_at: built_at.to_string(),
        data_offset: 0,
        node_count: n,
        law_node_count: graph.law_node_count,
        edge_count: e,
        law_edge_count: graph.law_edge_count,
        kinds: NodeKind::ALL.iter().map(|k| k.as_str()).collect(),
        edge_types: EdgeType::ALL.iter().map(|t| t.as_str()).collect(),
        layers: RegulatoryLayer::ALL.iter().map(|l| l.as_str()).collect(),
        clusters: graph.stats.clusters,
        enrichment_states: Enrichment::ALL.iter().map(|e| e.as_str()).collect(),
        status_built_at: built_at.to_string(),
        status_sections: STATUS_SECTIONS.to_vec(),
        strings,
        sections,
        stats: graph.stats.clone(),
    };

    // The header records where the section block starts, and the header's own
    // length changes when that number is written into it. Serialising twice with
    // a fixed-width placeholder is the cheap way out and costs a millisecond.
    let probe = serde_json::to_vec(&header).unwrap_or_default();
    let data_offset = align8(MAGIC.len() + 4 + probe.len() + 24);
    let header = Header {
        data_offset,
        ..header
    };
    let mut json = serde_json::to_vec(&header).unwrap_or_default();
    json.resize(data_offset - MAGIC.len() - 4, b' ');

    let mut out = Vec::with_capacity(data_offset + blob.len());
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&(json.len() as u32).to_le_bytes());
    out.extend_from_slice(&json);
    debug_assert_eq!(out.len(), data_offset);
    out.extend_from_slice(&blob);
    out
}

/// A content hash over the graph and its layout: the nodes, where they are, and
/// the edges.
///
/// Two builds of the same corpus produce the same snapshot id, which is what
/// makes it usable as a cache key and as the "did anything change" check.
/// Deliberately excluded is the enrichment status, so that refreshing the
/// status tail of a payload leaves the identity alone and a client can tell
/// that its coordinates are still the ones it already has. FNV-1a over the
/// canonical node and edge order; an identifier, not a security claim.
pub fn snapshot_id(graph: &CorpusGraph) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    let feed = |bytes: &[u8], hash: &mut u64| {
        for &b in bytes {
            *hash ^= b as u64;
            *hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
    };
    for node in &graph.nodes {
        feed(node.id.as_bytes(), &mut hash);
        feed(&node.x.to_le_bytes(), &mut hash);
        feed(&node.y.to_le_bytes(), &mut hash);
        feed(&node.z.to_le_bytes(), &mut hash);
        feed(&[node.cluster as u8, (node.cluster >> 8) as u8], &mut hash);
    }
    for edge in &graph.edges {
        feed(&edge.source.to_le_bytes(), &mut hash);
        feed(&edge.target.to_le_bytes(), &mut hash);
        feed(&[type_index(edge.edge_type)], &mut hash);
        feed(&edge.count.to_le_bytes(), &mut hash);
    }
    format!("{hash:016x}")
}

/// The same graph as readable JSON. Slower and several times larger; meant for
/// eyeballing a build and for tests, not for the wire.
#[derive(Debug, Serialize)]
pub struct JsonPayload {
    pub format: &'static str,
    pub version: u32,
    pub snapshot_id: String,
    pub layout_version: &'static str,
    pub built_at: String,
    pub law_node_count: usize,
    pub law_edge_count: usize,
    pub clusters: usize,
    pub stats: crate::graph::BuildStats,
    pub nodes: Vec<JsonNode>,
    pub edges: Vec<JsonEdge>,
}

#[derive(Debug, Serialize)]
pub struct JsonNode {
    pub id: String,
    pub label: String,
    pub kind: &'static str,
    pub layer: &'static str,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub weight: u32,
    pub rank: f32,
    pub out: u32,
    pub citers: u32,
    pub cluster: u16,
    pub parent: Option<u32>,
    pub framework: bool,
    pub enrichment: &'static str,
    pub articles: u32,
    pub articles_enriched: u32,
}

#[derive(Debug, Serialize)]
pub struct JsonEdge {
    pub source: u32,
    pub target: u32,
    #[serde(rename = "type")]
    pub edge_type: &'static str,
    pub count: u32,
}

pub fn encode_json(graph: &CorpusGraph, built_at: &str) -> JsonPayload {
    JsonPayload {
        format: "rrgraph-json",
        version: 1,
        snapshot_id: snapshot_id(graph),
        layout_version: LAYOUT_VERSION,
        built_at: built_at.to_string(),
        law_node_count: graph.law_node_count,
        law_edge_count: graph.law_edge_count,
        clusters: graph.stats.clusters,
        stats: graph.stats.clone(),
        nodes: graph
            .nodes
            .iter()
            .map(|n| JsonNode {
                id: n.id.clone(),
                label: n.label.clone(),
                kind: n.kind.as_str(),
                layer: n.layer.as_str(),
                x: n.x,
                y: n.y,
                z: n.z,
                weight: n.in_refs,
                rank: n.rank,
                out: n.out_refs,
                citers: n.citers,
                cluster: n.cluster,
                parent: n.parent,
                framework: n.framework,
                enrichment: n.enrichment.as_str(),
                articles: n.articles,
                articles_enriched: n.articles_enriched,
            })
            .collect(),
        edges: graph
            .edges
            .iter()
            .map(|e| JsonEdge {
                source: e.source,
                target: e.target,
                edge_type: e.edge_type.as_str(),
                count: e.count,
            })
            .collect(),
    }
}

fn kind_index(kind: NodeKind) -> u8 {
    NodeKind::ALL.iter().position(|&k| k == kind).unwrap_or(0) as u8
}

fn type_index(edge_type: EdgeType) -> u8 {
    EdgeType::ALL
        .iter()
        .position(|&t| t == edge_type)
        .unwrap_or(0) as u8
}

fn enrichment_index(state: Enrichment) -> u8 {
    Enrichment::ALL
        .iter()
        .position(|&e| e == state)
        .unwrap_or(0) as u8
}

fn layer_index(layer: RegulatoryLayer) -> u8 {
    RegulatoryLayer::ALL
        .iter()
        .position(|&l| l == layer)
        .unwrap_or(RegulatoryLayer::ALL.len() - 1) as u8
}

fn align8(n: usize) -> usize {
    n.div_ceil(8) * 8
}

fn pad(blob: &mut Vec<u8>) {
    while !blob.len().is_multiple_of(8) {
        blob.push(0);
    }
}

fn push_f32(blob: &mut Vec<u8>, sections: &mut Vec<Section>, name: &'static str, data: &[f32]) {
    pad(blob);
    let offset = blob.len();
    for v in data {
        blob.extend_from_slice(&v.to_le_bytes());
    }
    sections.push(Section {
        name,
        kind: "f32",
        len: data.len(),
        offset,
        bytes: data.len() * 4,
    });
}

fn push_u32(blob: &mut Vec<u8>, sections: &mut Vec<Section>, name: &'static str, data: &[u32]) {
    pad(blob);
    let offset = blob.len();
    for v in data {
        blob.extend_from_slice(&v.to_le_bytes());
    }
    sections.push(Section {
        name,
        kind: "u32",
        len: data.len(),
        offset,
        bytes: data.len() * 4,
    });
}

fn push_u16(blob: &mut Vec<u8>, sections: &mut Vec<Section>, name: &'static str, data: &[u16]) {
    pad(blob);
    let offset = blob.len();
    for v in data {
        blob.extend_from_slice(&v.to_le_bytes());
    }
    sections.push(Section {
        name,
        kind: "u16",
        len: data.len(),
        offset,
        bytes: data.len() * 2,
    });
}

fn push_u8(blob: &mut Vec<u8>, sections: &mut Vec<Section>, name: &'static str, data: &[u8]) {
    pad(blob);
    let offset = blob.len();
    blob.extend_from_slice(data);
    sections.push(Section {
        name,
        kind: "u8",
        len: data.len(),
        offset,
        bytes: data.len(),
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testkit::three_communities;

    fn decode_header(bytes: &[u8]) -> serde_json::Value {
        assert_eq!(&bytes[..8], MAGIC);
        let len = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]) as usize;
        let json = std::str::from_utf8(&bytes[12..12 + len]).expect("utf8");
        serde_json::from_str(json.trim_end()).expect("json-header")
    }

    #[test]
    fn binary_payload_round_trips_its_own_description() {
        let mut graph = three_communities(6);
        crate::metrics::compute(&mut graph, &crate::kaderwet::Kaderwetten::default());
        crate::cluster::assign(&mut graph);
        let bytes = encode_binary(&graph, "2026-01-01T00:00:00Z");
        let header = decode_header(&bytes);

        assert_eq!(header["format"], "rrgraph");
        assert_eq!(header["node_count"], graph.nodes.len());
        assert_eq!(header["edge_count"], graph.edges.len());

        let data_offset = header["data_offset"].as_u64().expect("data_offset") as usize;
        assert_eq!(data_offset % 8, 0);
        assert!(bytes.len() >= data_offset);

        // Every section must sit inside the file, be aligned, and be exactly as
        // long as it claims.
        for section in header["sections"].as_array().expect("sections") {
            let offset = section["offset"].as_u64().expect("offset") as usize;
            let size = section["bytes"].as_u64().expect("bytes") as usize;
            assert_eq!(offset % 8, 0, "sectie {} niet uitgelijnd", section["name"]);
            assert!(
                data_offset + offset + size <= bytes.len(),
                "sectie {} valt buiten het bestand",
                section["name"]
            );
        }

        // And the positions must be readable back as f32 exactly.
        let pos = header["sections"]
            .as_array()
            .expect("sections")
            .iter()
            .find(|s| s["name"] == "node_pos")
            .expect("node_pos");
        let offset = data_offset + pos["offset"].as_u64().expect("offset") as usize;
        let x = f32::from_le_bytes([
            bytes[offset],
            bytes[offset + 1],
            bytes[offset + 2],
            bytes[offset + 3],
        ]);
        assert_eq!(x, graph.nodes[0].x);
    }

    #[test]
    fn snapshot_id_ignores_build_time_and_follows_content() {
        let a = three_communities(6);
        let b = three_communities(6);
        assert_eq!(snapshot_id(&a), snapshot_id(&b));
        let mut c = three_communities(6);
        c.nodes[0].x += 1.0;
        assert_ne!(snapshot_id(&a), snapshot_id(&c));
    }

    #[test]
    fn json_payload_carries_the_agreed_core_fields() {
        let mut graph = three_communities(6);
        crate::metrics::compute(&mut graph, &crate::kaderwet::Kaderwetten::default());
        let payload = encode_json(&graph, "2026-01-01T00:00:00Z");
        let value = serde_json::to_value(&payload).expect("serialiseer");
        let node = &value["nodes"][0];
        for field in ["id", "label", "x", "y", "z", "kind", "weight"] {
            assert!(!node[field].is_null(), "knoopveld {field} ontbreekt");
        }
        let edge = &value["edges"][0];
        for field in ["source", "target", "type"] {
            assert!(!edge[field].is_null(), "kantveld {field} ontbreekt");
        }
    }
}
