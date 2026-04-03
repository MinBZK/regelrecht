//! Execution Receipt (RFC-013)
//!
//! An Execution Receipt is an output envelope that records everything needed
//! to reproduce an engine execution: which engine, schema, and regulations
//! were used, the execution parameters, and the results.

use std::collections::BTreeMap;

use serde::Serialize;

use crate::engine::OutputProvenance;
use crate::trace::PathNode;
use crate::types::Value;

/// Top-level execution receipt (RFC-013).
#[derive(Debug, Clone, Serialize)]
pub struct ExecutionReceipt {
    pub provenance: ReceiptProvenance,
    pub engine_config: EngineConfig,
    pub scope: ReceiptScope,
    pub execution: ReceiptExecution,
    pub results: ReceiptResults,
    pub accepted_values: Vec<AcceptedValue>,
    pub timestamp: String,
}

/// Which engine and regulation produced the result.
#[derive(Debug, Clone, Serialize)]
pub struct ReceiptProvenance {
    pub engine: String,
    pub engine_version: String,
    pub schema_version: Option<String>,
    pub regulation_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub regulation_valid_from: Option<String>,
    pub regulation_hash: Option<String>,
}

/// Engine startup configuration that affects execution behavior.
#[derive(Debug, Clone, Serialize)]
pub struct EngineConfig {
    pub connectivity: String,
    pub legal_status: String,
    pub untranslatable_mode: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub identity: Option<EngineIdentity>,
}

/// Engine identity for multi-org execution (RFC-009).
#[derive(Debug, Clone, Serialize)]
pub struct EngineIdentity {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub organisation_id: Option<String>,
    #[serde(rename = "type")]
    pub identity_type: String,
}

/// Which regulations were loaded during execution.
#[derive(Debug, Clone, Serialize)]
pub struct ReceiptScope {
    pub sources: Vec<ReceiptSource>,
    pub loaded_regulations: Vec<LoadedRegulation>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub scopes: Vec<ScopeEntry>,
}

/// A corpus source that was loaded.
#[derive(Debug, Clone, Serialize)]
pub struct ReceiptSource {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// A regulation that was loaded during execution.
#[derive(Debug, Clone, Serialize)]
pub struct LoadedRegulation {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub valid_from: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hash: Option<String>,
}

/// An active jurisdiction scope.
#[derive(Debug, Clone, Serialize)]
pub struct ScopeEntry {
    #[serde(rename = "type")]
    pub scope_type: String,
    pub value: String,
}

/// Execution parameters and context.
#[derive(Debug, Clone, Serialize)]
pub struct ReceiptExecution {
    pub calculation_date: String,
    pub parameters: BTreeMap<String, Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference_date: Option<String>,
}

/// Execution outputs and optional trace.
#[derive(Debug, Clone, Serialize)]
pub struct ReceiptResults {
    /// Which outputs were explicitly requested (privacy-by-design audit trail).
    /// Omitted from JSON when empty (legacy callers that don't specify outputs).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub requested_outputs: Vec<String>,
    pub outputs: BTreeMap<String, Value>,
    /// Per-output provenance: which mechanism produced each output.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub output_provenance: BTreeMap<String, OutputProvenance>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace: Option<PathNode>,
}

/// A value accepted from another organisation's engine (RFC-009).
#[derive(Debug, Clone, Serialize)]
pub struct AcceptedValue {
    pub output: String,
    pub value: Value,
    pub authority: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub engine: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub engine_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub regulation_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub regulation_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signed: Option<bool>,
}
