//! `regelrecht-graph-build` — turn a corpus checkout into a graph payload.
//!
//! ```text
//! regelrecht-graph-build --corpus ../regelrecht-corpus --out graaf.rrgraph
//! regelrecht-graph-build --corpus … --format json --out graaf.json
//! regelrecht-graph-build --corpus … --articles --peildatum 2026-01-01
//! ```
//!
//! Writes the payload and prints a one-screen report: counts, timings, peak
//! memory, and the laws the corpus leans on hardest. That last list is working
//! material for whoever fills the kaderwetlijst, and nothing else. It is not a
//! verdict and the builder does not read a qualification off it; see
//! [`regelrecht_graph::kaderwet`] for why.

use std::path::PathBuf;
use std::process::ExitCode;

use regelrecht_graph::graph::NodeKind;
use regelrecht_graph::kaderwet::{Kaderwetten, DEFAULT_FILE};
use regelrecht_graph::{build, cluster, layout, metrics, payload, Options};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "-h" || a == "--help") {
        print!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    let mut options = match parse(&args) {
        Ok(options) => options,
        Err(message) => {
            eprintln!("fout: {message}\n\n{USAGE}");
            return ExitCode::FAILURE;
        }
    };

    // The designation list. A missing file is not an error and not silently an
    // empty list either: the report says which of the two happened, because
    // "nobody has designated anything" and "the list was not found" have very
    // different consequences for what you are looking at.
    let list_path = options
        .kaderwetten_path
        .clone()
        .unwrap_or_else(|| options.cli.build.root.join(DEFAULT_FILE));
    let herkomst = match Kaderwetten::load(&list_path) {
        Ok(list) => {
            let herkomst = format!(
                "{} ({} aangewezen, beheerder: {})",
                list_path.display(),
                list.len(),
                list.beheerder.as_deref().unwrap_or("niet belegd")
            );
            options.cli.kaderwetten = list;
            herkomst
        }
        Err(err) => {
            eprintln!("let op: {err}. Geen enkele wet is als kaderwet aangewezen.");
            format!("{err}; niets aangewezen")
        }
    };

    let started = std::time::Instant::now();
    let mut graph = build::build(&options.cli.build);
    eprintln!(
        "gelezen: {} bestanden, {} mislukt, {:.1}s",
        graph.stats.files_parsed,
        graph.stats.files_failed,
        graph.stats.parse_ms as f64 / 1000.0
    );

    graph.stats.kaderwetlijst = herkomst;
    let citers = metrics::compute(&mut graph, &options.cli.kaderwetten);
    if !options.cli.skip_clusters {
        let started = std::time::Instant::now();
        cluster::assign(&mut graph);
        graph.stats.cluster_ms = started.elapsed().as_millis();
    }
    if !options.cli.skip_layout {
        let started = std::time::Instant::now();
        layout::apply(&mut graph, &options.cli.layout);
        graph.stats.layout_ms = started.elapsed().as_millis();
    }
    graph.stats.peak_rss_kb = regelrecht_graph::peak_rss_kb();

    let built_at = chrono::Utc::now().to_rfc3339();
    let bytes = if options.json {
        match serde_json::to_vec_pretty(&payload::encode_json(&graph, &built_at)) {
            Ok(bytes) => bytes,
            Err(err) => {
                eprintln!("fout: kon JSON niet schrijven: {err}");
                return ExitCode::FAILURE;
            }
        }
    } else {
        payload::encode_binary(&graph, &built_at)
    };
    if let Err(err) = std::fs::write(&options.out, &bytes) {
        eprintln!("fout: kon {} niet schrijven: {err}", options.out.display());
        return ExitCode::FAILURE;
    }

    report(&graph, &citers, &bytes, started.elapsed());
    ExitCode::SUCCESS
}

fn report(
    graph: &regelrecht_graph::CorpusGraph,
    citers: &[u32],
    bytes: &[u8],
    elapsed: std::time::Duration,
) {
    let s = &graph.stats;
    println!("--- corpusgraaf ---");
    println!("snapshot          {}", payload::snapshot_id(graph));
    println!("layout            {}", payload::LAYOUT_VERSION);
    println!(
        "bestanden         {} gescand, {} gelezen, {} mislukt",
        s.files_scanned, s.files_parsed, s.files_failed
    );
    println!(
        "knopen            {} totaal ({} wetniveau: {} wetten, {} extern, {} verwacht; {} artikelen)",
        graph.nodes.len(),
        graph.law_node_count,
        graph
            .nodes
            .iter()
            .filter(|n| n.kind == NodeKind::Law)
            .count(),
        s.external_nodes,
        s.expected_nodes,
        s.articles
    );
    println!(
        "kanten            {} geaggregeerd ({} wetniveau) uit {} ruwe verwijzingen, {} zonder doel",
        graph.edges.len(),
        graph.law_edge_count,
        s.raw_references,
        s.dangling_references
    );
    println!(
        "verrijking        {} van {} wetten deels verrijkt, {} volledig; {} van {} artikelen gemodelleerd",
        s.laws_partly_enriched,
        s.laws,
        s.laws_fully_enriched,
        s.enriched_articles,
        graph.nodes[..graph.law_node_count]
            .iter()
            .map(|n| n.articles as usize)
            .sum::<usize>()
    );
    println!("kaderwetlijst     {}", s.kaderwetlijst);
    println!(
        "kaderwetten       {} ({} aangewezen, {} uit een toepasselijkheidsrelatie)   gemeenschappen {}",
        s.framework_laws, s.designated_framework_laws, s.derived_framework_laws, s.clusters
    );
    println!(
        "uitgeconvergeerd  {:.5} (verplaatsing in de laatste iteratie, als deel van de schaal)",
        s.layout_unsettled
    );
    println!(
        "spreiding         {:.3} (gemiddelde afstand tot zwaartepunt / grootste afstand)",
        layout::dispersion(graph)
    );
    println!(
        "tijd              lezen {:.1}s, metrieken {:.1}s, clusteren {:.1}s, layout {:.1}s, totaal {:.1}s",
        s.parse_ms as f64 / 1000.0,
        s.metrics_ms as f64 / 1000.0,
        s.cluster_ms as f64 / 1000.0,
        s.layout_ms as f64 / 1000.0,
        elapsed.as_secs_f64()
    );
    println!(
        "geheugen          {:.0} MB piek",
        s.peak_rss_kb as f64 / 1024.0
    );
    println!(
        "payload           {:.1} MB",
        bytes.len() as f64 / 1_048_576.0
    );

    if !s.failures.is_empty() {
        println!("\nniet gelezen (deze wetten staan niet op de kaart):");
        for failure in s.failures.iter().take(10) {
            println!("  {}: {}", failure.path.display(), failure.error);
        }
    }

    println!(
        "\nwaar het corpus het zwaarst op leunt (aantal wetten dat ernaar verwijst).\n\
         Werkmateriaal voor wie de kaderwetlijst vult, geen kwalificatie: een wet is een\n\
         kaderwet omdat iemand dat vaststelt, niet omdat dit getal hoog is."
    );
    for (ix, count) in metrics::top_by_citers(graph, citers, 15) {
        let node = graph.node(ix);
        println!(
            "  {count:>6}  {}{}  {}",
            if node.framework {
                "[aangewezen] "
            } else {
                "             "
            },
            node.bwb_id.as_deref().unwrap_or("-"),
            node.label
        );
    }
}

struct Cli {
    cli: Options,
    out: PathBuf,
    json: bool,
    kaderwetten_path: Option<PathBuf>,
}

fn parse(args: &[String]) -> Result<Cli, String> {
    let mut options = Options::default();
    options.build.peildatum = chrono::Utc::now().format("%Y-%m-%d").to_string();
    options.build.threads = std::thread::available_parallelism()
        .map(|n| n.get().min(8))
        .unwrap_or(4);
    let mut out = PathBuf::from("corpusgraaf.rrgraph");
    let mut json = false;
    let mut kaderwetten_path: Option<PathBuf> = None;

    let mut i = 0;
    while i < args.len() {
        let arg = args[i].as_str();
        let mut value = || -> Result<String, String> {
            i += 1;
            args.get(i)
                .cloned()
                .ok_or_else(|| format!("{arg} verwacht een waarde"))
        };
        match arg {
            "--corpus" => options.build.root = PathBuf::from(value()?),
            "--out" => out = PathBuf::from(value()?),
            "--peildatum" => options.build.peildatum = value()?,
            "--threads" => {
                options.build.threads = value()?
                    .parse()
                    .map_err(|_| "--threads verwacht een getal")?
            }
            "--iterations" => {
                options.layout.iterations = value()?
                    .parse()
                    .map_err(|_| "--iterations verwacht een getal")?
            }
            "--format" => json = value()? == "json",
            "--articles" => options.build.articles = true,
            "--all-versions" => options.build.all_versions = true,
            "--no-external" => options.build.external_nodes = false,
            "--no-clusters" => options.skip_clusters = true,
            "--no-layout" => options.skip_layout = true,
            "--flat-init" => options.layout.hierarchical_init = false,
            "--cluster-pull" => {
                options.layout.cluster_pull = value()?
                    .parse()
                    .map_err(|_| "--cluster-pull verwacht een getal")?
            }
            "--gravity" => {
                options.layout.gravity = value()?
                    .parse()
                    .map_err(|_| "--gravity verwacht een getal")?
            }
            "--repulsion" => {
                options.layout.repulsion = value()?
                    .parse()
                    .map_err(|_| "--repulsion verwacht een getal")?
            }
            "--theta" => {
                options.layout.theta = value()?.parse().map_err(|_| "--theta verwacht een getal")?
            }
            "--kaderwetten" => kaderwetten_path = Some(PathBuf::from(value()?)),
            other => return Err(format!("onbekende optie {other}")),
        }
        i += 1;
    }
    Ok(Cli {
        cli: options,
        out,
        json,
        kaderwetten_path,
    })
}

const USAGE: &str = "\
regelrecht-graph-build — corpus naar knopen, kanten en een voorberekende layout

  --corpus <pad>        wortel van de corpus-checkout (map met regulation/)
  --out <pad>           doelbestand (standaard corpusgraaf.rrgraph)
  --format <bin|json>   payloadvorm (standaard bin)
  --peildatum <datum>   welke versie van een wet meetelt (standaard vandaag)
  --all-versions        elke versie een eigen knoop in plaats van een per wet
  --articles            ook artikelknopen en artikelkanten
  --no-external         geen knoop voor een BWB-nummer buiten het corpus
  --no-clusters         gemeenschapsdetectie overslaan
  --no-layout           layout overslaan
  --flat-init           de hele component in een keer inbedden in plaats van
                        eerst de gemeenschappen (langzamer, zelfde eindstand)
  --cluster-pull <f>    veerstijfheid richting het clustermidden (standaard 0,
                        want het is een kracht die niet in de graaf zit)
  --kaderwetten <pad>   de lijst met aangewezen kaderwetten
                        (standaard <corpus>/kaderwetten.yaml)
  --iterations <n>      ForceAtlas2-iteraties (standaard 1500)
  --gravity <f>         zwaartekracht naar de oorsprong (standaard 1.0)
  --repulsion <f>       afstotingssterkte (standaard 1.0)
  --theta <f>           Barnes-Hut-openingshoek (standaard 1.2)
  --threads <n>         leesthreads (standaard min(cores, 8))
";
