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
//! `--session-reuse` says whether the calls in this window share one agent
//! session: `window` (the default — every call continues the same session,
//! and each resumed feedback prompt opens by ordering the agent to read the
//! file again), `repair` (only the schema gate continues; the two gates that
//! ask for judgement stay cold) or `off` (every call its own cold process,
//! the behaviour before this existed). The run prints what every call cost, so
//! the two modes can be held against each other on the same law.
//!
//! `--rounds` sets how many feedback rounds a gate may run, either one number
//! for all three or per gate (`schema=1,checks=2`). The run prints what each
//! round did per gate, with the marking count beside the finding count: a
//! round can lower the findings by translating better and by declaring more
//! of the law unmodellable, and the two must not read the same.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;

use regelrecht_pipeline::enrich::{
    execute_enrich_with_runner, AgentCallRecord, AgentUsage, EnrichConfig, EnrichPayload,
    FeedbackRounds, LlmProvider, ProcessLlmRunner, SessionReuse,
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
    session_reuse: SessionReuse,
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
    let mut session_reuse = SessionReuse::default();

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
            "--session-reuse" => session_reuse = SessionReuse::parse(&value("--session-reuse")?)?,
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
                     [--rounds 2|checks=2,marking=2] [--article 2.1.i] \
                     [--session-reuse window|repair|off]"
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
        session_reuse,
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

    // The agent reads its instructions from `.claude/skills/` under the corpus
    // root. Nothing put them there: `ensure_skills` is called from the worker
    // and never from here, so every experiment run so far handed the agent the
    // prompt and nothing else. It wrote `untranslatables` into a v0.6.0 file
    // because that is what it knew, and the schema gate had to repair what an
    // instruction would have prevented.
    //
    // Refusing is the point. A run without skills produces a plausible corpus
    // and an unusable measurement, and the difference is invisible afterwards.
    if let Err(e) = regelrecht_pipeline::enrich::ensure_skills(&args.corpus).await {
        eprintln!(
            "could not place skills under {}: {e}",
            args.corpus.display()
        );
        return ExitCode::from(2);
    }
    if let Err(missing) = skills_present(&args.corpus) {
        eprintln!(
            "the corpus root carries no agent instructions: {missing} is absent.\n\
             Point SKILLS_DIR at a checkout that has .claude/skills, e.g.\n\
             \x20 SKILLS_DIR=/path/to/regelrecht enrich-once --corpus {} ...",
            args.corpus.display()
        );
        return ExitCode::from(2);
    }

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
        args.session_reuse,
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
            print_cost(&result.session_reuse, &result.agent_calls, result.usage);
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

/// Print what the window cost: one line per call, then the total.
///
/// Per call and not only per window, because the question this setting exists
/// to answer is which call got cheaper. A resumed round beside a cold one, on
/// the same law and in the same window, is the only comparison that settles
/// whether continuing the session paid for itself.
fn print_cost(mode: &str, calls: &[AgentCallRecord], total: Option<AgentUsage>) {
    if calls.is_empty() {
        return;
    }
    println!("\n=== cost (session reuse: {mode})");
    println!(
        "  {:<10} {:>8} {:>10} {:>10} {:>12} {:>9}",
        "step", "session", "input", "output", "cache read", "cost"
    );
    for call in calls {
        let (input, output, cache, cost) = call.usage.map_or((0, 0, 0, 0), |u| {
            (
                u.input_tokens,
                u.output_tokens,
                u.cache_read_tokens,
                u.cost_millicents,
            )
        });
        println!(
            "  {:<10} {:>8} {:>10} {:>10} {:>12} {:>9}",
            call.step,
            if call.resumed { "resumed" } else { "cold" },
            input,
            output,
            cache,
            money(cost)
        );
    }
    if let Some(u) = total {
        println!(
            "  {:<10} {:>8} {:>10} {:>10} {:>12} {:>9}",
            "window",
            calls.len(),
            u.input_tokens,
            u.output_tokens,
            u.cache_read_tokens,
            money(u.cost_millicents)
        );
    }
}

/// Tenths of a cent as dollars, because that is the unit the provider reports
/// and an integer is the only honest way to carry money through a struct.
fn money(millicents: u64) -> String {
    format!("${}.{:04}", millicents / 100_000, millicents % 100_000 / 10)
}

/// Whether the corpus root carries the instructions the agent needs.
///
/// Checked by path rather than by content: the prompt names these files and
/// the agent opens them itself, so their absence is silent on both sides.
fn skills_present(corpus: &Path) -> Result<(), String> {
    for required in [
        ".claude/skills/law-generate/SKILL.md",
        ".claude/skills/law-generate/reference.md",
    ] {
        if !corpus.join(required).exists() {
            return Err(required.to_string());
        }
    }
    Ok(())
}
