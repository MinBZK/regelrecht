//! Run the deterministic enrichment checks over one or more law files.
//!
//! No database, no git, no model, no shell: it reads the YAML, validates it
//! against the schema version the file declares, and reports what the
//! statutory text signals that the model does not carry.
//!
//! ```text
//! law-check [--corpus <root>] <file.yaml>...
//! ```
//!
//! `--corpus` enables the cross-law half of the binding check by pointing at
//! the directory that holds `<country>/<layer>/<law>/…` (in this repo:
//! `corpus/regulation`). Exit code is 1 when any file has schema errors, so
//! it can gate a pipeline step.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use regelrecht_pipeline::enrich_v2::checks;

fn main() -> ExitCode {
    let mut corpus: Option<PathBuf> = None;
    let mut files: Vec<PathBuf> = Vec::new();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--corpus" => match args.next() {
                Some(v) => corpus = Some(PathBuf::from(v)),
                None => {
                    eprintln!("--corpus requires a path");
                    return ExitCode::from(2);
                }
            },
            "--help" | "-h" => {
                println!("usage: law-check [--corpus <root>] <file.yaml>...");
                return ExitCode::SUCCESS;
            }
            other if other.starts_with('-') => {
                eprintln!("unknown option: {other}");
                return ExitCode::from(2);
            }
            other => files.push(PathBuf::from(other)),
        }
    }

    if files.is_empty() {
        eprintln!("usage: law-check [--corpus <root>] <file.yaml>...");
        return ExitCode::from(2);
    }

    let mut invalid = 0usize;
    for file in &files {
        match report_one(file, corpus.as_deref()) {
            Ok(has_schema_errors) => {
                if has_schema_errors {
                    invalid += 1;
                }
            }
            Err(e) => {
                eprintln!("{}: {e}", file.display());
                invalid += 1;
            }
        }
    }

    if invalid > 0 {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn report_one(path: &Path, corpus: Option<&Path>) -> std::io::Result<bool> {
    let yaml = std::fs::read_to_string(path)?;
    let report = checks::run(&yaml, corpus);

    println!("\n=== {}", path.display());
    if report.schema.is_empty() {
        println!("  schema: valid");
    } else {
        println!("  schema: {} error(s)", report.schema.len());
        for e in &report.schema {
            println!("    - {e}");
        }
    }

    // What the file attempted, beside what it got wrong. Without this a run
    // that translated less scores better, which is how round 3 flattered the
    // variant that laid no cross-law binding at all.
    if let Ok(doc) = serde_yaml_ng::from_str::<serde_yaml_ng::Value>(&yaml) {
        let t = checks::tally(&doc);
        println!(
            "  attempted: {}/{} articles with logic, {} marked only, {} bare",
            t.with_logic, t.articles, t.marked_only, t.bare
        );
        // The buckets moved between rounds; both definitions are printed so a
        // comparison with a round-4 figure does not need a footnote.
        println!(
            "  (round 4 buckets: {} with logic, {} marked only, {} bare)",
            t.with_logic_r4, t.marked_only_r4, t.bare_r4
        );
        println!(
            "  reaches:   {} cross-law bindings into {} of {} cited laws, \
             {} sources naming no law, {} inputs with no source",
            t.cross_law_bindings, t.laws_read, t.laws_cited, t.unnamed_sources, t.bare_inputs
        );
        println!(
            "  marks:     {} markings ({} operation, {} model, {} blocking, {} accepted)",
            t.markings,
            t.markings_operation,
            t.markings_model,
            t.markings_blocking,
            t.markings_accepted
        );
        println!(
            "  leaves:    {} open terms ({} delegated, {} naming nobody), \
             {} declares, {} overrides",
            t.open_terms,
            t.open_terms_delegated,
            t.open_terms_unattributed,
            t.declares,
            t.overrides
        );
        println!(
            "  outputs:   {} declared, {} read by something",
            t.outputs, t.outputs_consumed
        );
    }

    let counts = report.by_check();
    if counts.is_empty() {
        println!("  checks: nothing to report");
    } else {
        let summary: Vec<String> = counts.iter().map(|(k, v)| format!("{k}={v}")).collect();
        println!("  checks: {}", summary.join(" "));
        for f in &report.findings {
            let article = f.article.as_deref().unwrap_or("-");
            println!("    [{}] art. {article}: {}", f.check, f.detail);
        }
    }

    Ok(!report.schema.is_empty())
}
