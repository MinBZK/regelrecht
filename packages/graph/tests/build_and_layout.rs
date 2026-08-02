//! End-to-end tests over a real corpus directory, and the two properties the
//! whole thing is built for: the layout is stable, and a framework star does
//! not flatten the picture.

use std::path::Path;

use regelrecht_graph::graph::{EdgeType, NodeKind};
use regelrecht_graph::{build_all, layout, run_passes, testkit, BuildOptions, Options};

fn options(root: &Path) -> Options {
    Options {
        build: BuildOptions {
            root: root.to_path_buf(),
            peildatum: "2026-06-01".to_string(),
            articles: true,
            threads: 2,
            ..BuildOptions::default()
        },
        // Fewer iterations keeps the test quick; nothing about determinism
        // depends on the count.
        layout: layout::LayoutOptions {
            iterations: 60,
            ..layout::LayoutOptions::default()
        },
        ..Options::default()
    }
}

fn sample_corpus() -> tempfile::TempDir {
    #[allow(clippy::expect_used)]
    let dir = tempfile::tempdir().expect("tempdir");
    #[allow(clippy::expect_used)]
    testkit::write_sample_corpus(dir.path()).expect("schrijf corpus");
    dir
}

#[test]
fn reads_the_corpus_into_the_expected_nodes_and_edges() {
    let dir = sample_corpus();
    let graph = build_all(&options(dir.path()));

    // Three harvested laws, one BWB nobody harvested, one expected regulation.
    let laws: Vec<&str> = graph
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Law)
        .map(|n| n.id.as_str())
        .collect();
    assert_eq!(
        laws,
        vec![
            "algemene_wet_bestuursrecht",
            "regeling_standaardpremie",
            "wet_op_de_zorgtoeslag"
        ]
    );
    assert_eq!(graph.stats.external_nodes, 1, "BWBR0009999 is niet geoogst");
    assert_eq!(
        graph.stats.expected_nodes, 1,
        "Regeling zorgverzekering ontbreekt in het corpus"
    );

    #[allow(clippy::expect_used)]
    let expected = graph
        .lookup("expected:regeling_zorgverzekering")
        .expect("verwachte knoop");
    assert_eq!(graph.node(expected).kind, NodeKind::Expected);
    assert_eq!(graph.node(expected).label, "Regeling zorgverzekering");

    // Only the newest version on the peildatum is on the map, so the graph has
    // the 2026 references and not the 2024 ones.
    #[allow(clippy::expect_used)]
    let zorgtoeslag = graph.lookup("wet_op_de_zorgtoeslag").expect("zorgtoeslag");
    assert_eq!(
        graph.node(zorgtoeslag).valid_from.as_deref(),
        Some("2026-01-01")
    );

    // The three edge kinds the corpus can state today all made it through.
    let types: Vec<EdgeType> = graph.edges[..graph.law_edge_count]
        .iter()
        .map(|e| e.edge_type)
        .collect();
    assert!(types.contains(&EdgeType::Citation));
    assert!(
        types.contains(&EdgeType::Delegation),
        "implements ontbreekt"
    );
    assert!(
        types.contains(&EdgeType::Source),
        "source.regulation ontbreekt"
    );
    assert!(types.contains(&EdgeType::ExpectedDelegation));

    // Two articles of the zorgtoeslag cite the Awb, so the aggregated law edge
    // counts two underlying references, not two edges.
    #[allow(clippy::expect_used)]
    let awb = graph.lookup("algemene_wet_bestuursrecht").expect("awb");
    #[allow(clippy::expect_used)]
    let edge = graph.edges[..graph.law_edge_count]
        .iter()
        .find(|e| e.source == zorgtoeslag && e.target == awb && e.edge_type == EdgeType::Citation)
        .expect("citatiekant naar de Awb");
    assert_eq!(edge.count, 2);
    assert_eq!(
        graph.node(awb).in_refs,
        3,
        "twee uit de wet, een uit de regeling"
    );
}

#[test]
fn peildatum_selects_the_version() {
    let dir = sample_corpus();
    let mut opts = options(dir.path());
    opts.build.peildatum = "2025-01-01".to_string();
    let graph = build_all(&opts);
    #[allow(clippy::expect_used)]
    let ix = graph.lookup("wet_op_de_zorgtoeslag").expect("zorgtoeslag");
    assert_eq!(graph.node(ix).valid_from.as_deref(), Some("2024-01-01"));
    // The old version has no open term, so no expected node exists at all.
    assert_eq!(graph.stats.expected_nodes, 0);
}

#[test]
fn all_versions_gives_every_version_its_own_node() {
    let dir = sample_corpus();
    let mut opts = options(dir.path());
    opts.build.all_versions = true;
    let graph = build_all(&opts);
    assert!(graph.lookup("wet_op_de_zorgtoeslag@2024-01-01").is_some());
    assert!(graph.lookup("wet_op_de_zorgtoeslag@2026-01-01").is_some());
}

#[test]
fn articles_get_local_coordinates_around_their_law() {
    let dir = sample_corpus();
    let graph = build_all(&options(dir.path()));
    let articles: Vec<_> = graph
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Article)
        .collect();
    assert!(!articles.is_empty());
    for article in &articles {
        assert!(article.parent.is_some(), "artikel zonder wet");
        let r = (article.x * article.x + article.y * article.y + article.z * article.z).sqrt();
        assert!(
            r < 10.0,
            "artikelcoordinaten zijn lokaal, dus klein; kreeg straal {r}"
        );
    }
}

/// The stability requirement, taken literally: build twice, compare.
#[test]
fn two_builds_of_the_same_corpus_are_bit_identical() {
    let dir = sample_corpus();
    let first = build_all(&options(dir.path()));
    let second = build_all(&options(dir.path()));

    assert_eq!(first.nodes.len(), second.nodes.len());
    for (a, b) in first.nodes.iter().zip(second.nodes.iter()) {
        assert_eq!(a.id, b.id);
        assert_eq!(a.x.to_bits(), b.x.to_bits(), "{} verschoof in x", a.id);
        assert_eq!(a.y.to_bits(), b.y.to_bits(), "{} verschoof in y", a.id);
        assert_eq!(a.z.to_bits(), b.z.to_bits(), "{} verschoof in z", a.id);
        assert_eq!(a.cluster, b.cluster);
    }
    assert_eq!(
        regelrecht_graph::payload::snapshot_id(&first),
        regelrecht_graph::payload::snapshot_id(&second)
    );
}

/// The stronger version of the same claim. Bit-identical output from an
/// identical run only proves the code has no clock in it; what matters is that
/// the result does not depend on the order the filesystem hands the corpus
/// over. Copying the corpus into a directory whose names sort differently
/// changes the walk order and must change nothing else.
#[test]
fn the_layout_does_not_depend_on_file_order() {
    let dir = sample_corpus();
    let reference = build_all(&options(dir.path()));

    #[allow(clippy::expect_used)]
    let shuffled = tempfile::tempdir().expect("tempdir");
    copy_with_prefixed_dirs(dir.path(), shuffled.path());
    let mut opts = options(shuffled.path());
    opts.build.root = shuffled.path().to_path_buf();
    let other = build_all(&opts);

    assert_eq!(reference.nodes.len(), other.nodes.len());
    for (a, b) in reference.nodes.iter().zip(other.nodes.iter()) {
        assert_eq!(a.id, b.id, "de knoopvolgorde is niet canoniek");
        assert_eq!(a.x.to_bits(), b.x.to_bits(), "{} verschoof in x", a.id);
        assert_eq!(a.y.to_bits(), b.y.to_bits(), "{} verschoof in y", a.id);
        assert_eq!(a.z.to_bits(), b.z.to_bits(), "{} verschoof in z", a.id);
    }
}

/// Copy a corpus tree, renaming every law directory so the walk visits them in
/// the opposite order. The `$id` inside the files is untouched, so the graph
/// must come out the same.
fn copy_with_prefixed_dirs(from: &Path, to: &Path) {
    #[allow(clippy::expect_used)]
    for entry in walkdir_files(from) {
        let relative = entry
            .strip_prefix(from)
            .expect("relatief pad")
            .to_string_lossy()
            .to_string();
        // Flip the sort order of the layer directories.
        let flipped = relative
            .replace("nl/wet/", "nl/zzz_wet/")
            .replace("nl/ministeriele_regeling/", "nl/aaa_ministeriele_regeling/");
        let target = to.join(flipped);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).expect("map");
        }
        std::fs::copy(&entry, &target).expect("kopieer");
    }
}

fn walkdir_files(root: &Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}

/// The framework-star problem, measured.
///
/// Three dense communities plus one law every single member cites. The question
/// is whether the planted structure survives into the coordinates, so the
/// measure is neighbourhood purity: of the thirteen nodes nearest to a law in
/// the layout, how many belong to its own community. Perfect separation scores
/// 1.0, a ball with everything mixed scores about a third. It is scale-free,
/// which a raw distance ratio is not: a layout that squeezes every community
/// into a point scores well on distance ratios and tells you nothing.
#[test]
fn the_communities_survive_into_the_coordinates() {
    let (purity, framework_detected) = neighbourhood_purity(true);
    assert!(framework_detected, "de kaderwet moet herkend worden");
    assert!(
        purity > 0.9,
        "de gemeenschappen zijn niet terug te zien in de layout: zuiverheid {purity:.3}"
    );
}

/// The same measurement with the machinery switched off, so the number above
/// means something.
///
/// Without the framework rule the star is an ordinary node with 42 incoming
/// edges, and without dissuade-hubs it pulls proportionally to all of them. The
/// design's claim is that this is what turns the picture into a ball; if
/// switching it off changed nothing, the machinery would be decoration.
#[test]
fn the_damping_is_what_keeps_the_star_from_flattening_the_picture() {
    let (damped, _) = neighbourhood_purity(true);
    let (naive, _) = neighbourhood_purity(false);
    assert!(
        damped > naive + 0.05,
        "demping levert geen meetbaar verschil op: gedempt {damped:.3}, naief {naive:.3}"
    );
}

/// Mean fraction of a law's nearest neighbours that share its community, and
/// whether the framework rule fired.
fn neighbourhood_purity(damped: bool) -> (f64, bool) {
    let sizes = [40usize, 14, 6];
    let mut graph = testkit::star_dominated(&sizes);
    let opts = Options {
        layout: layout::LayoutOptions {
            iterations: 400,
            dissuade_hubs: damped,
            ..layout::LayoutOptions::default()
        },
        framework: regelrecht_graph::FrameworkRule {
            // A fraction above 1.0 can never be met, so with `damped` off no
            // law is ever pulled out of the force computation.
            fraction: if damped { 0.20 } else { 2.0 },
            min_citers: if damped { 25 } else { usize::MAX },
        },
        ..Options::default()
    };
    run_passes(&mut graph, &opts);

    let members: Vec<&regelrecht_graph::Node> = graph.nodes[..graph.law_node_count]
        .iter()
        .filter(|n| n.id != "kaderwet")
        .collect();
    let mut total = 0.0f64;
    let mut counted = 0usize;
    for a in &members {
        let group: usize = a.id[1..2].parse().unwrap_or(0);
        let k = sizes[group] - 1;
        let mut distances: Vec<(f64, &str)> = members
            .iter()
            .filter(|b| b.id != a.id)
            .map(|b| {
                let d = (((a.x - b.x).powi(2) + (a.y - b.y).powi(2) + (a.z - b.z).powi(2)) as f64)
                    .sqrt();
                (d, b.id.as_str())
            })
            .collect();
        distances.sort_by(|x, y| x.0.total_cmp(&y.0).then_with(|| x.1.cmp(y.1)));
        let same = distances[..k]
            .iter()
            .filter(|(_, id)| id[..2] == a.id[..2])
            .count();
        total += same as f64 / k as f64;
        counted += 1;
    }
    let framework = graph
        .lookup("kaderwet")
        .is_some_and(|ix| graph.node(ix).framework);
    (total / counted as f64, framework)
}

/// A framework law belongs to no community and must not be given one.
#[test]
fn the_framework_law_sits_outside_the_clusters_and_above_them() {
    let mut graph = testkit::three_communities(14);
    run_passes(&mut graph, &Options::default());
    #[allow(clippy::expect_used)]
    let frame = graph.lookup("kaderwet").expect("kaderwet");
    assert_eq!(
        graph.node(frame).cluster,
        regelrecht_graph::cluster::FRAMEWORK_CLUSTER
    );
    let highest_member = graph.nodes[..graph.law_node_count]
        .iter()
        .filter(|n| !n.framework)
        .map(|n| n.y)
        .fold(f32::MIN, f32::max);
    assert!(
        graph.node(frame).y > highest_member,
        "de kaderlaag hoort boven het veld te liggen"
    );
}
