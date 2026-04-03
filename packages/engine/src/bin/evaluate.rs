//! CLI binary for evaluating a law YAML via stdin.
//!
//! Usage:
//!   echo '{"law_yaml": "...", "output_name": "...", "params": {...}, "date": "2025-01-01"}' \
//!     | cargo run --bin evaluate
//!
//! Input (JSON on stdin):
//!   - law_yaml: String — the full YAML content of the law
//!   - output_name: String — the output to evaluate (e.g. "heeft_recht_op_zorgtoeslag")
//!   - params: Object — key-value parameters to pass to the engine
//!   - date: String — evaluation date (YYYY-MM-DD)
//!   - extra_laws: Optional<Vec<String>> — additional YAML laws for cross-law resolution
//!
//! Output (JSON on stdout):
//!   - outputs: Object — computed output values
//!   - resolved_inputs: Object — resolved input values from cross-law references
//!   - article_number: String — the article that was executed
//!   - law_id: String — the law ID that was evaluated
//!   - law_uuid: Optional<String> — the law UUID if available
//!   - error: Optional<String> — error message if evaluation failed

use regelrecht_engine::{LawExecutionService, UntranslatableMode, Value};
use std::collections::BTreeMap;
use std::io::Read;

#[derive(serde::Deserialize)]
struct EvaluateRequest {
    law_yaml: String,
    output_name: String,
    params: BTreeMap<String, serde_json::Value>,
    date: String,
    #[serde(default)]
    extra_laws: Vec<String>,
}

#[derive(serde::Serialize)]
struct EvaluateResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    outputs: Option<BTreeMap<String, serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    resolved_inputs: Option<BTreeMap<String, serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    article_number: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    law_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    law_uuid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    /// Engine version that produced this result (RFC-013)
    engine_version: String,
    /// Schema version of the evaluated regulation (RFC-013)
    #[serde(skip_serializing_if = "Option::is_none")]
    schema_version: Option<String>,
    /// SHA-256 hash of the regulation YAML content (RFC-013)
    #[serde(skip_serializing_if = "Option::is_none")]
    regulation_hash: Option<String>,
}

fn error_response(msg: String) -> EvaluateResponse {
    EvaluateResponse {
        outputs: None,
        resolved_inputs: None,
        article_number: None,
        law_id: None,
        law_uuid: None,
        error: Some(msg),
        engine_version: regelrecht_engine::VERSION.to_string(),
        schema_version: None,
        regulation_hash: None,
    }
}

fn main() {
    // Initialize OpenTelemetry if the otel feature is enabled and the endpoint is configured
    #[cfg(feature = "otel")]
    let _otel_guard = if std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT").is_ok_and(|v| !v.is_empty()) {
        match regelrecht_engine::telemetry::init_otel_subscriber("regelrecht-engine") {
            Ok(guard) => Some(guard),
            Err(e) => {
                eprintln!("Warning: failed to initialize OpenTelemetry: {e}");
                None
            }
        }
    } else {
        None
    };

    let mut input = String::new();
    if let Err(e) = std::io::stdin().read_to_string(&mut input) {
        let resp = error_response(format!("Failed to read stdin: {e}"));
        println!("{}", serde_json::to_string(&resp).unwrap_or_default());
        std::process::exit(1);
    }

    let request: EvaluateRequest = match serde_json::from_str(&input) {
        Ok(r) => r,
        Err(e) => {
            let resp = error_response(format!("Failed to parse request JSON: {e}"));
            println!("{}", serde_json::to_string(&resp).unwrap_or_default());
            std::process::exit(1);
        }
    };

    // Validate date format (YYYY-MM-DD)
    if chrono::NaiveDate::parse_from_str(&request.date, "%Y-%m-%d").is_err() {
        let resp = error_response(format!(
            "Invalid date format '{}': expected YYYY-MM-DD",
            request.date
        ));
        println!("{}", serde_json::to_string(&resp).unwrap_or_default());
        std::process::exit(1);
    }

    let mut service = LawExecutionService::new();

    // Parse CLI flags
    let mut emit_receipt = false;
    for arg in std::env::args().skip(1) {
        if let Some(mode_str) = arg.strip_prefix("--untranslatable=") {
            // RFC-012: untranslatable handling mode
            match mode_str.parse::<UntranslatableMode>() {
                Ok(mode) => service.set_untranslatable_mode(mode),
                Err(e) => {
                    let resp = error_response(e);
                    println!("{}", serde_json::to_string(&resp).unwrap_or_default());
                    std::process::exit(1);
                }
            }
        } else if arg == "--receipt" {
            // RFC-013: emit Execution Receipt envelope
            emit_receipt = true;
        }
    }

    // Load the primary law and capture its ID
    let law_id = match service.load_law(&request.law_yaml) {
        Ok(id) => id,
        Err(e) => {
            let resp = error_response(format!("Failed to load law YAML: {e}"));
            println!("{}", serde_json::to_string(&resp).unwrap_or_default());
            std::process::exit(1);
        }
    };

    // Load additional laws for cross-law resolution
    for extra_yaml in &request.extra_laws {
        if let Err(e) = service.load_law(extra_yaml) {
            let resp = error_response(format!("Failed to load extra law YAML: {e}"));
            println!("{}", serde_json::to_string(&resp).unwrap_or_default());
            std::process::exit(1);
        }
    }

    // Convert params
    let params: BTreeMap<String, Value> = request
        .params
        .iter()
        .map(|(k, v)| (k.clone(), Value::from(v)))
        .collect();

    // Evaluate
    match service.evaluate_law_output(&law_id, &request.output_name, params.clone(), &request.date)
    {
        Ok(result) => {
            if emit_receipt {
                let receipt = service.build_receipt(&result, &params, &request.date);
                println!(
                    "{}",
                    serde_json::to_string_pretty(&receipt).unwrap_or_default()
                );
            } else {
                let outputs: BTreeMap<String, serde_json::Value> = result
                    .outputs
                    .iter()
                    .map(|(k, v)| (k.clone(), serde_json::Value::from(v)))
                    .collect();
                let resolved_inputs: BTreeMap<String, serde_json::Value> = result
                    .resolved_inputs
                    .iter()
                    .map(|(k, v)| (k.clone(), serde_json::Value::from(v)))
                    .collect();
                let resp = EvaluateResponse {
                    outputs: Some(outputs),
                    resolved_inputs: Some(resolved_inputs),
                    article_number: Some(result.article_number),
                    law_id: Some(result.law_id),
                    law_uuid: result.law_uuid,
                    error: None,
                    engine_version: result.engine_version,
                    schema_version: result.schema_version,
                    regulation_hash: result.regulation_hash,
                };
                println!("{}", serde_json::to_string(&resp).unwrap_or_default());
            }
        }
        Err(e) => {
            let resp = error_response(format!("{e}"));
            println!("{}", serde_json::to_string(&resp).unwrap_or_default());
            std::process::exit(1);
        }
    }
}
