//! HTTP execute endpoint for deterministic law evaluation (4LM / external callers).
//!
//! `POST /api/v1/execute` loads the local corpus, evaluates named outputs, and
//! returns values plus a box-drawing explanation trail.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use axum::http::StatusCode;
use axum::Json;
use regelrecht_engine::{LawExecutionService, Value};
use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

/// Request body for `POST /api/v1/execute`.
#[derive(Debug, Deserialize)]
pub struct ExecuteRequest {
    /// Law `$id` to evaluate.
    pub regulation: String,
    /// Optional version date (YYYY-MM-DD); currently informational — the
    /// engine picks the version by `calculation_date`.
    #[serde(default)]
    pub version: Option<String>,
    /// Calculation date (YYYY-MM-DD). Defaults to today (UTC) when omitted.
    #[serde(default)]
    pub calculation_date: Option<String>,
    /// Input parameters for the law.
    #[serde(default)]
    pub parameters: BTreeMap<String, serde_json::Value>,
    /// Outputs to evaluate. When omitted, every output declared on the law is
    /// evaluated.
    #[serde(default)]
    pub outputs: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
pub struct ExecuteResponse {
    pub regulation: String,
    pub calculation_date: String,
    pub results: BTreeMap<String, OutputResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub explanation: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct OutputResult {
    pub value: serde_json::Value,
}

/// Shared service with the local corpus pre-loaded (lazy, once per process).
fn corpus_service() -> Result<&'static LawExecutionService, String> {
    static SERVICE: OnceLock<Result<LawExecutionService, String>> = OnceLock::new();
    match SERVICE.get_or_init(load_corpus_service) {
        Ok(svc) => Ok(svc),
        Err(e) => Err(e.clone()),
    }
}

fn regulation_root() -> PathBuf {
    if let Ok(p) = std::env::var("REGULATION_PATH") {
        return PathBuf::from(p);
    }
    // editor-api binary runs from repo root in `just` recipes; fall back to
    // walking up from CARGO_MANIFEST_DIR when compiled as a test.
    let from_manifest = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("corpus")
        .join("regulation");
    if from_manifest.is_dir() {
        return from_manifest;
    }
    PathBuf::from("corpus/regulation")
}

fn load_corpus_service() -> Result<LawExecutionService, String> {
    let root = regulation_root();
    if !root.is_dir() {
        return Err(format!(
            "regulation corpus not found at {} (set REGULATION_PATH)",
            root.display()
        ));
    }

    let mut service = LawExecutionService::new();
    let mut count = 0usize;
    for entry in WalkDir::new(&root).follow_links(true).into_iter().flatten() {
        let path = entry.path();
        if !(path.is_file() && path.extension().is_some_and(|e| e == "yaml")) {
            continue;
        }
        let Ok(content) = std::fs::read_to_string(path) else {
            continue;
        };
        if service.load_law(&content).is_ok() {
            count += 1;
        }
    }
    if count == 0 {
        return Err(format!(
            "no laws loaded from {} (set REGULATION_PATH)",
            root.display()
        ));
    }
    tracing::info!(count, path = %root.display(), "execute API loaded corpus");
    Ok(service)
}

fn today_utc() -> String {
    chrono::Utc::now().format("%Y-%m-%d").to_string()
}

/// Core evaluate logic (also used by tests without HTTP).
pub fn execute_regulation(req: ExecuteRequest) -> Result<ExecuteResponse, (StatusCode, String)> {
    let date = req
        .calculation_date
        .clone()
        .unwrap_or_else(today_utc);
    if chrono::NaiveDate::parse_from_str(&date, "%Y-%m-%d").is_err() {
        return Err((
            StatusCode::BAD_REQUEST,
            format!("Invalid calculation_date '{date}': expected YYYY-MM-DD"),
        ));
    }

    let service = corpus_service().map_err(|e| (StatusCode::SERVICE_UNAVAILABLE, e))?;

    if !service.has_law(&req.regulation) {
        return Err((
            StatusCode::NOT_FOUND,
            format!("regulation '{}' not found in corpus", req.regulation),
        ));
    }

    let params: BTreeMap<String, Value> = req
        .parameters
        .iter()
        .map(|(k, v)| (k.clone(), Value::from(v)))
        .collect();

    let output_names: Vec<String> = match &req.outputs {
        Some(names) if !names.is_empty() => names.clone(),
        Some(_) => {
            return Err((
                StatusCode::BAD_REQUEST,
                "outputs must not be empty when provided".to_string(),
            ));
        }
        None => service
            .get_law_info(&req.regulation)
            .map(|info| info.outputs)
            .unwrap_or_default(),
    };
    if output_names.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "no outputs to evaluate".to_string(),
        ));
    }

    let output_refs: Vec<&str> = output_names.iter().map(String::as_str).collect();
    // Evaluate the first output with a full trace; remaining outputs reuse
    // the same parameter set via evaluate_law (batch).
    let primary = output_refs[0];
    let traced = service
        .evaluate_law_output_with_trace(&req.regulation, primary, params.clone(), &date)
        .map_err(|e| (StatusCode::UNPROCESSABLE_ENTITY, e.to_string()))?;

    let batch = if output_refs.len() == 1 {
        None
    } else {
        Some(
            service
                .evaluate_law(&req.regulation, &output_refs, params, &date)
                .map_err(|e| (StatusCode::UNPROCESSABLE_ENTITY, e.to_string()))?,
        )
    };

    let mut results = BTreeMap::new();
    if let Some(batch) = batch {
        for (name, value) in batch.outputs {
            results.insert(
                name,
                OutputResult {
                    value: serde_json::Value::from(&value),
                },
            );
        }
    } else {
        for (name, value) in traced.outputs {
            results.insert(
                name,
                OutputResult {
                    value: serde_json::Value::from(&value),
                },
            );
        }
    }

    let explanation = traced.trace.as_ref().map(|t| t.render_box_drawing());

    let _ = req.version; // reserved for future version pinning

    Ok(ExecuteResponse {
        regulation: req.regulation,
        calculation_date: date,
        results,
        explanation,
        error: None,
    })
}

/// `GET /api/v1/regulations/{law_id}/nrml` — NRML export of a loaded law.
pub async fn nrml_export_handler(
    axum::extract::Path(law_id): axum::extract::Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let root = regulation_root();
    let mut found: Option<String> = None;
    for entry in WalkDir::new(&root).follow_links(true).into_iter().flatten() {
        let path = entry.path();
        if !(path.is_file() && path.extension().is_some_and(|e| e == "yaml")) {
            continue;
        }
        let Ok(content) = std::fs::read_to_string(path) else {
            continue;
        };
        if content.contains(&format!("$id: {law_id}"))
            || content.contains(&format!("$id: '{law_id}'"))
            || content.contains(&format!("$id: \"{law_id}\""))
        {
            found = Some(content);
            break;
        }
    }
    let yaml = found.ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            format!("regulation '{law_id}' not found"),
        )
    })?;
    let nrml = regelrecht_nrml_bridge::yaml_to_nrml(&yaml)
        .map_err(|e| (StatusCode::UNPROCESSABLE_ENTITY, e.to_string()))?;
    Ok(Json(nrml))
}

/// `POST /api/v1/execute`
pub async fn execute_handler(
    Json(req): Json<ExecuteRequest>,
) -> Result<Json<ExecuteResponse>, (StatusCode, String)> {
    execute_regulation(req).map(Json)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lu_flight_tax_short_distance_discount() {
        let resp = execute_regulation(ExecuteRequest {
            regulation: "vliegbelasting_korting_klimaatneutraal_lu".into(),
            version: Some("2026-07-21".into()),
            calculation_date: Some("2026-07-21".into()),
            parameters: BTreeMap::from([
                ("klimaatneutraal".into(), serde_json::json!(true)),
                ("binnenlands".into(), serde_json::json!(false)),
                ("afstand_km".into(), serde_json::json!(480)),
                ("vliegbelasting_bedrag".into(), serde_json::json!(50)),
            ]),
            outputs: Some(vec![
                "recht_op_korting".into(),
                "korting_bedrag".into(),
            ]),
        })
        .expect("execute");

        assert_eq!(
            resp.results["recht_op_korting"].value,
            serde_json::json!(true)
        );
        let korting = &resp.results["korting_bedrag"].value;
        assert!(
            korting == &serde_json::json!(25) || korting == &serde_json::json!(25.0),
            "expected 25, got {korting}"
        );
        assert!(resp.explanation.as_ref().is_some_and(|e| !e.is_empty()));
    }
}
