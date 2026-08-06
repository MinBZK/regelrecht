//! Corpusgraaf: the corpus as nodes, edges and a precomputed layout.
//!
//! This is the data layer under the 3D corpus graph of issue #1082. It reads
//! the regulation YAML, turns every reference into an edge, computes centrality
//! and communities, lays the whole thing out in three dimensions, and writes a
//! payload a renderer can read straight into typed arrays.
//!
//! It deliberately does not touch Postgres. The build is a pure function from a
//! corpus checkout to a file: run it twice, get the same bytes. That property is
//! worth more than a table right now, and the file is a snapshot in exactly the
//! sense the design's `snapshot_id` means. What the tables would look like when
//! they land is written down in [`README`](../README.md) next to this crate.
//!
//! ```no_run
//! use regelrecht_graph::{build_all, Options};
//!
//! let options = Options::default();
//! let graph = build_all(&options);
//! let bytes = regelrecht_graph::payload::encode_binary(&graph, "2026-08-02T00:00:00Z");
//! # let _ = bytes;
//! ```

pub mod build;
pub mod cluster;
pub mod graph;
pub mod kaderwet;
pub mod layout;
pub mod metrics;
pub mod model;
pub mod payload;
pub mod testkit;

pub use build::BuildOptions;
pub use graph::{CorpusGraph, Edge, EdgeType, Node, NodeKind, RegulatoryLayer};
pub use kaderwet::{Kaderwetkaart, Kaderwetten};
pub use layout::LayoutOptions;

/// Everything one build needs to know.
#[derive(Debug, Clone, Default)]
pub struct Options {
    pub build: BuildOptions,
    pub layout: LayoutOptions,
    /// The designation list. Empty means nothing is designated, which is a
    /// legitimate state and not a fallback.
    pub kaderwetten: Kaderwetten,
    /// Skip community detection. Only useful for measuring.
    pub skip_clusters: bool,
    /// Skip the layout. Only useful for measuring the read side.
    pub skip_layout: bool,
}

/// The whole pipeline: read, measure, cluster, lay out.
///
/// The order is not free. The hub damping needs the citer counts, the
/// clustering needs the framework qualification (a framework law belongs to no
/// community), and the layout needs both.
pub fn build_all(options: &Options) -> CorpusGraph {
    let mut graph = build::build(&options.build);
    run_passes(&mut graph, options);
    graph
}

/// The passes after the corpus read, split out so tests can run them on a
/// synthetic graph.
pub fn run_passes(graph: &mut CorpusGraph, options: &Options) {
    let started = std::time::Instant::now();
    metrics::compute(graph, &options.kaderwetten);
    graph.stats.metrics_ms = started.elapsed().as_millis();

    let started = std::time::Instant::now();
    if !options.skip_clusters {
        cluster::assign(graph);
    }
    graph.stats.cluster_ms = started.elapsed().as_millis();

    let started = std::time::Instant::now();
    if !options.skip_layout {
        layout::apply(graph, &options.layout);
    }
    graph.stats.layout_ms = started.elapsed().as_millis();
    graph.stats.peak_rss_kb = peak_rss_kb();
}

/// High-water mark of resident memory, in kilobytes. Linux only; zero
/// elsewhere. Reported by the builder so a scale claim comes with a number.
pub fn peak_rss_kb() -> u64 {
    #[cfg(target_os = "linux")]
    {
        let Ok(status) = std::fs::read_to_string("/proc/self/status") else {
            return 0;
        };
        for line in status.lines() {
            if let Some(rest) = line.strip_prefix("VmHWM:") {
                return rest
                    .trim()
                    .trim_end_matches(" kB")
                    .trim()
                    .parse()
                    .unwrap_or(0);
            }
        }
        0
    }
    #[cfg(not(target_os = "linux"))]
    {
        0
    }
}
