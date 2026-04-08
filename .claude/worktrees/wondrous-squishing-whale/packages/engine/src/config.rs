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

/// Maximum depth for internal reference resolution within a single law.
///
/// Prevents stack overflow from deeply nested article references.
/// 50 levels is far beyond what any legitimate law structure would need.
pub const MAX_RESOLUTION_DEPTH: usize = 50;

/// Maximum depth for cross-law reference resolution.
///
/// Prevents infinite loops in cross-law reference chains.
/// 20 levels is conservative - Dutch regulations typically have at most
/// 3-5 levels (Wet -> Ministerieel Regeling -> Gemeentelijke Verordening).
pub const MAX_CROSS_LAW_DEPTH: usize = 20;

/// Maximum nesting depth for operations during evaluation.
///
/// Prevents stack overflow from deeply nested operation expressions.
/// 100 levels is sufficient for complex calculations while preventing abuse.
pub const MAX_OPERATION_DEPTH: usize = 100;

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

        assert!(
            MAX_RESOLUTION_DEPTH >= 10,
            "Should allow reasonable nesting"
        );
        assert!(MAX_RESOLUTION_DEPTH <= 100, "Should limit deep nesting");

        assert!(MAX_CROSS_LAW_DEPTH >= 5, "Should allow typical chains");
        assert!(MAX_CROSS_LAW_DEPTH <= 50, "Should limit deep chains");

        assert!(MAX_OPERATION_DEPTH >= 50, "Should allow complex ops");
        assert!(MAX_OPERATION_DEPTH <= 500, "Should limit extreme nesting");

        assert!(MAX_PROPERTY_DEPTH >= 10, "Should allow nested objects");
        assert!(MAX_PROPERTY_DEPTH <= 100, "Should limit extreme depth");
    }
}
