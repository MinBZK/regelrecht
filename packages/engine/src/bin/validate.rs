use std::collections::HashMap;
use std::path::Path;
use std::process;

use jsonschema::Validator;
use regelrecht_engine::article::ArticleBasedLaw;

/// Embedded schemas keyed by their `$id` URL suffix (version path).
///
/// These are compiled-in from the repo's schema/ directory and are guaranteed
/// to be valid JSON at build time.
fn load_schemas() -> Result<HashMap<&'static str, serde_json::Value>, String> {
    let mut schemas = HashMap::new();
    let v020: serde_json::Value =
        serde_json::from_str(include_str!("../../../../schema/v0.2.0/schema.json"))
            .map_err(|e| format!("invalid v0.2.0 schema JSON: {e}"))?;
    let v030: serde_json::Value =
        serde_json::from_str(include_str!("../../../../schema/v0.3.0/schema.json"))
            .map_err(|e| format!("invalid v0.3.0 schema JSON: {e}"))?;
    let v031: serde_json::Value =
        serde_json::from_str(include_str!("../../../../schema/v0.3.1/schema.json"))
            .map_err(|e| format!("invalid v0.3.1 schema JSON: {e}"))?;
    let v032: serde_json::Value =
        serde_json::from_str(include_str!("../../../../schema/v0.3.2/schema.json"))
            .map_err(|e| format!("invalid v0.3.2 schema JSON: {e}"))?;
    let v040: serde_json::Value =
        serde_json::from_str(include_str!("../../../../schema/v0.4.0/schema.json"))
            .map_err(|e| format!("invalid v0.4.0 schema JSON: {e}"))?;
    let v050: serde_json::Value =
        serde_json::from_str(include_str!("../../../../schema/v0.5.0/schema.json"))
            .map_err(|e| format!("invalid v0.5.0 schema JSON: {e}"))?;
    let v051: serde_json::Value =
        serde_json::from_str(include_str!("../../../../schema/v0.5.1/schema.json"))
            .map_err(|e| format!("invalid v0.5.1 schema JSON: {e}"))?;
    let v052: serde_json::Value =
        serde_json::from_str(include_str!("../../../../schema/v0.5.2/schema.json"))
            .map_err(|e| format!("invalid v0.5.2 schema JSON: {e}"))?;
    schemas.insert("v0.2.0", v020);
    schemas.insert("v0.3.0", v030);
    schemas.insert("v0.3.1", v031);
    schemas.insert("v0.3.2", v032);
    schemas.insert("v0.4.0", v040);
    schemas.insert("v0.5.0", v050);
    schemas.insert("v0.5.1", v051);
    schemas.insert("v0.5.2", v052);
    Ok(schemas)
}

/// Detect schema version from the `$schema` field in the YAML document.
fn detect_version(value: &serde_json::Value) -> Option<&str> {
    let schema_url = value.get("$schema")?.as_str()?;
    if schema_url.contains("v0.5.2") {
        Some("v0.5.2")
    } else if schema_url.contains("v0.5.1") {
        Some("v0.5.1")
    } else if schema_url.contains("v0.5.0") {
        Some("v0.5.0")
    } else if schema_url.contains("v0.4.0") {
        Some("v0.4.0")
    } else if schema_url.contains("v0.3.2") {
        Some("v0.3.2")
    } else if schema_url.contains("v0.3.1") {
        Some("v0.3.1")
    } else if schema_url.contains("v0.3.0") {
        Some("v0.3.0")
    } else if schema_url.contains("v0.2.0") {
        Some("v0.2.0")
    } else {
        None
    }
}

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
        if let Err(e) = ArticleBasedLaw::from_yaml_file(path) {
            eprintln!("FAIL: {}: serde: {e}", path.display());
            failed = true;
            continue;
        }

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
        match version {
            Some(ver) => {
                let schema = &schemas[ver];
                match Validator::new(schema) {
                    Ok(validator) => {
                        let errors: Vec<_> = validator.iter_errors(&value).collect();
                        if errors.is_empty() {
                            eprintln!("OK: {} (schema {ver})", path.display());
                        } else {
                            eprintln!("FAIL: {}: schema ({ver})", path.display());
                            for error in &errors {
                                eprintln!("  - {}: {}", error.instance_path(), error);
                            }
                            failed = true;
                        }
                    }
                    Err(e) => {
                        eprintln!(
                            "FAIL: {}: could not compile schema {ver}: {e}",
                            path.display()
                        );
                        failed = true;
                    }
                }
            }
            None => {
                // Check if $schema field exists but version is unrecognized
                if value.get("$schema").is_some() {
                    eprintln!("FAIL: {}: unrecognized $schema version", path.display());
                    failed = true;
                } else {
                    eprintln!("FAIL: {}: missing $schema field", path.display());
                    failed = true;
                }
            }
        }
    }

    if failed {
        process::exit(1);
    }
}
