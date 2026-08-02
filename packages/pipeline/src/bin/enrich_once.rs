//! Run one enrichment against a directory on disk, without a database and
//! without git.
//!
//! `execute_enrich_with_runner` needs a path and a payload, not a job queue
//! and not a checkout, so the real loop can run on a developer machine
//! against a single law. That is the difference between a design that can
//! be tried and one that can only be argued about: every layer of RFC-026
//! is a change to the worker, and this is how those changes get exercised
//! before they touch the pipeline.
//!
//! ```text
//! enrich-once --corpus <root> --law <relative/path.yaml> [--provider claude]
//!             [--model opus] [--effort medium] [--timeout 900]
//!             [--articles 15] [--rounds 2|checks=2,marking=2]
//!             [--article 2.1.i]
//! ```
//!
//! `--corpus` is the directory the law path is relative to, standing in for
//! the checkout the worker would make. Everything the worker writes beside
//! the law (`.enrichment.yaml`, the progress file, the result envelope)
//! lands there, so a second run continues where the first stopped, exactly
//! as it does in production.
//!
//! `--article` names one entry and enriches exactly that one: the run does
//! not walk the document and does not move the cursor, because repairing an
//! entry is not progress through the law. A number the law does not have
//! fails the run rather than enriching nothing in silence.
//!
//! `--rounds` sets how many feedback rounds a gate may run, either one number
//! for all three or per gate (`schema=1,checks=2`). The run prints what each
//! round did per gate, with the marking count beside the finding count: a
//! round can lower the findings by translating better and by declaring more
//! of the law unmodellable, and the two must not read the same.

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use regelrecht_pipeline::enrich::{
    execute_enrich_with_runner, EnrichConfig, EnrichPayload, FeedbackRounds, LlmProvider,
    ProcessLlmRunner,
};
use regelrecht_pipeline::enrich_v2::checks;

struct Args {
    corpus: PathBuf,
    law: String,
    provider: String,
    model: Option<String>,
    effort: Option<String>,
    timeout: u64,
    articles: usize,
    article: Option<String>,
    rounds: FeedbackRounds,
}

fn parse_args() -> Result<Args, String> {
    let mut corpus = None;
    let mut law = None;
    let mut provider = "claude".to_string();
    let mut model = None;
    let mut effort = None;
    let mut timeout = 900u64;
    let mut articles = 15usize;
    let mut article = None;
    let mut rounds = FeedbackRounds::default();

    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        let mut value = |name: &str| -> Result<String, String> {
            it.next().ok_or_else(|| format!("{name} requires a value"))
        };
        match arg.as_str() {
            "--corpus" => corpus = Some(PathBuf::from(value("--corpus")?)),
            "--law" => law = Some(value("--law")?),
            "--provider" => provider = value("--provider")?,
            "--model" => model = Some(value("--model")?),
            "--effort" => effort = Some(value("--effort")?),
            "--article" => article = Some(value("--article")?),
            "--rounds" => rounds = FeedbackRounds::parse(&value("--rounds")?)?,
            "--timeout" => {
                timeout = value("--timeout")?
                    .parse()
                    .map_err(|_| "--timeout wants a number of seconds".to_string())?;
            }
            "--articles" => {
                articles = value("--articles")?
                    .parse()
                    .map_err(|_| "--articles wants a number".to_string())?;
            }
            "--help" | "-h" => {
                return Err(
                    "usage: enrich-once --corpus <root> --law <path.yaml> [--provider claude] \
                     [--model opus] [--effort medium] [--timeout 900] [--articles 15] \
                     [--rounds 2|checks=2,marking=2] [--article 2.1.i]"
                        .to_string(),
                )
            }
            other => return Err(format!("unknown option: {other}")),
        }
    }

    Ok(Args {
        corpus: corpus.ok_or("--corpus is required")?,
        law: law.ok_or("--law is required")?,
        provider,
        model,
        effort,
        timeout,
        articles,
        article,
        rounds,
    })
}

#[tokio::main]
async fn main() -> ExitCode {
    let args = match parse_args() {
        Ok(a) => a,
        Err(msg) => {
            eprintln!("{msg}");
            return ExitCode::from(2);
        }
    };

    let before = report(&args, "before");

    let provider = match args.provider.as_str() {
        "claude" => LlmProvider::Claude {
            path: "claude".into(),
            model: args.model.clone(),
        },
        "opencode" => LlmProvider::OpenCode {
            path: "opencode".into(),
            model: args.model.clone(),
        },
        other => {
            eprintln!("unknown provider: {other}");
            return ExitCode::from(2);
        }
    };

    let config = EnrichConfig::for_local_run(
        provider,
        Duration::from_secs(args.timeout),
        args.articles,
        args.article.clone(),
        args.rounds,
        args.effort.clone(),
    );
    let payload = EnrichPayload {
        law_id: String::new(),
        yaml_path: args.law.clone(),
        provider: Some(args.provider.clone()),
        ..Default::default()
    };

    // `source_hash` guards against enriching a law whose text moved under
    // the previous run. Locally there is no base branch to compare with, so
    // an empty hash means "adopt whatever is here", which is what a first
    // run does in production too.
    // Without a subscriber the accounting line the worker emits goes nowhere,
    // and a round can then only be compared on wall clock. Default to info so
    // a plain run reports what it cost; RUST_LOG still overrides.
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_target(false)
        .init();

    let outcome =
        execute_enrich_with_runner(&payload, &args.corpus, &config, "", &ProcessLlmRunner).await;

    match outcome {
        Ok((result, changed)) => {
            println!("\n=== enrichment finished");
            println!(
                "  {} of {} articles carry machine_readable, coverage {:.2}",
                result.articles_with_machine_readable, result.articles_total, result.coverage_score
            );
            if !result.untranslatables.is_empty() {
                println!("  untranslatables: {}", result.untranslatables.len());
            }
            println!("  files touched: {}", changed.len());
            print_feedback(&result.feedback);
        }
        Err(e) => {
            println!("\n=== enrichment failed: {e}");
            report(&args, "after");
            return ExitCode::FAILURE;
        }
    }

    let after = report(&args, "after");
    println!("\n=== deterministic checks");
    println!("  schema errors {} → {}", before.0, after.0);
    println!("  findings      {} → {}", before.1, after.1);

    if after.0 > 0 {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

/// Print what every feedback round did, per gate.
///
/// One line per round, and each line carries both numbers: the findings the
/// round took away and the markings the file gained while doing it. A round
/// that halves the findings by declaring half the law unmodellable looks
/// identical to a good one on the finding count alone, which is the exact
/// confound this measurement has to survive.
fn print_feedback(feedback: &[regelrecht_pipeline::enrich::GateFeedback]) {
    if feedback.is_empty() {
        return;
    }
    println!("\n=== feedback rounds (findings | markings)");
    for gate in feedback {
        if gate.rounds.is_empty() {
            println!("  {:<8} no findings, no round run", gate.gate);
            continue;
        }
        println!(
            "  {:<8} {} → {} findings over {} round(s)",
            gate.gate,
            gate.findings_initial,
            gate.findings_final,
            gate.rounds.len()
        );
        for round in &gate.rounds {
            let markings = |n: Option<usize>| n.map_or("?".to_string(), |n| n.to_string());
            println!(
                "    round {}: findings {} → {} ({:+}) | markings {} → {} | file {} | {}",
                round.round,
                round.findings_before,
                round.findings_after,
                round.findings_after as i64 - round.findings_before as i64,
                markings(round.markings_before),
                markings(round.markings_after),
                if round.file_changed {
                    "changed"
                } else {
                    "unchanged"
                },
                match round.stopped {
                    None => "continues".to_string(),
                    Some(stop) => format!("stopped: {stop:?}"),
                }
            );
        }
    }
}

/// Run the deterministic checks and print a one-line summary. Returns
/// `(schema errors, findings)` so the two runs can be compared.
fn report(args: &Args, when: &str) -> (usize, usize) {
    let path = args.corpus.join(&args.law);
    let Ok(yaml) = std::fs::read_to_string(&path) else {
        println!("=== {when}: cannot read {}", path.display());
        return (0, 0);
    };
    let report = checks::run(&yaml, Some(&args.corpus));
    let counts = report.by_check();
    let summary: Vec<String> = counts.iter().map(|(k, v)| format!("{k}={v}")).collect();
    println!(
        "=== {when}: schema {}, {}",
        if report.schema.is_empty() {
            "valid".to_string()
        } else {
            format!("{} error(s)", report.schema.len())
        },
        if summary.is_empty() {
            "nothing to report".to_string()
        } else {
            summary.join(" ")
        }
    );
    for e in report.schema.iter().take(5) {
        println!("    schema: {e}");
    }
    (report.schema.len(), report.findings.len())
}
