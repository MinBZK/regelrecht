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
mod render;
mod syn_pass;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use model::Model;

/// Default, repo-relative location of the committed model.
const DEFAULT_OUT: &str = "docs/src/content/architecture/model.json";

enum Command {
    Generate,
    Check,
}

/// Which crates get the deep source-level pass. The default is every crate:
/// the architecture site renders the deep structure (modules/types/methods) for
/// all ten crates. `--deep a,b` narrows it to a subset (e.g. for a quick run).
enum DeepScope {
    /// An explicit `--deep` list — only these crates get the deep pass.
    Only(Vec<String>),
    /// Every workspace crate (the default, also selectable with `--deep-all`).
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
    let mut deep = DeepScope::All;

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

    // The derived docs pages (C4 views + per-crate pages). Only written when the
    // model goes to its default committed location — a `--stdout`/`--out` run is
    // a one-off inspection and must not scatter page files.
    let pages = render::render(&mvalue);
    let default_out = args.out.is_none();

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
            if default_out {
                for page in &pages {
                    let path = repo_root.join(&page.rel_path);
                    if let Some(dir) = path.parent() {
                        std::fs::create_dir_all(dir)?;
                    }
                    std::fs::write(&path, &page.content)?;
                }
                eprintln!(
                    "arch-extract: wrote {} page(s) → {}",
                    pages.len(),
                    render::PAGES_DIR
                );
            }
            Ok(ExitCode::SUCCESS)
        }
        Command::Check => {
            let out = resolve_out(args.out, &repo_root);
            let mut stale: Vec<String> = Vec::new();

            let existing = std::fs::read_to_string(&out).unwrap_or_default();
            if existing != json {
                stale.push(out.display().to_string());
            }
            if default_out {
                for page in &pages {
                    let path = repo_root.join(&page.rel_path);
                    let existing = std::fs::read_to_string(&path).unwrap_or_default();
                    if existing != page.content {
                        stale.push(page.rel_path.clone());
                    }
                }
            }

            if stale.is_empty() {
                eprintln!("arch-extract: model and generated pages are up to date");
                Ok(ExitCode::SUCCESS)
            } else {
                eprintln!(
                    "arch-extract: stale — run `just arch-generate` and commit the result:\n  {}",
                    stale.join("\n  ")
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
