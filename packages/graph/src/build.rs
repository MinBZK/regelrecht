//! Corpus scan: files on disk become nodes and edges.
//!
//! The scan is deliberately version-aware. A law directory holds one YAML file
//! per `valid_from` date, and the corpus has 22.471 of those for 4.138 laws.
//! Drawing every version as its own node triples the graph and makes the
//! picture unreadable (design, open keuze 5), so the default is one node per
//! law: the newest version valid on the peildatum. `--all-versions` keeps them
//! all, which is what you want when measuring and nothing else.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Instant;

use walkdir::WalkDir;

use crate::graph::{
    CorpusGraph, Edge, EdgeType, Enrichment, Node, NodeIx, NodeKind, RegulatoryLayer,
};
use crate::model::LawFile;

/// How the corpus is read.
#[derive(Debug, Clone)]
pub struct BuildOptions {
    /// Root of the corpus checkout (the directory holding `regulation/`).
    pub root: PathBuf,
    /// Only versions valid on or before this date are eligible, newest wins.
    /// `YYYY-MM-DD`.
    pub peildatum: String,
    /// Keep every version as its own node instead of one per law.
    pub all_versions: bool,
    /// Also build article nodes and article-level edges.
    pub articles: bool,
    /// Create a node for a BWB identifier the corpus does not hold. Off makes
    /// the graph smaller and dishonest: the references still exist.
    pub external_nodes: bool,
    /// Worker threads for parsing.
    pub threads: usize,
}

impl Default for BuildOptions {
    fn default() -> Self {
        Self {
            root: PathBuf::from("."),
            peildatum: "9999-12-31".to_string(),
            all_versions: false,
            articles: false,
            external_nodes: true,
            threads: 4,
        }
    }
}

/// A file that could not be read, kept so the builder can name it instead of
/// only counting it. A silent failure here is a law missing from the map.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ParseFailure {
    pub path: PathBuf,
    pub error: String,
}

/// A parsed file plus where it came from.
struct Parsed {
    law: LawFile,
    /// Directory-derived slug, used when `$id` is missing.
    dir_slug: String,
    version: String,
}

/// Find the files to read: walk the corpus, group YAML by law directory, pick
/// the version(s) the options ask for.
///
/// Returns `(path, dir_slug, version)` triples sorted by path so the result
/// does not depend on filesystem iteration order.
pub fn discover(opts: &BuildOptions) -> Vec<(PathBuf, String, String)> {
    let mut by_dir: HashMap<PathBuf, Vec<(String, PathBuf)>> = HashMap::new();
    for entry in WalkDir::new(&opts.root)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
    {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("yaml") {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        if !is_version_stem(stem) {
            // `status.yaml` and friends carry pipeline bookkeeping, not law.
            continue;
        }
        let Some(dir) = path.parent() else { continue };
        by_dir
            .entry(dir.to_path_buf())
            .or_default()
            .push((stem.to_string(), path.to_path_buf()));
    }

    let mut selected: Vec<(PathBuf, String, String)> = Vec::new();
    let mut dirs: Vec<&PathBuf> = by_dir.keys().collect();
    dirs.sort();
    for dir in dirs {
        let mut versions = by_dir[dir].clone();
        versions.sort();
        let slug = dir
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("onbekend")
            .to_string();
        if opts.all_versions {
            for (version, path) in versions {
                selected.push((path, slug.clone(), version));
            }
            continue;
        }
        // Newest version valid on the peildatum; if the law only starts after
        // it, take the earliest so the law is still on the map.
        let pick = versions
            .iter()
            .rev()
            .find(|(v, _)| v.as_str() <= opts.peildatum.as_str())
            .or_else(|| versions.first());
        if let Some((version, path)) = pick {
            selected.push((path.clone(), slug, version.clone()));
        }
    }
    selected
}

fn is_version_stem(stem: &str) -> bool {
    let b = stem.as_bytes();
    b.len() == 10
        && b[4] == b'-'
        && b[7] == b'-'
        && b.iter()
            .enumerate()
            .all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit())
}

/// Read the corpus and produce nodes and aggregated edges. No metrics, no
/// clustering, no layout: those are separate passes over the result.
pub fn build(opts: &BuildOptions) -> CorpusGraph {
    let started = Instant::now();
    let files = discover(opts);
    let scanned = files.len();

    let (parsed, failures) = parse_all(&files, opts.threads.max(1));
    let failed = failures.len();

    let mut graph = CorpusGraph::default();
    graph.stats.files_scanned = scanned;
    graph.stats.files_parsed = parsed.len();
    graph.stats.files_failed = failed;
    graph.stats.failures = failures;
    graph.stats.parse_ms = started.elapsed().as_millis();

    assemble(&mut graph, parsed, opts);
    graph
}

/// Parse in a fixed number of worker threads, then restore canonical order.
///
/// Threads are a throughput trick only: the results are sorted by the source
/// path afterwards, so the number of threads never shows up in the output.
fn parse_all(
    files: &[(PathBuf, String, String)],
    threads: usize,
) -> (Vec<Parsed>, Vec<ParseFailure>) {
    let out: Mutex<Vec<(usize, Parsed)>> = Mutex::new(Vec::with_capacity(files.len()));
    let failed: Mutex<Vec<(usize, ParseFailure)>> = Mutex::new(Vec::new());
    let chunk = files.len().div_ceil(threads).max(1);
    std::thread::scope(|scope| {
        for (chunk_ix, slice) in files.chunks(chunk).enumerate() {
            let out = &out;
            let failed = &failed;
            scope.spawn(move || {
                let mut local = Vec::with_capacity(slice.len());
                let mut local_failed = Vec::new();
                for (offset, (path, slug, version)) in slice.iter().enumerate() {
                    match parse_one(path) {
                        Ok(law) => local.push((
                            chunk_ix * chunk + offset,
                            Parsed {
                                law,
                                dir_slug: slug.clone(),
                                version: version.clone(),
                            },
                        )),
                        Err(error) => {
                            tracing::warn!(path = %path.display(), %error, "kon regeling niet lezen");
                            local_failed.push((
                                chunk_ix * chunk + offset,
                                ParseFailure {
                                    path: path.clone(),
                                    error,
                                },
                            ));
                        }
                    }
                }
                #[allow(clippy::expect_used)]
                out.lock().expect("parse-resultaten").extend(local);
                #[allow(clippy::expect_used)]
                failed.lock().expect("parse-fouten").extend(local_failed);
            });
        }
    });
    #[allow(clippy::expect_used)]
    let mut collected = out.into_inner().expect("parse-resultaten");
    collected.sort_by_key(|(ix, _)| *ix);
    #[allow(clippy::expect_used)]
    let mut failures = failed.into_inner().expect("parse-fouten");
    failures.sort_by_key(|(ix, _)| *ix);
    (
        collected.into_iter().map(|(_, p)| p).collect(),
        failures.into_iter().map(|(_, f)| f).collect(),
    )
}

fn parse_one(path: &Path) -> Result<LawFile, String> {
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    serde_yaml_ng::from_str::<LawFile>(&text).map_err(|e| e.to_string())
}

/// Raw, unaggregated relation before it is collapsed per `(src, dst, type)`.
type RawEdge = (NodeIx, NodeIx, EdgeType);

fn assemble(graph: &mut CorpusGraph, parsed: Vec<Parsed>, opts: &BuildOptions) {
    // Pass one: every document the corpus holds becomes a node, and both its
    // identifiers (slug and BWB) become keys, so a reference can arrive by
    // either. Slug and BWB live in one namespace on purpose: a reference by
    // BWB and a `source.regulation` by slug must land on the same node.
    let mut by_bwb: HashMap<String, NodeIx> = HashMap::new();
    let mut by_slug: HashMap<String, NodeIx> = HashMap::new();
    let mut law_ix: Vec<NodeIx> = Vec::with_capacity(parsed.len());

    for p in &parsed {
        let slug = p.law.id.clone().unwrap_or_else(|| p.dir_slug.clone());
        let id = if opts.all_versions {
            format!("{slug}@{}", p.version)
        } else {
            slug.clone()
        };
        // `name` may be a reference into the law's own definitions
        // (`#wet_naam`), which is unreadable on a node. Fall back on the slug
        // rather than print the placeholder.
        let label = p
            .law
            .name
            .clone()
            .filter(|n| !n.starts_with('#'))
            .or_else(|| p.law.officiele_titel.clone())
            .unwrap_or_else(|| humanise(&slug));
        let layer = p
            .law
            .regulatory_layer
            .as_deref()
            .map(RegulatoryLayer::parse)
            .unwrap_or(RegulatoryLayer::Onbekend);
        let ix = graph.intern(Node {
            id,
            label,
            kind: NodeKind::Law,
            layer,
            bwb_id: p.law.bwb_id.clone(),
            valid_from: p.law.valid_from.clone().or(Some(p.version.clone())),
            parent: None,
            x: 0.0,
            y: 0.0,
            z: 0.0,
            in_refs: 0,
            out_refs: 0,
            citers: 0,
            enrichment: Enrichment::None,
            articles: 0,
            articles_enriched: 0,
            rank: 0.0,
            cluster: 0,
            framework: false,
        });
        law_ix.push(ix);
        if let Some(bwb) = &p.law.bwb_id {
            by_bwb.entry(bwb.clone()).or_insert(ix);
        }
        by_slug.entry(slug).or_insert(ix);
    }
    graph.stats.laws = graph.nodes.len();

    // How much of each law is modelled. Counted from the file, always, whether
    // or not article nodes are being built: the overview is exactly where this
    // has to be visible, and in the first version of the map it will be almost
    // entirely grey.
    for (p, &law) in parsed.iter().zip(&law_ix) {
        let total = p.law.articles.len() as u32;
        let modelled = p
            .law
            .articles
            .iter()
            .filter(|a| {
                a.machine_readable
                    .as_ref()
                    .is_some_and(|m| m.is_substantive())
            })
            .count() as u32;
        let node = &mut graph.nodes[law as usize];
        node.articles = total;
        node.articles_enriched = modelled;
        node.enrichment = Enrichment::of(total, modelled);
    }

    // Pass two: article nodes. They are interned before any edge so that a
    // citation can point straight at the article it names.
    let mut article_ix: HashMap<(NodeIx, String), NodeIx> = HashMap::new();
    if opts.articles {
        for (p, &law) in parsed.iter().zip(&law_ix) {
            for article in &p.law.articles {
                let Some(number) = article.number.as_deref() else {
                    continue;
                };
                let law_id = graph.node(law).id.clone();
                let layer = graph.node(law).layer;
                let modelled = article
                    .machine_readable
                    .as_ref()
                    .is_some_and(|m| m.is_substantive());
                let ix = graph.intern(Node {
                    id: format!("{law_id}#{number}"),
                    label: format!("artikel {number}"),
                    kind: NodeKind::Article,
                    layer,
                    bwb_id: None,
                    valid_from: None,
                    parent: Some(law),
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                    in_refs: 0,
                    out_refs: 0,
                    citers: 0,
                    enrichment: if modelled {
                        Enrichment::Full
                    } else {
                        Enrichment::None
                    },
                    articles: 1,
                    articles_enriched: u32::from(modelled),
                    rank: 0.0,
                    cluster: 0,
                    framework: false,
                });
                article_ix.insert((law, number.to_string()), ix);
            }
        }
        graph.stats.articles = graph.nodes.len() - graph.stats.laws;
    }

    // Pass three: the relations. Law-level and article-level edges are
    // collected apart so the payload can put the law level first and let a
    // renderer that only draws laws stop reading after it.
    let mut law_edges: Vec<RawEdge> = Vec::new();
    let mut art_edges: Vec<RawEdge> = Vec::new();
    let mut raw_refs: u64 = 0;
    let mut dangling: u64 = 0;

    for (p, &law) in parsed.iter().zip(&law_ix) {
        for article in &p.law.articles {
            let src_article = article
                .number
                .as_deref()
                .and_then(|n| article_ix.get(&(law, n.to_string())).copied());

            for reference in &article.references {
                raw_refs += 1;
                let Some(bwb) = reference.bwb_id.as_deref() else {
                    dangling += 1;
                    continue;
                };
                let target_law = match by_bwb.get(bwb) {
                    Some(&ix) => Some(ix),
                    None if opts.external_nodes => Some(intern_external(graph, &mut by_bwb, bwb)),
                    None => None,
                };
                let Some(target_law) = target_law else {
                    dangling += 1;
                    continue;
                };
                law_edges.push((law, target_law, EdgeType::Citation));

                if opts.articles {
                    let target = reference
                        .anchor()
                        .and_then(|a| article_ix.get(&(target_law, a.to_string())).copied())
                        .unwrap_or(target_law);
                    if let Some(src) = src_article {
                        art_edges.push((src, target, EdgeType::Citation));
                    }
                }
            }

            let Some(mr) = &article.machine_readable else {
                continue;
            };

            // `source.regulation` — a computed dependency, the strongest edge
            // the corpus states.
            if let Some(exec) = &mr.execution {
                for input in &exec.input {
                    let Some(regulation) =
                        input.source.as_ref().and_then(|s| s.regulation.as_deref())
                    else {
                        continue;
                    };
                    let Some(&target) = by_slug.get(regulation).or_else(|| by_bwb.get(regulation))
                    else {
                        dangling += 1;
                        continue;
                    };
                    law_edges.push((law, target, EdgeType::Source));
                    if let Some(src) = src_article {
                        art_edges.push((src, target, EdgeType::Source));
                    }
                }
            }

            // `implements` — the lower regulation filling in a higher law.
            for imp in &mr.implements {
                let Some(target_law_id) = imp.law.as_deref() else {
                    continue;
                };
                let Some(&target_law) = by_slug
                    .get(target_law_id)
                    .or_else(|| by_bwb.get(target_law_id))
                else {
                    dangling += 1;
                    continue;
                };
                law_edges.push((law, target_law, EdgeType::Delegation));
                if opts.articles {
                    let target = imp
                        .article
                        .as_deref()
                        .and_then(|a| article_ix.get(&(target_law, a.to_string())).copied())
                        .unwrap_or(target_law);
                    if let Some(src) = src_article {
                        art_edges.push((src, target, EdgeType::Delegation));
                    }
                }
            }

            // `open_terms` — a term the law leaves open. Nothing to draw when
            // an implementer exists (the delegation edge already says it); a
            // named-but-unharvested invuller becomes an expected node; an
            // unnamed one is decision room and gets no edge at all.
            for term in &mr.open_terms {
                if term.delegated_to.is_none() && term.delegation_type.is_none() {
                    continue;
                }
                let expected_key = expected_key(
                    term.expected_source.as_deref(),
                    term.delegated_to.as_deref(),
                );
                let Some(key) = expected_key else { continue };
                if by_slug.contains_key(&key) {
                    continue;
                }
                let layer = term
                    .delegation_type
                    .as_deref()
                    .map(RegulatoryLayer::parse)
                    .unwrap_or(RegulatoryLayer::Onbekend);
                let label = term
                    .expected_source
                    .clone()
                    .or_else(|| term.delegated_to.clone())
                    .unwrap_or_else(|| key.clone());
                let target = graph.intern(Node {
                    id: format!("expected:{key}"),
                    label,
                    kind: NodeKind::Expected,
                    layer,
                    bwb_id: None,
                    valid_from: None,
                    parent: None,
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                    in_refs: 0,
                    out_refs: 0,
                    citers: 0,
                    enrichment: Enrichment::None,
                    articles: 0,
                    articles_enriched: 0,
                    rank: 0.0,
                    cluster: 0,
                    framework: false,
                });
                law_edges.push((law, target, EdgeType::ExpectedDelegation));
                if let Some(src) = src_article {
                    art_edges.push((src, target, EdgeType::ExpectedDelegation));
                }
            }
        }
    }

    // An expected node that later turns out to be a harvested law would be a
    // duplicate; the corpus is scanned first precisely so that cannot happen.
    graph.stats.raw_references = raw_refs;
    graph.stats.dangling_references = dangling;
    graph.stats.laws_partly_enriched = graph.nodes[..graph.stats.laws]
        .iter()
        .filter(|n| n.enrichment != Enrichment::None)
        .count();
    graph.stats.laws_fully_enriched = graph.nodes[..graph.stats.laws]
        .iter()
        .filter(|n| n.enrichment == Enrichment::Full && n.articles > 0)
        .count();
    graph.stats.enriched_articles = graph.nodes[..graph.stats.laws]
        .iter()
        .map(|n| n.articles_enriched as usize)
        .sum();
    graph.stats.external_nodes = graph
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::External)
        .count();
    graph.stats.expected_nodes = graph
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Expected)
        .count();

    let mut edges = aggregate(law_edges);
    edges.extend(aggregate(art_edges));
    graph.stats.aggregated_edges = edges.len();
    graph.edges = edges;
    graph.canonicalise();

    // Reference counts come from the underlying counts, not from the number of
    // aggregated edges: "hoe vaak wordt deze wet aangehaald" counts references,
    // not neighbours. Only law-level edges count, so turning `--articles` on
    // does not silently double every number.
    for edge in &graph.edges[..graph.law_edge_count] {
        graph.nodes[edge.source as usize].out_refs += edge.count;
        graph.nodes[edge.target as usize].in_refs += edge.count;
    }
}

fn intern_external(
    graph: &mut CorpusGraph,
    by_bwb: &mut HashMap<String, NodeIx>,
    bwb: &str,
) -> NodeIx {
    let ix = graph.intern(Node {
        id: format!("bwb:{bwb}"),
        label: bwb.to_string(),
        kind: NodeKind::External,
        layer: RegulatoryLayer::Onbekend,
        bwb_id: Some(bwb.to_string()),
        valid_from: None,
        parent: None,
        x: 0.0,
        y: 0.0,
        z: 0.0,
        in_refs: 0,
        out_refs: 0,
        citers: 0,
        enrichment: Enrichment::None,
        articles: 0,
        articles_enriched: 0,
        rank: 0.0,
        cluster: 0,
        framework: false,
    });
    by_bwb.insert(bwb.to_string(), ix);
    ix
}

/// Normalise a delegation target into a key an expected node can be identified
/// by.
///
/// This is the cheap half of open keuze 10 in the design and it is exactly as
/// unreliable as the design says: two laws naming the same regulation with
/// different words produce two nodes. Normalising the title is what we can do
/// without a resolve step, and the key is written into the node id so the
/// mistake is visible rather than silent.
fn expected_key(expected_source: Option<&str>, delegated_to: Option<&str>) -> Option<String> {
    let raw = expected_source.or(delegated_to)?.trim();
    if raw.is_empty() {
        return None;
    }
    let mut key = String::with_capacity(raw.len());
    let mut last_sep = true;
    for ch in raw.chars().flat_map(|c| c.to_lowercase()) {
        if ch.is_alphanumeric() {
            key.push(ch);
            last_sep = false;
        } else if !last_sep {
            key.push('_');
            last_sep = true;
        }
    }
    let key = key.trim_matches('_').to_string();
    (!key.is_empty()).then_some(key)
}

/// Collapse raw relations onto `(source, target, type)` with a count.
///
/// Sorting rather than hashing: at five million references the sort is faster
/// and, more to the point, the output order is canonical without a second pass.
fn aggregate(mut raw: Vec<RawEdge>) -> Vec<Edge> {
    raw.sort_unstable();
    let mut out: Vec<Edge> = Vec::new();
    for (source, target, edge_type) in raw {
        match out.last_mut() {
            Some(last)
                if last.source == source
                    && last.target == target
                    && last.edge_type == edge_type =>
            {
                last.count += 1;
            }
            _ => out.push(Edge {
                source,
                target,
                edge_type,
                count: 1,
            }),
        }
    }
    out
}

fn humanise(slug: &str) -> String {
    let mut out = String::with_capacity(slug.len());
    for (i, part) in slug.split('_').enumerate() {
        if i > 0 {
            out.push(' ');
        }
        if i == 0 {
            let mut chars = part.chars();
            if let Some(first) = chars.next() {
                out.extend(first.to_uppercase());
                out.push_str(chars.as_str());
            }
        } else {
            out.push_str(part);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_stems_are_recognised() {
        assert!(is_version_stem("2025-01-01"));
        assert!(!is_version_stem("status"));
        assert!(!is_version_stem("2025-01-0"));
        assert!(!is_version_stem("20xx-01-01"));
    }

    #[test]
    fn expected_keys_normalise_titles() {
        assert_eq!(
            expected_key(Some("Regeling  zorgverzekering!"), None).as_deref(),
            Some("regeling_zorgverzekering")
        );
        assert_eq!(
            expected_key(None, Some("minister")).as_deref(),
            Some("minister")
        );
        assert_eq!(expected_key(None, None), None);
        assert_eq!(expected_key(Some("   "), None), None);
    }

    #[test]
    fn aggregation_counts_and_orders() {
        let raw = vec![
            (1, 2, EdgeType::Citation),
            (0, 1, EdgeType::Citation),
            (1, 2, EdgeType::Citation),
            (1, 2, EdgeType::Source),
        ];
        let out = aggregate(raw);
        assert_eq!(out.len(), 3);
        assert_eq!(out[0].source, 0);
        assert_eq!(out[1].count, 2);
        assert_eq!(out[1].edge_type, EdgeType::Citation);
        assert_eq!(out[2].edge_type, EdgeType::Source);
    }
}
