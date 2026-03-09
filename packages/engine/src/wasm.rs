//! WASM bindings for the RegelRecht engine
//!
//! This module provides WebAssembly bindings that match the Python `LawExecutionService` API.
//! It is feature-gated behind the `wasm` feature flag.
//!
//! # Key Constraints
//!
//! - **No filesystem access in WASM**: Laws must be passed as YAML strings via `load_law()`
//! - **Efficient serialization**: Uses `serde-wasm-bindgen` for Rust <-> JavaScript conversion
//!
//! # Limitations
//!
//! The WASM environment cannot perform cross-law resolution or delegation lookups because
//! these require a ServiceProvider implementation (filesystem access, database queries, etc.)
//! that is not available in browser environments.
//!
//! ## Cross-Law References (`source.regulation`)
//!
//! When an article has an input that references another law:
//!
//! ```yaml
//! input:
//!   - name: standaardpremie
//!     source:
//!       regulation: regeling_standaardpremie
//!       output: standaardpremie
//! ```
//!
//! **WASM cannot resolve this automatically**. Instead:
//!
//! 1. Load and execute the referenced law separately
//! 2. Pass the result as a parameter:
//!
//! ```javascript
//! // First, get the standaardpremie value
//! const spResult = engine.execute('regeling_standaardpremie', 'standaardpremie', {}, '2025-01-01');
//! const standaardpremie = spResult.outputs.standaardpremie;
//!
//! // Then pass it as a parameter to the dependent law
//! const result = engine.execute('zorgtoeslagwet', 'hoogte_zorgtoeslag', {
//!     standaardpremie: standaardpremie,  // Pre-resolved value
//!     // ... other parameters
//! }, '2025-01-01');
//! ```
//!
//! ## Delegation (`source.delegation`)
//!
//! When an article delegates to local regulations:
//!
//! ```yaml
//! input:
//!   - name: verlaging_percentage
//!     source:
//!       delegation:
//!         law_id: participatiewet
//!         article: '8'
//!         select_on:
//!           - name: gemeente_code
//!             value: $gemeente_code
//!       output: verlaging_percentage
//! ```
//!
//! **WASM cannot look up delegated regulations**. Instead:
//!
//! 1. Determine which local regulation applies (using `gemeente_code`, etc.)
//! 2. Load and execute that regulation
//! 3. Pass the result as a parameter:
//!
//! ```javascript
//! // Load the applicable local regulation
//! const localLawId = engine.loadLaw(localVerordeningYaml);
//!
//! // Execute to get the delegated value
//! const delegatedResult = engine.execute(localLawId, 'verlaging_percentage', {
//!     gemeente_code: 'GM0363'
//! }, '2025-01-01');
//!
//! // Pass it to the parent law
//! const result = engine.execute('participatiewet', 'bijstandsnorm', {
//!     verlaging_percentage: delegatedResult.outputs.verlaging_percentage,
//!     // ... other parameters
//! }, '2025-01-01');
//! ```
//!
//! # Example (JavaScript)
//!
//! ```javascript
//! import init, { WasmEngine } from 'regelrecht-engine';
//!
//! await init();
//! const engine = new WasmEngine();
//!
//! // Load law from HTTP
//! const response = await fetch('/laws/zorgtoeslagwet.yaml');
//! const yaml = await response.text();
//! const lawId = engine.loadLaw(yaml);
//!
//! // Execute (calculation_date is required)
//! const result = engine.execute(
//!     'zorgtoeslagwet',
//!     'heeft_recht_op_zorgtoeslag',
//!     { BSN: '123456789', vermogen: 50000, heeft_toeslagpartner: false },
//!     '2025-01-01'
//! );
//! console.log(result.outputs);
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
use std::collections::HashMap;
use wasm_bindgen::prelude::*;

use crate::article::ArticleBasedLaw;
use crate::config;
use crate::engine::ArticleEngine;
use crate::error::EngineError;
use crate::types::{RegulatoryLayer, Value};

/// Create a serializer that converts HashMaps to JavaScript objects (not Maps)
fn js_serializer() -> Serializer {
    Serializer::new().serialize_maps_as_objects(true)
}

/// Helper to create consistent error JsValues.
///
/// Formats error messages for JavaScript consumption.
fn wasm_error(msg: &str) -> JsValue {
    JsValue::from_str(msg)
}

/// Convert internal EngineError to user-friendly WASM error with actionable guidance.
fn engine_error_to_wasm(err: EngineError) -> JsValue {
    match err {
        EngineError::ExternalReferenceNotResolved {
            input_name,
            regulation,
            output,
        } => {
            wasm_error(&format!(
                "Cross-law resolution not supported in WASM: input '{}' requires value from '{}' output '{}'. \
                 Pre-resolve this value and pass it as a parameter: {{ \"{}\": <resolved_value> }}",
                input_name, regulation, output, input_name
            ))
        }
        EngineError::DelegationNotResolved {
            input_name,
            law_id,
            article,
            select_on,
        } => {
            wasm_error(&format!(
                "Delegation resolution not supported in WASM: input '{}' requires lookup from '{}' article '{}' \
                 (select_on: [{}]). Load the delegated regulation, execute it, and pass the result as a parameter: \
                 {{ \"{}\": <resolved_value> }}",
                input_name, law_id, article, select_on, input_name
            ))
        }
        EngineError::DelegationError(msg) => {
            wasm_error(&format!(
                "Delegation error: {}. In WASM, delegation must be pre-resolved. \
                 Load and execute the delegated regulation, then pass results as parameters.",
                msg
            ))
        }
        // For other errors, use the standard conversion
        other => wasm_error(&other.to_string()),
    }
}

/// Serializable result for execute() - avoids double serialization through serde_json
#[derive(Serialize)]
struct WasmExecuteResult {
    outputs: HashMap<String, Value>,
    resolved_inputs: HashMap<String, Value>,
    article_number: String,
    law_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    law_uuid: Option<String>,
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

/// WASM-compatible law execution engine.
///
/// Provides the same functionality as Python's `LawExecutionService`, but adapted
/// for WASM constraints (no filesystem access).
#[wasm_bindgen]
pub struct WasmEngine {
    laws: HashMap<String, ArticleBasedLaw>,
}

#[wasm_bindgen]
impl WasmEngine {
    /// Create a new empty engine instance.
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            laws: HashMap::new(),
        }
    }

    /// Load a law from a YAML string.
    ///
    /// # Arguments
    /// * `yaml` - YAML string containing the law definition (max 1 MB)
    ///
    /// # Returns
    /// * `Ok(String)` - The law ID (used for subsequent `execute()` calls)
    /// * `Err(JsValue)` - Error message if parsing fails
    ///
    /// # Example (JavaScript)
    /// ```javascript
    /// const response = await fetch('/laws/zorgtoeslagwet.yaml');
    /// const yaml = await response.text();
    /// const lawId = engine.loadLaw(yaml);  // Returns "zorgtoeslagwet"
    /// ```
    #[wasm_bindgen(js_name = loadLaw)]
    pub fn load_law(&mut self, yaml: &str) -> Result<String, JsValue> {
        // Input validation
        if yaml.len() > config::MAX_YAML_SIZE {
            return Err(wasm_error(&format!(
                "YAML exceeds maximum size ({} bytes)",
                config::MAX_YAML_SIZE
            )));
        }
        if self.laws.len() >= config::MAX_LOADED_LAWS {
            return Err(wasm_error(&format!(
                "Maximum number of laws reached ({})",
                config::MAX_LOADED_LAWS
            )));
        }

        let law = ArticleBasedLaw::from_yaml_str(yaml).map_err(engine_error_to_wasm)?;
        let id = law.id.clone();

        // Check for duplicate - require explicit unload first
        if self.laws.contains_key(&id) {
            return Err(wasm_error(&format!(
                "Law '{}' is already loaded. Call unloadLaw('{}') first to replace it.",
                id, id
            )));
        }

        self.laws.insert(id.clone(), law);
        Ok(id)
    }

    /// Execute an article's output with the given parameters.
    ///
    /// # Arguments
    /// * `law_id` - ID of the loaded law (returned by `loadLaw()`)
    /// * `output_name` - Name of the output to calculate
    /// * `parameters` - JavaScript object with input parameters
    /// * `calculation_date` - Date string (YYYY-MM-DD) for which to calculate
    ///
    /// # Returns
    /// * `Ok(JsValue)` - JavaScript object with `outputs`, `resolved_inputs`, etc.
    /// * `Err(JsValue)` - Error message if execution fails
    ///
    /// # Example (JavaScript)
    /// ```javascript
    /// const result = engine.execute(
    ///     'zorgtoeslagwet',
    ///     'heeft_recht_op_zorgtoeslag',
    ///     { vermogen: 50000, heeft_toeslagpartner: false },
    ///     '2025-01-01'
    /// );
    /// console.log(result.outputs.heeft_recht_op_zorgtoeslag);  // true or false
    /// ```
    #[wasm_bindgen(js_name = execute)]
    pub fn execute(
        &self,
        law_id: &str,
        output_name: &str,
        parameters: JsValue,
        calculation_date: &str,
    ) -> Result<JsValue, JsValue> {
        // Find law
        let law = self
            .laws
            .get(law_id)
            .ok_or_else(|| EngineError::LawNotFound(law_id.to_string()))?;

        // Find article by output
        let article =
            law.find_article_by_output(output_name)
                .ok_or_else(|| EngineError::OutputNotFound {
                    law_id: law_id.to_string(),
                    output: output_name.to_string(),
                })?;

        // Parse parameters from JsValue
        let params: HashMap<String, Value> = serde_wasm_bindgen::from_value(parameters)
            .map_err(|e| wasm_error(&format!("Failed to parse parameters: {}", e)))?;

        // Execute
        let engine = ArticleEngine::new(article, law);
        let result = engine
            .evaluate(params, calculation_date)
            .map_err(engine_error_to_wasm)?;

        // Serialize result directly (no intermediate serde_json::Value)
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

    /// List all loaded law IDs (sorted alphabetically).
    ///
    /// # Returns
    /// Array of law ID strings in alphabetical order.
    ///
    /// # Example (JavaScript)
    /// ```javascript
    /// const laws = engine.listLaws();  // ["awir", "zorgtoeslagwet", ...]
    /// ```
    #[wasm_bindgen(js_name = listLaws)]
    pub fn list_laws(&self) -> Vec<String> {
        let mut keys: Vec<String> = self.laws.keys().cloned().collect();
        keys.sort();
        keys
    }

    /// Get metadata about a loaded law.
    ///
    /// # Arguments
    /// * `law_id` - ID of the law to query
    ///
    /// # Returns
    /// * `Ok(JsValue)` - JavaScript object with law metadata
    /// * `Err(JsValue)` - Error if law not found
    ///
    /// # Example (JavaScript)
    /// ```javascript
    /// const info = engine.getLawInfo('zorgtoeslagwet');
    /// console.log(info.outputs);  // ["heeft_recht_op_zorgtoeslag", "hoogte_zorgtoeslag", ...]
    /// console.log(info.article_count);  // 5
    /// ```
    #[wasm_bindgen(js_name = getLawInfo)]
    pub fn get_law_info(&self, law_id: &str) -> Result<JsValue, JsValue> {
        let law = self
            .laws
            .get(law_id)
            .ok_or_else(|| EngineError::LawNotFound(law_id.to_string()))?;

        // Collect and sort outputs for consistent ordering
        let mut outputs: Vec<String> = law.get_all_outputs().keys().cloned().collect();
        outputs.sort();

        let info = WasmLawInfo {
            id: law.id.clone(),
            regulatory_layer: law.regulatory_layer,
            publication_date: law.publication_date.clone(),
            bwb_id: law.bwb_id.clone(),
            url: law.url.clone(),
            outputs,
            article_count: law.articles.len(),
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
    /// # Arguments
    /// * `law_id` - ID of the law to remove
    ///
    /// # Returns
    /// * `true` if the law was removed, `false` if it wasn't loaded
    #[wasm_bindgen(js_name = unloadLaw)]
    pub fn unload_law(&mut self, law_id: &str) -> bool {
        self.laws.remove(law_id).is_some()
    }

    /// Check if a law is loaded.
    ///
    /// # Arguments
    /// * `law_id` - ID of the law to check
    ///
    /// # Returns
    /// * `true` if the law is loaded, `false` otherwise
    #[wasm_bindgen(js_name = hasLaw)]
    pub fn has_law(&self, law_id: &str) -> bool {
        self.laws.contains_key(law_id)
    }

    /// Get the number of loaded laws.
    #[wasm_bindgen(js_name = lawCount)]
    pub fn law_count(&self) -> usize {
        self.laws.len()
    }

    /// Get the engine version.
    ///
    /// # Returns
    /// Version string (e.g., "0.1.0")
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

// Tests for WasmEngine
//
// Note: Most WASM-specific functionality (JsValue conversion, execute, get_law_info)
// can only be tested in an actual WASM environment. These tests focus on the
// non-WASM-dependent parts of the API.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::article::ArticleBasedLaw;

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
    fn test_wasm_engine_load_law_directly() {
        // Test the underlying law loading without going through JsValue conversion
        let mut engine = WasmEngine::new();

        // Manually parse and add the law (simulating what load_law does internally)
        let law = ArticleBasedLaw::from_yaml_str(MINIMAL_LAW_YAML).unwrap();
        let id = law.id.clone();
        engine.laws.insert(id.clone(), law);

        assert_eq!(engine.law_count(), 1);
        assert!(engine.has_law("test_law"));
        assert_eq!(engine.list_laws(), vec!["test_law".to_string()]);
    }

    #[test]
    fn test_wasm_engine_unload_law() {
        let mut engine = WasmEngine::new();

        // Add law directly
        let law = ArticleBasedLaw::from_yaml_str(MINIMAL_LAW_YAML).unwrap();
        engine.laws.insert(law.id.clone(), law);

        assert!(engine.has_law("test_law"));
        assert!(engine.unload_law("test_law"));
        assert!(!engine.has_law("test_law"));
        assert!(!engine.unload_law("nonexistent"));
    }

    #[test]
    fn test_wasm_engine_list_laws() {
        let mut engine = WasmEngine::new();

        // Add law directly
        let law = ArticleBasedLaw::from_yaml_str(MINIMAL_LAW_YAML).unwrap();
        engine.laws.insert(law.id.clone(), law);

        let laws = engine.list_laws();
        assert_eq!(laws.len(), 1);
        assert!(laws.contains(&"test_law".to_string()));
    }

    #[test]
    fn test_wasm_engine_has_law() {
        let mut engine = WasmEngine::new();
        assert!(!engine.has_law("test_law"));

        let law = ArticleBasedLaw::from_yaml_str(MINIMAL_LAW_YAML).unwrap();
        engine.laws.insert(law.id.clone(), law);

        assert!(engine.has_law("test_law"));
        assert!(!engine.has_law("other_law"));
    }

    #[test]
    fn test_wasm_engine_law_count() {
        let mut engine = WasmEngine::new();
        assert_eq!(engine.law_count(), 0);

        let law = ArticleBasedLaw::from_yaml_str(MINIMAL_LAW_YAML).unwrap();
        engine.laws.insert(law.id.clone(), law);
        assert_eq!(engine.law_count(), 1);

        let yaml2 = r#"
$id: second_law
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Second test article
"#;
        let law2 = ArticleBasedLaw::from_yaml_str(yaml2).unwrap();
        engine.laws.insert(law2.id.clone(), law2);
        assert_eq!(engine.law_count(), 2);
    }
}
