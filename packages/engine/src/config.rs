//! Configuration constants for the RegelRecht engine
//!
//! Centralized configuration values used throughout the engine for:
//! - Security limits (prevent DoS attacks)
//! - Resource constraints (memory, CPU)
//! - Recursion depth limits (prevent stack overflow)
//!
//! # Security Considerations
//!
//! These limits are designed to prevent:
//! - YAML bombs (deeply nested or very large documents)
//! - Infinite recursion (circular references)
//! - Memory exhaustion (too many laws or large arrays)
//!
//! # Customization
//!
//! Currently these are compile-time constants. Future versions may
//! support runtime configuration via environment variables or a
//! configuration file.

/// Maximum number of laws that can be loaded simultaneously.
///
/// Prevents memory exhaustion from loading too many laws.
/// 100 laws is sufficient for most use cases (Dutch legal system
/// typically involves ~10-20 interconnected regulations).
pub const MAX_LOADED_LAWS: usize = 100;

/// Maximum YAML document size in bytes (1 MB).
///
/// Prevents YAML bomb attacks and excessive memory usage during parsing.
/// 1 MB is sufficient for any reasonable law document (typical laws are 10-100 KB).
pub const MAX_YAML_SIZE: usize = 1_000_000;

/// Maximum number of elements in any array within a law document.
///
/// Prevents DoS via documents with extremely large arrays.
/// 1000 elements is sufficient for any reasonable law structure.
pub const MAX_ARRAY_SIZE: usize = 1_000;

/// Maximum combined depth for reference resolution in the service layer.
///
/// A single shared counter governs cross-law reference chains and same-law
/// (internal) article-reference chains *together*: every hop of either kind
/// draws on this one budget within a resolution chain, not on separate
/// per-kind budgets. Prevents infinite loops and stack overflow.
/// 20 levels is conservative: cross-law chains in Dutch regulations are
/// typically 3-5 levels (Wet -> Ministeriele Regeling -> Gemeentelijke
/// Verordening), with internal article chains within a law adding only a few
/// more — well under the shared budget.
pub const MAX_CROSS_LAW_DEPTH: usize = 20;

/// Maximum nesting depth for operations during evaluation.
///
/// Prevents stack overflow from deeply nested operation expressions.
/// 100 levels is sufficient for complex calculations while preventing abuse.
pub const MAX_OPERATION_DEPTH: usize = 100;

/// The schema versions this engine knows about, in one place.
///
/// `include_str!` needs a literal path, so the embedded-schema table in
/// [`crate::schema`] can't read a runtime array — it takes the list from this
/// macro instead. [`SUPPORTED_SCHEMAS`] is built from the same expansion, and
/// the tests below hold it against the `schema/` directories and the
/// `supported-schemas` metadata in Cargo.toml. Adding a schema version is a
/// one-line change here.
macro_rules! with_schema_versions {
    ($callback:ident) => {
        $callback! {
            "v0.2.0", "v0.3.0", "v0.3.1", "v0.3.2", "v0.4.0", "v0.5.0",
            "v0.5.1", "v0.5.2", "v0.5.3", "v0.5.4", "v0.5.5", "v0.5.6",
        }
    };
}
// Its only consumer, `crate::schema`, is behind the `validate` feature.
#[allow(unused_imports)]
pub(crate) use with_schema_versions;

macro_rules! as_slice {
    ($($version:literal),* $(,)?) => { &[$($version),*] };
}

/// Schema versions supported by this engine version (RFC-013).
///
/// A regulation referencing a schema version outside this list is rejected at
/// load time.
pub const SUPPORTED_SCHEMAS: &[&str] = with_schema_versions!(as_slice);

/// Maximum recursion depth for dot notation property access.
///
/// Prevents stack overflow on malicious input like "a.a.a.a.a...".
/// 32 levels is far beyond what any legitimate data structure would need.
pub const MAX_PROPERTY_DEPTH: usize = 32;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constants_are_reasonable() {
        // Sanity checks that limits are within reasonable bounds
        assert!(MAX_LOADED_LAWS >= 10, "Should allow at least 10 laws");
        assert!(MAX_LOADED_LAWS <= 1000, "Should not allow excessive laws");

        assert!(MAX_YAML_SIZE >= 100_000, "Should allow at least 100KB");
        assert!(MAX_YAML_SIZE <= 10_000_000, "Should not allow 10MB+");

        assert!(MAX_ARRAY_SIZE >= 100, "Should allow reasonable arrays");
        assert!(MAX_ARRAY_SIZE <= 10_000, "Should not allow huge arrays");

        assert!(MAX_CROSS_LAW_DEPTH >= 5, "Should allow typical chains");
        assert!(MAX_CROSS_LAW_DEPTH <= 50, "Should limit deep chains");

        assert!(MAX_OPERATION_DEPTH >= 50, "Should allow complex ops");
        assert!(MAX_OPERATION_DEPTH <= 500, "Should limit extreme nesting");

        assert!(MAX_PROPERTY_DEPTH >= 10, "Should allow nested objects");
        assert!(MAX_PROPERTY_DEPTH <= 100, "Should limit extreme depth");
    }

    /// A schema version that exists on disk but not here validates green with
    /// `just validate` and is then refused by `Article::load` — officially
    /// valid, not executable. The `schema/` tree is the ground truth.
    #[test]
    fn supported_schemas_covers_every_schema_directory() {
        let schema_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../schema");
        let mut on_disk: Vec<String> = std::fs::read_dir(&schema_root)
            .expect("schema/ directory must be readable")
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|name| name.starts_with('v'))
            .collect();
        on_disk.sort();

        let mut supported: Vec<String> = SUPPORTED_SCHEMAS.iter().map(|s| s.to_string()).collect();
        supported.sort();

        assert_eq!(
            supported, on_disk,
            "SUPPORTED_SCHEMAS and schema/ have diverged; update with_schema_versions! in config.rs"
        );
    }

    /// The `supported-schemas` metadata is the crate's published claim about
    /// which contracts it honours. Nothing reads it at runtime, so only a test
    /// keeps it honest.
    #[test]
    fn cargo_metadata_matches_supported_schemas() {
        let manifest = include_str!("../Cargo.toml");
        let line = manifest
            .lines()
            .find(|l| l.trim_start().starts_with("supported-schemas"))
            .expect("Cargo.toml must declare supported-schemas metadata");
        let declared: Vec<&str> = line
            .split_once('[')
            .and_then(|(_, rest)| rest.split_once(']'))
            .expect("supported-schemas must be a single-line array")
            .0
            .split(',')
            .map(|s| s.trim().trim_matches('"'))
            .filter(|s| !s.is_empty())
            .collect();

        assert_eq!(declared, SUPPORTED_SCHEMAS);
    }
}
