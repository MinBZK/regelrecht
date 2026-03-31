//! WASM bindings for the RegelRecht engine
//!
//! This module provides WebAssembly bindings backed by `LawExecutionService`,
//! giving full cross-law resolution and data source support in the browser.
//! It is feature-gated behind the `wasm` feature flag.
//!
//! # Key Constraints
//!
//! - **No filesystem access in WASM**: Laws must be passed as YAML strings via `load_law()`
//! - **Efficient serialization**: Uses `serde-wasm-bindgen` for Rust <-> JavaScript conversion
//!
//! # Cross-Law Resolution
//!
//! Unlike the previous `ArticleEngine`-based implementation, this version uses
//! `LawExecutionService` internally. When all referenced laws are loaded,
//! cross-law references are resolved automatically:
//!
//! ```javascript
//! const engine = new WasmEngine();
//! engine.loadLaw(zorgtoeslagwetYaml);
//! engine.loadLaw(regelingStandaardpremieYaml);
//! engine.loadLaw(awirYaml);
//!
//! // Cross-law references are resolved automatically
//! const result = engine.execute(
//!     'zorgtoeslagwet',
//!     'heeft_recht_op_zorgtoeslag',
//!     { bsn: '999993653' },
//!     '2025-01-01'
//! );
//! ```
//!
//! # Data Sources
//!
//! Register tabular data sources (e.g., personal records) that are queried
//! during execution to resolve inputs:
//!
//! ```javascript
//! engine.registerDataSource('personal_data', 'bsn', [
//!     { bsn: '999993653', geboortedatum: '2000-01-01', land_verblijf: 'NEDERLAND' }
//! ]);
//! ```
//!
//! # Error Handling
//!
//! All methods that can fail return `Result<T, JsValue>`. In JavaScript:
//!
//! ```javascript
//! try {
//!     const result = engine.execute(...);
//! } catch (e) {
//!     console.error('Execution failed:', e);  // e is a string with error details
//! }
//! ```

use serde::Serialize;
use serde_wasm_bindgen::Serializer;
use std::collections::BTreeMap;
use wasm_bindgen::prelude::*;

use crate::config;
use crate::error::EngineError;
use crate::service::LawExecutionService;
use crate::trace::{PathNode, TraceBuilder};
use crate::types::{RegulatoryLayer, Value};

/// Create a serializer that converts HashMaps to JavaScript objects (not Maps)
fn js_serializer() -> Serializer {
    Serializer::new().serialize_maps_as_objects(true)
}

/// Helper to create consistent error JsValues.
fn wasm_error(msg: &str) -> JsValue {
    JsValue::from_str(msg)
}

/// Convert internal EngineError to user-friendly WASM error.
fn engine_error_to_wasm(err: EngineError) -> JsValue {
    match err {
        EngineError::LawNotFound(ref law_id) => wasm_error(&format!(
            "Law '{}' not found. Load it first with loadLaw().",
            law_id
        )),
        other => wasm_error(&other.to_string()),
    }
}

/// Serializable result for execute()
#[derive(Serialize)]
struct WasmExecuteResult {
    outputs: BTreeMap<String, Value>,
    resolved_inputs: BTreeMap<String, Value>,
    article_number: String,
    law_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    law_uuid: Option<String>,
}

/// Serializable result for executeWithTrace()
#[derive(Serialize)]
struct WasmExecuteResultWithTrace {
    outputs: BTreeMap<String, Value>,
    resolved_inputs: BTreeMap<String, Value>,
    article_number: String,
    law_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    law_uuid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    trace: Option<PathNode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    trace_text: Option<String>,
}

/// Serializable law info for get_law_info()
#[derive(Serialize)]
struct WasmLawInfo {
    id: String,
    regulatory_layer: RegulatoryLayer,
    publication_date: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    bwb_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    url: Option<String>,
    outputs: Vec<String>,
    article_count: usize,
}

/// WASM-compatible law execution engine with cross-law resolution.
///
/// Backed by `LawExecutionService`, providing automatic resolution of
/// cross-law references and data source support in the browser.
#[wasm_bindgen]
pub struct WasmEngine {
    service: LawExecutionService,
}

#[wasm_bindgen]
impl WasmEngine {
    /// Create a new empty engine instance.
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            service: LawExecutionService::new(),
        }
    }

    /// Load a law from a YAML string.
    ///
    /// If a law with the same ID and valid_from is already loaded, it will be replaced.
    /// Multiple versions (same ID, different valid_from) can coexist.
    ///
    /// # Arguments
    /// * `yaml` - YAML string containing the law definition (max 1 MB)
    ///
    /// # Returns
    /// * `Ok(String)` - The law ID
    /// * `Err(JsValue)` - Error message if parsing fails
    #[wasm_bindgen(js_name = loadLaw)]
    pub fn load_law(&mut self, yaml: &str) -> Result<String, JsValue> {
        if yaml.len() > config::MAX_YAML_SIZE {
            return Err(wasm_error(&format!(
                "YAML exceeds maximum size ({} bytes)",
                config::MAX_YAML_SIZE
            )));
        }

        self.service.load_law(yaml).map_err(engine_error_to_wasm)
    }

    /// Execute a law output with automatic cross-law resolution.
    ///
    /// All referenced laws must be loaded via `loadLaw()` first.
    /// Data sources registered via `registerDataSource()` are queried
    /// to resolve inputs before falling back to cross-law resolution.
    ///
    /// # Arguments
    /// * `law_id` - ID of the loaded law
    /// * `output_name` - Name of the output to calculate
    /// * `parameters` - JavaScript object with input parameters
    /// * `calculation_date` - Date string (YYYY-MM-DD) for which to calculate
    ///
    /// # Returns
    /// * `Ok(JsValue)` - JavaScript object with `outputs`, `resolved_inputs`, etc.
    /// * `Err(JsValue)` - Error message if execution fails
    #[wasm_bindgen(js_name = execute)]
    pub fn execute(
        &self,
        law_id: &str,
        output_name: &str,
        parameters: JsValue,
        calculation_date: &str,
    ) -> Result<JsValue, JsValue> {
        let params: BTreeMap<String, Value> = serde_wasm_bindgen::from_value(parameters)
            .map_err(|e| wasm_error(&format!("Failed to parse parameters: {}", e)))?;

        let result = self
            .service
            .evaluate_law_output(law_id, output_name, params, calculation_date)
            .map_err(engine_error_to_wasm)?;

        let wasm_result = WasmExecuteResult {
            outputs: result.outputs,
            resolved_inputs: result.resolved_inputs,
            article_number: result.article_number,
            law_id: result.law_id,
            law_uuid: result.law_uuid,
        };

        wasm_result.serialize(&js_serializer()).map_err(|e| {
            wasm_error(&format!(
                "Failed to serialize result for law '{}': {}",
                law_id, e
            ))
        })
    }

    /// Execute a law output with tracing enabled.
    ///
    /// Same as `execute()` but includes a full execution trace tree in the
    /// result. The trace captures every resolution step, cross-law call, and
    /// operation performed during evaluation.
    ///
    /// # Returns
    /// * `Ok(JsValue)` - JavaScript object with `outputs`, `trace` (tree), `trace_text` (box-drawing)
    /// * `Err(JsValue)` - Error message if execution fails (may include partial trace)
    #[wasm_bindgen(js_name = executeWithTrace)]
    pub fn execute_with_trace(
        &self,
        law_id: &str,
        output_name: &str,
        parameters: JsValue,
        calculation_date: &str,
    ) -> Result<JsValue, JsValue> {
        let params: BTreeMap<String, Value> = serde_wasm_bindgen::from_value(parameters)
            .map_err(|e| wasm_error(&format!("Failed to parse parameters: {}", e)))?;

        // Use untimed trace builder to avoid Instant::now() JS FFI calls
        // that cause RefCell aliasing panics in wasm-bindgen.
        match self.service.evaluate_law_output_with_trace_builder(
            law_id,
            output_name,
            params,
            calculation_date,
            TraceBuilder::new_untimed(),
        ) {
            Ok(result) => {
                let trace_text = result.trace.as_ref().map(|t| t.render_box_drawing());
                let wasm_result = WasmExecuteResultWithTrace {
                    outputs: result.outputs,
                    resolved_inputs: result.resolved_inputs,
                    article_number: result.article_number,
                    law_id: result.law_id,
                    law_uuid: result.law_uuid,
                    trace: result.trace,
                    trace_text,
                };

                wasm_result.serialize(&js_serializer()).map_err(|e| {
                    wasm_error(&format!(
                        "Failed to serialize traced result for law '{}': {}",
                        law_id, e
                    ))
                })
            }
            Err(EngineError::TracedError { source, trace }) => {
                // Return partial trace alongside the error by encoding both
                // into a structured error object
                let trace_node = trace.map(|t| *t);
                let trace_text = trace_node.as_ref().map(|t| t.render_box_drawing());

                #[derive(Serialize)]
                struct TracedErrorResult {
                    error: String,
                    #[serde(skip_serializing_if = "Option::is_none")]
                    trace: Option<PathNode>,
                    #[serde(skip_serializing_if = "Option::is_none")]
                    trace_text: Option<String>,
                }

                let err_result = TracedErrorResult {
                    error: source.to_string(),
                    trace: trace_node,
                    trace_text,
                };

                match err_result.serialize(&js_serializer()) {
                    Ok(js_val) => Err(js_val),
                    Err(_) => Err(wasm_error(&source.to_string())),
                }
            }
            Err(other) => Err(engine_error_to_wasm(other)),
        }
    }

    /// List all loaded law IDs (sorted alphabetically).
    #[wasm_bindgen(js_name = listLaws)]
    pub fn list_laws(&self) -> Vec<String> {
        self.service
            .list_laws()
            .into_iter()
            .map(String::from)
            .collect()
    }

    /// Get metadata about a loaded law.
    #[wasm_bindgen(js_name = getLawInfo)]
    pub fn get_law_info(&self, law_id: &str) -> Result<JsValue, JsValue> {
        let law_info = self
            .service
            .get_law_info(law_id)
            .ok_or_else(|| wasm_error(&format!("Law '{}' not found", law_id)))?;

        let info = WasmLawInfo {
            id: law_info.id,
            regulatory_layer: law_info.regulatory_layer,
            publication_date: law_info.publication_date,
            bwb_id: law_info.bwb_id,
            url: law_info.url,
            outputs: law_info.outputs,
            article_count: law_info.article_count,
        };

        info.serialize(&js_serializer()).map_err(|e| {
            wasm_error(&format!(
                "Failed to serialize law info for '{}': {}",
                law_id, e
            ))
        })
    }

    /// Remove a loaded law from the engine.
    ///
    /// # Returns
    /// * `true` if the law was removed, `false` if it wasn't loaded
    #[wasm_bindgen(js_name = unloadLaw)]
    pub fn unload_law(&mut self, law_id: &str) -> bool {
        self.service.unload_law(law_id)
    }

    /// Check if a law is loaded.
    #[wasm_bindgen(js_name = hasLaw)]
    pub fn has_law(&self, law_id: &str) -> bool {
        self.service.has_law(law_id)
    }

    /// Get the number of loaded laws.
    #[wasm_bindgen(js_name = lawCount)]
    pub fn law_count(&self) -> usize {
        self.service.law_count()
    }

    /// Register a tabular data source from flat records.
    ///
    /// Data sources are queried during execution to resolve inputs before
    /// falling back to cross-law resolution.
    ///
    /// # Arguments
    /// * `name` - Data source name (e.g., "personal_data")
    /// * `key_field` - Field name used as record key (e.g., "bsn")
    /// * `records` - JavaScript array of objects, each representing a record
    ///
    /// # Example (JavaScript)
    /// ```javascript
    /// engine.registerDataSource('personal_data', 'bsn', [
    ///     { bsn: '999993653', geboortedatum: '2000-01-01', land_verblijf: 'NEDERLAND' }
    /// ]);
    /// ```
    #[wasm_bindgen(js_name = registerDataSource)]
    pub fn register_data_source(
        &mut self,
        name: &str,
        key_field: &str,
        records: JsValue,
    ) -> Result<(), JsValue> {
        let parsed: Vec<BTreeMap<String, Value>> = serde_wasm_bindgen::from_value(records)
            .map_err(|e| wasm_error(&format!("Failed to parse records: {}", e)))?;

        self.service
            .register_dict_source(name, key_field, parsed)
            .map_err(engine_error_to_wasm)
    }

    /// Remove all registered data sources.
    #[wasm_bindgen(js_name = clearDataSources)]
    pub fn clear_data_sources(&mut self) {
        self.service.clear_data_sources();
    }

    /// Get the engine version.
    #[wasm_bindgen(js_name = version)]
    pub fn version(&self) -> String {
        env!("CARGO_PKG_VERSION").to_string()
    }
}

impl Default for WasmEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL_LAW_YAML: &str = r#"
$id: test_law
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Test article
    machine_readable:
      execution:
        parameters:
          - name: value
            type: number
            required: true
        output:
          - name: result
            type: number
        actions:
          - output: result
            operation: MULTIPLY
            values:
              - $value
              - 2
"#;

    fn load_law(engine: &mut WasmEngine, yaml: &str) -> String {
        engine.service.load_law(yaml).unwrap()
    }

    #[test]
    fn test_wasm_engine_new() {
        let engine = WasmEngine::new();
        assert_eq!(engine.law_count(), 0);
        assert!(engine.list_laws().is_empty());
    }

    #[test]
    fn test_wasm_engine_default() {
        let engine = WasmEngine::default();
        assert_eq!(engine.law_count(), 0);
    }

    #[test]
    fn test_wasm_engine_load_law() {
        let mut engine = WasmEngine::new();
        load_law(&mut engine, MINIMAL_LAW_YAML);

        assert_eq!(engine.law_count(), 1);
        assert!(engine.has_law("test_law"));
        assert_eq!(engine.list_laws(), vec!["test_law".to_string()]);
    }

    #[test]
    fn test_wasm_engine_unload_law() {
        let mut engine = WasmEngine::new();
        load_law(&mut engine, MINIMAL_LAW_YAML);

        assert!(engine.has_law("test_law"));
        assert!(engine.unload_law("test_law"));
        assert!(!engine.has_law("test_law"));
        assert!(!engine.unload_law("nonexistent"));
    }

    #[test]
    fn test_wasm_engine_list_laws() {
        let mut engine = WasmEngine::new();
        load_law(&mut engine, MINIMAL_LAW_YAML);

        let laws = engine.list_laws();
        assert_eq!(laws.len(), 1);
        assert!(laws.contains(&"test_law".to_string()));
    }

    #[test]
    fn test_wasm_engine_has_law() {
        let mut engine = WasmEngine::new();
        assert!(!engine.has_law("test_law"));

        load_law(&mut engine, MINIMAL_LAW_YAML);

        assert!(engine.has_law("test_law"));
        assert!(!engine.has_law("other_law"));
    }

    #[test]
    fn test_wasm_engine_law_count() {
        let mut engine = WasmEngine::new();
        assert_eq!(engine.law_count(), 0);

        load_law(&mut engine, MINIMAL_LAW_YAML);
        assert_eq!(engine.law_count(), 1);

        let yaml2 = r#"
$id: second_law
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Second test article
"#;
        load_law(&mut engine, yaml2);
        assert_eq!(engine.law_count(), 2);
    }

    #[test]
    fn test_wasm_engine_reload_law_replaces() {
        let mut engine = WasmEngine::new();
        load_law(&mut engine, MINIMAL_LAW_YAML);
        assert_eq!(engine.law_count(), 1);

        // Loading the same law again replaces the existing version
        load_law(&mut engine, MINIMAL_LAW_YAML);
        assert_eq!(engine.law_count(), 1);
    }

    #[test]
    fn test_wasm_engine_cross_law_execution() {
        let mut engine = WasmEngine::new();

        // Law A: outputs a constant value
        let law_a = r#"
$id: law_a
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Provides base value
    machine_readable:
      execution:
        output:
          - name: base_value
            type: number
        actions:
          - output: base_value
            value: 100
"#;

        // Law B: references law_a's output
        let law_b = r#"
$id: law_b
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Uses base value from law A
    machine_readable:
      execution:
        input:
          - name: base_value
            type: number
            source:
              regulation: law_a
              output: base_value
        output:
          - name: doubled
            type: number
        actions:
          - output: doubled
            operation: MULTIPLY
            values:
              - $base_value
              - 2
"#;

        load_law(&mut engine, law_a);
        load_law(&mut engine, law_b);

        // Execute via the service directly (can't use JsValue in native tests)
        let params = BTreeMap::new();
        let result = engine
            .service
            .evaluate_law_output("law_b", "doubled", params, "2025-01-01")
            .unwrap();

        assert_eq!(result.outputs.get("doubled"), Some(&Value::Int(200)));
    }

    #[test]
    fn test_wasm_engine_data_source() {
        let mut engine = WasmEngine::new();

        // Law that reads age from a data source via source reference
        let law = r#"
$id: data_law
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Uses data source
    machine_readable:
      execution:
        parameters:
          - name: bsn
            type: string
            required: true
        input:
          - name: age
            type: number
            source:
              output: age
        output:
          - name: is_adult
            type: boolean
        actions:
          - output: is_adult
            operation: GREATER_THAN_OR_EQUAL
            subject: $age
            value: 18
"#;
        load_law(&mut engine, law);

        // Register data source
        let records = vec![{
            let mut r = BTreeMap::new();
            r.insert("bsn".to_string(), Value::String("123".to_string()));
            r.insert("age".to_string(), Value::Int(25));
            r
        }];
        engine
            .service
            .register_dict_source("people", "bsn", records)
            .unwrap();

        let mut params = BTreeMap::new();
        params.insert("bsn".to_string(), Value::String("123".to_string()));

        let result = engine
            .service
            .evaluate_law_output("data_law", "is_adult", params, "2025-01-01")
            .unwrap();

        assert_eq!(result.outputs.get("is_adult"), Some(&Value::Bool(true)));

        // Clear and verify
        engine.clear_data_sources();
        assert_eq!(engine.service.data_source_count(), 0);
    }

    #[test]
    fn test_wasm_engine_execute_with_trace() {
        let mut engine = WasmEngine::new();
        load_law(&mut engine, MINIMAL_LAW_YAML);

        let mut params = BTreeMap::new();
        params.insert("value".to_string(), Value::Int(21));

        let result = engine
            .service
            .evaluate_law_output_with_trace("test_law", "result", params, "2025-01-01")
            .unwrap();

        assert_eq!(result.outputs.get("result"), Some(&Value::Int(42)));
        assert!(result.trace.is_some(), "Trace should be populated");

        let trace = result.trace.unwrap();
        assert!(!trace.children.is_empty(), "Trace should have children");

        // Verify box-drawing rendering works
        let text = trace.render_box_drawing();
        assert!(!text.is_empty(), "Box-drawing trace should not be empty");
    }
}
