//! Embedded JSON Schemas + version detection, shared by the `validate` binary
//! and the schema↔model conformance test suite.
//!
//! Only compiled with the `validate` feature (which pulls in `jsonschema`).
//! Keeping the schema-loading list and version detection here means there is a
//! single copy of the 12-version `include_str!` table — see the CI guard
//! "Check schema versions registered in schema.rs" which greps this file.

use std::collections::HashMap;

use jsonschema::Validator;

/// Embedded schemas keyed by their `$id` URL suffix (version path).
///
/// These are compiled-in from the repo's schema/ directory and are guaranteed
/// to be valid JSON at build time.
pub fn load_schemas() -> Result<HashMap<&'static str, serde_json::Value>, String> {
    let mut schemas = HashMap::new();
    let v020: serde_json::Value =
        serde_json::from_str(include_str!("../../../schema/v0.2.0/schema.json"))
            .map_err(|e| format!("invalid v0.2.0 schema JSON: {e}"))?;
    let v030: serde_json::Value =
        serde_json::from_str(include_str!("../../../schema/v0.3.0/schema.json"))
            .map_err(|e| format!("invalid v0.3.0 schema JSON: {e}"))?;
    let v031: serde_json::Value =
        serde_json::from_str(include_str!("../../../schema/v0.3.1/schema.json"))
            .map_err(|e| format!("invalid v0.3.1 schema JSON: {e}"))?;
    let v032: serde_json::Value =
        serde_json::from_str(include_str!("../../../schema/v0.3.2/schema.json"))
            .map_err(|e| format!("invalid v0.3.2 schema JSON: {e}"))?;
    let v040: serde_json::Value =
        serde_json::from_str(include_str!("../../../schema/v0.4.0/schema.json"))
            .map_err(|e| format!("invalid v0.4.0 schema JSON: {e}"))?;
    let v050: serde_json::Value =
        serde_json::from_str(include_str!("../../../schema/v0.5.0/schema.json"))
            .map_err(|e| format!("invalid v0.5.0 schema JSON: {e}"))?;
    let v051: serde_json::Value =
        serde_json::from_str(include_str!("../../../schema/v0.5.1/schema.json"))
            .map_err(|e| format!("invalid v0.5.1 schema JSON: {e}"))?;
    let v052: serde_json::Value =
        serde_json::from_str(include_str!("../../../schema/v0.5.2/schema.json"))
            .map_err(|e| format!("invalid v0.5.2 schema JSON: {e}"))?;
    let v053: serde_json::Value =
        serde_json::from_str(include_str!("../../../schema/v0.5.3/schema.json"))
            .map_err(|e| format!("invalid v0.5.3 schema JSON: {e}"))?;
    let v054: serde_json::Value =
        serde_json::from_str(include_str!("../../../schema/v0.5.4/schema.json"))
            .map_err(|e| format!("invalid v0.5.4 schema JSON: {e}"))?;
    let v055: serde_json::Value =
        serde_json::from_str(include_str!("../../../schema/v0.5.5/schema.json"))
            .map_err(|e| format!("invalid v0.5.5 schema JSON: {e}"))?;
    let v056: serde_json::Value =
        serde_json::from_str(include_str!("../../../schema/v0.5.6/schema.json"))
            .map_err(|e| format!("invalid v0.5.6 schema JSON: {e}"))?;
    schemas.insert("v0.2.0", v020);
    schemas.insert("v0.3.0", v030);
    schemas.insert("v0.3.1", v031);
    schemas.insert("v0.3.2", v032);
    schemas.insert("v0.4.0", v040);
    schemas.insert("v0.5.0", v050);
    schemas.insert("v0.5.1", v051);
    schemas.insert("v0.5.2", v052);
    schemas.insert("v0.5.3", v053);
    schemas.insert("v0.5.4", v054);
    schemas.insert("v0.5.5", v055);
    schemas.insert("v0.5.6", v056);
    Ok(schemas)
}

/// Detect schema version from the `$schema` field in the YAML document.
pub fn detect_version(value: &serde_json::Value) -> Option<&'static str> {
    let schema_url = value.get("$schema")?.as_str()?;
    if schema_url.contains("v0.5.6") {
        Some("v0.5.6")
    } else if schema_url.contains("v0.5.5") {
        Some("v0.5.5")
    } else if schema_url.contains("v0.5.4") {
        Some("v0.5.4")
    } else if schema_url.contains("v0.5.3") {
        Some("v0.5.3")
    } else if schema_url.contains("v0.5.2") {
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

/// Validate `value` against `schema`, returning the validation errors as
/// formatted `"{instance_path}: {message}"` strings. An empty vec means the
/// document is valid. `Err` is only returned when the schema itself fails to
/// compile.
pub fn validation_errors(
    schema: &serde_json::Value,
    value: &serde_json::Value,
) -> Result<Vec<String>, String> {
    let validator = Validator::new(schema).map_err(|e| e.to_string())?;
    Ok(validator
        .iter_errors(value)
        .map(|error| format!("{}: {error}", error.instance_path()))
        .collect())
}

/// All embedded schemas compiled once, keyed by version. Built lazily on first
/// use and cached for the process — callers validating many documents (e.g. the
/// conformance suite over the whole corpus) avoid recompiling the schema per
/// call. `Err` means a schema failed to load or compile.
fn compiled_validators() -> &'static Result<HashMap<&'static str, Validator>, String> {
    static CACHE: std::sync::OnceLock<Result<HashMap<&'static str, Validator>, String>> =
        std::sync::OnceLock::new();
    CACHE.get_or_init(|| {
        let schemas = load_schemas()?;
        let mut validators = HashMap::with_capacity(schemas.len());
        for (version, schema) in &schemas {
            let validator =
                Validator::new(schema).map_err(|e| format!("compile schema {version}: {e}"))?;
            validators.insert(*version, validator);
        }
        Ok(validators)
    })
}

/// Validate `value` against the cached validator for `version`, returning the
/// validation errors as formatted `"{instance_path}: {message}"` strings (empty
/// == valid). Unlike [`validation_errors`], the validator is compiled once and
/// reused across calls. `Err` means the version is unknown or a schema failed to
/// compile.
pub fn validation_errors_for(
    version: &str,
    value: &serde_json::Value,
) -> Result<Vec<String>, String> {
    let validators = compiled_validators().as_ref().map_err(String::clone)?;
    let validator = validators
        .get(version)
        .ok_or_else(|| format!("unknown schema version {version}"))?;
    Ok(validator
        .iter_errors(value)
        .map(|error| format!("{}: {error}", error.instance_path()))
        .collect())
}
