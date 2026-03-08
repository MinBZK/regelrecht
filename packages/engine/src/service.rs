//! Service layer for cross-law resolution
//!
//! Provides the `ServiceProvider` trait and `LawExecutionService` implementation
//! for resolving cross-law references and delegation lookups.
//!
//! # Example
//!
//! ```ignore
//! use regelrecht_engine::{LawExecutionService, Value};
//! use std::collections::HashMap;
//!
//! let mut service = LawExecutionService::new();
//! service.load_law(zorgtoeslagwet_yaml)?;
//! service.load_law(regeling_standaardpremie_yaml)?;
//!
//! let mut params = HashMap::new();
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

use crate::article::{
    Action, Article, ArticleBasedLaw, Input, LegalBasisForDefaults, Resolve, SelectOnCriteria,
};
use crate::config;
use crate::context::RuleContext;
use crate::data_source::{DataSource, DataSourceRegistry, DictDataSource};
use crate::engine::{
    evaluate_select_on_criteria, get_delegation_info, ArticleEngine, ArticleResult,
};
use crate::error::{EngineError, Result};
use crate::operations::evaluate_value;
use crate::operations::values_equal;
use crate::operations::ValueResolver;
use crate::resolver::RuleResolver;
use crate::trace::TraceBuilder;
use crate::types::{PathNodeType, RegulatoryLayer, ResolveType, Value};
use crate::uri::RegelrechtUri;
use chrono::NaiveDate;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
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
    /// Per-execution memoization cache: canonical key → outputs map
    cache: HashMap<String, HashMap<String, Value>>,
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
}

/// Build a deterministic cache key from law_id, output_name, and parameters.
fn cache_key(law_id: &str, output_name: &str, params: &HashMap<String, Value>) -> String {
    let mut sorted: Vec<_> = params.iter().collect();
    sorted.sort_by_key(|(k, _)| k.as_str());
    let params_str: String = sorted
        .iter()
        .map(|(k, v)| format!("{}={:?}", k, v))
        .collect::<Vec<_>>()
        .join(",");
    format!("{}#{}({})", law_id, output_name, params_str)
}

/// Reference to a delegation source for resolution.
///
/// Bundles the delegation-specific parameters to reduce argument count
/// in delegation resolution functions.
struct DelegationRef<'a> {
    /// The law that grants the delegation
    law_id: &'a str,
    /// The article that grants the delegation
    article: &'a str,
    /// Criteria for selecting the delegated regulation
    select_on: Option<&'a [SelectOnCriteria]>,
}

/// Trait for resolving cross-law references and delegations.
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
        parameters: &HashMap<String, Value>,
        calculation_date: &str,
    ) -> Result<ArticleResult>;

    /// Find a delegated regulation matching the given criteria.
    ///
    /// # Arguments
    /// * `law_id` - The law that grants the delegation
    /// * `article` - The article number that grants the delegation
    /// * `criteria` - Evaluated select_on criteria to match
    /// * `reference_date` - Optional date to select the appropriate law version
    ///
    /// # Returns
    /// Reference to the matching regulation, if found.
    fn find_delegated_regulation(
        &self,
        law_id: &str,
        article: &str,
        criteria: &HashMap<String, Value>,
        reference_date: Option<NaiveDate>,
    ) -> Result<Option<&ArticleBasedLaw>>;

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
        source_parameters: Option<&HashMap<String, String>>,
        context: &RuleContext,
        calculation_date: &str,
    ) -> Result<Value>;

    /// Resolve a delegation input source.
    ///
    /// This resolves `source.delegation` references by finding the matching
    /// regulation and executing its article.
    ///
    /// # Arguments
    /// * `delegation_law_id` - The law that grants the delegation
    /// * `delegation_article` - The article that grants the delegation
    /// * `select_on` - Criteria for selecting the delegated regulation
    /// * `output` - The output name to resolve
    /// * `source_parameters` - Parameters mapping from source
    /// * `context` - Current execution context
    /// * `calculation_date` - Date for calculations
    #[allow(clippy::too_many_arguments)]
    fn resolve_delegation_input(
        &self,
        delegation_law_id: &str,
        delegation_article: &str,
        select_on: Option<&[SelectOnCriteria]>,
        output: &str,
        source_parameters: Option<&HashMap<String, String>>,
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

/// Parse a resolve type string to a `RegulatoryLayer` for filtering.
///
/// Maps resolve type strings (from YAML `resolve.type` fields) to the
/// corresponding `RegulatoryLayer` enum variant.
fn parse_regulatory_layer(resolve_type: &str) -> Option<RegulatoryLayer> {
    match resolve_type.to_lowercase().as_str() {
        "grondwet" => Some(RegulatoryLayer::Grondwet),
        "wet" => Some(RegulatoryLayer::Wet),
        "amvb" => Some(RegulatoryLayer::Amvb),
        "ministeriele_regeling" => Some(RegulatoryLayer::MinisterieleRegeling),
        "beleidsregel" => Some(RegulatoryLayer::Beleidsregel),
        "eu_verordening" => Some(RegulatoryLayer::EuVerordening),
        "eu_richtlijn" => Some(RegulatoryLayer::EuRichtlijn),
        "verdrag" => Some(RegulatoryLayer::Verdrag),
        "uitvoeringsbeleid" => Some(RegulatoryLayer::Uitvoeringsbeleid),
        "gemeentelijke_verordening" => Some(RegulatoryLayer::GemeentelijkeVerordening),
        "provinciale_verordening" => Some(RegulatoryLayer::ProvincialeVerordening),
        _ => None,
    }
}

/// High-level service for executing laws with automatic cross-law resolution.
///
/// `LawExecutionService` wraps a `RuleResolver` and implements `ServiceProvider`
/// to enable automatic resolution of external references and delegations.
/// It also supports external data sources via `DataSourceRegistry`.
pub struct LawExecutionService {
    resolver: RuleResolver,
    /// Registry for external data sources. Queried during law execution
    /// to resolve inputs before falling back to cross-law resolution.
    /// Acts as an override layer: if a data source provides a field,
    /// it's used instead of triggering cross-law/delegation resolution.
    data_registry: DataSourceRegistry,
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
        }
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
    pub fn evaluate_law_output(
        &self,
        law_id: &str,
        output_name: &str,
        parameters: HashMap<String, Value>,
        calculation_date: &str,
    ) -> Result<ArticleResult> {
        let mut res_ctx = ResolutionContext::new(calculation_date);
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
        parameters: HashMap<String, Value>,
        calculation_date: &str,
    ) -> Result<ArticleResult> {
        let trace = Rc::new(RefCell::new(TraceBuilder::new()));

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
        let mut result =
            self.evaluate_law_output_internal(law_id, output_name, parameters, &mut res_ctx)?;

        // Drop res_ctx so it releases its Rc clone
        drop(res_ctx);

        // Set the result on the top-level article node and pop it
        {
            let mut tb = trace.borrow_mut();
            if let Some(value) = result.outputs.get(output_name) {
                tb.set_result(value.clone());
            }
            // Pop returns the completed top-level node
            result.trace = tb.pop();
        }

        Ok(result)
    }

    /// Internal method with cycle tracking.
    fn evaluate_law_output_internal(
        &self,
        law_id: &str,
        output_name: &str,
        parameters: HashMap<String, Value>,
        res_ctx: &mut ResolutionContext<'_>,
    ) -> Result<ArticleResult> {
        // --- Cache check (before depth check: cached results don't increase depth) ---
        let key = cache_key(law_id, output_name, &parameters);
        if let Some(cached_outputs) = res_ctx.cache.get(&key) {
            tracing::debug!(law_id, output_name, "Cache hit");
            if let Some(ref tb) = res_ctx.trace {
                let mut tb = tb.borrow_mut();
                tb.push(format!("{}#{}", law_id, output_name), PathNodeType::Cached);
                if let Some(val) = cached_outputs.get(output_name) {
                    tb.set_result(val.clone());
                }
                tb.pop();
            }
            return Ok(ArticleResult {
                outputs: cached_outputs.clone(),
                resolved_inputs: HashMap::new(),
                article_number: String::new(),
                law_id: law_id.to_string(),
                law_uuid: None,
                trace: None,
            });
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

        // Execute with service provider
        let result = self.evaluate_article_with_service(
            article,
            law,
            parameters,
            Some(output_name),
            res_ctx,
        )?;

        // --- Cache store (only on success) ---
        res_ctx.cache.insert(key, result.outputs.clone());

        Ok(result)
    }

    /// Execute an article with ServiceProvider support.
    fn evaluate_article_with_service(
        &self,
        article: &Article,
        law: &ArticleBasedLaw,
        parameters: HashMap<String, Value>,
        requested_output: Option<&str>,
        res_ctx: &mut ResolutionContext<'_>,
    ) -> Result<ArticleResult> {
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

        // Pre-resolve any resolve actions in this article
        let resolved_actions = self.pre_resolve_actions(article, law, &context, res_ctx)?;

        // Use ArticleEngine for action execution (it handles the internal logic)
        let engine = ArticleEngine::new(article, law);

        // Now execute with resolved inputs already in context
        // We need to pass resolved inputs as parameters to avoid re-resolution
        let mut combined_params = parameters;
        for (name, value) in context.resolved_inputs() {
            combined_params.insert(name.clone(), value.clone());
        }
        // Merge pre-resolved action outputs so the engine can pick them up
        for (name, value) in resolved_actions {
            combined_params.insert(name, value);
        }

        // Use traced evaluation if trace is available
        if let Some(ref tb) = res_ctx.trace {
            engine.evaluate_with_trace(
                combined_params,
                res_ctx.calculation_date,
                requested_output,
                Rc::clone(tb),
            )
        } else {
            engine.evaluate_with_output(combined_params, res_ctx.calculation_date, requested_output)
        }
    }

    /// Pre-resolve all resolve actions in an article.
    ///
    /// Scans the article's actions for `resolve:` specifications and resolves
    /// each one using `resolve_resolve_action()`. Returns a map of output names
    /// to resolved values.
    fn pre_resolve_actions(
        &self,
        article: &Article,
        law: &ArticleBasedLaw,
        context: &RuleContext,
        res_ctx: &mut ResolutionContext<'_>,
    ) -> Result<HashMap<String, Value>> {
        let mut resolved = HashMap::new();

        let actions = article
            .get_execution_spec()
            .and_then(|exec| exec.actions.as_deref())
            .unwrap_or(&[]);

        for action in actions {
            if let Some(resolve) = &action.resolve {
                if let Some(output_name) = &action.output {
                    let value = self.resolve_resolve_action(
                        resolve,
                        &law.id,
                        &article.number,
                        context,
                        res_ctx,
                    )?;
                    resolved.insert(output_name.clone(), value);
                }
            }
        }

        Ok(resolved)
    }

    /// Resolve a single resolve action.
    ///
    /// Implements the Python `_evaluate_resolve()` algorithm:
    /// 1. Find regulations by legal basis (filtered by resolve type)
    /// 2. If match criteria exist, evaluate expected value from context
    /// 3. For each candidate: execute to get match output, compare, skip on mismatch
    /// 4. Execute matching candidate for requested output
    /// 5. Require exactly 1 match (error on 0 or 2+)
    fn resolve_resolve_action(
        &self,
        resolve: &Resolve,
        law_id: &str,
        article_number: &str,
        context: &RuleContext,
        res_ctx: &mut ResolutionContext<'_>,
    ) -> Result<Value> {
        let layer_filter = parse_regulatory_layer(&resolve.resolve_type);

        tracing::debug!(
            law_id = %law_id,
            article = %article_number,
            resolve_type = %resolve.resolve_type,
            output = %resolve.output,
            "Resolving action via legal basis"
        );

        // Find regulations that have this article as their legal_basis
        let candidates = self.resolver.find_regulations_by_legal_basis(
            law_id,
            article_number,
            layer_filter.as_ref(),
            res_ctx.reference_date(),
        );

        if candidates.is_empty() {
            return Err(EngineError::DelegationError(format!(
                "No regulations found with legal_basis {}#{} (type={})",
                law_id, article_number, resolve.resolve_type
            )));
        }

        tracing::debug!(
            candidates = candidates.len(),
            ids = ?candidates.iter().map(|c| &c.id).collect::<Vec<_>>(),
            "Found candidate regulations"
        );

        // Evaluate expected match value if match criteria exist
        let expected_match_value = if let Some(match_spec) = &resolve.match_spec {
            Some(evaluate_value(&match_spec.value, context, 0)?)
        } else {
            None
        };

        // Track matches: we need exactly one
        let mut first_match: Option<(&str, Value)> = None;

        for candidate_law in &candidates {
            let candidate_id = &candidate_law.id;

            // Find the article that produces the requested output
            let candidate_article = match candidate_law.find_article_by_output(&resolve.output) {
                Some(a) => a,
                None => {
                    tracing::debug!(
                        candidate = %candidate_id,
                        output = %resolve.output,
                        "Candidate has no article with requested output, skipping"
                    );
                    continue;
                }
            };

            // Phase 1: Check match criteria if present
            if let (Some(match_spec), Some(expected)) = (&resolve.match_spec, &expected_match_value)
            {
                let match_result = self.try_evaluate_candidate(
                    candidate_article,
                    candidate_law,
                    Some(&match_spec.output),
                    res_ctx,
                );

                match match_result {
                    Ok(result) => {
                        let match_value = result.outputs.get(&match_spec.output);
                        match match_value {
                            Some(actual) if values_equal(actual, expected) => {
                                tracing::debug!(
                                    candidate = %candidate_id,
                                    "Match criteria satisfied"
                                );
                            }
                            Some(actual) => {
                                tracing::debug!(
                                    candidate = %candidate_id,
                                    expected = %expected,
                                    actual = %actual,
                                    "Match criteria not met, skipping"
                                );
                                continue;
                            }
                            None => {
                                tracing::debug!(
                                    candidate = %candidate_id,
                                    match_output = %match_spec.output,
                                    "Match output not found, skipping"
                                );
                                continue;
                            }
                        }
                    }
                    Err(e) => {
                        tracing::debug!(
                            candidate = %candidate_id,
                            error = %e,
                            "Error evaluating match criteria, skipping"
                        );
                        continue;
                    }
                }
            }

            // Phase 2: Get the actual requested output
            let output_result = self.try_evaluate_candidate(
                candidate_article,
                candidate_law,
                Some(&resolve.output),
                res_ctx,
            );

            match output_result {
                Ok(result) => {
                    if let Some(value) = result.outputs.get(&resolve.output).cloned() {
                        // Check for multiple matches
                        if let Some((prev_id, _)) = &first_match {
                            return Err(EngineError::DelegationError(format!(
                                "Multiple regulations match for {}#{} with resolve type '{}'. \
                                 Found at least: [{}, {}]. \
                                 Add more specific match criteria to ensure deterministic resolution.",
                                law_id, article_number, resolve.resolve_type, prev_id, candidate_id
                            )));
                        }
                        first_match = Some((candidate_id, value));
                    } else {
                        tracing::debug!(
                            candidate = %candidate_id,
                            output = %resolve.output,
                            "Output not found in result, skipping"
                        );
                    }
                }
                Err(e) => {
                    tracing::debug!(
                        candidate = %candidate_id,
                        error = %e,
                        "Error evaluating candidate, skipping"
                    );
                    continue;
                }
            }
        }

        match first_match {
            Some((matched_id, value)) => {
                tracing::info!(
                    law_id = %law_id,
                    article = %article_number,
                    matched = %matched_id,
                    "Resolved to unique regulation"
                );
                Ok(value)
            }
            None => Err(EngineError::DelegationError(format!(
                "No matching regulation found for {}#{} with resolve type '{}' and match criteria {:?}",
                law_id, article_number, resolve.resolve_type,
                resolve.match_spec.as_ref().map(|m| &m.output)
            ))),
        }
    }

    /// Try to evaluate a candidate regulation's article.
    ///
    /// Used by resolve_resolve_action to evaluate match criteria and output values.
    /// Returns the execution result, or an error if evaluation fails.
    fn try_evaluate_candidate(
        &self,
        article: &Article,
        law: &ArticleBasedLaw,
        requested_output: Option<&str>,
        res_ctx: &mut ResolutionContext<'_>,
    ) -> Result<ArticleResult> {
        self.evaluate_article_with_service(article, law, HashMap::new(), requested_output, res_ctx)
    }

    /// Resolve input sources using ServiceProvider.
    fn resolve_inputs_with_service(
        &self,
        article: &Article,
        law: &ArticleBasedLaw,
        context: &mut RuleContext,
        parameters: &HashMap<String, Value>,
        res_ctx: &mut ResolutionContext<'_>,
    ) -> Result<()> {
        let inputs = self.get_inputs(article);

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
                    if let Some(ref tb) = res_ctx.trace {
                        let mut tb = tb.borrow_mut();
                        tb.push(&input.name, PathNodeType::Resolve);
                        tb.set_resolve_type(ResolveType::DataSource);
                        tb.set_result(data_match.value.clone());
                        tb.set_message(format!(
                            "Resolving from SOURCE {}: {}",
                            data_match.source_name, data_match.value
                        ));
                        tb.pop();
                    }

                    context.set_resolved_input(&input.name, data_match.value);
                    continue;
                }
            }

            // For cross-law/delegation resolution, output defaults to input name
            let output_name = source.output.as_deref().unwrap_or(&input.name);

            if let Some(delegation) = &source.delegation {
                // Delegation reference
                let (del_law_id, del_article, select_on) = get_delegation_info(delegation);
                let del_ref = DelegationRef {
                    law_id: del_law_id,
                    article: del_article,
                    select_on,
                };

                let value = self.resolve_delegation_input_internal(
                    &del_ref,
                    output_name,
                    source.parameters.as_ref(),
                    context,
                    res_ctx,
                )?;

                context.set_resolved_input(&input.name, value);
            } else if let Some(regulation) = &source.regulation {
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
                let ref_article = law.find_article_by_output(output_name).ok_or_else(|| {
                    EngineError::OutputNotFound {
                        law_id: law.id.clone(),
                        output: output_name.to_string(),
                    }
                })?;

                let ref_params = parameters.clone();
                let result = self.evaluate_article_with_service(
                    ref_article,
                    law,
                    ref_params,
                    Some(output_name),
                    res_ctx,
                )?;

                if let Some(value) = result.outputs.get(output_name) {
                    context.set_resolved_input(&input.name, value.clone());
                }
            } else {
                // Empty source (source: {}) — resolved from DataSourceRegistry only.
                // If DataSourceRegistry didn't match above, leave unresolved
                // (engine will use null/default).
            }
        }

        Ok(())
    }

    /// Internal method for external input resolution with depth tracking.
    fn resolve_external_input_internal(
        &self,
        regulation: &str,
        output: &str,
        source_parameters: Option<&HashMap<String, String>>,
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

        // Trace cross-law call
        if let Some(ref tb) = res_ctx.trace {
            tb.borrow_mut()
                .push(format!("{}#{}", regulation, output), PathNodeType::UriCall);
        }

        // Build parameters for the target article
        let target_params = self.build_target_parameters(source_parameters, context)?;

        // Enter cross-law resolution scope
        res_ctx.enter(key.clone());

        // Execute the target article
        let result = self.evaluate_law_output_internal(regulation, output, target_params, res_ctx);

        // Leave scope (even on error, for correct cycle tracking)
        res_ctx.leave(&key);

        let value =
            result?
                .outputs
                .get(output)
                .cloned()
                .ok_or_else(|| EngineError::OutputNotFound {
                    law_id: regulation.to_string(),
                    output: output.to_string(),
                })?;

        // Complete trace node
        if let Some(ref tb) = res_ctx.trace {
            let mut tb = tb.borrow_mut();
            tb.set_result(value.clone());
            tb.pop();
        }

        Ok(value)
    }

    /// Internal method for delegation input resolution with depth tracking.
    fn resolve_delegation_input_internal(
        &self,
        delegation: &DelegationRef<'_>,
        output: &str,
        source_parameters: Option<&HashMap<String, String>>,
        context: &RuleContext,
        res_ctx: &mut ResolutionContext<'_>,
    ) -> Result<Value> {
        // Evaluate selection criteria
        let criteria = if let Some(criteria_spec) = delegation.select_on {
            evaluate_select_on_criteria(criteria_spec, context)?
        } else {
            HashMap::new()
        };

        // Format criteria for logging
        let criteria_str: Vec<String> = criteria
            .iter()
            .map(|(k, v)| format!("{}={:?}", k, v))
            .collect();

        tracing::debug!(
            law_id = %delegation.law_id,
            article = %delegation.article,
            criteria = ?criteria_str,
            "Resolving delegation"
        );

        // Find matching regulation
        let regulation_opt = self.find_delegated_regulation(
            delegation.law_id,
            delegation.article,
            &criteria,
            res_ctx.reference_date(),
        )?;

        match regulation_opt {
            Some(regulation) => {
                tracing::debug!(
                    delegation_from = %delegation.law_id,
                    delegation_article = %delegation.article,
                    found_regulation = %regulation.id,
                    "Found delegated regulation"
                );
                // Found a delegated regulation - execute it
                self.execute_delegated_regulation(
                    regulation,
                    output,
                    source_parameters,
                    context,
                    res_ctx,
                )
            }
            None => {
                tracing::debug!(
                    delegation_from = %delegation.law_id,
                    delegation_article = %delegation.article,
                    "No matching regulation found, checking for defaults"
                );
                // No delegated regulation found - try to use defaults from the delegating article
                self.try_execute_defaults(
                    delegation,
                    output,
                    source_parameters,
                    context,
                    &criteria,
                    res_ctx,
                )
            }
        }
    }

    /// Execute a found delegated regulation.
    fn execute_delegated_regulation(
        &self,
        regulation: &ArticleBasedLaw,
        output: &str,
        source_parameters: Option<&HashMap<String, String>>,
        context: &RuleContext,
        res_ctx: &mut ResolutionContext<'_>,
    ) -> Result<Value> {
        // Check for circular reference
        let key = format!("{}#{}", regulation.id, output);
        if res_ctx.is_visited(&key) {
            return Err(EngineError::CircularReference(format!(
                "Circular delegation reference detected: {} is already being resolved",
                key
            )));
        }

        // Build parameters for the delegated regulation
        let target_params = self.build_target_parameters(source_parameters, context)?;

        tracing::debug!(
            regulation_id = %regulation.id,
            output = %output,
            params = ?target_params.keys().collect::<Vec<_>>(),
            "Executing delegated regulation"
        );

        // Enter cross-law resolution scope
        res_ctx.enter(key.clone());

        // Execute the delegated regulation with cycle tracking
        let result =
            self.evaluate_law_output_internal(&regulation.id, output, target_params, res_ctx);

        // Leave scope (even on error, for correct cycle tracking)
        res_ctx.leave(&key);

        let value =
            result?
                .outputs
                .get(output)
                .cloned()
                .ok_or_else(|| EngineError::OutputNotFound {
                    law_id: regulation.id.clone(),
                    output: output.to_string(),
                })?;

        tracing::debug!(
            regulation_id = %regulation.id,
            "Delegation result: {} = {}", output, value
        );

        Ok(value)
    }

    /// Try to execute defaults from the delegating article's legal_basis_for section.
    ///
    /// This is called when no delegated regulation is found. If the delegating
    /// article has defaults defined, those are executed instead.
    fn try_execute_defaults(
        &self,
        delegation: &DelegationRef<'_>,
        output: &str,
        source_parameters: Option<&HashMap<String, String>>,
        context: &RuleContext,
        criteria: &HashMap<String, Value>,
        res_ctx: &mut ResolutionContext<'_>,
    ) -> Result<Value> {
        // Get the delegating law and article (version-aware)
        let law = self
            .resolver
            .get_law_for_date(delegation.law_id, res_ctx.reference_date())
            .ok_or_else(|| EngineError::LawNotFound(delegation.law_id.to_string()))?;

        let article = law
            .find_article_by_number(delegation.article)
            .ok_or_else(|| EngineError::ArticleNotFound {
                law_id: delegation.law_id.to_string(),
                article: delegation.article.to_string(),
            })?;

        // Look for legal_basis_for with defaults
        let defaults = article
            .get_legal_basis_for()
            .and_then(|basis_list| basis_list.iter().find_map(|basis| basis.defaults.as_ref()));

        match defaults {
            Some(defaults) => {
                tracing::info!(
                    law_id = %delegation.law_id,
                    article = %delegation.article,
                    "Using defaults (optional delegation)"
                );
                self.execute_defaults(defaults, output, source_parameters, context)
            }
            None => {
                // No defaults available - this is an error (mandatory delegation)
                let criteria_str = criteria
                    .iter()
                    .map(|(k, v)| format!("{}={:?}", k, v))
                    .collect::<Vec<_>>()
                    .join(", ");
                tracing::error!(
                    law_id = %delegation.law_id,
                    article = %delegation.article,
                    criteria = %criteria_str,
                    "No regulation found for mandatory delegation"
                );
                Err(EngineError::DelegationError(format!(
                    "No regulation found for mandatory delegation from {}#{} with criteria [{}]",
                    delegation.law_id, delegation.article, criteria_str
                )))
            }
        }
    }

    /// Execute defaults actions to produce the requested output.
    fn execute_defaults(
        &self,
        defaults: &LegalBasisForDefaults,
        output: &str,
        source_parameters: Option<&HashMap<String, String>>,
        context: &RuleContext,
    ) -> Result<Value> {
        // Build a new context for defaults execution
        // Start with the source parameters resolved
        let target_params = self.build_target_parameters(source_parameters, context)?;
        let mut defaults_context = RuleContext::new(target_params, context.get_calculation_date())?;

        // Set definitions from defaults
        if let Some(definitions) = &defaults.definitions {
            defaults_context.set_definitions(definitions);
        }

        // Execute actions
        if let Some(actions) = &defaults.actions {
            for action in actions {
                if let Some(output_name) = &action.output {
                    let value = self.evaluate_default_action(action, &defaults_context)?;
                    tracing::debug!("Output {} = {}", output_name, value);
                    defaults_context.set_output(output_name, value);
                }
            }
        }

        // Extract the requested output
        defaults_context
            .get_output(output)
            .cloned()
            .ok_or_else(|| EngineError::OutputNotFound {
                law_id: "defaults".to_string(),
                output: output.to_string(),
            })
    }

    /// Evaluate a single action from defaults.
    fn evaluate_default_action(&self, action: &Action, context: &RuleContext) -> Result<Value> {
        // Check for direct value
        if let Some(value) = &action.value {
            return evaluate_value(value, context, 0);
        }

        // No value specified - return null
        Ok(Value::Null)
    }

    /// Build parameters for a target article from source parameter mapping.
    fn build_target_parameters(
        &self,
        source_parameters: Option<&HashMap<String, String>>,
        context: &RuleContext,
    ) -> Result<HashMap<String, Value>> {
        let mut params = HashMap::new();

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

    /// Get inputs from article execution spec.
    fn get_inputs<'a>(&self, article: &'a Article) -> &'a [Input] {
        article
            .get_execution_spec()
            .and_then(|exec| exec.input.as_deref())
            .unwrap_or(&[])
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
        data: HashMap<String, HashMap<String, Value>>,
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
        records: Vec<HashMap<String, Value>>,
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
        parameters: &HashMap<String, Value>,
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

    fn find_delegated_regulation(
        &self,
        law_id: &str,
        article: &str,
        criteria: &HashMap<String, Value>,
        reference_date: Option<NaiveDate>,
    ) -> Result<Option<&ArticleBasedLaw>> {
        Ok(self
            .resolver
            .find_delegated_regulation(law_id, article, criteria, reference_date))
    }

    fn get_law(&self, law_id: &str) -> Option<&ArticleBasedLaw> {
        self.resolver.get_law(law_id)
    }

    fn resolve_external_input(
        &self,
        regulation: &str,
        output: &str,
        source_parameters: Option<&HashMap<String, String>>,
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

    fn resolve_delegation_input(
        &self,
        delegation_law_id: &str,
        delegation_article: &str,
        select_on: Option<&[SelectOnCriteria]>,
        output: &str,
        source_parameters: Option<&HashMap<String, String>>,
        context: &RuleContext,
        calculation_date: &str,
    ) -> Result<Value> {
        let mut res_ctx = ResolutionContext::new(calculation_date);
        let del_ref = DelegationRef {
            law_id: delegation_law_id,
            article: delegation_article,
            select_on,
        };
        self.resolve_delegation_input_internal(
            &del_ref,
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

    fn make_delegating_law() -> &'static str {
        r#"
$id: participatiewet
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '8'
    text: Delegation authority
    machine_readable:
      execution:
        output:
          - name: delegation_granted
            type: boolean
        actions:
          - output: delegation_granted
            value: true
"#
    }

    fn make_delegated_verordening(gemeente_code: &str, percentage: i32) -> String {
        format!(
            r#"
$id: {gemeente_code}_verordening
regulatory_layer: GEMEENTELIJKE_VERORDENING
publication_date: '2025-01-01'
gemeente_code: "{gemeente_code}"
legal_basis:
  - law_id: participatiewet
    article: '8'
articles:
  - number: '1'
    text: Local regulation
    machine_readable:
      execution:
        output:
          - name: verlaging_percentage
            type: number
        actions:
          - output: verlaging_percentage
            value: {percentage}
"#
        )
    }

    fn make_law_using_delegation() -> &'static str {
        r#"
$id: using_delegation_law
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Uses delegated value
    machine_readable:
      execution:
        parameters:
          - name: gemeente_code
            type: string
            required: true
        input:
          - name: local_percentage
            type: number
            source:
              delegation:
                law_id: participatiewet
                article: '8'
                select_on:
                  - name: gemeente_code
                    value: $gemeente_code
              output: verlaging_percentage
        output:
          - name: adjusted_amount
            type: number
        actions:
          - output: adjusted_amount
            operation: MULTIPLY
            values:
              - 1000
              - operation: DIVIDE
                values:
                  - $local_percentage
                  - 100
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
            .evaluate_law_output("base_law", "base_value", HashMap::new(), "2025-01-01")
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
                HashMap::new(),
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
            HashMap::new(),
            "2025-01-01",
        );

        assert!(
            matches!(result, Err(EngineError::LawNotFound(_))),
            "Expected LawNotFound error, got: {:?}",
            result
        );
    }

    // -------------------------------------------------------------------------
    // Delegation Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_service_delegation_resolution() {
        let mut service = LawExecutionService::new();
        service.load_law(make_delegating_law()).unwrap();
        service
            .load_law(&make_delegated_verordening("0363", 20))
            .unwrap();
        service
            .load_law(&make_delegated_verordening("0518", 15))
            .unwrap();
        service.load_law(make_law_using_delegation()).unwrap();

        // Execute with Amsterdam (0363)
        let mut params = HashMap::new();
        params.insert(
            "gemeente_code".to_string(),
            Value::String("0363".to_string()),
        );

        let result = service
            .evaluate_law_output(
                "using_delegation_law",
                "adjusted_amount",
                params,
                "2025-01-01",
            )
            .unwrap();

        // adjusted_amount = 1000 * (20 / 100) = 200.0 (DIVIDE produces float)
        assert_eq!(
            result.outputs.get("adjusted_amount"),
            Some(&Value::Float(200.0))
        );

        // Execute with Den Haag (0518)
        let mut params = HashMap::new();
        params.insert(
            "gemeente_code".to_string(),
            Value::String("0518".to_string()),
        );

        let result = service
            .evaluate_law_output(
                "using_delegation_law",
                "adjusted_amount",
                params,
                "2025-01-01",
            )
            .unwrap();

        // adjusted_amount = 1000 * (15 / 100) = 150.0 (DIVIDE produces float)
        assert_eq!(
            result.outputs.get("adjusted_amount"),
            Some(&Value::Float(150.0))
        );
    }

    #[test]
    fn test_service_delegation_no_match() {
        let mut service = LawExecutionService::new();
        service.load_law(make_delegating_law()).unwrap();
        service
            .load_law(&make_delegated_verordening("0363", 20))
            .unwrap();
        service.load_law(make_law_using_delegation()).unwrap();

        // Execute with non-existent gemeente
        let mut params = HashMap::new();
        params.insert(
            "gemeente_code".to_string(),
            Value::String("9999".to_string()),
        );

        let result = service.evaluate_law_output(
            "using_delegation_law",
            "adjusted_amount",
            params,
            "2025-01-01",
        );

        assert!(
            matches!(result, Err(EngineError::DelegationError(_))),
            "Expected DelegationError, got: {:?}",
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
                &HashMap::new(),
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

        let result = service.evaluate_law_output("law_a", "output_a", HashMap::new(), "2025-01-01");

        assert!(
            matches!(result, Err(EngineError::CircularReference(_))),
            "Expected CircularReference error, got: {:?}",
            result
        );
    }

    #[test]
    fn test_service_delegation_circular_reference() {
        // Test that delegation circular references are properly detected
        // Law A delegates to B, B references A
        let law_a = r#"
$id: law_a
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '8'
    text: Delegation authority
    machine_readable:
      execution:
        output:
          - name: delegation_granted
            type: boolean
        actions:
          - output: delegation_granted
            value: true
"#;

        let regulation_b = r#"
$id: regulation_b
regulatory_layer: GEMEENTELIJKE_VERORDENING
publication_date: '2025-01-01'
gemeente_code: "0363"
legal_basis:
  - law_id: law_a
    article: '8'
articles:
  - number: '1'
    text: References back to law_a via external reference
    machine_readable:
      execution:
        input:
          - name: from_a
            type: boolean
            source:
              regulation: law_a
              output: output_from_delegation
        output:
          - name: local_value
            type: number
        actions:
          - output: local_value
            value: 100
"#;

        let law_using_delegation = r#"
$id: law_using_delegation
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Uses delegation that creates circular reference
    machine_readable:
      execution:
        parameters:
          - name: gemeente_code
            type: string
        input:
          - name: local_value
            type: number
            source:
              delegation:
                law_id: law_a
                article: '8'
                select_on:
                  - name: gemeente_code
                    value: $gemeente_code
              output: local_value
        output:
          - name: output_from_delegation
            type: number
        actions:
          - output: output_from_delegation
            value: $local_value
"#;

        let mut service = LawExecutionService::new();
        service.load_law(law_a).unwrap();
        service.load_law(regulation_b).unwrap();
        service.load_law(law_using_delegation).unwrap();

        let mut params = HashMap::new();
        params.insert(
            "gemeente_code".to_string(),
            Value::String("0363".to_string()),
        );

        // This should fail with circular reference since:
        // law_using_delegation -> delegation to regulation_b -> external ref to law_a
        // But the output_from_delegation doesn't exist in law_a, so it will fail with OutputNotFound
        // That's fine - the important thing is we don't get infinite recursion
        let result = service.evaluate_law_output(
            "law_using_delegation",
            "output_from_delegation",
            params,
            "2025-01-01",
        );

        // Should fail (either circular reference or output not found, but not stack overflow)
        assert!(result.is_err());
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
        let mut params = HashMap::new();
        params.insert("external_value".to_string(), Value::Int(50));

        let result = service
            .evaluate_law_output("override_test_law", "result", params, "2025-01-01")
            .unwrap();

        // result = 50 * 2 = 100
        assert_eq!(result.outputs.get("result"), Some(&Value::Int(100)));
    }

    #[test]
    fn test_service_delegation_parameter_override() {
        // When a parameter is provided that matches a delegation input name,
        // the parameter value should be used instead of delegation resolution
        let mut service = LawExecutionService::new();
        service.load_law(make_delegating_law()).unwrap();
        // Note: we don't load any verordening - delegation would fail
        service.load_law(make_law_using_delegation()).unwrap();

        // Provide local_percentage directly - should skip delegation
        let mut params = HashMap::new();
        params.insert(
            "gemeente_code".to_string(),
            Value::String("9999".to_string()), // Non-existent gemeente
        );
        params.insert("local_percentage".to_string(), Value::Int(25)); // Pre-resolved

        let result = service
            .evaluate_law_output(
                "using_delegation_law",
                "adjusted_amount",
                params,
                "2025-01-01",
            )
            .unwrap();

        // adjusted_amount = 1000 * (25 / 100) = 250.0
        assert_eq!(
            result.outputs.get("adjusted_amount"),
            Some(&Value::Float(250.0))
        );
    }

    // -------------------------------------------------------------------------
    // Resolve Action Tests
    // -------------------------------------------------------------------------

    fn make_resolve_parent_law() -> &'static str {
        r#"
$id: parent_law
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '4'
    text: Article with resolve action
    machine_readable:
      execution:
        output:
          - name: standaardpremie
            type: number
        actions:
          - output: standaardpremie
            resolve:
              type: ministeriele_regeling
              output: standaardpremie
              match:
                output: berekeningsjaar
                value: $referencedate.year
"#
    }

    fn make_matching_regeling(year: i64, premium: i64) -> String {
        format!(
            r#"
$id: regeling_{year}
regulatory_layer: MINISTERIELE_REGELING
publication_date: '2025-01-01'
legal_basis:
  - law_id: parent_law
    article: '4'
articles:
  - number: '1'
    text: Regeling for {year}
    machine_readable:
      execution:
        output:
          - name: standaardpremie
            type: number
          - name: berekeningsjaar
            type: number
        actions:
          - output: standaardpremie
            value: {premium}
          - output: berekeningsjaar
            value: {year}
"#
        )
    }

    #[test]
    fn test_service_resolve_action_standaardpremie() {
        let mut service = LawExecutionService::new();
        service.load_law(make_resolve_parent_law()).unwrap();
        service
            .load_law(&make_matching_regeling(2025, 211200))
            .unwrap();
        service
            .load_law(&make_matching_regeling(2024, 197200))
            .unwrap();

        // Execute for 2025 - should resolve to regeling_2025
        let result = service
            .evaluate_law_output(
                "parent_law",
                "standaardpremie",
                HashMap::new(),
                "2025-01-01",
            )
            .unwrap();

        assert_eq!(
            result.outputs.get("standaardpremie"),
            Some(&Value::Int(211200))
        );

        // Execute for 2024 - should resolve to regeling_2024
        let result = service
            .evaluate_law_output(
                "parent_law",
                "standaardpremie",
                HashMap::new(),
                "2024-06-15",
            )
            .unwrap();

        assert_eq!(
            result.outputs.get("standaardpremie"),
            Some(&Value::Int(197200))
        );
    }

    #[test]
    fn test_resolve_action_no_match() {
        let mut service = LawExecutionService::new();
        service.load_law(make_resolve_parent_law()).unwrap();
        // Load a regeling for 2023 only - no match for 2025
        service
            .load_law(&make_matching_regeling(2023, 180000))
            .unwrap();

        let result = service.evaluate_law_output(
            "parent_law",
            "standaardpremie",
            HashMap::new(),
            "2025-01-01",
        );

        assert!(
            matches!(result, Err(EngineError::DelegationError(_))),
            "Expected DelegationError for no match, got: {:?}",
            result
        );
    }

    #[test]
    fn test_resolve_action_multiple_matches() {
        let mut service = LawExecutionService::new();
        service.load_law(make_resolve_parent_law()).unwrap();
        // Load two regelingen that both match 2025
        service
            .load_law(&make_matching_regeling(2025, 211200))
            .unwrap();

        // Create a second regeling with same year but different name
        let duplicate = r#"
$id: regeling_2025_alt
regulatory_layer: MINISTERIELE_REGELING
publication_date: '2025-01-01'
legal_basis:
  - law_id: parent_law
    article: '4'
articles:
  - number: '1'
    text: Alternate regeling for 2025
    machine_readable:
      execution:
        output:
          - name: standaardpremie
            type: number
          - name: berekeningsjaar
            type: number
        actions:
          - output: standaardpremie
            value: 999999
          - output: berekeningsjaar
            value: 2025
"#;
        service.load_law(duplicate).unwrap();

        let result = service.evaluate_law_output(
            "parent_law",
            "standaardpremie",
            HashMap::new(),
            "2025-01-01",
        );

        assert!(
            matches!(result, Err(EngineError::DelegationError(_))),
            "Expected DelegationError for multiple matches, got: {:?}",
            result
        );
    }

    #[test]
    fn test_resolve_action_without_match_criteria() {
        // Resolve action without match spec - should match if exactly one candidate
        let parent = r#"
$id: simple_parent
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Simple resolve without match
    machine_readable:
      execution:
        output:
          - name: some_value
            type: number
        actions:
          - output: some_value
            resolve:
              type: ministeriele_regeling
              output: some_value
"#;
        let regeling = r#"
$id: simple_regeling
regulatory_layer: MINISTERIELE_REGELING
publication_date: '2025-01-01'
legal_basis:
  - law_id: simple_parent
    article: '1'
articles:
  - number: '1'
    text: Simple regeling
    machine_readable:
      execution:
        output:
          - name: some_value
            type: number
        actions:
          - output: some_value
            value: 42
"#;
        let mut service = LawExecutionService::new();
        service.load_law(parent).unwrap();
        service.load_law(regeling).unwrap();

        let result = service
            .evaluate_law_output("simple_parent", "some_value", HashMap::new(), "2025-01-01")
            .unwrap();

        assert_eq!(result.outputs.get("some_value"), Some(&Value::Int(42)));
    }

    #[test]
    fn test_resolve_action_no_regulations() {
        // Resolve action when no regulations have this legal basis
        let parent = r#"
$id: orphan_parent
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Resolve with no candidates
    machine_readable:
      execution:
        output:
          - name: orphan_output
            type: number
        actions:
          - output: orphan_output
            resolve:
              type: ministeriele_regeling
              output: orphan_output
"#;
        let mut service = LawExecutionService::new();
        service.load_law(parent).unwrap();

        let result = service.evaluate_law_output(
            "orphan_parent",
            "orphan_output",
            HashMap::new(),
            "2025-01-01",
        );

        assert!(
            matches!(result, Err(EngineError::DelegationError(_))),
            "Expected DelegationError, got: {:?}",
            result
        );
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

    #[test]
    fn test_parse_regulatory_layer_helper() {
        assert_eq!(
            parse_regulatory_layer("ministeriele_regeling"),
            Some(RegulatoryLayer::MinisterieleRegeling)
        );
        assert_eq!(
            parse_regulatory_layer("MINISTERIELE_REGELING"),
            Some(RegulatoryLayer::MinisterieleRegeling)
        );
        assert_eq!(parse_regulatory_layer("wet"), Some(RegulatoryLayer::Wet));
        assert_eq!(parse_regulatory_layer("amvb"), Some(RegulatoryLayer::Amvb));
        assert_eq!(
            parse_regulatory_layer("gemeentelijke_verordening"),
            Some(RegulatoryLayer::GemeentelijkeVerordening)
        );
        assert_eq!(
            parse_regulatory_layer("beleidsregel"),
            Some(RegulatoryLayer::Beleidsregel)
        );
        assert_eq!(parse_regulatory_layer("unknown_type"), None);
    }

    // -------------------------------------------------------------------------
    // Integration Tests with Real Regulation Files
    // -------------------------------------------------------------------------

    mod integration {
        use super::*;
        use std::path::PathBuf;

        fn get_regulation_path() -> PathBuf {
            let manifest_dir = env!("CARGO_MANIFEST_DIR");
            PathBuf::from(manifest_dir)
                .join("..")
                .join("..")
                .join("regulation")
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
                    HashMap::new(),
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
                "Expected at least 10 laws loaded from regulation/nl, got {}",
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
            .evaluate_law_output("cross_law_consumer", "result", HashMap::new(), "2024-06-15")
            .unwrap();
        assert_eq!(
            result.outputs.get("result"),
            Some(&Value::Int(100)),
            "2024 reference date should resolve to v1 (100)"
        );

        // Reference date 2025-06-15 should use v2 (BASE_VALUE=200)
        let result = service
            .evaluate_law_output("cross_law_consumer", "result", HashMap::new(), "2025-06-15")
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
        let mut record = HashMap::new();
        record.insert("BSN".to_string(), Value::String("123".to_string()));
        record.insert("external_value".to_string(), Value::Int(42));

        service
            .register_dict_source("test_data", "BSN", vec![record])
            .unwrap();

        let mut params = HashMap::new();
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
        let mut record = HashMap::new();
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
                HashMap::new(),
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
        let mut record = HashMap::new();
        record.insert("BSN".to_string(), Value::String("123".to_string()));
        record.insert("external_value".to_string(), Value::Int(100));

        service
            .register_dict_source("test_data", "BSN", vec![record])
            .unwrap();

        // Pass external_value = 50 as parameter (should win)
        let mut params = HashMap::new();
        params.insert("BSN".to_string(), Value::String("123".to_string()));
        params.insert("external_value".to_string(), Value::Int(50));

        let result = service
            .evaluate_law_output("priority_test_law", "result", params, "2025-01-01")
            .unwrap();

        // Parameter value (50) should win over registry value (100)
        assert_eq!(result.outputs.get("result"), Some(&Value::Int(50)));
    }
}
