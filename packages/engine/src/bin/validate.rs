use std::path::Path;
use std::process;

use regelrecht_engine::article::{ArticleBasedLaw, LawLoad};
use regelrecht_engine::schema::{detect_version, load_schemas, validation_errors};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    if args.is_empty() {
        eprintln!("Usage: validate <file1.yaml> [file2.yaml ...]");
        process::exit(1);
    }

    let schemas = match load_schemas() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("FATAL: {e}");
            process::exit(2);
        }
    };
    let mut failed = false;

    for arg in &args {
        let path = Path::new(arg);

        // Step 1: serde deserialization check (catches type/structure errors)
        let law = match ArticleBasedLaw::from_yaml_file(path) {
            Ok(l) => l,
            Err(e) => {
                eprintln!("FAIL: {}: serde: {e}", path.display());
                failed = true;
                continue;
            }
        };

        // Step 2: JSON Schema validation
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("FAIL: {}: read: {e}", path.display());
                failed = true;
                continue;
            }
        };

        let value: serde_json::Value = match serde_yaml_ng::from_str(&content) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("FAIL: {}: yaml parse: {e}", path.display());
                failed = true;
                continue;
            }
        };

        let version = detect_version(&value);
        let schema_ok = match version {
            Some(ver) => {
                let schema = &schemas[ver];
                match validation_errors(schema, &value) {
                    Ok(errors) if errors.is_empty() => {
                        eprintln!("OK: {} (schema {ver})", path.display());
                        true
                    }
                    Ok(errors) => {
                        eprintln!("FAIL: {}: schema ({ver})", path.display());
                        for error in &errors {
                            eprintln!("  - {error}");
                        }
                        failed = true;
                        false
                    }
                    Err(e) => {
                        eprintln!(
                            "FAIL: {}: could not compile schema {ver}: {e}",
                            path.display()
                        );
                        failed = true;
                        false
                    }
                }
            }
            None => {
                // Check if $schema field exists but version is unrecognized
                if value.get("$schema").is_some() {
                    eprintln!("FAIL: {}: unrecognized $schema version", path.display());
                } else {
                    eprintln!("FAIL: {}: missing $schema field", path.display());
                }
                failed = true;
                false
            }
        };

        // Step 3: RFC-023 static unit check — only when the schema validated, so
        // unit findings aren't interleaved with (and possibly artefacts of) a
        // schema error. Mismatches between declared units (e.g. eurocent + days)
        // are FAILs; an `amount` output with no declared unit is a WARN.
        if !schema_ok {
            continue;
        }
        for finding in regelrecht_engine::units::check_law(&law) {
            let kind = if finding.is_error { "FAIL" } else { "WARN" };
            eprintln!(
                "{kind}: {}: units: article {} output '{}': {}",
                path.display(),
                finding.article,
                finding.output,
                finding.message
            );
            if finding.is_error {
                failed = true;
            }
        }
    }

    if failed {
        process::exit(1);
    }
}
