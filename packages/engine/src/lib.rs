//! RegelRecht Engine
//!
//! A Rust implementation of the RegelRecht law execution engine.
//! This library provides functionality for:
//! - Loading and parsing article-based law specifications (YAML)
//! - Executing law logic with variable resolution
//! - Cross-law references and IoC (open terms) resolution
//!
//! # Example
//!
//! ```ignore
//! use regelrecht_engine::{LawExecutionService, Value};
//! use std::collections::HashMap;
//!
//! let mut service = LawExecutionService::new();
//! service.load_law(yaml_str)?;
//!
//! let mut params = HashMap::new();
//! params.insert("BSN".to_string(), Value::String("123456789".to_string()));
//!
//! let result = service.evaluate_law_output(
//!     "zorgtoeslagwet",
//!     "heeft_recht_op_zorgtoeslag",
//!     params,
//!     "2024-01-01",
//! )?;
//! ```

pub mod article;
pub mod config;
pub mod context;
pub mod data_source;
pub mod engine;
pub mod error;
pub mod operations;
pub mod priority;
pub mod receipt;
pub mod resolver;
pub mod service;
pub mod trace;
pub mod types;
pub mod uri;

#[cfg(feature = "wasm")]
pub mod wasm;

/// OpenTelemetry integration (requires `otel` feature).
///
/// **Note:** The `otel` and `wasm` features are mutually exclusive in practice —
/// OTel requires network I/O that WASM cannot provide.
#[cfg(feature = "otel")]
pub mod telemetry;

// Re-export commonly used items
pub use article::{
    Action, ActionOperation, ActionValue, Article, ArticleBasedLaw, Case, Execution,
    HookDeclaration, HookFilter, HookPoint, MachineReadable, OverrideDeclaration,
    ProcedureDefinition, Source, Stage, UntranslatableEntry,
};
pub use config::{
    MAX_ARRAY_SIZE, MAX_CROSS_LAW_DEPTH, MAX_LOADED_LAWS, MAX_OPERATION_DEPTH, MAX_PROPERTY_DEPTH,
    MAX_RESOLUTION_DEPTH, MAX_YAML_SIZE,
};
pub use context::RuleContext;
pub use data_source::{DataSource, DataSourceMatch, DataSourceRegistry, DictDataSource};
pub use engine::{ArticleEngine, ArticleResult};
pub use error::{EngineError, ExternalError, Result};
pub use operations::{evaluate_value, execute_operation, ValueResolver};
pub use receipt::ExecutionReceipt;
pub use resolver::RuleResolver;
pub use service::{ExecutionOutcome, LawExecutionService, LawInfo, ServiceProvider, StageState};
pub use trace::{PathNode, TraceBuilder};
pub use types::{
    Operation, ParameterType, PathNodeType, RegulatoryLayer, ResolveType, UntranslatableMode, Value,
};
pub use uri::{internal_reference, ReferenceType, RegelrechtUri, RegelrechtUriBuilder};

/// Library version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        assert_eq!(VERSION, "0.2.0");
    }

    #[test]
    fn test_reexports() {
        // Verify re-exports work
        let _val = Value::Int(42);
        let _op = Operation::Equals;
        let _err = EngineError::DivisionByZero;
    }
}
