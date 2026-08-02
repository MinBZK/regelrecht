//! `regelrecht-graph-build` — turn a corpus checkout into a graph payload.
//!
//! ```text
//! regelrecht-graph-build --corpus ../regelrecht-corpus --out graaf.rrgraph
//! regelrecht-graph-build --corpus … --format json --out graaf.json
//! regelrecht-graph-build --corpus … --articles --peildatum 2026-01-01
//! ```
//!
//! Writes the payload and prints a one-screen report: counts, timings, peak
//! memory, and the laws with the most incoming references. That last list is
//! not decoration. It is how you check whether the framework threshold is set
//! anywhere near right, and the design says explicitly that the threshold is a
//! guess that has to be adjusted against real data.

use std::path::PathBuf;
use std::process::ExitCode;

use regelrecht_graph::graph::NodeKind;
use regelrecht_graph::{build, cluster, layout, metrics, payload, Options};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "-h" || a == "--help") {
        print!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    let options = match parse(&args) {
        Ok(options) => options,
        Err(message) => {
            eprintln!("fout: {message}\n\n{USAGE}");
            return ExitCode::FAILURE;
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

    let citers = metrics::compute(&mut graph, options.cli.framework);
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
        "kaderwetten       {}   gemeenschappen {}",
        s.framework_laws, s.clusters
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

    println!("\nzwaarste sterren (aantal wetten dat naar deze regeling verwijst):");
    for (ix, count) in metrics::top_by_citers(graph, citers, 15) {
        let node = graph.node(ix);
        println!(
            "  {count:>6}  {}{}  {}",
            if node.framework {
                "[kader] "
            } else {
                "        "
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
}

fn parse(args: &[String]) -> Result<Cli, String> {
    let mut options = Options::default();
    options.build.peildatum = chrono::Utc::now().format("%Y-%m-%d").to_string();
    options.build.threads = std::thread::available_parallelism()
        .map(|n| n.get().min(8))
        .unwrap_or(4);
    let mut out = PathBuf::from("corpusgraaf.rrgraph");
    let mut json = false;

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
            "--uniform-weights" => options.layout.dissuade_hubs = false,
            "--linear-attraction" => options.layout.linlog = false,
            "--degree-mass" => options.layout.log_mass = false,
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
            "--framework-fraction" => {
                options.framework.fraction = value()?
                    .parse()
                    .map_err(|_| "--framework-fraction verwacht een breuk")?
            }
            "--framework-min" => {
                options.framework.min_citers = value()?
                    .parse()
                    .map_err(|_| "--framework-min verwacht een getal")?
            }
            other => return Err(format!("onbekende optie {other}")),
        }
        i += 1;
    }
    Ok(Cli {
        cli: options,
        out,
        json,
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
  --uniform-weights     dissuade-hubs uit (om te laten zien wat het doet)
  --linear-attraction   lineaire aantrekking in plaats van logaritmische
  --degree-mass         ruwe graad als afstotingsmassa in plaats van log-graad
  --cluster-pull <f>    veerstijfheid richting het clustermidden (standaard 0.02)
  --framework-fraction  aandeel wetten dat naar een wet moet verwijzen voordat
                        die als kaderwet telt (standaard 0.05)
  --framework-min       ondergrens in aantal verwijzende wetten (standaard 25)
  --iterations <n>      ForceAtlas2-iteraties (standaard 300)
  --gravity <f>         zwaartekracht naar de oorsprong (standaard 1.0)
  --repulsion <f>       afstotingssterkte (standaard 1.0)
  --theta <f>           Barnes-Hut-openingshoek (standaard 1.2)
  --threads <n>         leesthreads (standaard min(cores, 8))
";
