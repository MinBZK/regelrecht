//! Embedded JSON Schemas + version detection, shared by the `validate` binary
//! and the schema↔model conformance test suite.
//!
//! Only compiled with the `validate` feature (which pulls in `jsonschema`).
//! The version list itself lives in [`crate::config`], which is compiled
//! unconditionally, so the embedded-schema table here and the
//! `SUPPORTED_SCHEMAS` the loader rejects on cannot drift apart.

use std::collections::HashMap;

use jsonschema::Validator;

use crate::config::with_schema_versions;

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

#[cfg(test)]
mod tests {
    use super::*;

    /// The repo's `schema/` directory, resolved from this crate's manifest dir.
    fn schema_dir() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../schema")
    }

    /// Every `schema/vX.Y.Z/` directory in the repo must be registered in
    /// `with_schema_versions!`, and nothing else may be. This is the local
    /// counterpart of the CI guard "Check schema versions registered in
    /// schema.rs": adding a schema version without wiring it up here means the
    /// validator silently rejects every document that declares it.
    #[test]
    fn every_schema_directory_is_embedded() {
        let mut on_disk: Vec<String> = std::fs::read_dir(schema_dir())
            .expect("read schema/ directory")
            .map(|entry| entry.expect("read schema/ entry").file_name())
            .map(|name| name.to_string_lossy().into_owned())
            // `schema/latest` is a symlink to the current version, not a
            // version of its own.
            .filter(|name| name.starts_with('v'))
            .collect();
        on_disk.sort();
        assert!(
            on_disk.len() >= 12,
            "expected the versioned schema directories, found {on_disk:?}"
        );

        let schemas = load_schemas().expect("embedded schemas parse as JSON");
        let mut embedded: Vec<String> = schemas.keys().map(|k| (*k).to_owned()).collect();
        embedded.sort();

        assert_eq!(
            embedded, on_disk,
            "with_schema_versions! is out of sync with the schema/ directory"
        );

        // Each embedded value is the actual schema document, not a placeholder.
        for (version, schema) in &schemas {
            let id = schema
                .get("$id")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_else(|| panic!("schema {version} has no $id"));
            assert!(
                id.contains(version),
                "schema {version} embedded under the wrong key: $id is {id}"
            );
        }
    }

    #[test]
    fn detect_version_reads_the_version_out_of_the_schema_url() {
        let value = serde_json::json!({
            "$schema": "https://raw.githubusercontent.com/MinBZK/regelrecht/refs/tags/\
                        schema-v0.5.6/schema/v0.5.6/schema.json"
        });
        assert_eq!(detect_version(&value), Some("v0.5.6"));

        // An older version resolves to itself, not to the newest entry.
        let older = serde_json::json!({ "$schema": "schema/v0.3.1/schema.json" });
        assert_eq!(detect_version(&older), Some("v0.3.1"));
    }

    #[test]
    fn detect_version_is_none_when_the_version_is_absent_or_unregistered() {
        assert_eq!(detect_version(&serde_json::json!({})), None);
        assert_eq!(detect_version(&serde_json::json!({ "$schema": 5 })), None);
        assert_eq!(
            detect_version(&serde_json::json!({ "$schema": "schema/v9.9.9/schema.json" })),
            None,
        );
    }

    /// A tiny schema, so the assertions are about `validation_errors` and not
    /// about the law format.
    fn tiny_schema() -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "required": ["naam"],
            "properties": { "naam": { "type": "string" } },
        })
    }

    #[test]
    fn validation_errors_is_empty_for_a_valid_document() {
        let errors = validation_errors(&tiny_schema(), &serde_json::json!({ "naam": "aow" }))
            .expect("schema compiles");
        assert!(errors.is_empty(), "unexpected errors: {errors:?}");
    }

    #[test]
    fn validation_errors_reports_each_violation_with_its_instance_path() {
        let errors = validation_errors(&tiny_schema(), &serde_json::json!({ "naam": 5 }))
            .expect("schema compiles");
        assert_eq!(errors.len(), 1, "unexpected errors: {errors:?}");
        assert!(
            errors[0].starts_with("/naam: "),
            "error is not formatted as \"{{instance_path}}: {{message}}\": {}",
            errors[0]
        );

        // A violation at the root keeps the empty instance path.
        let errors = validation_errors(&tiny_schema(), &serde_json::json!({})) //
            .expect("schema compiles");
        assert_eq!(errors.len(), 1, "unexpected errors: {errors:?}");
        assert!(
            errors[0].starts_with(": "),
            "root violation lost its instance path: {}",
            errors[0]
        );
    }

    #[test]
    fn validation_errors_fails_on_an_uncompilable_schema() {
        let broken = serde_json::json!({ "type": "geen-json-schema-type" });
        assert!(validation_errors(&broken, &serde_json::json!({})).is_err());
    }

    #[test]
    fn validation_errors_for_validates_against_the_cached_validator() {
        // An empty document misses every required top-level field, so the real
        // schema must produce errors — and each one formatted with its path.
        let errors =
            validation_errors_for("v0.5.6", &serde_json::json!({})).expect("v0.5.6 is embedded");
        assert!(
            !errors.is_empty(),
            "an empty document is not a valid law document"
        );
        for error in &errors {
            assert!(
                error.starts_with(": "),
                "error is not formatted as \"{{instance_path}}: {{message}}\": {error}"
            );
        }

        // Every embedded version is reachable through the cache, not just the
        // newest one.
        for version in load_schemas().expect("embedded schemas parse").keys() {
            assert!(
                validation_errors_for(version, &serde_json::json!({})).is_ok(),
                "version {version} is embedded but has no compiled validator"
            );
        }
    }

    #[test]
    fn validation_errors_for_fails_on_an_unknown_version() {
        let error = validation_errors_for("v9.9.9", &serde_json::json!({}))
            .expect_err("v9.9.9 is not embedded");
        assert!(error.contains("v9.9.9"), "unhelpful error: {error}");
    }
}
