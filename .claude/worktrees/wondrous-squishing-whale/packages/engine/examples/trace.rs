//! Print an execution trace for a law evaluation.
//!
//! Usage:
//!   cargo run --example trace -- <law_id> <output_name> <date> [key=value ...]
//!
//! Example:
//!   cargo run --example trace -- zorgtoeslagwet hoogte_zorgtoeslag 2025-01-01 bsn=999993653

use regelrecht_engine::{LawExecutionService, Value};
use std::collections::HashMap;
use std::path::Path;
use walkdir::WalkDir;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 3 {
        eprintln!("Usage: trace <law_id> <output_name> <date> [key=value ...]");
        eprintln!("Example: trace zorgtoeslagwet hoogte_zorgtoeslag 2025-01-01 bsn=999993653");
        std::process::exit(1);
    }

    let law_id = &args[0];
    let output_name = &args[1];
    let date = &args[2];

    let mut params: HashMap<String, Value> = HashMap::new();
    for arg in &args[3..] {
        if let Some((key, val)) = arg.split_once('=') {
            let value = parse_value(val);
            params.insert(key.to_string(), value);
        } else {
            eprintln!("Invalid parameter (expected key=value): {}", arg);
            std::process::exit(1);
        }
    }

    let mut service = LawExecutionService::new();

    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let regulation_dir = Path::new(manifest_dir)
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.join("corpus").join("regulation").join("nl"))
        .expect("Could not find regulation directory");

    let mut count = 0;
    for entry in WalkDir::new(&regulation_dir)
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if path.is_file() && path.extension().is_some_and(|ext| ext == "yaml") {
            let content = match std::fs::read_to_string(path) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("Warning: could not read {}: {}", path.display(), e);
                    continue;
                }
            };
            if service.load_law(&content).is_ok() {
                count += 1;
            }
        }
    }
    eprintln!("Loaded {} regulations", count);

    match service.evaluate_law_output_with_trace(law_id, output_name, params, date) {
        Ok(result) => {
            if let Some(trace) = result.trace {
                println!("{}", trace.render_box_drawing());
            } else {
                eprintln!("No trace produced");
            }
        }
        Err(e) => {
            eprintln!("Evaluation failed: {}", e);
            std::process::exit(1);
        }
    }
}

fn parse_value(s: &str) -> Value {
    if s == "true" {
        Value::Bool(true)
    } else if s == "false" {
        Value::Bool(false)
    } else if s == "null" {
        Value::Null
    } else if let Ok(i) = s.parse::<i64>() {
        Value::Int(i)
    } else if let Ok(f) = s.parse::<f64>() {
        Value::Float(f)
    } else {
        Value::String(s.to_string())
    }
}
