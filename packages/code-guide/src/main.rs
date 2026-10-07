//! `code-guide` — a guide to the regelrecht Rust workspace, layered by the
//! calls between its functions as rust-analyzer resolves them.
//!
//! ```text
//! code-guide index [--force] [--manifest-path <p>]   build the cached index
//! code-guide serve [--port <n>] [--ui-dir <d>] [--manifest-path <p>]
//! code-guide stats [--manifest-path <p>]             what the index yields
//! ```
//!
//! Run from `packages/` (as the `just code-guide*` recipes do) so cargo finds
//! the workspace. See README.md.

mod index;
mod layers;
mod model;
mod serve;
mod source;
#[cfg(test)]
mod testutil;
mod views;
mod workspace;

use std::path::PathBuf;
use std::process::ExitCode;

use protobuf::Message;

/// The model of the workspace, from the cached index and the sources.
pub fn build_model(ws: &workspace::Workspace) -> Result<model::Model, String> {
    let bytes = std::fs::read(&ws.paths.index_file).map_err(|e| {
        format!(
            "no index at {} ({e}); run `{}`",
            ws.paths.index_file.display(),
            index::REBUILD_COMMAND
        )
    })?;
    let index = scip::types::Index::parse_from_bytes(&bytes)
        .map_err(|e| format!("reading the index: {e}"))?;
    // The scan taken with the index, never a fresh one: the index's lines are
    // only ever read against the sources they were made from.
    let incomplete = || {
        format!(
            "the index at {} is incomplete; run `{} --force`",
            ws.paths.index_file.display(),
            index::REBUILD_COMMAND
        )
    };
    let snapshot = std::fs::read_to_string(&ws.paths.sources_file).map_err(|_| incomplete())?;
    let sources: source::Sources = serde_json::from_str(&snapshot).map_err(|_| incomplete())?;
    let changed = index::changed_files(&ws.paths)
        .map_err(|e| e.to_string())?
        .ok_or_else(incomplete)?;
    Ok(model::Model::build(
        &index,
        ws.crates.clone(),
        &sources,
        &changed,
    ))
}

struct Args {
    command: String,
    force: bool,
    port: Option<u16>,
    ui_dir: Option<PathBuf>,
    manifest_path: Option<PathBuf>,
}

fn parse_args() -> Result<Args, String> {
    let mut it = std::env::args().skip(1);
    let command = it.next().unwrap_or_else(|| "help".to_string());
    let mut args = Args {
        command,
        force: false,
        port: None,
        ui_dir: None,
        manifest_path: None,
    };
    while let Some(a) = it.next() {
        match a.as_str() {
            "--force" => args.force = true,
            "--port" => {
                let v = it.next().ok_or("--port needs a value")?;
                args.port = Some(v.parse().map_err(|_| format!("invalid --port: {v}"))?);
            }
            "--ui-dir" => {
                args.ui_dir = Some(PathBuf::from(it.next().ok_or("--ui-dir needs a value")?))
            }
            "--manifest-path" => {
                args.manifest_path = Some(PathBuf::from(
                    it.next().ok_or("--manifest-path needs a value")?,
                ));
            }
            other => return Err(format!("unexpected argument: {other}")),
        }
    }
    Ok(args)
}

fn run() -> Result<(), String> {
    let args = parse_args()?;
    match args.command.as_str() {
        "index" => {
            let ws = workspace::load(args.manifest_path.as_deref())?;
            let started = std::time::Instant::now();
            let dirs: Vec<String> = ws.crates.iter().map(|c| c.dir.clone()).collect();
            if index::build(&ws.paths, &dirs, args.force)? {
                eprintln!(
                    "code-guide: indexed the workspace in {:.0?} -> {}",
                    started.elapsed(),
                    ws.paths.index_file.display()
                );
            } else {
                eprintln!(
                    "code-guide: the index is up to date ({})",
                    ws.paths.index_file.display()
                );
            }
            Ok(())
        }
        "stats" => {
            let ws = workspace::load(args.manifest_path.as_deref())?;
            let version = index::ra_version()?;
            let state = index::state(&ws.paths, &version).map_err(|e| e.to_string())?;
            println!("index: {state:?} ({})", ws.paths.index_file.display());
            let model = build_model(&ws)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&model.counts).map_err(|e| e.to_string())?
            );
            Ok(())
        }
        "serve" => {
            let ws = workspace::load(args.manifest_path.as_deref())?;
            serve::run(ws, args.port, args.ui_dir)
        }
        _ => {
            println!(
                "code-guide index [--force] [--manifest-path <p>]\n\
                 code-guide serve [--port <n>] [--ui-dir <dir>] [--manifest-path <p>]\n\
                 code-guide stats [--manifest-path <p>]"
            );
            Ok(())
        }
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("code-guide: {e}");
            ExitCode::FAILURE
        }
    }
}
