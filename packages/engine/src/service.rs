//! Service layer for cross-law resolution
//!
//! Provides the `ServiceProvider` trait and `LawExecutionService` implementation
//! for resolving cross-law references and IoC open-term resolution.
//!
//! # Example
//!
//! ```ignore
//! use regelrecht_engine::{LawExecutionService, Value};
//! use std::collections::BTreeMap;
//!
//! let mut service = LawExecutionService::new();
//! service.load_law(zorgtoeslagwet_yaml)?;
//! service.load_law(regeling_standaardpremie_yaml)?;
//!
//! let mut params = BTreeMap::new();
//! params.insert("BSN".to_string(), Value::String("123456789".to_string()));
//!
//! // This will automatically resolve cross-law references
//! let result = service.evaluate_law_output(
//!     "wet_op_de_zorgtoeslag",
//!     "heeft_recht_op_zorgtoeslag",
//!     params,
//!     "2025-01-01",
//! )?;
//! ```

use crate::article::{Article, ArticleBasedLaw, Execution, HookPoint, MachineReadable};
use crate::config;
use crate::context::RuleContext;
use crate::data_source::{DataSource, DataSourceRegistry, DictDataSource};
use crate::engine::{ArticleEngine, ArticleResult};
use crate::error::{EngineError, Result};
use crate::operations::ValueResolver;
use crate::priority;
use crate::resolver::RuleResolver;
use crate::trace::TraceBuilder;
use crate::types::{PathNodeType, RegulatoryLayer, ResolveType, UntranslatableMode, Value};
use crate::uri::RegelrechtUri;
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::rc::Rc;

// =============================================================================
// Resolution Context
// =============================================================================

/// Context for tracking cross-law resolution state.
///
/// Bundles the state needed for cycle detection and depth tracking
/// during cross-law reference resolution. This reduces the number of
/// parameters passed between internal resolution functions.
///
/// Cache entry: (law_id, output_name, outputs, parameters).
/// Stores identity fields for runtime collision detection alongside cached outputs.
type CacheEntry = (
    String,
    String,
    BTreeMap<String, Value>,
    BTreeMap<String, Value>,
);

/// Uses a scoped push/pop pattern for the visited set to avoid
/// cloning the HashSet on every cross-law descent.
struct ResolutionContext<'a> {
    /// Date for calculations (YYYY-MM-DD)
    calculation_date: &'a str,
    /// Set of law#output keys already being resolved (cycle detection)
    visited: HashSet<String>,
    /// Current resolution depth
    depth: usize,
    /// Optional shared trace builder
    trace: Option<Rc<RefCell<TraceBuilder>>>,
    /// Per-execution memoization cache: hash key → CacheEntry
    cache: HashMap<u64, CacheEntry>,
    /// The law that initiated the current execution chain (for override scoping).
    /// Overrides only apply when declared by this law.
    contextual_law_id: Option<String>,
}

impl<'a> ResolutionContext<'a> {
    /// Create a new resolution context.
    fn new(calculation_date: &'a str) -> Self {
        Self {
            calculation_date,
            visited: HashSet::new(),
            depth: 0,
            trace: None,
            cache: HashMap::new(),
            contextual_law_id: None,
        }
    }

    /// Create a new resolution context with trace builder.
    fn with_trace(calculation_date: &'a str, trace: Rc<RefCell<TraceBuilder>>) -> Self {
        Self {
            calculation_date,
            visited: HashSet::new(),
            depth: 0,
            trace: Some(trace),
            cache: HashMap::new(),
            contextual_law_id: None,
        }
    }

    /// Parse the calculation_date as NaiveDate for version selection.
    fn reference_date(&self) -> Option<NaiveDate> {
        NaiveDate::parse_from_str(self.calculation_date, "%Y-%m-%d").ok()
    }

    /// Enter a cross-law resolution scope: mark key as visited and increment depth.
    fn enter(&mut self, key: String) {
        self.visited.insert(key);
        self.depth += 1;
    }

    /// Leave a cross-law resolution scope: unmark key and decrement depth.
    fn leave(&mut self, key: &str) {
        self.visited.remove(key);
        self.depth -= 1;
    }

    /// Check if a key is already being resolved (cycle detection).
    fn is_visited(&self, key: &str) -> bool {
        self.visited.contains(key)
    }

    /// Push a new trace node. No-op if tracing is disabled.
    fn trace_push(&self, name: impl Into<String>, node_type: PathNodeType) {
        if let Some(ref tb) = self.trace {
            tb.borrow_mut().push(name, node_type);
        }
    }

    /// Set the result on the current trace node. No-op if tracing is disabled.
    fn trace_set_result(&self, result: Value) {
        if let Some(ref tb) = self.trace {
            tb.borrow_mut().set_result(result);
        }
    }

    /// Set a message on the current trace node. No-op if tracing is disabled.
    fn trace_set_message(&self, msg: impl Into<String>) {
        if let Some(ref tb) = self.trace {
            tb.borrow_mut().set_message(msg);
        }
    }

    /// Set the resolve type on the current trace node. No-op if tracing is disabled.
    fn trace_set_resolve_type(&self, rt: ResolveType) {
        if let Some(ref tb) = self.trace {
            tb.borrow_mut().set_resolve_type(rt);
        }
    }

    /// Push a trace node and return a guard that auto-pops on drop.
    ///
    /// Guarantees balanced push/pop even on early returns or errors.
    /// Use `set_message`, `set_result`, etc. on `res_ctx` as usual —
    /// the guard only handles the pop.
    fn trace_guard(&self, name: impl Into<String>, node_type: PathNodeType) -> TraceGuard {
        self.trace_push(name, node_type);
        TraceGuard {
            trace: self.trace.clone(),
        }
    }
}

/// RAII guard that pops a trace node when dropped.
///
/// Created by `ResolutionContext::trace_guard`. Ensures balanced
/// push/pop even when errors cause early returns. Holds its own
/// `Rc` to the trace builder so it doesn't borrow `ResolutionContext`,
/// avoiding conflicts with mutable methods like `enter`/`leave`.
struct TraceGuard {
    trace: Option<Rc<RefCell<TraceBuilder>>>,
}

impl Drop for TraceGuard {
    fn drop(&mut self) {
        if let Some(ref tb) = self.trace {
            tb.borrow_mut().pop();
        }
    }
}

/// Build a cache key from law_id, output_name, and parameters.
///
/// Uses hashing instead of String building to avoid allocations.
/// BTreeMap guarantees sorted key order for deterministic hashing.
/// Note: `DefaultHasher` is randomly seeded per process, so keys are only
/// valid for per-execution memoization (not persisted across runs).
fn cache_key(law_id: &str, output_name: &str, params: &BTreeMap<String, Value>) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    law_id.hash(&mut hasher);
    output_name.hash(&mut hasher);
    // BTreeMap iterates in sorted key order — no explicit sort needed
    for (key, value) in params {
        key.hash(&mut hasher);
        hash_value(value, &mut hasher);
    }
    hasher.finish()
}

/// Hash a Value for cache key purposes.
fn hash_value(value: &Value, hasher: &mut impl Hasher) {
    std::mem::discriminant(value).hash(hasher);
    match value {
        Value::Null => {}
        Value::Bool(b) => b.hash(hasher),
        Value::Int(i) => i.hash(hasher),
        Value::Float(f) => {
            // Canonicalize -0.0 → +0.0 so hash matches PartialEq (IEEE 754: -0.0 == +0.0)
            let canonical = if *f == 0.0 { 0.0_f64 } else { *f };
            canonical.to_bits().hash(hasher);
        }
        Value::String(s) => s.hash(hasher),
        Value::Array(arr) => {
            arr.len().hash(hasher);
            for v in arr {
                hash_value(v, hasher);
            }
        }
        Value::Object(map) => {
            map.len().hash(hasher);
            // BTreeMap iterates in sorted key order — no explicit sort needed
            for (key, v) in map {
                key.hash(hasher);
                hash_value(v, hasher);
            }
        }
        Value::Untranslatable { article, construct } => {
            article.hash(hasher);
            construct.hash(hasher);
        }
    }
}

/// Trait for resolving cross-law references.
///
/// Implement this trait to provide custom law loading and resolution strategies.
/// The default implementation is `LawExecutionService`.
pub trait ServiceProvider {
    /// Resolve and execute a regelrecht:// URI.
    ///
    /// # Arguments
    /// * `uri` - The URI to resolve (e.g., "regelrecht://law_id/output")
    /// * `parameters` - Parameters to pass to the target article
    /// * `calculation_date` - Date for calculations (YYYY-MM-DD)
    ///
    /// # Returns
    /// The execution result from the referenced article.
    fn evaluate_uri(
        &self,
        uri: &str,
        parameters: &BTreeMap<String, Value>,
        calculation_date: &str,
    ) -> Result<ArticleResult>;

    /// Get a law by ID.
    fn get_law(&self, law_id: &str) -> Option<&ArticleBasedLaw>;

    /// Resolve an external input source.
    ///
    /// This is the main entry point for resolving `source.regulation` references.
    ///
    /// # Arguments
    /// * `regulation` - The target regulation ID
    /// * `output` - The output name to resolve
    /// * `source_parameters` - Parameters mapping from source (e.g., {"BSN": "$BSN"})
    /// * `context` - Current execution context for parameter resolution
    /// * `calculation_date` - Date for calculations
    fn resolve_external_input(
        &self,
        regulation: &str,
        output: &str,
        source_parameters: Option<&BTreeMap<String, String>>,
        context: &RuleContext,
        calculation_date: &str,
    ) -> Result<Value>;
}

/// Metadata about a loaded law.
///
/// Provides summary information about a law without requiring full article access.
#[derive(Debug, Clone)]
pub struct LawInfo {
    /// Law identifier
    pub id: String,
    /// Regulatory layer
    pub regulatory_layer: RegulatoryLayer,
    /// Publication date
    pub publication_date: String,
    /// BWB identifier (for national laws)
    pub bwb_id: Option<String>,
    /// URL to official source
    pub url: Option<String>,
    /// List of output names produced by this law
    pub outputs: Vec<String>,
    /// Number of articles in the law
    pub article_count: usize,
}

/// State of a decision progressing through an AWB-defined procedure lifecycle (RFC-008).
///
/// The engine is stateless — this struct is passed in by the caller and returned
/// with updates. The orchestration layer persists it between stages.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageState {
    /// Which AWB procedure this decision follows (e.g., "beschikking")
    pub procedure_id: String,
    /// The law that initiated execution (for override scoping)
    pub contextual_law: String,
    /// Current lifecycle stage (e.g., "BESLUIT", "BEKENDMAKING")
    pub current_stage: String,
    /// Outputs accumulated from all completed stages
    pub accumulated_outputs: BTreeMap<String, Value>,
    /// Original parameters from the initial execution
    pub parameters: BTreeMap<String, Value>,
}

/// Outcome of a stage-aware execution step.
#[derive(Debug)]
pub enum ExecutionOutcome {
    /// All stages completed — final result
    Complete(ArticleResult),
    /// Execution yielded — waiting for external input to advance
    Yielded {
        /// Updated decision state
        state: StageState,
        /// Outputs computed so far (including this stage)
        outputs: BTreeMap<String, Value>,
        /// Inputs required to advance to the next stage
        pending_inputs: Vec<String>,
    },
}

/// High-level service for executing laws with automatic cross-law resolution.
///
/// `LawExecutionService` wraps a `RuleResolver` and implements `ServiceProvider`
/// to enable automatic resolution of external references and open term implementations.
/// It also supports external data sources via `DataSourceRegistry`.
pub struct LawExecutionService {
    resolver: RuleResolver,
    /// Registry for external data sources. Queried during law execution
    /// to resolve inputs before falling back to cross-law resolution.
    /// Acts as an override layer: if a data source provides a field,
    /// it's used instead of triggering cross-law/IoC resolution.
    data_registry: DataSourceRegistry,
    /// Source provenance tracking: law_id → (source_id, source_name).
    source_info: HashMap<String, (String, String)>,
    /// How to handle articles with untranslatable constructs (RFC-012)
    untranslatable_mode: UntranslatableMode,
}

impl Default for LawExecutionService {
    fn default() -> Self {
        Self::new()
    }
}

impl LawExecutionService {
    /// Create a new empty service.
    pub fn new() -> Self {
        Self {
            resolver: RuleResolver::new(),
            data_registry: DataSourceRegistry::new(),
            source_info: HashMap::new(),
            untranslatable_mode: UntranslatableMode::default(),
        }
    }

    /// Set the untranslatable handling mode (RFC-012).
    pub fn set_untranslatable_mode(&mut self, mode: UntranslatableMode) {
        self.untranslatable_mode = mode;
    }

    /// Load a law from YAML string.
    ///
    /// # Returns
    /// The law ID on success.
    pub fn load_law(&mut self, yaml: &str) -> Result<String> {
        self.resolver.load_from_yaml(yaml)
    }

    /// Load a law struct directly.
    ///
    /// # Returns
    /// `Ok(())` on success, `Err` if the maximum number of laws would be exceeded.
    pub fn load_law_struct(&mut self, law: ArticleBasedLaw) -> Result<()> {
        self.resolver.load_law(law)
    }

    /// Load a law from YAML string and record its source provenance.
    ///
    /// # Returns
    /// The law ID on success.
    pub fn load_law_with_source(
        &mut self,
        yaml: &str,
        source_id: &str,
        source_name: &str,
    ) -> Result<String> {
        let law_id = self.resolver.load_from_yaml(yaml)?;
        self.source_info.insert(
            law_id.clone(),
            (source_id.to_string(), source_name.to_string()),
        );
        Ok(law_id)
    }

    /// Get the source provenance for a loaded law.
    ///
    /// Returns `(source_id, source_name)` if the law was loaded via
    /// [`load_law_with_source`](Self::load_law_with_source).
    pub fn get_law_source(&self, law_id: &str) -> Option<(&str, &str)> {
        self.source_info
            .get(law_id)
            .map(|(id, name)| (id.as_str(), name.as_str()))
    }

    /// Build an Execution Receipt (RFC-013) from an ArticleResult.
    ///
    /// Wraps the result with provenance, engine config, and scope metadata.
    pub fn build_receipt(
        &self,
        result: &ArticleResult,
        parameters: &BTreeMap<String, Value>,
        calculation_date: &str,
    ) -> crate::receipt::ExecutionReceipt {
        use crate::receipt::*;

        let loaded_regulations: Vec<LoadedRegulation> = self
            .resolver
            .all_law_versions()
            .map(|law| LoadedRegulation {
                id: law.id.clone(),
                valid_from: law.valid_from.clone(),
                hash: law.content_hash.clone(),
            })
            .collect();

        let sources: Vec<ReceiptSource> = self
            .source_info
            .values()
            .map(|(id, name)| ReceiptSource {
                id: id.clone(),
                name: Some(name.clone()),
            })
            .collect();

        ExecutionReceipt {
            provenance: ReceiptProvenance {
                engine: "regelrecht".to_string(),
                engine_version: result.engine_version.clone(),
                schema_version: result.schema_version.clone(),
                regulation_id: result.law_id.clone(),
                regulation_valid_from: result.regulation_valid_from.clone(),
                regulation_hash: result.regulation_hash.clone(),
            },
            engine_config: EngineConfig {
                // Hardcoded until RFC-009 engine identity and modes are implemented.
                // Solo simulation is the only mode the engine currently supports.
                connectivity: "solo".to_string(),
                legal_status: "simulation".to_string(),
                untranslatable_mode: format!("{:?}", self.untranslatable_mode).to_lowercase(),
                identity: None,
            },
            scope: ReceiptScope {
                sources,
                loaded_regulations,
                scopes: Vec::new(),
            },
            execution: ReceiptExecution {
                calculation_date: calculation_date.to_string(),
                parameters: parameters.clone(),
                reference_date: None,
            },
            results: ReceiptResults {
                outputs: result.outputs.clone(),
                trace: result.trace.clone(),
            },
            accepted_values: Vec::new(),
            timestamp: chrono::Utc::now().to_rfc3339(),
        }
    }

    /// Execute a law output by name.
    ///
    /// This is the main entry point for law execution. It finds the article
    /// that produces the requested output and executes it, automatically
    /// resolving any cross-law references.
    ///
    /// # Arguments
    /// * `law_id` - The law identifier (e.g., "zorgtoeslagwet")
    /// * `output_name` - The output to calculate
    /// * `parameters` - Input parameters
    /// * `calculation_date` - Date for calculations (YYYY-MM-DD)
    ///
    /// # Returns
    /// The execution result with outputs and metadata.
    #[cfg_attr(feature = "otel", tracing::instrument(skip(self, parameters), fields(law_id = %law_id, output = %output_name)))]
    pub fn evaluate_law_output(
        &self,
        law_id: &str,
        output_name: &str,
        parameters: BTreeMap<String, Value>,
        calculation_date: &str,
    ) -> Result<ArticleResult> {
        let mut res_ctx = ResolutionContext::new(calculation_date);
        res_ctx.contextual_law_id = Some(law_id.to_string());
        self.evaluate_law_output_internal(law_id, output_name, parameters, &mut res_ctx)
    }

    /// Execute a law output with tracing enabled.
    ///
    /// Same as `evaluate_law_output` but also returns an execution trace
    /// in the `ArticleResult.trace` field.
    pub fn evaluate_law_output_with_trace(
        &self,
        law_id: &str,
        output_name: &str,
        parameters: BTreeMap<String, Value>,
        calculation_date: &str,
    ) -> Result<ArticleResult> {
        self.evaluate_law_output_with_trace_builder(
            law_id,
            output_name,
            parameters,
            calculation_date,
            TraceBuilder::new(),
        )
    }

    /// Execute a law output with a caller-provided trace builder.
    ///
    /// Use `TraceBuilder::new()` for timed traces or
    /// `TraceBuilder::new_untimed()` to avoid `Instant::now()` overhead
    /// (useful in WASM where timing calls go through JS FFI).
    pub fn evaluate_law_output_with_trace_builder(
        &self,
        law_id: &str,
        output_name: &str,
        parameters: BTreeMap<String, Value>,
        calculation_date: &str,
        trace_builder: TraceBuilder,
    ) -> Result<ArticleResult> {
        let trace = Rc::new(RefCell::new(trace_builder));

        // Push the top-level article node
        {
            let mut tb = trace.borrow_mut();
            tb.push(
                format!("{} ({})", law_id, output_name),
                PathNodeType::Article,
            );
            let mut sorted_params: Vec<_> = parameters
                .iter()
                .map(|(k, v)| format!("{}: {}", k, v))
                .collect();
            sorted_params.sort();
            tb.set_message(format!(
                "{} ({} {{{}}} {})",
                law_id,
                calculation_date,
                sorted_params.join(", "),
                output_name,
            ));
        }

        let mut res_ctx = ResolutionContext::with_trace(calculation_date, Rc::clone(&trace));
        res_ctx.contextual_law_id = Some(law_id.to_string());
        let result =
            self.evaluate_law_output_internal(law_id, output_name, parameters, &mut res_ctx);

        // Drop res_ctx so it releases its Rc clone
        drop(res_ctx);

        match result {
            Ok(mut result) => {
                // Set the result on the top-level article node and pop it
                let mut tb = trace.borrow_mut();
                if let Some(value) = result.outputs.get(output_name) {
                    tb.set_result(value.clone());
                }
                // Pop returns the completed top-level node
                result.trace = tb.pop();
                Ok(result)
            }
            Err(e) => {
                // Pop the top-level node so the partial trace is not lost
                let mut tb = trace.borrow_mut();
                tb.set_message(format!("Execution failed: {}", e));
                let partial_trace = tb.pop();

                // Return an error result that still carries the partial trace.
                // Since Result<ArticleResult> can't carry both, we embed the
                // trace in the error via EngineError::TracedError.
                Err(EngineError::TracedError {
                    source: Box::new(e),
                    trace: partial_trace.map(Box::new),
                })
            }
        }
    }

    /// Execute a law output using an existing shared trace builder.
    ///
    /// Unlike `evaluate_law_output_with_trace` which creates its own root trace node,
    /// this method appends to an existing trace tree. Used by `execute_stage_internal`
    /// when falling through to non-procedure execution.
    fn evaluate_law_output_with_shared_trace(
        &self,
        law_id: &str,
        output_name: &str,
        parameters: BTreeMap<String, Value>,
        calculation_date: &str,
        trace: Rc<RefCell<TraceBuilder>>,
    ) -> Result<ArticleResult> {
        let mut res_ctx = ResolutionContext::with_trace(calculation_date, trace);
        res_ctx.contextual_law_id = Some(law_id.to_string());
        self.evaluate_law_output_internal(law_id, output_name, parameters, &mut res_ctx)
    }

    /// Execute a single lifecycle stage of a procedure-aware law (RFC-008).
    ///
    /// For laws that produce a `legal_character` with an associated AWB procedure,
    /// this method executes one stage at a time and yields when external input is needed.
    ///
    /// # Arguments
    /// * `law_id` - The law being executed (e.g., "vreemdelingenwet_2000")
    /// * `output_name` - The output to calculate
    /// * `state` - Current decision state (None for initial execution)
    /// * `parameters` - Input parameters (merged with accumulated outputs if resuming)
    /// * `calculation_date` - Date for calculations
    ///
    /// # Returns
    /// `ExecutionOutcome::Complete` if all stages are done, or
    /// `ExecutionOutcome::Yielded` if waiting for external input.
    #[cfg_attr(feature = "otel", tracing::instrument(skip(self, state, parameters), fields(law_id = %law_id, output = %output_name)))]
    pub fn execute_stage(
        &self,
        law_id: &str,
        output_name: &str,
        state: Option<StageState>,
        parameters: BTreeMap<String, Value>,
        calculation_date: &str,
    ) -> Result<ExecutionOutcome> {
        self.execute_stage_internal(
            law_id,
            output_name,
            state,
            parameters,
            calculation_date,
            None,
        )
    }

    /// Execute a single lifecycle stage with tracing enabled.
    ///
    /// Same as `execute_stage` but accepts a shared trace builder so the
    /// staged execution is recorded in the trace tree.
    pub fn execute_stage_with_trace(
        &self,
        law_id: &str,
        output_name: &str,
        state: Option<StageState>,
        parameters: BTreeMap<String, Value>,
        calculation_date: &str,
        trace: Rc<RefCell<TraceBuilder>>,
    ) -> Result<ExecutionOutcome> {
        // Push a top-level trace node so partial traces are preserved on error,
        // mirroring evaluate_law_output_with_trace_builder.
        {
            let mut tb = trace.borrow_mut();
            tb.push(
                format!("{} ({}) [stage]", law_id, output_name),
                PathNodeType::Article,
            );
            tb.set_message(format!(
                "Stage execution: {} ({} {})",
                law_id, calculation_date, output_name,
            ));
        }

        let result = self.execute_stage_internal(
            law_id,
            output_name,
            state,
            parameters,
            calculation_date,
            Some(Rc::clone(&trace)),
        );

        match result {
            Ok(outcome) => {
                let mut tb = trace.borrow_mut();
                tb.pop();
                Ok(outcome)
            }
            Err(e) => {
                let mut tb = trace.borrow_mut();
                tb.set_message(format!("Stage execution failed: {}", e));
                let partial_trace = tb.pop();
                Err(EngineError::TracedError {
                    source: Box::new(e),
                    trace: partial_trace.map(Box::new),
                })
            }
        }
    }

    /// Internal stage execution with optional tracing.
    fn execute_stage_internal(
        &self,
        law_id: &str,
        output_name: &str,
        state: Option<StageState>,
        parameters: BTreeMap<String, Value>,
        calculation_date: &str,
        trace: Option<Rc<RefCell<TraceBuilder>>>,
    ) -> Result<ExecutionOutcome> {
        // Look up the law and article
        let ref_date = NaiveDate::parse_from_str(calculation_date, "%Y-%m-%d").ok();
        let law = self
            .resolver
            .get_law_for_date(law_id, ref_date)
            .ok_or_else(|| EngineError::LawNotFound(law_id.to_string()))?;
        let article = self
            .resolver
            .get_article_by_output(law_id, output_name, ref_date)
            .ok_or_else(|| EngineError::OutputNotFound {
                law_id: law_id.to_string(),
                output: output_name.to_string(),
            })?;

        // Check if this article produces something with a procedure
        let produces = article.get_produces();
        let legal_character = produces.and_then(|p| p.legal_character.as_deref());
        let procedure_id = produces.and_then(|p| p.procedure_id.as_deref());

        // Look up procedure definition
        let procedure =
            legal_character.and_then(|lc| self.resolver.find_procedure(lc, procedure_id));

        // If no procedure, fall through to normal single-stage execution
        let Some(procedure) = procedure else {
            let result = if let Some(tb) = trace {
                self.evaluate_law_output_with_shared_trace(
                    law_id,
                    output_name,
                    parameters,
                    calculation_date,
                    tb,
                )?
            } else {
                self.evaluate_law_output(law_id, output_name, parameters, calculation_date)?
            };
            return Ok(ExecutionOutcome::Complete(result));
        };

        // Determine current stage
        let mut stage_state = match state {
            Some(s) => s,
            None => {
                // Initial execution: start at the first stage
                let first_stage = procedure.stages.first().ok_or_else(|| {
                    EngineError::InvalidOperation("Procedure has no stages".to_string())
                })?;

                StageState {
                    procedure_id: procedure.id.clone(),
                    contextual_law: law_id.to_string(),
                    current_stage: first_stage.name.clone(),
                    accumulated_outputs: BTreeMap::new(),
                    parameters: parameters.clone(),
                }
            }
        };

        // Find current stage in procedure
        let stage_idx = procedure
            .stages
            .iter()
            .position(|s| s.name == stage_state.current_stage)
            .ok_or_else(|| {
                EngineError::InvalidOperation(format!(
                    "Stage '{}' not found in procedure '{}'",
                    stage_state.current_stage, procedure.id
                ))
            })?;

        let stage = &procedure.stages[stage_idx];

        // Check if required inputs for this stage are present
        if let Some(requires) = &stage.requires {
            let mut missing = Vec::new();
            for req in requires {
                if !parameters.contains_key(&req.name)
                    && !stage_state.accumulated_outputs.contains_key(&req.name)
                {
                    missing.push(req.name.clone());
                }
            }
            if !missing.is_empty() {
                let outputs = stage_state.accumulated_outputs.clone();
                return Ok(ExecutionOutcome::Yielded {
                    state: stage_state,
                    outputs,
                    pending_inputs: missing,
                });
            }
        }

        // Merge accumulated outputs + new parameters for this stage's execution
        let mut stage_params = stage_state.parameters.clone();
        for (k, v) in &stage_state.accumulated_outputs {
            stage_params.insert(k.clone(), v.clone());
        }
        for (k, v) in &parameters {
            stage_params.insert(k.clone(), v.clone());
        }

        // Execute the article with stage-aware hook firing
        let mut res_ctx = if let Some(ref tb) = trace {
            ResolutionContext::with_trace(calculation_date, Rc::clone(tb))
        } else {
            ResolutionContext::new(calculation_date)
        };
        res_ctx.contextual_law_id = Some(stage_state.contextual_law.clone());

        // Execute the article with stage-aware hook firing.
        let result = self.evaluate_article_with_service(
            article,
            law,
            stage_params,
            Some(output_name),
            &stage.name,
            &mut res_ctx,
        )?;

        // Merge outputs into accumulated state
        for (k, v) in &result.outputs {
            stage_state.accumulated_outputs.insert(k.clone(), v.clone());
        }

        // Advance to next stage
        if stage_idx + 1 < procedure.stages.len() {
            let next_stage = &procedure.stages[stage_idx + 1];
            stage_state.current_stage = next_stage.name.clone();

            // Check if next stage needs external inputs
            if let Some(requires) = &next_stage.requires {
                let mut missing = Vec::new();
                for req in requires {
                    if !stage_state.accumulated_outputs.contains_key(&req.name)
                        && !stage_state.parameters.contains_key(&req.name)
                        && !parameters.contains_key(&req.name)
                    {
                        missing.push(req.name.clone());
                    }
                }

                if !missing.is_empty() {
                    return Ok(ExecutionOutcome::Yielded {
                        outputs: stage_state.accumulated_outputs.clone(),
                        state: stage_state,
                        pending_inputs: missing,
                    });
                }
            }

            // Next stage has all inputs — continue executing (recursive)
            return self.execute_stage_internal(
                law_id,
                output_name,
                Some(stage_state),
                parameters,
                calculation_date,
                trace,
            );
        }

        // All stages complete
        let mut final_result = result;
        final_result.outputs = stage_state.accumulated_outputs;
        Ok(ExecutionOutcome::Complete(final_result))
    }

    /// Internal method with cycle tracking.
    #[cfg_attr(feature = "otel", tracing::instrument(skip(self, parameters, res_ctx), fields(law_id = %law_id, output = %output_name, depth = res_ctx.depth)))]
    fn evaluate_law_output_internal(
        &self,
        law_id: &str,
        output_name: &str,
        parameters: BTreeMap<String, Value>,
        res_ctx: &mut ResolutionContext<'_>,
    ) -> Result<ArticleResult> {
        // --- Cache check (before depth check: cached results don't increase depth) ---
        let key = cache_key(law_id, output_name, &parameters);
        if let Some((cached_law, cached_output, cached_outputs, cached_params)) =
            res_ctx.cache.get(&key)
        {
            // Runtime collision check: hash keys are u64 so collisions are
            // theoretically possible. For legally binding decisions, we must
            // never silently return wrong results.
            if cached_law != law_id || cached_output != output_name || cached_params != &parameters
            {
                tracing::warn!(
                    cached_law,
                    cached_output,
                    law_id,
                    output_name,
                    "Cache key hash collision detected, bypassing cache"
                );
                // Fall through to re-evaluate
            } else {
                tracing::debug!(law_id, output_name, "Cache hit");
                let _guard = res_ctx
                    .trace_guard(format!("{}#{}", law_id, output_name), PathNodeType::Cached);
                if let Some(val) = cached_outputs.get(output_name) {
                    res_ctx.trace_set_result(val.clone());
                }
                return Ok(ArticleResult {
                    outputs: cached_outputs.clone(),
                    resolved_inputs: BTreeMap::new(),
                    article_number: String::new(),
                    law_id: law_id.to_string(),
                    law_uuid: None,
                    trace: None,
                    engine_version: crate::VERSION.to_string(),
                    schema_version: None,
                    regulation_hash: None,
                    regulation_valid_from: None,
                });
            }
        }

        tracing::debug!(
            law_id = %law_id,
            output = %output_name,
            depth = res_ctx.depth,
            "Resolving cross-law reference"
        );

        // Check depth limit
        if res_ctx.depth > config::MAX_CROSS_LAW_DEPTH {
            tracing::warn!(
                law_id = %law_id,
                output = %output_name,
                depth = res_ctx.depth,
                "Cross-law resolution depth exceeded"
            );
            let _guard =
                res_ctx.trace_guard(format!("{}#{}", law_id, output_name), PathNodeType::UriCall);
            res_ctx.trace_set_message(format!(
                "Cross-law resolution depth exceeded {} levels ({}:{})",
                config::MAX_CROSS_LAW_DEPTH,
                law_id,
                output_name
            ));
            return Err(EngineError::CircularReference(format!(
                "Cross-law resolution depth exceeded {} levels. \
                 Possible circular reference involving {}:{}",
                config::MAX_CROSS_LAW_DEPTH,
                law_id,
                output_name
            )));
        }

        // Get the law (version-aware: use the same reference date as the article lookup)
        let law = self
            .resolver
            .get_law_for_date(law_id, res_ctx.reference_date())
            .ok_or_else(|| EngineError::LawNotFound(law_id.to_string()))?;

        // Find the article
        let article = self
            .resolver
            .get_article_by_output(law_id, output_name, res_ctx.reference_date())
            .ok_or_else(|| EngineError::OutputNotFound {
                law_id: law_id.to_string(),
                output: output_name.to_string(),
            })?;

        // Clone parameters for cache storage before moving into evaluation
        let params_for_cache = parameters.clone();

        // Execute with service provider (default stage BESLUIT for cross-law calls)
        let result = self.evaluate_article_with_service(
            article,
            law,
            parameters,
            Some(output_name),
            "BESLUIT",
            res_ctx,
        )?;

        // --- Cache store (only on success) ---
        // Note: on a hash collision (astronomically unlikely, ~1e-18 per pair),
        // this overwrites the collider's entry. Both keys then thrash each other,
        // degrading to re-evaluation on every access. Correctness is preserved
        // because every read validates the stored identity fields.
        res_ctx.cache.insert(
            key,
            (
                law_id.to_string(),
                output_name.to_string(),
                result.outputs.clone(),
                params_for_cache,
            ),
        );

        Ok(result)
    }

    /// Fire hooks that match the given article's `produces` annotation.
    ///
    /// For each matching hook, executes the hook article and merges its outputs.
    /// When multiple hooks produce the same output, conflicts are resolved via
    /// `compare_law_priority` (lex superior / lex posterior).
    ///
    /// # Arguments
    /// * `hook_point` - Whether to fire pre_actions or post_actions hooks
    /// * `article` - The article whose `produces` triggers hooks
    /// * `law` - The law containing the article
    /// * `stage` - The lifecycle stage (e.g., "BESLUIT", "BEKENDMAKING")
    /// * `parameters` - Parameters available to hook articles
    /// * `res_ctx` - Resolution context for cycle detection and tracing
    #[cfg_attr(feature = "otel", tracing::instrument(skip(self, article, _law, parameters, res_ctx), fields(hook_point = ?hook_point, law_id = %_law.id, article = %article.number)))]
    fn fire_hooks(
        &self,
        hook_point: HookPoint,
        article: &Article,
        _law: &ArticleBasedLaw,
        stage: &str,
        parameters: &BTreeMap<String, Value>,
        res_ctx: &mut ResolutionContext<'_>,
    ) -> Result<BTreeMap<String, Value>> {
        let mut hook_outputs: BTreeMap<String, Value> = BTreeMap::new();
        // Track which law produced each output for priority resolution
        let mut output_sources: BTreeMap<String, &ArticleBasedLaw> = BTreeMap::new();

        // Only fire hooks if the article declares what it produces
        let produces = match article.get_produces() {
            Some(p) => p,
            None => return Ok(hook_outputs),
        };

        let legal_character = match &produces.legal_character {
            Some(lc) => lc.as_str(),
            None => return Ok(hook_outputs),
        };

        let decision_type = produces.decision_type.as_deref();

        // Find matching hooks
        let matching_hooks =
            self.resolver
                .find_hooks(hook_point, legal_character, decision_type, stage);

        if matching_hooks.is_empty() {
            return Ok(hook_outputs);
        }

        tracing::debug!(
            hook_point = ?hook_point,
            legal_character = legal_character,
            stage = stage,
            matches = matching_hooks.len(),
            "Firing hooks"
        );

        for (hook_law_id, hook_article_number, _filter) in matching_hooks {
            // Cycle detection: don't re-enter a hook we're already executing
            let hook_key = format!("hook:{}\0{}", hook_law_id, hook_article_number);
            if res_ctx.is_visited(&hook_key) {
                tracing::debug!(hook_key = %hook_key, "Skipping hook: cycle detected");
                continue;
            }

            // Look up the hook article
            let ref_date = res_ctx.reference_date();
            let Some(hook_law) = self.resolver.get_law_for_date(hook_law_id, ref_date) else {
                tracing::warn!(hook_law_id = %hook_law_id, "Hook law not found");
                continue;
            };
            let Some(hook_article) = hook_law.find_article_by_number(hook_article_number) else {
                tracing::warn!(
                    hook_law_id = %hook_law_id,
                    hook_article = %hook_article_number,
                    "Hook article not found"
                );
                continue;
            };

            // Filter parameters: only pass parameters declared by the hook article (least privilege)
            let hook_params = Self::filter_parameters_for_article(hook_article, parameters);

            // Trace the hook execution (guard auto-pops on all exit paths)
            let _guard = res_ctx.trace_guard(
                format!("{}:{}", hook_law_id, hook_article_number),
                PathNodeType::HookResolution,
            );
            res_ctx.trace_set_message(format!(
                "Hook {:?} on {} stage {} → {}:{}",
                hook_point, legal_character, stage, hook_law_id, hook_article_number
            ));

            // Enter scope for cycle detection
            res_ctx.enter(hook_key.clone());

            // Execute the hook article
            let hook_result = self.evaluate_article_with_service(
                hook_article,
                hook_law,
                hook_params,
                None,      // hooks produce all their outputs
                "BESLUIT", // hook articles themselves are not procedure-aware
                res_ctx,
            );

            // Leave scope (even on error)
            res_ctx.leave(&hook_key);

            // If a hook fires (stage matches), it must succeed.
            // A missing variable means the law cannot be applied — that's an error.
            let result = hook_result?;

            for (name, value) in result.outputs {
                if let Some(existing_law) = output_sources.get(&name) {
                    // Conflict: two hooks produce same output.
                    // Resolve via lex superior / lex posterior.
                    match priority::compare_law_priority(hook_law, existing_law)? {
                        std::cmp::Ordering::Greater => {
                            hook_outputs.insert(name.clone(), value);
                            output_sources.insert(name, hook_law);
                        }
                        std::cmp::Ordering::Less | std::cmp::Ordering::Equal => {
                            // existing wins or unreachable (Equal → Err above)
                        }
                    }
                } else {
                    output_sources.insert(name.clone(), hook_law);
                    hook_outputs.insert(name, value);
                }
            }
        }

        Ok(hook_outputs)
    }

    /// Handle untranslatable constructs based on the configured mode (RFC-012).
    ///
    /// Called before article execution when the article has `untranslatables` annotations.
    /// Behavior depends on `self.untranslatable_mode`:
    /// - `Error`: hard error on unaccepted entries, accepted ones log to trace
    /// - `Propagate`: always log to trace (taint propagation happens at output level)
    /// - `Warn`: log warning to trace, continue
    /// - `Ignore`: only error on unaccepted entries, otherwise silent
    ///
    /// Returns a list of (article, construct) pairs that should taint outputs
    /// in propagate mode. Empty vec means no tainting.
    fn handle_untranslatables(
        &self,
        law_id: &str,
        article_number: &str,
        untranslatables: &[crate::article::UntranslatableEntry],
        res_ctx: &mut ResolutionContext<'_>,
    ) -> Result<Vec<(String, String)>> {
        let mut taints = Vec::new();

        for entry in untranslatables {
            // Always record in trace regardless of mode
            let msg = format!("Untranslatable: {} — {}", entry.construct, entry.reason);
            {
                let _guard = res_ctx.trace_guard(
                    format!("untranslatable:{}", entry.construct),
                    PathNodeType::Article,
                );
                res_ctx.trace_set_message(msg.clone());
            }

            match self.untranslatable_mode {
                UntranslatableMode::Error => {
                    if !entry.accepted {
                        return Err(EngineError::Untranslatable {
                            law_id: law_id.to_string(),
                            article: article_number.to_string(),
                            construct: entry.construct.clone(),
                            reason: entry.reason.clone(),
                        });
                    }
                    tracing::info!(
                        law_id,
                        article = article_number,
                        construct = %entry.construct,
                        "Accepted untranslatable, proceeding with partial logic"
                    );
                }
                UntranslatableMode::Propagate => {
                    tracing::info!(
                        law_id,
                        article = article_number,
                        construct = %entry.construct,
                        "Untranslatable construct — tainting outputs (propagate mode)"
                    );
                    taints.push((article_number.to_string(), entry.construct.clone()));
                }
                UntranslatableMode::Warn => {
                    tracing::warn!(
                        law_id,
                        article = article_number,
                        construct = %entry.construct,
                        "Untranslatable construct — executing partial logic"
                    );
                }
                UntranslatableMode::Ignore => {
                    if !entry.accepted {
                        return Err(EngineError::Untranslatable {
                            law_id: law_id.to_string(),
                            article: article_number.to_string(),
                            construct: entry.construct.clone(),
                            reason: entry.reason.clone(),
                        });
                    }
                }
            }
        }
        Ok(taints)
    }

    /// Apply lex specialis overrides to an article's outputs.
    ///
    /// For each output in the result, checks if an override exists from the contextual law.
    /// If found, executes the overriding article and replaces the output value.
    #[cfg_attr(feature = "otel", tracing::instrument(skip(self, result, article, law, parameters, res_ctx), fields(law_id = %law.id, article = %article.number)))]
    fn apply_overrides(
        &self,
        result: &mut ArticleResult,
        article: &Article,
        law: &ArticleBasedLaw,
        parameters: &BTreeMap<String, Value>,
        res_ctx: &mut ResolutionContext<'_>,
    ) -> Result<()> {
        let contextual_law_id = match &res_ctx.contextual_law_id {
            Some(id) => id.clone(),
            None => return Ok(()), // No contextual law → no overrides apply
        };

        // Check each output for overrides
        let output_names: Vec<String> = result.outputs.keys().cloned().collect();
        for output_name in output_names {
            let overrides = self
                .resolver
                .find_overrides(&law.id, &article.number, &output_name);

            if overrides.is_empty() {
                continue;
            }

            // Filter: only overrides from the contextual law apply
            let applicable: Vec<_> = overrides
                .iter()
                .filter(|(ovr_law_id, _)| *ovr_law_id == contextual_law_id)
                .collect();

            if applicable.is_empty() {
                continue;
            }

            if applicable.len() > 1 {
                return Err(EngineError::InvalidOperation(format!(
                    "Multiple overrides from '{}' for output '{}' on '{}:{}'",
                    contextual_law_id, output_name, law.id, article.number
                )));
            }

            let (ovr_law_id, ovr_article_number) = applicable[0];

            // Cycle detection
            let ovr_key = format!("override:{}\0{}", ovr_law_id, ovr_article_number);
            if res_ctx.is_visited(&ovr_key) {
                tracing::debug!(ovr_key = %ovr_key, "Skipping override: cycle detected");
                continue;
            }

            // Look up overriding article
            let ref_date = res_ctx.reference_date();
            let Some(ovr_law) = self.resolver.get_law_for_date(ovr_law_id, ref_date) else {
                continue;
            };
            let Some(ovr_article) = ovr_law.find_article_by_number(ovr_article_number) else {
                continue;
            };

            // Trace (guard auto-pops on all exit paths)
            let _guard = res_ctx.trace_guard(
                format!("{}:{}", ovr_law_id, ovr_article_number),
                PathNodeType::OverrideResolution,
            );
            res_ctx.trace_set_message(format!(
                "Lex specialis: {}:{} overrides {}:{}.{}",
                ovr_law_id, ovr_article_number, law.id, article.number, output_name
            ));

            // Execute overriding article
            let ovr_params = Self::filter_parameters_for_article(ovr_article, parameters);
            res_ctx.enter(ovr_key.clone());

            let ovr_result = self.evaluate_article_with_service(
                ovr_article,
                ovr_law,
                ovr_params,
                Some(&output_name),
                "BESLUIT", // override articles are not procedure-aware
                res_ctx,
            );

            res_ctx.leave(&ovr_key);

            let ovr_output = ovr_result?;

            if let Some(value) = ovr_output.outputs.get(&output_name) {
                tracing::debug!(
                    output = %output_name,
                    from = %law.id,
                    to = %ovr_law_id,
                    "Override applied"
                );
                result.outputs.insert(output_name.clone(), value.clone());
                res_ctx.trace_set_result(value.clone());
            }
        }

        Ok(())
    }

    /// Execute an article with ServiceProvider support.
    ///
    /// The `stage` parameter controls which lifecycle stage hooks fire at.
    /// For direct (non-procedure) execution, pass `"BESLUIT"` as default.
    fn evaluate_article_with_service(
        &self,
        article: &Article,
        law: &ArticleBasedLaw,
        parameters: BTreeMap<String, Value>,
        requested_output: Option<&str>,
        stage: &str,
        res_ctx: &mut ResolutionContext<'_>,
    ) -> Result<ArticleResult> {
        // RFC-012: Check for untranslatable constructs before execution
        let taints = if let Some(untranslatables) = article
            .machine_readable
            .as_ref()
            .and_then(|mr| mr.untranslatables.as_ref())
        {
            if !untranslatables.is_empty() {
                self.handle_untranslatables(&law.id, &article.number, untranslatables, res_ctx)?
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };

        // Create execution context
        let mut context = RuleContext::new(parameters.clone(), res_ctx.calculation_date)?;

        // Attach trace builder if available
        if let Some(ref tb) = res_ctx.trace {
            context.set_trace(Rc::clone(tb));
        }

        // Set definitions from article
        if let Some(definitions) = article.get_definitions() {
            context.set_definitions(definitions);
        }

        // Resolve inputs with sources using ServiceProvider
        self.resolve_inputs_with_service(article, law, &mut context, &parameters, res_ctx)?;

        // Resolve open terms via IoC (implements index lookup)
        let open_term_values = self.resolve_open_terms(article, law, &context, res_ctx)?;

        // Use ArticleEngine for action execution (it handles the internal logic)
        let engine = ArticleEngine::new(article, law);

        // Now execute with resolved inputs already in context
        // We need to pass resolved inputs as parameters to avoid re-resolution
        let mut combined_params = parameters;
        for (name, value) in context.resolved_inputs() {
            combined_params.insert(name.clone(), value.clone());
        }
        // Merge open term values (IoC resolved)
        for (name, value) in open_term_values {
            combined_params.insert(name, value);
        }

        // Fire pre_actions hooks (between open term resolution and action execution).
        let pre_hook_outputs = self.fire_hooks(
            HookPoint::PreActions,
            article,
            law,
            stage,
            &combined_params,
            res_ctx,
        )?;
        for (name, value) in &pre_hook_outputs {
            combined_params.insert(name.clone(), value.clone());
        }

        // Use traced evaluation if trace is available
        let mut result = if let Some(ref tb) = res_ctx.trace {
            engine.evaluate_with_trace(
                combined_params.clone(),
                res_ctx.calculation_date,
                requested_output,
                Rc::clone(tb),
            )?
        } else {
            engine.evaluate_with_output(
                combined_params.clone(),
                res_ctx.calculation_date,
                requested_output,
            )?
        };

        // Fire post_actions hooks (between action execution and result return).
        // Post-hooks receive both parameters and article outputs.
        let mut post_params = combined_params;
        for (name, value) in &result.outputs {
            post_params.insert(name.clone(), value.clone());
        }
        let post_hook_outputs = self.fire_hooks(
            HookPoint::PostActions,
            article,
            law,
            stage,
            &post_params,
            res_ctx,
        )?;
        for (name, value) in post_hook_outputs {
            result.outputs.entry(name).or_insert(value);
        }
        // Merge pre-hook outputs into result too (they are part of the execution)
        for (name, value) in pre_hook_outputs {
            result.outputs.entry(name).or_insert(value);
        }

        // Apply lex specialis overrides
        self.apply_overrides(&mut result, article, law, &post_params, res_ctx)?;

        // Enforce TypeSpec: round eurocent outputs to integer.
        // This applies only to top-level article outputs (the API boundary).
        // Intermediate values within article logic remain as Float to preserve
        // precision during calculation; rounding happens here at the output edge.
        if let Some(exec) = article.get_execution_spec() {
            if let Some(outputs) = &exec.output {
                for output_spec in outputs {
                    let is_eurocent = output_spec
                        .type_spec
                        .as_ref()
                        .and_then(|ts| ts.unit.as_deref())
                        == Some("eurocent");
                    if is_eurocent {
                        if let Some(Value::Float(f)) = result.outputs.get(&output_spec.name) {
                            let rounded = crate::operations::f64_to_i64_safe(f.round())?;
                            result
                                .outputs
                                .insert(output_spec.name.clone(), Value::Int(rounded));
                        }
                    }
                }
            }
        }

        // RFC-012 Propagate mode: taint all outputs from articles with untranslatables
        if !taints.is_empty() {
            if taints.len() > 1 {
                tracing::warn!(
                    article = %taints[0].0,
                    count = taints.len(),
                    "Article has multiple untranslatable constructs; combining into single taint"
                );
            }
            let taint_article = taints[0].0.clone();
            let taint_construct = taints
                .iter()
                .map(|(_, c)| c.as_str())
                .collect::<Vec<_>>()
                .join("; ");
            for value in result.outputs.values_mut() {
                *value = Value::Untranslatable {
                    article: taint_article.clone(),
                    construct: taint_construct.clone(),
                };
            }
        }

        Ok(result)
    }

    /// Resolve open terms declared on an article via IoC (implements index).
    ///
    /// For each open term:
    /// 1. Look up implementations in the resolver's implements_index
    /// 2. If found: execute the implementing article to get the value
    /// 3. If not found + has default: execute the default actions
    /// 4. If not found + required + no default: error
    /// 5. If not found + not required + no default: skip
    #[cfg_attr(feature = "otel", tracing::instrument(skip(self, article, law, context, res_ctx), fields(law_id = %law.id, article = %article.number)))]
    fn resolve_open_terms(
        &self,
        article: &Article,
        law: &ArticleBasedLaw,
        context: &RuleContext,
        res_ctx: &mut ResolutionContext<'_>,
    ) -> Result<BTreeMap<String, Value>> {
        let mut resolved = BTreeMap::new();

        let open_terms = match article.get_open_terms() {
            Some(terms) => terms,
            None => return Ok(resolved),
        };

        for term in open_terms {
            // Cycle detection: check if we're already resolving this open term
            // Use \0 as separator to prevent key collisions when IDs contain #
            let ot_key = format!("open_term:{}\0{}\0{}", law.id, article.number, term.id);
            if res_ctx.is_visited(&ot_key) {
                tracing::warn!(
                    law_id = %law.id,
                    article = %article.number,
                    open_term = %term.id,
                    "Circular open term dependency detected"
                );
                let _guard = res_ctx.trace_guard(&term.id, PathNodeType::OpenTermResolution);
                res_ctx.trace_set_message(format!(
                    "Circular dependency: open term '{}' on {}#{} is already being resolved",
                    term.id, law.id, article.number
                ));
                return Err(EngineError::CircularReference(format!(
                    "Circular open term dependency: '{}' on {} article {} is already being resolved",
                    term.id, law.id, article.number
                )));
            }
            res_ctx.enter(ot_key.clone());

            tracing::debug!(
                law_id = %law.id,
                article = %article.number,
                open_term = %term.id,
                "Resolving open term"
            );

            // Trace the open term resolution (guard auto-pops on all exit paths)
            let _guard = res_ctx.trace_guard(&term.id, PathNodeType::OpenTermResolution);
            res_ctx.trace_set_resolve_type(ResolveType::OpenTerm);

            // Look up implementations (filtered by execution scope)
            // Convert BTreeMap to HashMap at the resolver boundary
            let scope: HashMap<String, Value> = context
                .parameters()
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            let implementations = match self.resolver.find_implementations(
                &law.id,
                &article.number,
                &term.id,
                res_ctx.reference_date(),
                &scope,
            ) {
                Ok(impls) => impls,
                Err(e) => {
                    res_ctx.trace_set_message(format!(
                        "Open term '{}': implementation lookup failed: {}",
                        term.id, e
                    ));
                    res_ctx.leave(&ot_key);
                    return Err(e);
                }
            };

            if let Some((impl_law, impl_article)) = implementations.first() {
                // Validate that the implementing regulation's layer matches the
                // delegation_type declared on the open term (if specified).
                if let Some(ref expected_type) = term.delegation_type {
                    let actual_layer = impl_law.regulatory_layer.as_str();
                    if actual_layer != expected_type {
                        res_ctx.trace_set_message(format!(
                            "Open term '{}': implementation {} has regulatory_layer {} but delegation_type requires {}",
                            term.id, impl_law.id, actual_layer, expected_type
                        ));
                        res_ctx.leave(&ot_key);
                        return Err(EngineError::ResolutionError(format!(
                            "Implementation {} for open term '{}' has regulatory_layer {} but delegation_type requires {}",
                            impl_law.id, term.id, actual_layer, expected_type
                        )));
                    }
                }

                tracing::debug!(
                    open_term = %term.id,
                    implementing_law = %impl_law.id,
                    implementing_article = %impl_article.number,
                    "Found implementation for open term"
                );

                // Execute the implementing article to get the value.
                // Only forward parameters that the implementing article declares
                // in its execution.parameters — principle of least privilege.
                let impl_params =
                    Self::filter_parameters_for_article(impl_article, context.parameters());
                let result = match self.evaluate_article_with_service(
                    impl_article,
                    impl_law,
                    impl_params,
                    Some(&term.id),
                    "BESLUIT",
                    res_ctx,
                ) {
                    Ok(r) => r,
                    Err(e) => {
                        res_ctx.trace_set_message(format!(
                            "Open term '{}': implementation execution failed: {}",
                            term.id, e
                        ));
                        res_ctx.leave(&ot_key);
                        return Err(e);
                    }
                };

                if let Some(value) = result.outputs.get(&term.id) {
                    res_ctx.trace_set_result(value.clone());
                    res_ctx.trace_set_message(format!(
                        "Open term '{}' implemented by {} article {}",
                        term.id, impl_law.id, impl_article.number
                    ));
                    resolved.insert(term.id.clone(), value.clone());
                } else {
                    // Implementation executed but didn't produce the expected output
                    res_ctx.trace_set_message(format!(
                        "Open term '{}': implementation {} article {} produced no matching output",
                        term.id, impl_law.id, impl_article.number
                    ));
                    res_ctx.leave(&ot_key);
                    return Err(EngineError::InvalidOperation(format!(
                        "Implementation {} article {} for open term '{}' did not produce output named '{}'",
                        impl_law.id, impl_article.number, term.id, term.id
                    )));
                }
            } else if let Some(ref default) = term.default {
                // No implementation found — execute default actions
                tracing::debug!(
                    open_term = %term.id,
                    "No implementation found, using default"
                );

                if let Some(ref actions) = default.actions {
                    // Build a synthetic article from the default actions and evaluate
                    // it through ArticleEngine — this correctly handles action.output,
                    // intermediate variables, and all operation patterns.
                    let synthetic_article = Article {
                        number: format!("default:{}", term.id),
                        text: String::new(),
                        url: None,
                        machine_readable: Some(MachineReadable {
                            definitions: None,
                            execution: Some(Execution {
                                produces: None,
                                parameters: None,
                                input: None,
                                output: None,
                                actions: Some(actions.clone()),
                            }),
                            requires: None,
                            competent_authority: None,
                            open_terms: None,
                            implements: None,
                            hooks: None,
                            overrides: None,
                            untranslatables: None,
                        }),
                    };

                    let engine = ArticleEngine::new(&synthetic_article, law);

                    // Pass current context parameters so default actions can
                    // reference variables like $type_beplanting
                    let mut default_params = context.parameters().clone();
                    // Include already-resolved open terms from this evaluation
                    for (k, v) in &resolved {
                        default_params.insert(k.clone(), v.clone());
                    }

                    let default_result = match engine.evaluate_with_output(
                        default_params,
                        res_ctx.calculation_date,
                        Some(&term.id),
                    ) {
                        Ok(r) => r,
                        Err(e) => {
                            res_ctx.trace_set_message(format!(
                                "Open term '{}': default evaluation failed: {}",
                                term.id, e
                            ));
                            res_ctx.leave(&ot_key);
                            return Err(e);
                        }
                    };

                    let default_value = default_result
                        .outputs
                        .get(&term.id)
                        .cloned()
                        .unwrap_or(Value::Null);

                    res_ctx.trace_set_result(default_value.clone());
                    res_ctx
                        .trace_set_message(format!("Open term '{}' using default value", term.id));
                    resolved.insert(term.id.clone(), default_value);
                } else {
                    // Default exists but has no actions — treat as null
                    res_ctx.trace_set_result(Value::Null);
                    res_ctx
                        .trace_set_message(format!("Open term '{}' using empty default", term.id));
                    resolved.insert(term.id.clone(), Value::Null);
                }
            } else if term.required {
                // Required but no implementation and no default
                res_ctx.trace_set_message(format!(
                    "Open term '{}' is required but no implementation found",
                    term.id
                ));
                res_ctx.leave(&ot_key);
                return Err(EngineError::ResolutionError(format!(
                    "Required open term '{}' on {}#{} has no implementation and no default",
                    term.id, law.id, article.number
                )));
            } else {
                // Not required, no implementation, no default — resolve as null
                // so downstream actions can check for null and fall back to their
                // own defaults (e.g., BW 5:42 falls back to the statutory distance
                // when no municipal verordening overrides it).
                tracing::debug!(
                    open_term = %term.id,
                    "Optional open term not implemented, resolving as null"
                );

                res_ctx.trace_set_message(format!(
                    "Open term '{}' not required, no implementation, resolved as null",
                    term.id
                ));
                resolved.insert(term.id.clone(), Value::Null);
            }

            res_ctx.leave(&ot_key);
        }

        Ok(resolved)
    }

    /// Resolve input sources using ServiceProvider.
    fn resolve_inputs_with_service(
        &self,
        article: &Article,
        law: &ArticleBasedLaw,
        context: &mut RuleContext,
        parameters: &BTreeMap<String, Value>,
        res_ctx: &mut ResolutionContext<'_>,
    ) -> Result<()> {
        let inputs = article.get_inputs();

        for input in inputs {
            let source = match &input.source {
                Some(s) => s,
                None => continue,
            };

            // Check if already provided as parameter
            if parameters.contains_key(&input.name) {
                continue;
            }

            // Check DataSourceRegistry before cross-law resolution
            if self.data_registry.source_count() > 0 {
                if let Some(data_match) = self.data_registry.resolve(&input.name, parameters) {
                    tracing::debug!(
                        input = %input.name,
                        source = %data_match.source_name,
                        "Resolved input from data registry"
                    );

                    // Trace the data source resolution
                    {
                        let _guard = res_ctx.trace_guard(&input.name, PathNodeType::Resolve);
                        res_ctx.trace_set_resolve_type(ResolveType::DataSource);
                        res_ctx.trace_set_result(data_match.value.clone());
                        res_ctx.trace_set_message(format!(
                            "Resolving from SOURCE {}: {}",
                            data_match.source_name, data_match.value
                        ));
                    }

                    context.set_resolved_input(&input.name, data_match.value);
                    continue;
                }
            }

            // For cross-law resolution, output defaults to input name
            let output_name = source.output.as_deref().unwrap_or(&input.name);

            if let Some(regulation) = &source.regulation {
                // External reference
                let value = self.resolve_external_input_internal(
                    regulation,
                    output_name,
                    source.parameters.as_ref(),
                    context,
                    res_ctx,
                )?;

                context.set_resolved_input(&input.name, value);
            } else if source.output.is_some() {
                // Internal reference (same-law) with output specified.
                // Resolve through the service layer so cross-law inputs of the
                // referenced article are properly handled.
                let _guard = res_ctx
                    .trace_guard(format!("{}#{}", law.id, output_name), PathNodeType::Resolve);
                res_ctx.trace_set_resolve_type(ResolveType::ResolvedInput);
                res_ctx
                    .trace_set_message(format!("Internal reference: {}#{}", law.id, output_name));

                let ref_article = match law.find_article_by_output(output_name) {
                    Some(a) => a,
                    None => {
                        res_ctx.trace_set_message(format!(
                            "Internal reference failed: output '{}' not found in {}",
                            output_name, law.id
                        ));
                        return Err(EngineError::OutputNotFound {
                            law_id: law.id.clone(),
                            output: output_name.to_string(),
                        });
                    }
                };

                let ref_params = parameters.clone();
                let result = match self.evaluate_article_with_service(
                    ref_article,
                    law,
                    ref_params,
                    Some(output_name),
                    "BESLUIT",
                    res_ctx,
                ) {
                    Ok(r) => r,
                    Err(e) => {
                        res_ctx.trace_set_message(format!("Internal reference failed: {}", e));
                        return Err(e);
                    }
                };

                if let Some(value) = result.outputs.get(output_name) {
                    res_ctx.trace_set_result(value.clone());
                    context.set_resolved_input(&input.name, value.clone());
                } else {
                    res_ctx.trace_set_message(format!(
                        "Internal reference: output '{}' not in result from article {}",
                        output_name, ref_article.number
                    ));
                }
            } else {
                // Empty source (source: {}) — resolved from DataSourceRegistry only.
                // If DataSourceRegistry didn't match above, leave unresolved.
                let _guard = res_ctx.trace_guard(&input.name, PathNodeType::Resolve);
                res_ctx.trace_set_message(format!(
                    "Input '{}' has empty source and no data source match, left unresolved",
                    input.name
                ));
            }
        }

        Ok(())
    }

    /// Internal method for external input resolution with depth tracking.
    fn resolve_external_input_internal(
        &self,
        regulation: &str,
        output: &str,
        source_parameters: Option<&BTreeMap<String, String>>,
        context: &RuleContext,
        res_ctx: &mut ResolutionContext<'_>,
    ) -> Result<Value> {
        // Check for circular reference before proceeding
        let key = format!("{}#{}", regulation, output);
        if res_ctx.is_visited(&key) {
            return Err(EngineError::CircularReference(format!(
                "Circular cross-law reference detected: {} is already being resolved",
                key
            )));
        }

        // Trace cross-law call (guard auto-pops on all exit paths)
        let _guard =
            res_ctx.trace_guard(format!("{}#{}", regulation, output), PathNodeType::UriCall);

        // Build parameters for the target article
        let target_params = match self.build_target_parameters(source_parameters, context) {
            Ok(p) => p,
            Err(e) => {
                res_ctx.trace_set_message(format!("Failed to build parameters: {}", e));
                return Err(e);
            }
        };

        // Enter cross-law resolution scope
        res_ctx.enter(key.clone());

        // Execute the target article
        let result = self.evaluate_law_output_internal(regulation, output, target_params, res_ctx);

        // Leave scope (even on error, for correct cycle tracking)
        res_ctx.leave(&key);

        let value = match result {
            Ok(r) => match r.outputs.get(output).cloned() {
                Some(v) => v,
                None => {
                    res_ctx.trace_set_message(format!(
                        "Output '{}' not found in result from {}",
                        output, regulation
                    ));
                    return Err(EngineError::OutputNotFound {
                        law_id: regulation.to_string(),
                        output: output.to_string(),
                    });
                }
            },
            Err(e) => {
                res_ctx.trace_set_message(format!("Execution failed: {}", e));
                return Err(e);
            }
        };

        // Complete trace node
        res_ctx.trace_set_result(value.clone());

        Ok(value)
    }

    /// Filter execution parameters to only those declared by the target article.
    ///
    /// When resolving open terms, we don't want to forward all parameters from
    /// the calling context (which may include sensitive data like BSN). Instead,
    /// we only pass parameters that the implementing article declares in its
    /// execution.parameters section.
    fn filter_parameters_for_article(
        article: &Article,
        all_params: &BTreeMap<String, Value>,
    ) -> BTreeMap<String, Value> {
        let Some(exec) = article.get_execution_spec() else {
            return BTreeMap::new();
        };
        let Some(declared_params) = &exec.parameters else {
            return BTreeMap::new();
        };

        let mut filtered = BTreeMap::new();
        for param in declared_params {
            if let Some(value) = all_params.get(&param.name) {
                filtered.insert(param.name.clone(), value.clone());
            }
        }
        filtered
    }

    /// Build parameters for a target article from source parameter mapping.
    fn build_target_parameters(
        &self,
        source_parameters: Option<&BTreeMap<String, String>>,
        context: &RuleContext,
    ) -> Result<BTreeMap<String, Value>> {
        let mut params = BTreeMap::new();

        if let Some(param_map) = source_parameters {
            for (target_name, source_ref) in param_map {
                // Source ref can be "$variable" or a literal
                let value = if let Some(var_name) = source_ref.strip_prefix('$') {
                    context.resolve(var_name)?
                } else {
                    // Literal value (as string)
                    Value::String(source_ref.clone())
                };
                params.insert(target_name.clone(), value);
            }
        }

        Ok(params)
    }

    /// List all loaded law IDs.
    pub fn list_laws(&self) -> Vec<&str> {
        self.resolver.list_laws()
    }

    /// Get the number of loaded laws.
    pub fn law_count(&self) -> usize {
        self.resolver.law_count()
    }

    /// Check if a law is loaded.
    pub fn has_law(&self, law_id: &str) -> bool {
        self.resolver.has_law(law_id)
    }

    /// Unload a law.
    pub fn unload_law(&mut self, law_id: &str) -> bool {
        self.resolver.unload_law(law_id)
    }

    /// Get direct access to the resolver.
    pub fn resolver(&self) -> &RuleResolver {
        &self.resolver
    }

    /// Get metadata about a loaded law.
    ///
    /// # Arguments
    /// * `law_id` - The law identifier
    ///
    /// # Returns
    /// `LawInfo` with metadata, or `None` if the law is not loaded.
    pub fn get_law_info(&self, law_id: &str) -> Option<LawInfo> {
        let law = self.resolver.get_law(law_id)?;

        // Collect output names from all articles
        let mut outputs = Vec::new();
        for article in &law.articles {
            if let Some(exec) = article.get_execution_spec() {
                if let Some(output_specs) = &exec.output {
                    for output in output_specs {
                        outputs.push(output.name.clone());
                    }
                }
            }
        }

        Some(LawInfo {
            id: law.id.clone(),
            regulatory_layer: law.regulatory_layer,
            publication_date: law.publication_date.clone(),
            bwb_id: law.bwb_id.clone(),
            url: law.url.clone(),
            outputs,
            article_count: law.articles.len(),
        })
    }

    /// List all (law_id, output_name) pairs across all loaded laws.
    pub fn list_all_outputs(&self) -> Vec<(&str, &str)> {
        self.resolver.list_all_outputs()
    }

    /// Get the total number of outputs across all loaded laws.
    pub fn get_output_count(&self) -> usize {
        self.resolver.output_count()
    }

    // -------------------------------------------------------------------------
    // Data Source Management
    // -------------------------------------------------------------------------

    /// Add a data source to the registry.
    ///
    /// Data sources are queried in priority order (highest first) during
    /// input resolution, before falling back to cross-law resolution.
    pub fn add_data_source(&mut self, source: Box<dyn DataSource>) {
        self.data_registry.add_source(source);
    }

    /// Add a dictionary-based data source.
    ///
    /// Convenience method for adding a `DictDataSource` with the given data.
    ///
    /// # Arguments
    /// * `name` - Name identifier for this data source
    /// * `priority` - Priority for resolution order (higher = checked first)
    /// * `data` - Data as record_key -> field_name -> value
    pub fn add_dict_source(
        &mut self,
        name: impl Into<String>,
        priority: i32,
        data: BTreeMap<String, BTreeMap<String, Value>>,
    ) {
        self.data_registry
            .add_source(Box::new(DictDataSource::new(name, priority, data)));
    }

    /// Remove a data source by name.
    pub fn remove_data_source(&mut self, name: &str) -> bool {
        self.data_registry.remove_source(name)
    }

    /// Clear all data sources from the registry.
    pub fn clear_data_sources(&mut self) {
        self.data_registry.clear();
    }

    /// Register a dictionary data source from flat records.
    ///
    /// # Arguments
    /// * `name` - Name identifier for this data source
    /// * `key_field` - Field name to use as the record key (case-insensitive)
    /// * `records` - List of records as field -> value maps
    pub fn register_dict_source(
        &mut self,
        name: &str,
        key_field: &str,
        records: Vec<BTreeMap<String, Value>>,
    ) -> Result<()> {
        match DictDataSource::from_records(name, 10, key_field, records) {
            Some(source) => {
                self.data_registry.add_source(Box::new(source));
                Ok(())
            }
            None => Err(EngineError::DataSourceError(format!(
                "Key field '{}' not found in records for source '{}'",
                key_field, name
            ))),
        }
    }

    /// Get the number of registered data sources.
    pub fn data_source_count(&self) -> usize {
        self.data_registry.source_count()
    }

    /// List all registered data source names.
    pub fn list_data_sources(&self) -> Vec<&str> {
        self.data_registry.list_sources()
    }

    /// Get direct access to the data registry.
    pub fn data_registry(&self) -> &DataSourceRegistry {
        &self.data_registry
    }
}

impl ServiceProvider for LawExecutionService {
    fn evaluate_uri(
        &self,
        uri: &str,
        parameters: &BTreeMap<String, Value>,
        calculation_date: &str,
    ) -> Result<ArticleResult> {
        let parsed = RegelrechtUri::parse(uri)?;
        self.evaluate_law_output(
            parsed.law_id(),
            parsed.output(),
            parameters.clone(),
            calculation_date,
        )
    }

    fn get_law(&self, law_id: &str) -> Option<&ArticleBasedLaw> {
        self.resolver.get_law(law_id)
    }

    #[cfg_attr(feature = "otel", tracing::instrument(skip(self, source_parameters, context), fields(regulation = %regulation, output = %output)))]
    fn resolve_external_input(
        &self,
        regulation: &str,
        output: &str,
        source_parameters: Option<&BTreeMap<String, String>>,
        context: &RuleContext,
        calculation_date: &str,
    ) -> Result<Value> {
        let mut res_ctx = ResolutionContext::new(calculation_date);
        self.resolve_external_input_internal(
            regulation,
            output,
            source_parameters,
            context,
            &mut res_ctx,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_base_law() -> &'static str {
        r#"
$id: base_law
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Provides a base value
    machine_readable:
      definitions:
        BASE_VALUE:
          value: 100
      execution:
        output:
          - name: base_value
            type: number
        actions:
          - output: base_value
            value: $BASE_VALUE
"#
    }

    fn make_dependent_law() -> &'static str {
        r#"
$id: dependent_law
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Uses value from another law
    machine_readable:
      execution:
        input:
          - name: external_base
            type: number
            source:
              regulation: base_law
              output: base_value
        output:
          - name: doubled_value
            type: number
        actions:
          - output: doubled_value
            operation: MULTIPLY
            values:
              - $external_base
              - 2
"#
    }

    // -------------------------------------------------------------------------
    // Basic Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_service_basic() {
        let mut service = LawExecutionService::new();
        service.load_law(make_base_law()).unwrap();

        assert!(service.has_law("base_law"));
        assert_eq!(service.law_count(), 1);

        let laws = service.list_laws();
        assert_eq!(laws, vec!["base_law"]);
    }

    #[test]
    fn test_service_execute_simple() {
        let mut service = LawExecutionService::new();
        service.load_law(make_base_law()).unwrap();

        let result = service
            .evaluate_law_output("base_law", "base_value", BTreeMap::new(), "2025-01-01")
            .unwrap();

        assert_eq!(result.outputs.get("base_value"), Some(&Value::Int(100)));
    }

    // -------------------------------------------------------------------------
    // Cross-Law Resolution Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_service_cross_law_resolution() {
        let mut service = LawExecutionService::new();
        service.load_law(make_base_law()).unwrap();
        service.load_law(make_dependent_law()).unwrap();

        // Execute dependent law - should automatically resolve base_law reference
        let result = service
            .evaluate_law_output(
                "dependent_law",
                "doubled_value",
                BTreeMap::new(),
                "2025-01-01",
            )
            .unwrap();

        // doubled_value = base_value (100) * 2 = 200
        assert_eq!(result.outputs.get("doubled_value"), Some(&Value::Int(200)));
    }

    #[test]
    fn test_service_missing_dependency() {
        let mut service = LawExecutionService::new();
        // Only load dependent law, not base law
        service.load_law(make_dependent_law()).unwrap();

        let result = service.evaluate_law_output(
            "dependent_law",
            "doubled_value",
            BTreeMap::new(),
            "2025-01-01",
        );

        assert!(
            matches!(result, Err(EngineError::LawNotFound(_))),
            "Expected LawNotFound error, got: {:?}",
            result
        );
    }

    // -------------------------------------------------------------------------
    // URI Resolution Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_service_uri_resolution() {
        let mut service = LawExecutionService::new();
        service.load_law(make_base_law()).unwrap();

        let result = service
            .evaluate_uri(
                "regelrecht://base_law/base_value",
                &BTreeMap::new(),
                "2025-01-01",
            )
            .unwrap();

        assert_eq!(result.outputs.get("base_value"), Some(&Value::Int(100)));
    }

    // -------------------------------------------------------------------------
    // Circular Reference Detection Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_service_cross_law_circular_reference() {
        // Law A references Law B, which references Law A
        let law_a = r#"
$id: law_a
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: References law_b
    machine_readable:
      execution:
        input:
          - name: from_b
            type: number
            source:
              regulation: law_b
              output: output_b
        output:
          - name: output_a
            type: number
        actions:
          - output: output_a
            value: $from_b
"#;

        let law_b = r#"
$id: law_b
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: References law_a
    machine_readable:
      execution:
        input:
          - name: from_a
            type: number
            source:
              regulation: law_a
              output: output_a
        output:
          - name: output_b
            type: number
        actions:
          - output: output_b
            value: $from_a
"#;

        let mut service = LawExecutionService::new();
        service.load_law(law_a).unwrap();
        service.load_law(law_b).unwrap();

        let result =
            service.evaluate_law_output("law_a", "output_a", BTreeMap::new(), "2025-01-01");

        assert!(
            matches!(result, Err(EngineError::CircularReference(_))),
            "Expected CircularReference error, got: {:?}",
            result
        );
    }

    // -------------------------------------------------------------------------
    // Parameter Override Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_service_parameter_override() {
        // When a parameter is provided that matches an input name,
        // the parameter value should be used instead of resolving the source
        let law = r#"
$id: override_test_law
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Has external reference but can be overridden
    machine_readable:
      execution:
        parameters:
          - name: external_value
            type: number
        input:
          - name: external_value
            type: number
            source:
              regulation: nonexistent_law
              output: some_output
        output:
          - name: result
            type: number
        actions:
          - output: result
            operation: MULTIPLY
            values:
              - $external_value
              - 2
"#;

        let mut service = LawExecutionService::new();
        service.load_law(law).unwrap();

        // Provide the value directly - should skip external resolution
        let mut params = BTreeMap::new();
        params.insert("external_value".to_string(), Value::Int(50));

        let result = service
            .evaluate_law_output("override_test_law", "result", params, "2025-01-01")
            .unwrap();

        // result = 50 * 2 = 100
        assert_eq!(result.outputs.get("result"), Some(&Value::Int(100)));
    }

    // -------------------------------------------------------------------------
    // API Method Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_get_law_info() {
        let mut service = LawExecutionService::new();
        service.load_law(make_base_law()).unwrap();

        let info = service.get_law_info("base_law").unwrap();
        assert_eq!(info.id, "base_law");
        assert_eq!(info.regulatory_layer, RegulatoryLayer::Wet);
        assert_eq!(info.publication_date, "2025-01-01");
        assert!(info.bwb_id.is_none());
        assert!(info.url.is_none());
        assert_eq!(info.outputs, vec!["base_value"]);
        assert_eq!(info.article_count, 1);

        // Non-existent law
        assert!(service.get_law_info("nonexistent").is_none());
    }

    #[test]
    fn test_list_all_outputs() {
        let mut service = LawExecutionService::new();
        service.load_law(make_base_law()).unwrap();
        service.load_law(make_dependent_law()).unwrap();

        let outputs = service.list_all_outputs();
        assert!(outputs.contains(&("base_law", "base_value")));
        assert!(outputs.contains(&("dependent_law", "doubled_value")));
        assert_eq!(outputs.len(), 2);
    }

    #[test]
    fn test_output_count() {
        let mut service = LawExecutionService::new();
        assert_eq!(service.get_output_count(), 0);

        service.load_law(make_base_law()).unwrap();
        assert_eq!(service.get_output_count(), 1);

        service.load_law(make_dependent_law()).unwrap();
        assert_eq!(service.get_output_count(), 2);
    }

    // -------------------------------------------------------------------------
    // Integration Tests with Real Regulation Files
    // -------------------------------------------------------------------------

    mod integration {
        use super::*;
        use std::path::PathBuf;

        fn get_regulation_path() -> PathBuf {
            std::env::var("REGULATION_PATH")
                .map(PathBuf::from)
                .unwrap_or_else(|_| {
                    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                        .join("..")
                        .join("..")
                        .join("corpus")
                        .join("regulation")
                })
        }

        #[test]
        fn test_service_resolve_with_real_files() {
            // Integration test: load real zorgtoeslagwet + regeling_standaardpremie
            // and verify resolve action works end-to-end
            let regulation_path = get_regulation_path();

            let zorgtoeslagwet_path =
                regulation_path.join("nl/wet/wet_op_de_zorgtoeslag/2025-01-01.yaml");
            let regeling_path = regulation_path
                .join("nl/ministeriele_regeling/regeling_standaardpremie/2025-01-01.yaml");

            let zt_law = ArticleBasedLaw::from_yaml_file(&zorgtoeslagwet_path).unwrap();
            let rsp_law = ArticleBasedLaw::from_yaml_file(&regeling_path).unwrap();

            let mut service = LawExecutionService::new();
            service.load_law_struct(zt_law).unwrap();
            service.load_law_struct(rsp_law).unwrap();

            // Execute zorgtoeslagwet standaardpremie output
            // This should resolve the regeling_standaardpremie via legal_basis
            let result = service
                .evaluate_law_output(
                    "zorgtoeslagwet",
                    "standaardpremie",
                    BTreeMap::new(),
                    "2025-01-01",
                )
                .unwrap();

            // standaardpremie for 2025 = 211200 eurocent (€2112)
            assert_eq!(
                result.outputs.get("standaardpremie"),
                Some(&Value::Int(211200)),
                "Expected standaardpremie=211200 for 2025"
            );
        }

        #[test]
        fn test_load_from_directory() {
            let regulation_path = get_regulation_path().join("nl");

            let mut resolver = crate::resolver::RuleResolver::new();
            let count = resolver.load_from_directory(&regulation_path).unwrap();

            // Should load all YAML files from the regulation directory
            assert!(
                count >= 10,
                "Expected at least 10 laws loaded from corpus/regulation/nl, got {}",
                count
            );

            // Verify known laws are loaded
            assert!(resolver.has_law("zorgtoeslagwet"));
            assert!(resolver.has_law("regeling_standaardpremie"));
            assert!(resolver.has_law("participatiewet"));
        }

        #[test]
        fn test_get_law_info_real() {
            let regulation_path = get_regulation_path();
            let path = regulation_path.join("nl/wet/wet_op_de_zorgtoeslag/2025-01-01.yaml");
            let law = ArticleBasedLaw::from_yaml_file(&path).unwrap();

            let mut service = LawExecutionService::new();
            service.load_law_struct(law).unwrap();

            let info = service.get_law_info("zorgtoeslagwet").unwrap();
            assert_eq!(info.id, "zorgtoeslagwet");
            assert_eq!(info.regulatory_layer, RegulatoryLayer::Wet);
            assert!(info.article_count > 0);
            assert!(!info.outputs.is_empty());
            // Should include standaardpremie output
            assert!(
                info.outputs.contains(&"standaardpremie".to_string()),
                "Expected standaardpremie in outputs: {:?}",
                info.outputs
            );
        }
    }

    #[test]
    fn test_cross_law_uses_version_aware_lookup() {
        // Two versions of a referenced law with different definitions.
        // Cross-law resolution should pick the correct version based on
        // the reference date, not just the latest version.
        // Version selection uses `valid_from` to determine applicability.
        let base_v1 = r#"
$id: versioned_base
regulatory_layer: WET
publication_date: '2024-01-01'
valid_from: '2024-01-01'
articles:
  - number: '1'
    text: Base value v1
    machine_readable:
      definitions:
        BASE_VALUE:
          value: 100
      execution:
        output:
          - name: base_value
            type: number
        actions:
          - output: base_value
            value: $BASE_VALUE
"#;
        let base_v2 = r#"
$id: versioned_base
regulatory_layer: WET
publication_date: '2025-01-01'
valid_from: '2025-01-01'
articles:
  - number: '1'
    text: Base value v2
    machine_readable:
      definitions:
        BASE_VALUE:
          value: 200
      execution:
        output:
          - name: base_value
            type: number
        actions:
          - output: base_value
            value: $BASE_VALUE
"#;
        let dependent = r#"
$id: cross_law_consumer
regulatory_layer: WET
publication_date: '2024-01-01'
articles:
  - number: '1'
    text: Uses versioned base
    machine_readable:
      execution:
        input:
          - name: external_base
            type: number
            source:
              regulation: versioned_base
              output: base_value
        output:
          - name: result
            type: number
        actions:
          - output: result
            value: $external_base
"#;
        let mut service = LawExecutionService::new();
        service.load_law(base_v1).unwrap();
        service.load_law(base_v2).unwrap();
        service.load_law(dependent).unwrap();

        // Reference date 2024-06-15 should use v1 (BASE_VALUE=100)
        let result = service
            .evaluate_law_output(
                "cross_law_consumer",
                "result",
                BTreeMap::new(),
                "2024-06-15",
            )
            .unwrap();
        assert_eq!(
            result.outputs.get("result"),
            Some(&Value::Int(100)),
            "2024 reference date should resolve to v1 (100)"
        );

        // Reference date 2025-06-15 should use v2 (BASE_VALUE=200)
        let result = service
            .evaluate_law_output(
                "cross_law_consumer",
                "result",
                BTreeMap::new(),
                "2025-06-15",
            )
            .unwrap();
        assert_eq!(
            result.outputs.get("result"),
            Some(&Value::Int(200)),
            "2025 reference date should resolve to v2 (200)"
        );
    }

    // -------------------------------------------------------------------------
    // DataSourceRegistry Integration Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_data_registry_provides_input() {
        // A law references a regulation for an input, but the data registry
        // provides the value directly. The referenced regulation is NOT loaded,
        // proving the registry short-circuits cross-law resolution.
        let law = r#"
$id: registry_test_law
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Has external reference resolved by registry
    machine_readable:
      execution:
        parameters:
          - name: BSN
            type: string
            required: true
        input:
          - name: external_value
            type: number
            source:
              regulation: nonexistent_law
              output: some_output
              parameters:
                BSN: $BSN
        output:
          - name: result
            type: number
        actions:
          - output: result
            operation: MULTIPLY
            values:
              - $external_value
              - 3
"#;
        let mut service = LawExecutionService::new();
        service.load_law(law).unwrap();

        // Register data source with the input field
        let mut record = BTreeMap::new();
        record.insert("BSN".to_string(), Value::String("123".to_string()));
        record.insert("external_value".to_string(), Value::Int(42));

        service
            .register_dict_source("test_data", "BSN", vec![record])
            .unwrap();

        let mut params = BTreeMap::new();
        params.insert("BSN".to_string(), Value::String("123".to_string()));

        let result = service
            .evaluate_law_output("registry_test_law", "result", params, "2025-01-01")
            .unwrap();

        // result = 42 * 3 = 126
        assert_eq!(result.outputs.get("result"), Some(&Value::Int(126)));
    }

    #[test]
    fn test_data_registry_fallback_to_cross_law() {
        // Registry has no matching field → cross-law resolution should still work
        let mut service = LawExecutionService::new();
        service.load_law(make_base_law()).unwrap();
        service.load_law(make_dependent_law()).unwrap();

        // Register a data source with an unrelated field
        let mut record = BTreeMap::new();
        record.insert("key".to_string(), Value::String("x".to_string()));
        record.insert("unrelated_field".to_string(), Value::Int(999));

        service
            .register_dict_source("unrelated_data", "key", vec![record])
            .unwrap();

        // Execute dependent law - should fall back to cross-law resolution
        let result = service
            .evaluate_law_output(
                "dependent_law",
                "doubled_value",
                BTreeMap::new(),
                "2025-01-01",
            )
            .unwrap();

        // doubled_value = base_value (100) * 2 = 200
        assert_eq!(result.outputs.get("doubled_value"), Some(&Value::Int(200)));
    }

    #[test]
    fn test_parameters_take_priority_over_registry() {
        // Both a parameter and a registry entry exist for the same field.
        // The parameter should win because it's checked first.
        let law = r#"
$id: priority_test_law
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Tests parameter vs registry priority
    machine_readable:
      execution:
        parameters:
          - name: BSN
            type: string
          - name: external_value
            type: number
        input:
          - name: external_value
            type: number
            source:
              regulation: some_law
              output: some_output
              parameters:
                BSN: $BSN
        output:
          - name: result
            type: number
        actions:
          - output: result
            value: $external_value
"#;
        let mut service = LawExecutionService::new();
        service.load_law(law).unwrap();

        // Register data source with external_value = 100
        let mut record = BTreeMap::new();
        record.insert("BSN".to_string(), Value::String("123".to_string()));
        record.insert("external_value".to_string(), Value::Int(100));

        service
            .register_dict_source("test_data", "BSN", vec![record])
            .unwrap();

        // Pass external_value = 50 as parameter (should win)
        let mut params = BTreeMap::new();
        params.insert("BSN".to_string(), Value::String("123".to_string()));
        params.insert("external_value".to_string(), Value::Int(50));

        let result = service
            .evaluate_law_output("priority_test_law", "result", params, "2025-01-01")
            .unwrap();

        // Parameter value (50) should win over registry value (100)
        assert_eq!(result.outputs.get("result"), Some(&Value::Int(50)));
    }

    // -------------------------------------------------------------------------
    // IoC (open_terms + implements) Tests
    // -------------------------------------------------------------------------

    fn make_law_with_open_term() -> &'static str {
        r#"
$id: zorgtoeslag_ioc
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '4'
    text: De standaardpremie wordt vastgesteld bij ministeriele regeling
    machine_readable:
      open_terms:
        - id: standaardpremie
          type: amount
          required: true
          delegated_to: minister
          delegation_type: MINISTERIELE_REGELING
      execution:
        output:
          - name: standaardpremie
            type: number
        actions:
          - output: standaardpremie
            value: "$standaardpremie"
"#
    }

    fn make_implementing_regulation() -> &'static str {
        r#"
$id: regeling_sp_ioc
regulatory_layer: MINISTERIELE_REGELING
publication_date: '2025-01-01'
valid_from: '2025-01-01'
articles:
  - number: '1'
    text: De standaardpremie bedraagt 1928
    machine_readable:
      implements:
        - law: zorgtoeslag_ioc
          article: '4'
          open_term: standaardpremie
          gelet_op: "Gelet op artikel 4 van de Wet op de zorgtoeslag"
      execution:
        output:
          - name: standaardpremie
            type: number
        actions:
          - output: standaardpremie
            value: 1928
"#
    }

    #[test]
    fn test_ioc_resolve_open_term() {
        let mut service = LawExecutionService::new();
        service.load_law(make_law_with_open_term()).unwrap();
        service.load_law(make_implementing_regulation()).unwrap();

        let result = service
            .evaluate_law_output(
                "zorgtoeslag_ioc",
                "standaardpremie",
                BTreeMap::new(),
                "2025-01-01",
            )
            .unwrap();

        assert_eq!(
            result.outputs.get("standaardpremie"),
            Some(&Value::Int(1928))
        );
    }

    #[test]
    fn test_ioc_required_no_implementation() {
        let mut service = LawExecutionService::new();
        service.load_law(make_law_with_open_term()).unwrap();
        // No implementing regulation loaded

        let result = service.evaluate_law_output(
            "zorgtoeslag_ioc",
            "standaardpremie",
            BTreeMap::new(),
            "2025-01-01",
        );

        assert!(
            matches!(result, Err(EngineError::ResolutionError(_))),
            "Expected ResolutionError for missing required implementation, got: {:?}",
            result
        );
    }

    #[test]
    fn test_ioc_optional_no_implementation() {
        let yaml = r#"
$id: optional_term_law
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Optional open term
    machine_readable:
      open_terms:
        - id: bijzondere_premie
          type: amount
          required: false
      execution:
        output:
          - name: result
            type: number
        actions:
          - output: result
            value: 42
"#;
        let mut service = LawExecutionService::new();
        service.load_law(yaml).unwrap();

        // Should succeed — optional term not implemented is fine
        let result = service
            .evaluate_law_output("optional_term_law", "result", BTreeMap::new(), "2025-01-01")
            .unwrap();

        assert_eq!(result.outputs.get("result"), Some(&Value::Int(42)));
    }

    #[test]
    fn test_ioc_with_default() {
        let yaml = r#"
$id: default_term_law
regulatory_layer: BELEIDSREGEL
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Default open term
    machine_readable:
      open_terms:
        - id: redelijk_percentage
          type: number
          required: true
          default:
            actions:
              - output: redelijk_percentage
                value: 6
      execution:
        output:
          - name: redelijk_percentage
            type: number
        actions:
          - output: redelijk_percentage
            value: "$redelijk_percentage"
"#;
        let mut service = LawExecutionService::new();
        service.load_law(yaml).unwrap();

        // No implementation loaded — should fall back to default
        let result = service
            .evaluate_law_output(
                "default_term_law",
                "redelijk_percentage",
                BTreeMap::new(),
                "2025-01-01",
            )
            .unwrap();

        assert_eq!(
            result.outputs.get("redelijk_percentage"),
            Some(&Value::Int(6))
        );
    }

    #[test]
    fn test_ioc_implementation_overrides_default() {
        let law_yaml = r#"
$id: default_override_law
regulatory_layer: BELEIDSREGEL
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Default open term
    machine_readable:
      open_terms:
        - id: percentage
          type: number
          required: true
          default:
            actions:
              - output: percentage
                value: 6
      execution:
        output:
          - name: percentage
            type: number
        actions:
          - output: percentage
            value: "$percentage"
"#;

        let impl_yaml = r#"
$id: override_implementation
regulatory_layer: UITVOERINGSBELEID
publication_date: '2025-06-01'
valid_from: '2025-06-01'
articles:
  - number: '1'
    text: Override percentage
    machine_readable:
      implements:
        - law: default_override_law
          article: '1'
          open_term: percentage
      execution:
        output:
          - name: percentage
            type: number
        actions:
          - output: percentage
            value: 4
"#;

        let mut service = LawExecutionService::new();
        service.load_law(law_yaml).unwrap();
        service.load_law(impl_yaml).unwrap();

        // Implementation should override default
        let result = service
            .evaluate_law_output(
                "default_override_law",
                "percentage",
                BTreeMap::new(),
                "2025-07-01",
            )
            .unwrap();

        assert_eq!(result.outputs.get("percentage"), Some(&Value::Int(4)));
    }

    #[test]
    fn test_ioc_temporal_filtering() {
        // Two versions of the same implementing regulation (same $id, different valid_from).
        // The engine should select the version valid for the calculation date.
        let higher_law = r#"
$id: test_higher_law
regulatory_layer: WET
publication_date: '2024-01-01'
articles:
  - number: '1'
    text: Test article with open term
    machine_readable:
      open_terms:
        - id: yearly_amount
          type: number
          required: true
          delegation_type: MINISTERIELE_REGELING
      execution:
        output:
          - name: result
            type: number
        actions:
          - output: result
            value: $yearly_amount
"#;

        let impl_v2025 = r#"
$id: test_impl_regulation
regulatory_layer: MINISTERIELE_REGELING
publication_date: '2024-11-01'
valid_from: '2025-01-01'
articles:
  - number: '1'
    text: 2025 amount
    machine_readable:
      implements:
        - law: test_higher_law
          article: '1'
          open_term: yearly_amount
      execution:
        output:
          - name: yearly_amount
            type: number
        actions:
          - output: yearly_amount
            value: 211200
"#;

        let impl_v2026 = r#"
$id: test_impl_regulation
regulatory_layer: MINISTERIELE_REGELING
publication_date: '2025-11-01'
valid_from: '2026-01-01'
articles:
  - number: '1'
    text: 2026 amount
    machine_readable:
      implements:
        - law: test_higher_law
          article: '1'
          open_term: yearly_amount
      execution:
        output:
          - name: yearly_amount
            type: number
        actions:
          - output: yearly_amount
            value: 220000
"#;

        let mut service = LawExecutionService::new();
        service.load_law(higher_law).unwrap();
        service.load_law(impl_v2025).unwrap();
        service.load_law(impl_v2026).unwrap();

        // Calculate for 2025: should use the 2025 version
        let result = service
            .evaluate_law_output("test_higher_law", "result", BTreeMap::new(), "2025-06-01")
            .unwrap();
        assert_eq!(
            result.outputs.get("result"),
            Some(&Value::Int(211200)),
            "2025 calculation should use 2025 version"
        );

        // Calculate for 2026: should use the 2026 version
        let result = service
            .evaluate_law_output("test_higher_law", "result", BTreeMap::new(), "2026-06-01")
            .unwrap();
        assert_eq!(
            result.outputs.get("result"),
            Some(&Value::Int(220000)),
            "2026 calculation should use 2026 version"
        );
    }
}
