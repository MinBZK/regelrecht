//! End-to-end tests over a real corpus directory, and the two properties the
//! whole thing is built for: the layout is stable, and a framework star does
//! not flatten the picture.

use std::path::Path;

use regelrecht_graph::graph::{EdgeType, Enrichment, NodeKind};
use regelrecht_graph::{
    build_all, layout, run_passes, testkit, BuildOptions, LayoutOptions, Options,
};

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

    // "Local" means it fits in the gap to the next law. The whole layout is
    // scaled to a canonical radius and the article spheres are sized against
    // that, so the test has to be relative too.
    let laws = &graph.nodes[..graph.law_node_count];
    let mut closest = f32::MAX;
    for (i, a) in laws.iter().enumerate() {
        for b in &laws[i + 1..] {
            let d = ((a.x - b.x).powi(2) + (a.y - b.y).powi(2) + (a.z - b.z).powi(2)).sqrt();
            closest = closest.min(d);
        }
    }
    for article in &articles {
        assert!(article.parent.is_some(), "artikel zonder wet");
        let r = (article.x * article.x + article.y * article.y + article.z * article.z).sqrt();
        assert!(
            r < 0.6 * closest,
            "een uitgeklapte wet moet in het gat naar de buurwet passen ({closest:.0}); kreeg straal {r}"
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

/// The claim a force layout actually makes: laws that cite each other end up
/// closer together than laws that do not.
///
/// This is the strong property and it holds regardless of what the community
/// structure looks like, so it is the one worth asserting.
#[test]
fn cited_laws_end_up_near_the_laws_that_cite_them() {
    let sizes = [40usize, 14, 6];
    let mut graph = testkit::star_dominated(&sizes);
    run_passes(&mut graph, &Options::default());

    let position = |ix: u32| {
        let n = graph.node(ix);
        [n.x, n.y, n.z]
    };
    let distance = |a: [f32; 3], b: [f32; 3]| {
        (((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)) as f64).sqrt()
    };

    let connected: f64 = graph.edges[..graph.law_edge_count]
        .iter()
        .map(|e| distance(position(e.source), position(e.target)))
        .sum::<f64>()
        / graph.law_edge_count as f64;

    let n = graph.law_node_count;
    let mut all = 0.0f64;
    let mut pairs = 0u32;
    for a in 0..n as u32 {
        for b in (a + 1)..n as u32 {
            all += distance(position(a), position(b));
            pairs += 1;
        }
    }
    let all = all / pairs as f64;

    assert!(
        connected < 0.6 * all,
        "verbonden wetten liggen niet dichter bij elkaar: verbonden {connected:.1}, willekeurig {all:.1}"
    );
}

/// And the weaker one, which is measured rather than demanded.
///
/// Three communities, each a ring, plus one law that every member cites several
/// times. Neighbourhood purity asks: of the nodes nearest to a law, how many
/// belong to its own community, against what that number would be if the
/// coordinates said nothing.
///
/// The margin is thin on purpose. Nothing lifts the heavily cited law out of
/// the way and nothing damps the edges into it, so it pulls all three rings
/// towards itself and they genuinely overlap. A score near 1.0 would be the
/// alarming result: on a graph with a star in it, clean separation means
/// somebody arranged it. Measured here at 0.59 against a chance level of 0.50.
#[test]
fn the_communities_are_visible_but_only_just() {
    let (purity, chance) = neighbourhood_purity(&LayoutOptions::default());
    assert!(
        purity > chance + 0.05,
        "de layout zegt niets over de gemeenschappen: zuiverheid {purity:.3}, kans {chance:.3}"
    );
}

/// Normalising the output scale is a multiplication and must behave like one.
///
/// The opdrachtgever asked whether the nodes could be further apart, and this
/// is the only honest yes: the whole cloud can be made any size without saying
/// anything new. The test pins that it really is uniform, because a "scale"
/// that quietly differed per node would be a statement about the graph.
#[test]
fn the_canonical_scale_is_a_pure_multiplication() {
    let sizes = [40usize, 14, 6];
    let small = laid_out(
        &sizes,
        &LayoutOptions {
            canonical_radius: 100.0,
            ..LayoutOptions::default()
        },
    );
    let large = laid_out(
        &sizes,
        &LayoutOptions {
            canonical_radius: 5000.0,
            ..LayoutOptions::default()
        },
    );

    // Every coordinate scaled by one and the same factor.
    let factor = large[0].1[0] / small[0].1[0];
    for ((id, a), (_, b)) in small.iter().zip(large.iter()) {
        for k in 0..3 {
            let expected = a[k] * factor;
            assert!(
                (b[k] - expected).abs() <= 1e-3 * expected.abs().max(1.0),
                "{id} schaalt niet uniform op as {k}: {} tegen {expected}",
                b[k]
            );
        }
    }
    // Which means the relative positions are untouched.
    assert!(distance_rank_correlation(&small, &large) > 0.999);
}

/// The iteration has to settle, and that is all the numerical measures are for.
///
/// Three of them, all touching the step and not the forces: the spectral
/// embedding starts at the size the forces themselves imply, a step is capped
/// so a badly conditioned moment cannot throw a node further than the structure
/// it is looking for, and the adaptive speed control uses ForceAtlas2's own
/// step-size tolerance of 1.0.
///
/// That last one was set to 0.05 and it was the reason the map looked like a
/// dense ball: the speed control sat on its floor, the cloud crawled outwards a
/// hundred times slower than it should, and after 1500 iterations the layout
/// agreed with a 60.000-iteration reference at a rank correlation of 0.24. With
/// the tolerance at 1.0 the same 1500 iterations reach 0.91 against that
/// reference. None of it changes the fixed point, which is what this test
/// checks: run it twice at different lengths and the relative positions must
/// agree.
///
/// The comparison is on the rank order of pairwise distances, because the two
/// runs may legitimately differ in overall scale and orientation. Everything
/// about which law is near which must survive.
///
/// Two earlier candidates failed this standard and were removed: logarithmic
/// attraction and a logarithmically damped repulsion mass. Both made the
/// picture look better and both moved laws relative to each other — measured at
/// a rank correlation of 0.59 against the plain model, which converges perfectly
/// well without them. A measure that changes the answer is not a numerical
/// measure.
#[test]
fn the_force_iteration_has_settled() {
    let sizes = [40usize, 14, 6];
    let short = laid_out(
        &sizes,
        &LayoutOptions {
            iterations: 4000,
            ..LayoutOptions::default()
        },
    );
    let long = laid_out(
        &sizes,
        &LayoutOptions {
            iterations: 12000,
            ..LayoutOptions::default()
        },
    );
    let correlation = distance_rank_correlation(&short, &long);
    assert!(
        correlation > 0.95,
        "de layout is niet uitgeconvergeerd: rangcorrelatie {correlation:.3} tussen 4000 en 12000 iteraties"
    );
}

/// Laid out coordinates per law id, for comparing two runs.
fn laid_out(sizes: &[usize], layout: &LayoutOptions) -> Vec<(String, [f32; 3])> {
    let mut graph = testkit::star_dominated(sizes);
    run_passes(
        &mut graph,
        &Options {
            layout: layout.clone(),
            ..Options::default()
        },
    );
    graph.nodes[..graph.law_node_count]
        .iter()
        .map(|n| (n.id.clone(), [n.x, n.y, n.z]))
        .collect()
}

/// Spearman correlation between the pairwise distances of two layouts of the
/// same nodes.
fn distance_rank_correlation(a: &[(String, [f32; 3])], b: &[(String, [f32; 3])]) -> f64 {
    assert_eq!(a.len(), b.len());
    let dist = |p: [f32; 3], q: [f32; 3]| -> f64 {
        (((p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2) + (p[2] - q[2]).powi(2)) as f64).sqrt()
    };
    let mut da = Vec::new();
    let mut db = Vec::new();
    for i in 0..a.len() {
        for j in (i + 1)..a.len() {
            assert_eq!(a[i].0, b[i].0, "beide runs moeten dezelfde knopen hebben");
            da.push(dist(a[i].1, a[j].1));
            db.push(dist(b[i].1, b[j].1));
        }
    }
    spearman(&da, &db)
}

fn spearman(x: &[f64], y: &[f64]) -> f64 {
    let rank = |v: &[f64]| -> Vec<f64> {
        let mut order: Vec<usize> = (0..v.len()).collect();
        order.sort_by(|&i, &j| v[i].total_cmp(&v[j]));
        let mut out = vec![0.0; v.len()];
        for (r, &i) in order.iter().enumerate() {
            out[i] = r as f64;
        }
        out
    };
    let (rx, ry) = (rank(x), rank(y));
    let n = rx.len() as f64;
    let mx = rx.iter().sum::<f64>() / n;
    let my = ry.iter().sum::<f64>() / n;
    let mut num = 0.0;
    let mut dx = 0.0;
    let mut dy = 0.0;
    for i in 0..rx.len() {
        num += (rx[i] - mx) * (ry[i] - my);
        dx += (rx[i] - mx).powi(2);
        dy += (ry[i] - my).powi(2);
    }
    if dx == 0.0 || dy == 0.0 {
        return 0.0;
    }
    num / (dx * dy).sqrt()
}

/// Mean fraction of a law's nearest neighbours that share its community, and
/// the same fraction for a layout that carries no information at all.
fn neighbourhood_purity(layout: &LayoutOptions) -> (f64, f64) {
    let sizes = [40usize, 14, 6];
    let mut graph = testkit::star_dominated(&sizes);
    run_passes(
        &mut graph,
        &Options {
            layout: layout.clone(),
            ..Options::default()
        },
    );

    let members: Vec<&regelrecht_graph::Node> = graph.nodes[..graph.law_node_count]
        .iter()
        .filter(|n| n.id != "kaderwet")
        .collect();
    let mut total = 0.0f64;
    let mut chance = 0.0f64;
    let mut counted = 0usize;
    for a in &members {
        let group: usize = a.id[1..2].parse().unwrap_or(0);
        let k = sizes[group] - 1;
        chance += k as f64 / (members.len() - 1) as f64;
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
    (total / counted as f64, chance / counted as f64)
}

/// Nothing is lifted out of the field.
///
/// A designated kaderwet is annotated, not relocated. The law the corpus leans
/// on hardest has to end up where its edges put it, which for a node every
/// other node cites is somewhere near the middle. A map that parks it on a ring
/// above the field to look calmer has thrown away the one thing the distance to
/// that law was going to tell a lawyer.
#[test]
fn a_designated_kaderwet_stays_in_the_field() {
    let mut graph = testkit::star_dominated(&[40, 14, 6]);
    let mut options = Options::default();
    options
        .kaderwetten
        .kaderwetten
        .push(regelrecht_graph::Kaderwetkaart {
            law_id: Some("kaderwet".to_string()),
            ..regelrecht_graph::Kaderwetkaart::default()
        });
    run_passes(&mut graph, &options);

    #[allow(clippy::expect_used)]
    let frame = graph.lookup("kaderwet").expect("kaderwet");
    assert!(graph.node(frame).framework, "de aanwijzing moet aankomen");

    // It takes part in the communities like anything else.
    assert!(
        graph.node(frame).cluster < u16::MAX,
        "een aangewezen kaderwet hoort gewoon in een gemeenschap te vallen"
    );

    // And it sits inside the cloud rather than above it: the law everything
    // cites belongs near the centre of mass, not on a shelf.
    let members: Vec<&regelrecht_graph::Node> = graph.nodes[..graph.law_node_count]
        .iter()
        .filter(|n| n.id != "kaderwet")
        .collect();
    let centre = {
        let mut c = [0.0f32; 3];
        for n in &members {
            c[0] += n.x;
            c[1] += n.y;
            c[2] += n.z;
        }
        [
            c[0] / members.len() as f32,
            c[1] / members.len() as f32,
            c[2] / members.len() as f32,
        ]
    };
    let radius = |n: &regelrecht_graph::Node| {
        ((n.x - centre[0]).powi(2) + (n.y - centre[1]).powi(2) + (n.z - centre[2]).powi(2)).sqrt()
    };
    let mut radii: Vec<f32> = members.iter().map(|n| radius(n)).collect();
    radii.sort_by(f32::total_cmp);
    let median = radii[radii.len() / 2];
    assert!(
        radius(graph.node(frame)) < median,
        "de meest aangehaalde wet hoort binnen de mediane straal te liggen, niet erbuiten"
    );
}

/// The enrichment status has to be readable off a node, and an empty
/// `machine_readable` section must not colour a law in.
#[test]
fn enrichment_status_follows_the_substantive_sections() {
    let dir = sample_corpus();
    let graph = build_all(&options(dir.path()));

    #[allow(clippy::expect_used)]
    let regeling = graph.lookup("regeling_standaardpremie").expect("regeling");
    let regeling = graph.node(regeling);
    assert_eq!(regeling.articles, 1);
    assert_eq!(regeling.articles_enriched, 1);
    assert_eq!(regeling.enrichment, Enrichment::Full);

    // The zorgtoeslag has two articles and only one of them is modelled.
    #[allow(clippy::expect_used)]
    let zorgtoeslag = graph.lookup("wet_op_de_zorgtoeslag").expect("zorgtoeslag");
    let zorgtoeslag = graph.node(zorgtoeslag);
    assert_eq!(zorgtoeslag.articles, 2);
    assert_eq!(zorgtoeslag.articles_enriched, 1);
    assert_eq!(zorgtoeslag.enrichment, Enrichment::Partial);

    // The Awb article in this sample carries nothing at all.
    #[allow(clippy::expect_used)]
    let awb = graph.lookup("algemene_wet_bestuursrecht").expect("awb");
    assert_eq!(graph.node(awb).enrichment, Enrichment::None);
    assert_eq!(graph.node(awb).articles_enriched, 0);

    // A node that is not a held document has nothing to enrich.
    #[allow(clippy::expect_used)]
    let expected = graph
        .lookup("expected:regeling_zorgverzekering")
        .expect("verwachte knoop");
    assert_eq!(graph.node(expected).articles, 0);
    assert_eq!(graph.node(expected).enrichment, Enrichment::None);

    assert_eq!(graph.stats.laws_partly_enriched, 2);
    assert_eq!(graph.stats.laws_fully_enriched, 1);
}

/// An article node carries the binary version of the same thing.
#[test]
fn an_article_node_is_enriched_or_it_is_not() {
    let dir = sample_corpus();
    let graph = build_all(&options(dir.path()));
    #[allow(clippy::expect_used)]
    let modelled = graph
        .lookup("regeling_standaardpremie#1")
        .expect("artikel 1");
    assert_eq!(graph.node(modelled).enrichment, Enrichment::Full);
    #[allow(clippy::expect_used)]
    let plain = graph.lookup("wet_op_de_zorgtoeslag#1").expect("artikel 1");
    assert_eq!(graph.node(plain).enrichment, Enrichment::None);
}
