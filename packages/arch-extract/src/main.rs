//! `arch-extract` — generates the code-derived architecture model for the
//! regelrecht workspace.
//!
//! ```text
//! arch-extract [generate|check] [--out <path>] [--stdout] [--manifest-path <p>]
//! ```
//!
//! * `generate` (default) writes the canonical `model.json`.
//! * `check` regenerates in memory and compares against the committed file,
//!   exiting non-zero on drift — the primitive a CI staleness gate wraps.
//!
//! Run it from `packages/` (as `just arch-generate` does) so `cargo metadata`
//! discovers the workspace; the repo root is derived from there and the model
//! defaults to `docs/src/content/architecture/model.json`.

mod crate_graph;
mod model;
mod syn_pass;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use model::Model;

/// Default, repo-relative location of the committed model.
const DEFAULT_OUT: &str = "docs/src/content/architecture/model.json";

/// Crates for which we run the deep source-level (`syn`) pass by default. The
/// crate graph always covers all 10 workspace crates; the deeper
/// module/type/method extraction is v1-scoped to these two (the ticket's Phase
/// 1). Pass `--deep-all` to extract every crate, or `--deep a,b` to override.
const DEFAULT_DEEP_CRATES: &[&str] = &["engine", "corpus"];

enum Command {
    Generate,
    Check,
}

/// Which crates get the deep source-level pass.
enum DeepScope {
    /// The default set (`DEFAULT_DEEP_CRATES`) or an explicit `--deep` list.
    Only(Vec<String>),
    /// Every workspace crate (`--deep-all`).
    All,
}

struct Args {
    command: Command,
    out: Option<PathBuf>,
    stdout: bool,
    manifest_path: Option<PathBuf>,
    deep: DeepScope,
}

fn parse_args() -> Result<Args, String> {
    let mut command = Command::Generate;
    let mut out = None;
    let mut stdout = false;
    let mut manifest_path = None;
    let mut deep = DeepScope::Only(DEFAULT_DEEP_CRATES.iter().map(|s| s.to_string()).collect());

    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "generate" => command = Command::Generate,
            "check" => command = Command::Check,
            "--stdout" => stdout = true,
            "--deep-all" => deep = DeepScope::All,
            "--deep" => {
                let list = it.next().ok_or("--deep needs a comma-separated value")?;
                deep = DeepScope::Only(list.split(',').map(|s| s.trim().to_string()).collect());
            }
            "--out" => {
                out = Some(PathBuf::from(it.next().ok_or("--out needs a value")?));
            }
            "--manifest-path" => {
                manifest_path = Some(PathBuf::from(
                    it.next().ok_or("--manifest-path needs a value")?,
                ));
            }
            "-h" | "--help" => {
                println!(
                    "arch-extract [generate|check] [--out <path>] [--stdout] [--deep a,b | --deep-all] [--manifest-path <p>]"
                );
                std::process::exit(0);
            }
            other => return Err(format!("unexpected argument: {other}")),
        }
    }

    Ok(Args {
        command,
        out,
        stdout,
        manifest_path,
        deep,
    })
}

fn build_model(
    manifest_path: Option<&Path>,
    deep: &DeepScope,
) -> Result<(Model, PathBuf), Box<dyn std::error::Error>> {
    let graph = crate_graph::load(manifest_path)?;

    let mut nodes = graph.nodes;
    let mut edges = graph.edges;
    for krate in &graph.crates {
        let deep_this = match deep {
            DeepScope::All => true,
            DeepScope::Only(list) => list.iter().any(|s| s == &krate.short),
        };
        if deep_this {
            syn_pass::extract_crate(&graph.repo_root, krate, &mut nodes, &mut edges);
        }
    }

    Ok((Model::new(nodes, edges), graph.repo_root))
}

fn resolve_out(out: Option<PathBuf>, repo_root: &Path) -> PathBuf {
    out.unwrap_or_else(|| repo_root.join(DEFAULT_OUT))
}

fn run() -> Result<ExitCode, Box<dyn std::error::Error>> {
    let args = parse_args()?;
    let (mvalue, repo_root) = build_model(args.manifest_path.as_deref(), &args.deep)?;
    let json = mvalue.to_json()?;

    match args.command {
        Command::Generate => {
            if args.stdout {
                print!("{json}");
                return Ok(ExitCode::SUCCESS);
            }
            let out = resolve_out(args.out, &repo_root);
            if let Some(dir) = out.parent() {
                std::fs::create_dir_all(dir)?;
            }
            std::fs::write(&out, &json)?;
            eprintln!(
                "arch-extract: wrote {} node(s), {} edge(s) → {}",
                mvalue.nodes.len(),
                mvalue.edges.len(),
                out.display()
            );
            Ok(ExitCode::SUCCESS)
        }
        Command::Check => {
            let out = resolve_out(args.out, &repo_root);
            let existing = std::fs::read_to_string(&out).unwrap_or_default();
            if existing == json {
                eprintln!("arch-extract: {} is up to date", out.display());
                Ok(ExitCode::SUCCESS)
            } else {
                eprintln!(
                    "arch-extract: {} is stale — run `just arch-generate` and commit the result",
                    out.display()
                );
                Ok(ExitCode::FAILURE)
            }
        }
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(e) => {
            eprintln!("arch-extract: error: {e}");
            ExitCode::FAILURE
        }
    }
}
