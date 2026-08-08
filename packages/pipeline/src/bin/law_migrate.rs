//! Lift law files to schema v0.6.0.
//!
//! No database, no git, no model, no shell: it reads the YAML, converts what
//! the new schema has a place for, and validates the result against v0.6.0.
//!
//! ```text
//! law-migrate [--write] <file.yaml>...
//! ```
//!
//! Without `--write` nothing is touched and the report is the whole output.
//! Exit code is 1 when any file ends up with a blocker or a schema error, so
//! it can gate a pipeline step: a bump that did not validate is the failure
//! this binary exists to make loud.
//!
//! A blocker is a required field of the new shape with no source in the old
//! one. It is reported, never guessed at, and the file is written with the
//! field missing so schema validation names the same place independently.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use regelrecht_pipeline::law_migrate;

fn main() -> ExitCode {
    let mut write = false;
    let mut files: Vec<PathBuf> = Vec::new();
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--write" => write = true,
            "--help" | "-h" => {
                println!("usage: law-migrate [--write] <file.yaml>...");
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
        eprintln!("usage: law-migrate [--write] <file.yaml>...");
        return ExitCode::from(2);
    }

    let mut failed = 0usize;
    for file in &files {
        match migrate_one(file, write) {
            Ok(true) => {}
            Ok(false) => failed += 1,
            Err(e) => {
                eprintln!("{}: {e}", file.display());
                failed += 1;
            }
        }
    }

    if failed > 0 {
        eprintln!("\n{failed} of {} file(s) need a decision", files.len());
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

/// Returns Ok(true) when the file migrated clean.
fn migrate_one(path: &Path, write: bool) -> std::io::Result<bool> {
    let yaml = std::fs::read_to_string(path)?;
    let result = match law_migrate::migrate(&yaml) {
        Ok(r) => r,
        Err(e) => {
            println!("\n=== {}\n  cannot migrate: {e}", path.display());
            return Ok(false);
        }
    };

    println!("\n=== {}", path.display());
    println!(
        "  from:      {} -> {}",
        result.from_version.as_deref().unwrap_or("unknown"),
        law_migrate::TARGET_VERSION
    );
    println!(
        "  change:    {}",
        if result.structural_changes {
            "schema line + machine_readable sections"
        } else {
            "schema line only"
        }
    );

    if result.dropped.is_empty() {
        println!("  dropped:   nothing");
    } else {
        let total: usize = result.dropped.iter().map(|d| d.count).sum();
        println!(
            "  dropped:   {total} value(s) with no field in {}",
            law_migrate::TARGET_VERSION
        );
        let mut by_field: std::collections::BTreeMap<&str, usize> =
            std::collections::BTreeMap::new();
        for d in &result.dropped {
            *by_field.entry(d.field).or_default() += d.count;
        }
        for (field, count) in by_field {
            println!("    - {field}: {count}");
        }
    }

    if result.blockers.is_empty() {
        println!("  blockers:  none");
    } else {
        println!("  blockers:  {}", result.blockers.len());
        let mut by_missing: std::collections::BTreeMap<&str, usize> =
            std::collections::BTreeMap::new();
        for b in &result.blockers {
            *by_missing.entry(b.missing).or_default() += 1;
        }
        for (missing, count) in &by_missing {
            println!("    {missing}: {count}");
        }
        for b in &result.blockers {
            println!(
                "    - art. {} {}[{}] needs {}: {}",
                b.article, b.source_field, b.index, b.missing, b.reason
            );
        }
    }

    if result.schema_errors.is_empty() {
        println!("  schema:    valid");
    } else {
        println!("  schema:    {} error(s)", result.schema_errors.len());
        for e in result.schema_errors.iter().take(20) {
            println!("    - {e}");
        }
        if result.schema_errors.len() > 20 {
            println!("    ... and {} more", result.schema_errors.len() - 20);
        }
    }

    if write {
        if result.is_clean() {
            std::fs::write(path, &result.yaml)?;
            println!("  written:   yes");
        } else {
            // Writing a file that does not validate would put the decision
            // in the corpus instead of in front of a human.
            println!("  written:   no (not clean)");
        }
    }

    Ok(result.is_clean())
}
