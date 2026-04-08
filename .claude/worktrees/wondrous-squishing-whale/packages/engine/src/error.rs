//! Error types for the RegelRecht engine
//!
//! This module provides two error types:
//!
//! - [`EngineError`]: Internal error type with full details for debugging
//! - [`ExternalError`]: Sanitized error type safe for external exposure
//!
//! # Security Considerations
//!
//! Internal errors may contain sensitive information like file paths,
//! internal state, or system details. Use `ExternalError` when returning
//! errors to external callers (API responses, WASM, etc.) to prevent
//! information disclosure.

use thiserror::Error;

/// Main error type for engine operations
#[derive(Error, Debug)]
pub enum EngineError {
    /// Failed to load or parse a law file
    #[error("Failed to load law: {0}")]
    LoadError(String),

    /// YAML parsing error
    #[error("YAML parse error: {0}")]
    YamlError(#[from] serde_yaml_ng::Error),

    /// JSON serialization error
    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),

    /// IO error (file operations)
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    /// Variable not found during resolution
    #[error("Variable not found: {0}")]
    VariableNotFound(String),

    /// Invalid operation
    #[error("Invalid operation: {0}")]
    InvalidOperation(String),

    /// Type mismatch during operation
    #[error("Type mismatch: expected {expected}, got {actual}")]
    TypeMismatch { expected: String, actual: String },

    /// Division by zero
    #[error("Division by zero")]
    DivisionByZero,

    /// Invalid URI format
    #[error("Invalid URI: {0}")]
    InvalidUri(String),

    /// Law not found
    #[error("Law not found: {0}")]
    LawNotFound(String),

    /// Article not found
    #[error("Article not found: {law_id}#{article}")]
    ArticleNotFound { law_id: String, article: String },

    /// Output not found in article
    #[error("Output '{output}' not found in law '{law_id}'")]
    OutputNotFound { law_id: String, output: String },

    /// Circular reference detected
    #[error("Circular reference detected: {0}")]
    CircularReference(String),

    /// Required parameter missing
    #[error("Required parameter missing: {0}")]
    MissingParameter(String),

    /// Arithmetic overflow when converting f64 to i64
    #[error("Arithmetic overflow: {0}")]
    ArithmeticOverflow(String),

    /// Maximum operation nesting depth exceeded
    #[error("Maximum operation depth exceeded: {0} levels")]
    MaxDepthExceeded(usize),

    /// Resolution error (IoC open term resolution, priority conflicts, etc.)
    #[error("Resolution error: {0}")]
    ResolutionError(String),

    /// Data source error
    #[error("Data source error: {0}")]
    DataSourceError(String),

    /// Invalid date format
    #[error("Invalid date format: {0}")]
    InvalidDate(String),

    /// External reference not resolved - requires ServiceProvider
    #[error(
        "External reference not resolved: input '{input_name}' requires resolution from \
         regulation '{regulation}' output '{output}'. \
         Pass the value as a parameter or use ServiceProvider."
    )]
    ExternalReferenceNotResolved {
        input_name: String,
        regulation: String,
        output: String,
    },
}

/// Result type alias for engine operations
pub type Result<T> = std::result::Result<T, EngineError>;

/// External-safe error type that sanitizes internal details.
///
/// Use this type when returning errors to external callers (API responses,
/// WASM bindings, etc.) to prevent information disclosure.
///
/// # Example
///
/// ```
/// use regelrecht_engine::error::{EngineError, ExternalError};
///
/// fn handle_request() -> Result<(), ExternalError> {
///     let internal_result: Result<(), EngineError> = Err(EngineError::IoError(
///         std::io::Error::new(std::io::ErrorKind::NotFound, "secret/path/file.yaml")
///     ));
///
///     // Convert to external error (sanitizes path)
///     internal_result.map_err(ExternalError::from)
/// }
/// ```
#[derive(Error, Debug)]
pub enum ExternalError {
    /// Failed to load law configuration
    #[error("Failed to load law configuration")]
    LoadError,

    /// YAML parsing failed
    #[error("Invalid law format")]
    ParseError,

    /// Variable not found during resolution
    #[error("Variable not found: {0}")]
    VariableNotFound(String),

    /// Invalid operation
    #[error("Invalid operation: {0}")]
    InvalidOperation(String),

    /// Type mismatch during operation
    #[error("Type mismatch: expected {expected}, got {actual}")]
    TypeMismatch { expected: String, actual: String },

    /// Division by zero
    #[error("Division by zero")]
    DivisionByZero,

    /// Invalid URI format
    #[error("Invalid URI format")]
    InvalidUri,

    /// Law not found
    #[error("Law not found: {0}")]
    LawNotFound(String),

    /// Article not found
    #[error("Article not found in law")]
    ArticleNotFound,

    /// Output not found
    #[error("Output not found: {0}")]
    OutputNotFound(String),

    /// Circular reference detected
    #[error("Circular reference detected")]
    CircularReference,

    /// Required parameter missing
    #[error("Required parameter missing: {0}")]
    MissingParameter(String),

    /// Arithmetic overflow
    #[error("Arithmetic overflow")]
    ArithmeticOverflow,

    /// Maximum depth exceeded
    #[error("Maximum nesting depth exceeded")]
    MaxDepthExceeded,

    /// Resolution error (IoC open term resolution failed)
    #[error("Resolution failed")]
    ResolutionError,

    /// External reference not resolved
    #[error("External reference not resolved: {0}")]
    ExternalReferenceNotResolved(String),

    /// Data source error
    #[error("Data source error")]
    DataSourceError,

    /// Invalid date format
    #[error("Invalid date format")]
    InvalidDate,
}

impl From<EngineError> for ExternalError {
    fn from(err: EngineError) -> Self {
        // Log the internal error for debugging (if tracing is configured)
        tracing::debug!(internal_error = ?err, "Converting internal error to external");

        match err {
            EngineError::LoadError(_) | EngineError::IoError(_) => ExternalError::LoadError,
            EngineError::YamlError(_) | EngineError::JsonError(_) => ExternalError::ParseError,
            EngineError::VariableNotFound(name) => ExternalError::VariableNotFound(name),
            EngineError::InvalidOperation(msg) => ExternalError::InvalidOperation(msg),
            EngineError::TypeMismatch { expected, actual } => {
                ExternalError::TypeMismatch { expected, actual }
            }
            EngineError::DivisionByZero => ExternalError::DivisionByZero,
            EngineError::InvalidUri(_) => ExternalError::InvalidUri,
            EngineError::LawNotFound(id) => ExternalError::LawNotFound(id),
            EngineError::ArticleNotFound { .. } => ExternalError::ArticleNotFound,
            EngineError::OutputNotFound { output, .. } => ExternalError::OutputNotFound(output),
            EngineError::CircularReference(_) => ExternalError::CircularReference,
            EngineError::MissingParameter(name) => ExternalError::MissingParameter(name),
            EngineError::ArithmeticOverflow(_) => ExternalError::ArithmeticOverflow,
            EngineError::MaxDepthExceeded(_) => ExternalError::MaxDepthExceeded,
            EngineError::ResolutionError(_) => ExternalError::ResolutionError,
            EngineError::ExternalReferenceNotResolved { input_name, .. } => {
                ExternalError::ExternalReferenceNotResolved(input_name)
            }
            EngineError::DataSourceError(_) => ExternalError::DataSourceError,
            EngineError::InvalidDate(_) => ExternalError::InvalidDate,
        }
    }
}

// WASM error conversion
#[cfg(feature = "wasm")]
impl From<EngineError> for wasm_bindgen::JsValue {
    fn from(err: EngineError) -> Self {
        wasm_bindgen::JsValue::from_str(&err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = EngineError::VariableNotFound("test_var".to_string());
        assert_eq!(err.to_string(), "Variable not found: test_var");
    }

    #[test]
    fn test_type_mismatch_display() {
        let err = EngineError::TypeMismatch {
            expected: "number".to_string(),
            actual: "string".to_string(),
        };
        assert_eq!(
            err.to_string(),
            "Type mismatch: expected number, got string"
        );
    }

    #[test]
    fn test_external_error_sanitizes_paths() {
        // IoError with path should be sanitized
        let internal = EngineError::IoError(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "/secret/internal/path/file.yaml",
        ));
        let external: ExternalError = internal.into();

        let msg = external.to_string();
        assert!(!msg.contains("/secret"), "Path should be sanitized");
        assert!(!msg.contains("file.yaml"), "Filename should be sanitized");
        assert_eq!(msg, "Failed to load law configuration");
    }

    #[test]
    fn test_external_error_preserves_safe_info() {
        // Variable names are safe to expose
        let internal = EngineError::VariableNotFound("user_age".to_string());
        let external: ExternalError = internal.into();
        assert_eq!(external.to_string(), "Variable not found: user_age");

        // Law IDs are safe to expose
        let internal = EngineError::LawNotFound("zorgtoeslagwet".to_string());
        let external: ExternalError = internal.into();
        assert_eq!(external.to_string(), "Law not found: zorgtoeslagwet");
    }

    #[test]
    fn test_external_error_hides_internal_details() {
        // CircularReference details are hidden
        let internal =
            EngineError::CircularReference("Complex circular chain: a -> b -> c -> a".to_string());
        let external: ExternalError = internal.into();
        assert_eq!(external.to_string(), "Circular reference detected");

        // ArticleNotFound hides law/article details
        let internal = EngineError::ArticleNotFound {
            law_id: "internal_law".to_string(),
            article: "secret_article".to_string(),
        };
        let external: ExternalError = internal.into();
        assert!(!external.to_string().contains("internal_law"));
        assert!(!external.to_string().contains("secret_article"));
    }
}
