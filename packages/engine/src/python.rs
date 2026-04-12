//! Python bindings for the RegelRecht engine via PyO3.
//!
//! This module provides a native Python extension module, feature-gated
//! behind the `python` flag. It mirrors the WASM bindings but converts
//! between Python objects and Rust `Value` types instead of JavaScript.
//!
//! # Usage (Python)
//!
//! ```python
//! from regelrecht_engine import RegelrechtEngine
//!
//! engine = RegelrechtEngine()
//! law_id = engine.load_law(yaml_string)
//! engine.register_data_source("personen", "bsn", [
//!     {"bsn": "123", "leeftijd": 25}
//! ])
//! result = engine.evaluate("zorgtoeslagwet", ["hoogte_toeslag"], {"bsn": "123"}, "2025-01-01")
//! # result["outputs"]["hoogte_toeslag"] == 177262
//! ```

use pyo3::prelude::*;
use pyo3::types::{PyBool, PyDict, PyFloat, PyInt, PyList, PyString};
use std::collections::BTreeMap;

use crate::service::LawExecutionService;
use crate::types::Value;

// =============================================================================
// Value conversion: Python -> Rust
// =============================================================================

/// Convert a Python object to a Rust `Value`.
fn py_to_value(obj: &Bound<'_, PyAny>) -> PyResult<Value> {
    if obj.is_none() {
        return Ok(Value::Null);
    }
    // Order matters: bool before int (Python bool is a subclass of int)
    if let Ok(b) = obj.downcast::<PyBool>() {
        return Ok(Value::Bool(b.is_true()));
    }
    if let Ok(i) = obj.downcast::<PyInt>() {
        let val: i64 = i.extract()?;
        return Ok(Value::Int(val));
    }
    if let Ok(f) = obj.downcast::<PyFloat>() {
        let val: f64 = f.extract()?;
        return Ok(Value::Float(val));
    }
    if let Ok(s) = obj.downcast::<PyString>() {
        let val: String = s.extract()?;
        return Ok(Value::String(val));
    }
    if let Ok(list) = obj.downcast::<PyList>() {
        let mut arr = Vec::with_capacity(list.len());
        for item in list.iter() {
            arr.push(py_to_value(&item)?);
        }
        return Ok(Value::Array(arr));
    }
    if let Ok(dict) = obj.downcast::<PyDict>() {
        let mut map = BTreeMap::new();
        for (k, v) in dict.iter() {
            let key: String = k.extract()?;
            map.insert(key, py_to_value(&v)?);
        }
        return Ok(Value::Object(map));
    }
    Err(PyErr::new::<pyo3::exceptions::PyTypeError, _>(format!(
        "Cannot convert Python type '{}' to Value",
        obj.get_type().name()?
    )))
}

/// Convert a `PyDict` to `BTreeMap<String, Value>`.
fn pydict_to_btreemap(dict: &Bound<'_, PyDict>) -> PyResult<BTreeMap<String, Value>> {
    let mut map = BTreeMap::new();
    for (k, v) in dict.iter() {
        let key: String = k.extract()?;
        map.insert(key, py_to_value(&v)?);
    }
    Ok(map)
}

// =============================================================================
// Value conversion: Rust -> Python
// =============================================================================

/// Convert a Rust `Value` to a Python object.
fn value_to_py(py: Python<'_>, val: &Value) -> PyResult<PyObject> {
    match val {
        Value::Null => Ok(py.None()),
        Value::Bool(b) => Ok((*b).into_pyobject(py)?.to_owned().into_any().unbind()),
        Value::Int(i) => Ok((*i).into_pyobject(py)?.into_any().unbind()),
        Value::Float(f) => Ok((*f).into_pyobject(py)?.into_any().unbind()),
        Value::String(s) => Ok(s.as_str().into_pyobject(py)?.into_any().unbind()),
        Value::Array(arr) => {
            let list = PyList::empty(py);
            for item in arr {
                list.append(value_to_py(py, item)?)?;
            }
            Ok(list.into_any().unbind())
        }
        Value::Object(map) => {
            let dict = PyDict::new(py);
            for (k, v) in map {
                dict.set_item(k, value_to_py(py, v)?)?;
            }
            Ok(dict.into_any().unbind())
        }
        Value::Untranslatable { article, construct } => {
            let dict = PyDict::new(py);
            dict.set_item("__untranslatable", true)?;
            dict.set_item("article", article.as_str())?;
            dict.set_item("construct", construct.as_str())?;
            Ok(dict.into_any().unbind())
        }
    }
}

/// Convert a `BTreeMap<String, Value>` to a Python dict.
fn btreemap_to_pydict(py: Python<'_>, map: &BTreeMap<String, Value>) -> PyResult<PyObject> {
    let dict = PyDict::new(py);
    for (k, v) in map {
        dict.set_item(k, value_to_py(py, v)?)?;
    }
    Ok(dict.into_any().unbind())
}

// =============================================================================
// Python class
// =============================================================================

/// Native RegelRecht law execution engine.
///
/// Wraps the Rust `LawExecutionService` for direct use from Python
/// without subprocess or WASM overhead.
#[pyclass]
struct RegelrechtEngine {
    service: LawExecutionService,
}

#[pymethods]
impl RegelrechtEngine {
    /// Create a new empty engine instance.
    #[new]
    fn new() -> Self {
        RegelrechtEngine {
            service: LawExecutionService::new(),
        }
    }

    /// Load a law from a YAML string.
    ///
    /// Returns the law ID on success.
    /// Raises ValueError if parsing fails.
    fn load_law(&mut self, yaml: &str) -> PyResult<String> {
        self.service
            .load_law(yaml)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))
    }

    /// Evaluate multiple outputs from a loaded law.
    ///
    /// Args:
    ///     law_id: ID of the loaded law
    ///     output_names: list of output names to calculate
    ///     parameters: dict of input parameters
    ///     calculation_date: date string (YYYY-MM-DD)
    ///
    /// Returns:
    ///     dict with keys: outputs, resolved_inputs, article_number, law_id,
    ///     engine_version, and optional schema_version, regulation_hash,
    ///     regulation_valid_from
    fn evaluate(
        &self,
        py: Python<'_>,
        law_id: &str,
        output_names: Vec<String>,
        parameters: &Bound<'_, PyDict>,
        calculation_date: &str,
    ) -> PyResult<PyObject> {
        let params = pydict_to_btreemap(parameters)?;
        let name_refs: Vec<&str> = output_names.iter().map(|s| s.as_str()).collect();

        let result = self
            .service
            .evaluate_law(law_id, &name_refs, params, calculation_date)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;

        let dict = PyDict::new(py);
        dict.set_item("outputs", btreemap_to_pydict(py, &result.outputs)?)?;
        dict.set_item(
            "resolved_inputs",
            btreemap_to_pydict(py, &result.resolved_inputs)?,
        )?;
        dict.set_item("article_number", &result.article_number)?;
        dict.set_item("law_id", &result.law_id)?;
        dict.set_item("engine_version", &result.engine_version)?;
        if let Some(ref v) = result.schema_version {
            dict.set_item("schema_version", v)?;
        }
        if let Some(ref h) = result.regulation_hash {
            dict.set_item("regulation_hash", h)?;
        }
        if let Some(ref d) = result.regulation_valid_from {
            dict.set_item("regulation_valid_from", d)?;
        }
        if let Some(ref u) = result.law_uuid {
            dict.set_item("law_uuid", u)?;
        }

        Ok(dict.into_any().unbind())
    }

    /// Evaluate a single output from a loaded law.
    ///
    /// Convenience wrapper around evaluate() for a single output.
    fn evaluate_output(
        &self,
        py: Python<'_>,
        law_id: &str,
        output_name: &str,
        parameters: &Bound<'_, PyDict>,
        calculation_date: &str,
    ) -> PyResult<PyObject> {
        let params = pydict_to_btreemap(parameters)?;

        let result = self
            .service
            .evaluate_law_output(law_id, output_name, params, calculation_date)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;

        let dict = PyDict::new(py);
        dict.set_item("outputs", btreemap_to_pydict(py, &result.outputs)?)?;
        dict.set_item(
            "resolved_inputs",
            btreemap_to_pydict(py, &result.resolved_inputs)?,
        )?;
        dict.set_item("article_number", &result.article_number)?;
        dict.set_item("law_id", &result.law_id)?;
        dict.set_item("engine_version", &result.engine_version)?;
        if let Some(ref v) = result.schema_version {
            dict.set_item("schema_version", v)?;
        }
        if let Some(ref h) = result.regulation_hash {
            dict.set_item("regulation_hash", h)?;
        }
        if let Some(ref d) = result.regulation_valid_from {
            dict.set_item("regulation_valid_from", d)?;
        }
        if let Some(ref u) = result.law_uuid {
            dict.set_item("law_uuid", u)?;
        }

        Ok(dict.into_any().unbind())
    }

    /// Evaluate with full execution trace.
    ///
    /// Same as evaluate() but also returns a JSON-serialized execution trace
    /// that shows every step of the evaluation: variable resolution, operations,
    /// requirement checks, data source lookups, etc.
    ///
    /// Args:
    ///     law_id: ID of the loaded law
    ///     output_names: list of output names to calculate
    ///     parameters: dict of input parameters
    ///     calculation_date: date string (YYYY-MM-DD)
    ///
    /// Returns:
    ///     dict with same keys as evaluate() plus 'trace' (JSON string of the
    ///     execution trace tree)
    fn evaluate_with_trace(
        &self,
        py: Python<'_>,
        law_id: &str,
        output_names: Vec<String>,
        parameters: &Bound<'_, PyDict>,
        calculation_date: &str,
    ) -> PyResult<PyObject> {
        let params = pydict_to_btreemap(parameters)?;
        let name_refs: Vec<&str> = output_names.iter().map(|s| s.as_str()).collect();

        let result = self
            .service
            .evaluate_law_with_trace(law_id, &name_refs, params, calculation_date)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;

        let dict = PyDict::new(py);
        dict.set_item("outputs", btreemap_to_pydict(py, &result.outputs)?)?;
        dict.set_item(
            "resolved_inputs",
            btreemap_to_pydict(py, &result.resolved_inputs)?,
        )?;
        dict.set_item("article_number", &result.article_number)?;
        dict.set_item("law_id", &result.law_id)?;
        dict.set_item("engine_version", &result.engine_version)?;
        if let Some(ref v) = result.schema_version {
            dict.set_item("schema_version", v)?;
        }
        if let Some(ref h) = result.regulation_hash {
            dict.set_item("regulation_hash", h)?;
        }
        if let Some(ref d) = result.regulation_valid_from {
            dict.set_item("regulation_valid_from", d)?;
        }
        if let Some(ref u) = result.law_uuid {
            dict.set_item("law_uuid", u)?;
        }

        // Serialize the trace tree as JSON string
        if let Some(ref trace) = result.trace {
            let trace_json = serde_json::to_string(trace)
                .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;
            dict.set_item("trace", trace_json)?;
        }

        Ok(dict.into_any().unbind())
    }

    /// Register a tabular data source from flat records.
    ///
    /// Args:
    ///     name: data source name (e.g., "personal_data")
    ///     key_field: field name used as record key (e.g., "bsn")
    ///     records: list of dicts, each representing a record
    fn register_data_source(
        &mut self,
        name: &str,
        key_field: &str,
        records: &Bound<'_, PyList>,
    ) -> PyResult<()> {
        let mut parsed: Vec<BTreeMap<String, Value>> = Vec::with_capacity(records.len());
        for item in records.iter() {
            let dict = item.downcast::<PyDict>().map_err(|_| {
                PyErr::new::<pyo3::exceptions::PyTypeError, _>("Each record must be a dict")
            })?;
            parsed.push(pydict_to_btreemap(dict)?);
        }

        self.service
            .register_dict_source(name, key_field, parsed)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))
    }

    /// Register a record-set data source with rich options.
    ///
    /// Use this when a single key-field lookup is not enough — when you need
    /// multi-criteria filtering, field aliases (input name differs from
    /// column name), or array-of-records output for FOREACH iteration.
    ///
    /// Args:
    ///     name: data source name
    ///     records: list of dicts (each row is a column → value map)
    ///     key_field: optional single-key field for fast lookup
    ///     select_on: optional list of criterion field names (multi-key filter)
    ///     aliases: optional dict mapping input_name → column_name
    ///     array_field: optional input name that returns the entire matched
    ///         set as an array of records (for FOREACH). Use array_field_projection
    ///         to limit which columns are included in each record.
    ///     array_field_projection: optional list of column names; only these
    ///         columns are kept in each record returned via array_field. Empty
    ///         or omitted means "include the whole record".
    #[pyo3(signature = (name, records, key_field=None, select_on=None, aliases=None, array_field=None, array_field_projection=None))]
    fn register_record_set_source(
        &mut self,
        name: &str,
        records: &Bound<'_, PyList>,
        key_field: Option<&str>,
        select_on: Option<Vec<String>>,
        aliases: Option<&Bound<'_, PyDict>>,
        array_field: Option<&str>,
        array_field_projection: Option<Vec<String>>,
    ) -> PyResult<()> {
        // Parse records
        let mut parsed: Vec<BTreeMap<String, Value>> = Vec::with_capacity(records.len());
        for item in records.iter() {
            let dict = item.downcast::<PyDict>().map_err(|_| {
                PyErr::new::<pyo3::exceptions::PyTypeError, _>("Each record must be a dict")
            })?;
            parsed.push(pydict_to_btreemap(dict)?);
        }

        // Parse aliases
        let aliases_map = if let Some(d) = aliases {
            let mut m = BTreeMap::new();
            for (k, v) in d.iter() {
                let key: String = k.extract()?;
                let val: String = v.extract()?;
                m.insert(key, val);
            }
            Some(m)
        } else {
            None
        };

        // Build array_field tuple
        let array_field_opt = array_field.map(|f| {
            let proj = array_field_projection.unwrap_or_default();
            (f.to_string(), proj)
        });

        self.service
            .register_record_set_source(
                name,
                parsed,
                key_field,
                select_on,
                aliases_map,
                array_field_opt,
            )
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))
    }

    /// Remove a loaded law from the engine.
    ///
    /// Returns True if the law was removed, False if it wasn't loaded.
    fn unload_law(&mut self, law_id: &str) -> bool {
        self.service.unload_law(law_id)
    }

    /// List all loaded law IDs.
    fn list_laws(&self) -> Vec<String> {
        self.service
            .list_laws()
            .into_iter()
            .map(String::from)
            .collect()
    }

    /// Check if a law is loaded.
    fn has_law(&self, law_id: &str) -> bool {
        self.service.has_law(law_id)
    }

    /// Get the number of loaded laws.
    fn law_count(&self) -> usize {
        self.service.law_count()
    }

    /// Remove all registered data sources.
    fn clear_data_sources(&mut self) {
        self.service.clear_data_sources();
    }

    /// Get the number of registered data sources.
    fn data_source_count(&self) -> usize {
        self.service.data_source_count()
    }

    /// Get the engine version string.
    fn version(&self) -> String {
        env!("CARGO_PKG_VERSION").to_string()
    }
}

// =============================================================================
// Python module definition
// =============================================================================

/// RegelRecht native law execution engine for Python.
#[pymodule]
fn regelrecht_engine(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<RegelrechtEngine>()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Test value conversion round-trips via the Rust types directly
    // (PyO3 integration tests require a Python interpreter)

    #[test]
    fn test_value_variants() {
        // Verify we handle all Value variants in our conversion functions
        let values = vec![
            Value::Null,
            Value::Bool(true),
            Value::Int(42),
            Value::Float(3.14),
            Value::String("hello".to_string()),
            Value::Array(vec![Value::Int(1), Value::Int(2)]),
            Value::Object({
                let mut m = BTreeMap::new();
                m.insert("key".to_string(), Value::String("val".to_string()));
                m
            }),
            Value::Untranslatable {
                article: "1".to_string(),
                construct: "test".to_string(),
            },
        ];
        // This test just checks compilation and that the match arms exist.
        // Actual Python round-trip tests need a Python interpreter.
        assert_eq!(values.len(), 8);
    }
}
