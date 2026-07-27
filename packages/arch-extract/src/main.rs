//! `arch-extract` — generates the code-derived architecture model for the
//! regelrecht workspace.
//!
//! ```text
//! arch-extract [generate] [--out <path>] [--stdout] [--deep a,b | --deep-all] [--manifest-path <p>]
//! ```
//!
//! `generate` (the only, default command) writes the `model.json` that the local
//! architecture explorer (a separate tool) renders. The model is **generated
//! on-demand and never committed** — the explorer regenerates it from the working
//! tree, so it is always current by construction. `--stdout` prints the model
//! instead of writing it, handy for a quick inspection.
//!
//! Run it from `packages/` (as `just arch-generate` does) so `cargo metadata`
//! discovers the workspace; the repo root is derived from there and the model
//! defaults to `docs/src/content/architecture/model.json` (a gitignored path).

mod crate_graph;
mod model;
mod syn_pass;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use model::Model;

/// Default, repo-relative location the generated model is written to. This path
/// is gitignored: the model is a local, on-demand artifact, not committed.
const DEFAULT_OUT: &str = "docs/src/content/architecture/model.json";

/// Which crates get the deep source-level pass. The default is every crate:
/// the architecture explorer renders the deep structure (modules/types/methods)
/// for the whole workspace. `--deep a,b` narrows it to a subset (e.g. for a
/// quick run); `--deep-all` restores the default explicitly.
enum DeepScope {
    /// An explicit `--deep` list — only these crates get the deep pass.
    Only(Vec<String>),
    /// Every workspace crate (the default, also selectable with `--deep-all`).
    All,
}

struct Args {
    out: Option<PathBuf>,
    stdout: bool,
    manifest_path: Option<PathBuf>,
    deep: DeepScope,
}

fn parse_args() -> Result<Args, String> {
    let mut out = None;
    let mut stdout = false;
    let mut manifest_path = None;
    let mut deep = DeepScope::All;

    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "generate" => {}
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
                    "arch-extract [generate] [--out <path>] [--stdout] [--deep a,b | --deep-all] [--manifest-path <p>]"
                );
                std::process::exit(0);
            }
            other => return Err(format!("unexpected argument: {other}")),
        }
    }

    Ok(Args {
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

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(e) => {
            eprintln!("arch-extract: error: {e}");
            ExitCode::FAILURE
        }
    }
}
