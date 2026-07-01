//! Embedded JSON Schemas + version detection, shared by the `validate` binary
//! and the schema↔model conformance test suite.
//!
//! Only compiled with the `validate` feature (which pulls in `jsonschema`).
//! Keeping the schema-loading list and version detection here means there is a
//! single copy of the 12-version `include_str!` table — see the CI guard
//! "Check schema versions registered in schema.rs" which greps this file.

use std::collections::HashMap;

use jsonschema::Validator;

/// Single source of truth for the embedded schema versions.
///
/// `include_str!` requires a literal path, so the version list can't be a
/// runtime array — it lives here as a macro that hands the list to a callback
/// macro. `load_schemas` and `detect_version` are both driven from it, so
/// adding a schema version is a one-line change. The individual `"vX.Y.Z"`
/// string literals also satisfy the CI guard "Check schema versions registered
/// in schema.rs", which greps this file for each `schema/vX.Y.Z/` directory.
macro_rules! with_schema_versions {
    ($callback:ident) => {
        $callback! {
            "v0.2.0", "v0.3.0", "v0.3.1", "v0.3.2", "v0.4.0", "v0.5.0",
            "v0.5.1", "v0.5.2", "v0.5.3", "v0.5.4", "v0.5.5", "v0.5.6",
        }
    };
}

/// Embedded schemas keyed by their `$id` URL suffix (version path).
///
/// These are compiled-in from the repo's schema/ directory and are guaranteed
/// to be valid JSON at build time.
pub fn load_schemas() -> Result<HashMap<&'static str, serde_json::Value>, String> {
    macro_rules! load {
        ($($version:literal),* $(,)?) => {{
            let mut schemas = HashMap::new();
            $(
                let schema: serde_json::Value = serde_json::from_str(include_str!(
                    concat!("../../../schema/", $version, "/schema.json")
                ))
                .map_err(|e| format!("invalid {} schema JSON: {e}", $version))?;
                schemas.insert($version, schema);
            )*
            schemas
        }};
    }
    Ok(with_schema_versions!(load))
}

/// Detect schema version from the `$schema` field in the YAML document.
pub fn detect_version(value: &serde_json::Value) -> Option<&'static str> {
    let schema_url = value.get("$schema")?.as_str()?;
    macro_rules! detect {
        ($($version:literal),* $(,)?) => {
            // Version strings are mutually non-substring, so match order is
            // irrelevant to correctness.
            $(
                if schema_url.contains($version) {
                    return Some($version);
                }
            )*
        };
    }
    with_schema_versions!(detect);
    None
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
