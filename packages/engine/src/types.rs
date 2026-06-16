//! Engine runtime types.
//!
//! `Value`, `Operation`, `ParameterType` and `RegulatoryLayer` are the
//! document-model types: they now live in the dependency-light
//! [`regelrecht_law_model`] crate and are re-exported here so the historical
//! `crate::types::*` paths keep working unchanged. The enums defined below
//! describe engine runtime / execution-trace concerns, not the document format,
//! so they stay in the engine.

use serde::{Deserialize, Serialize};

/// Re-export the canonical document-model value types from the law-model crate.
pub use regelrecht_law_model::{Operation, ParameterType, RegulatoryLayer, Value};

/// How the engine handles articles with `untranslatables` annotations (RFC-012).
///
/// Controls runtime behavior when an article declares legal constructs that
/// cannot be faithfully expressed with the current engine operation set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum UntranslatableMode {
    /// Hard error on any unaccepted untranslatable. Accepted ones execute partial logic.
    #[default]
    Error,
    /// Execute partial logic. Outputs from articles with untranslatables carry an
    /// `UNTRANSLATABLE` taint that propagates through downstream operations (like NaN).
    Propagate,
    /// Execute partial logic, log warning in trace. No taint propagation.
    Warn,
    /// Execute partial logic silently. Only valid for entries with `accepted: true` —
    /// unaccepted untranslatables still error.
    Ignore,
}

impl std::str::FromStr for UntranslatableMode {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "error" => Ok(UntranslatableMode::Error),
            "propagate" => Ok(UntranslatableMode::Propagate),
            "warn" => Ok(UntranslatableMode::Warn),
            "ignore" => Ok(UntranslatableMode::Ignore),
            _ => Err(format!(
                "unknown untranslatable mode '{s}', expected: error, propagate, warn, ignore"
            )),
        }
    }
}

/// Engine connectivity mode — whether this engine resolves cross-law references.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Connectivity {
    /// Engine runs standalone, no cross-law resolution.
    Solo,
}

/// Legal status of execution results.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LegalStatus {
    /// Results are for simulation/testing purposes only.
    Simulation,
}

/// Node type in execution trace
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PathNodeType {
    /// Variable/value resolution step
    Resolve,
    /// Operation execution (e.g., ADD, EQUALS)
    Operation,
    /// Action execution within an article
    Action,
    /// Requirement check
    Requirement,
    /// Cross-law reference resolution (source.regulation lookup)
    CrossLawReference,
    /// Article-level execution
    Article,
    /// Cached cross-law result (memoized)
    Cached,
    /// Open term resolution via IoC (implements lookup)
    OpenTermResolution,
    /// Hook resolution (lifecycle hook firing, RFC-007)
    HookResolution,
    /// Override resolution (lex specialis replacement, RFC-007)
    OverrideResolution,
}

/// Resolve type for variable resolution
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ResolveType {
    /// Value resolved from a regelrecht:// URI
    Uri,
    /// Value resolved from input parameters
    Parameter,
    /// Value resolved from article definitions (constants)
    Definition,
    /// Value resolved from calculated outputs
    Output,
    /// Value resolved from input specification
    Input,
    /// Value resolved from local scope (loop variables)
    Local,
    /// Value resolved from context variables (referencedate)
    Context,
    /// Value resolved from cached cross-law results
    ResolvedInput,
    /// Value resolved from external data source
    DataSource,
    /// Value resolved via open term implementation (IoC)
    OpenTerm,
    /// Value resolved via lifecycle hook (RFC-007)
    Hook,
    /// Value resolved via lex specialis override (RFC-007)
    Override,
}
