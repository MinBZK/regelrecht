//! Article execution engine
//!
//! Core engine for evaluating article-level machine_readable.execution sections.
//!
//! # Example
//!
//! ```ignore
//! use regelrecht_engine::{ArticleEngine, ArticleBasedLaw, Value};
//! use std::collections::HashMap;
//!
//! let law = ArticleBasedLaw::from_yaml_file("path/to/law.yaml")?;
//! let article = law.find_article_by_output("some_output").unwrap();
//!
//! let engine = ArticleEngine::new(article, &law);
//! let mut params = HashMap::new();
//! params.insert("BSN".to_string(), Value::String("123456789".to_string()));
//!
//! let result = engine.evaluate(params, "2025-01-01")?;
//! println!("Output: {:?}", result.outputs);
//! ```

use crate::article::{
    Action, ActionOperation, Article, ArticleBasedLaw, Delegation, Input, SelectOnCriteria,
};
use crate::config;
use crate::context::RuleContext;
use crate::error::{EngineError, Result};
use crate::operations::{evaluate_value, execute_operation, values_equal, ValueResolver};
use crate::trace::{PathNode, TraceBuilder};
use crate::types::{PathNodeType, Value};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

/// Result of article execution
#[derive(Debug, Clone)]
pub struct ArticleResult {
    /// Calculated output values
    pub outputs: HashMap<String, Value>,
    /// Resolved input values (from cross-law references)
    pub resolved_inputs: HashMap<String, Value>,
    /// Article number that was executed
    pub article_number: String,
    /// Law ID containing the article
    pub law_id: String,
    /// Law UUID if available
    pub law_uuid: Option<String>,
    /// Execution trace tree (only populated when tracing is enabled)
    pub trace: Option<PathNode>,
}

/// Executes a single article's machine_readable.execution section.
///
/// The engine orchestrates the execution of an article's actions,
/// resolving variables and evaluating operations to produce outputs.
pub struct ArticleEngine<'a> {
    /// Article to execute
    article: &'a Article,
    /// Law containing the article
    law: &'a ArticleBasedLaw,
}

impl<'a> ArticleEngine<'a> {
    /// Create a new article engine.
    ///
    /// # Arguments
    /// * `article` - Article to execute
    /// * `law` - Law containing the article
    pub fn new(article: &'a Article, law: &'a ArticleBasedLaw) -> Self {
        Self { article, law }
    }

    /// Execute this article's logic.
    ///
    /// # Arguments
    /// * `parameters` - Input parameters (e.g., {"BSN": "123456789"})
    /// * `calculation_date` - Date for which calculations are performed (YYYY-MM-DD)
    ///
    /// # Returns
    /// * `Ok(ArticleResult)` - Execution result with outputs and metadata
    /// * `Err(EngineError)` - If execution fails
    pub fn evaluate(
        &self,
        parameters: HashMap<String, Value>,
        calculation_date: &str,
    ) -> Result<ArticleResult> {
        self.evaluate_with_output(parameters, calculation_date, None)
    }

    /// Execute this article's logic, optionally calculating only a specific output.
    ///
    /// # Arguments
    /// * `parameters` - Input parameters (e.g., {"BSN": "123456789"})
    /// * `calculation_date` - Date for which calculations are performed (YYYY-MM-DD)
    /// * `requested_output` - Specific output to calculate (optional, calculates all if None)
    ///
    /// # Returns
    /// * `Ok(ArticleResult)` - Execution result with outputs and metadata
    /// * `Err(EngineError)` - If execution fails
    pub fn evaluate_with_output(
        &self,
        parameters: HashMap<String, Value>,
        calculation_date: &str,
        requested_output: Option<&str>,
    ) -> Result<ArticleResult> {
        // Initialize visited set with current article to detect circular references
        let mut visited = HashSet::new();
        visited.insert(self.article.number.clone());

        self.evaluate_internal(parameters, calculation_date, requested_output, visited, 0)
    }

    /// Execute this article's logic with trace support.
    ///
    /// Same as `evaluate_with_output` but accepts a shared trace builder.
    pub fn evaluate_with_trace(
        &self,
        parameters: HashMap<String, Value>,
        calculation_date: &str,
        requested_output: Option<&str>,
        trace: Rc<RefCell<TraceBuilder>>,
    ) -> Result<ArticleResult> {
        let mut visited = HashSet::new();
        visited.insert(self.article.number.clone());

        self.evaluate_internal_traced(
            parameters,
            calculation_date,
            requested_output,
            visited,
            0,
            Some(trace),
        )
    }

    /// Internal evaluation method that tracks visited articles for circular reference detection.
    ///
    /// # Arguments
    /// * `parameters` - Input parameters
    /// * `calculation_date` - Date for calculations
    /// * `requested_output` - Specific output to calculate (optional)
    /// * `visited` - Set of article numbers already in the resolution chain
    /// * `depth` - Current resolution depth
    fn evaluate_internal(
        &self,
        parameters: HashMap<String, Value>,
        calculation_date: &str,
        requested_output: Option<&str>,
        visited: HashSet<String>,
        depth: usize,
    ) -> Result<ArticleResult> {
        self.evaluate_internal_traced(
            parameters,
            calculation_date,
            requested_output,
            visited,
            depth,
            None,
        )
    }

    /// Internal evaluation with optional tracing.
    fn evaluate_internal_traced(
        &self,
        parameters: HashMap<String, Value>,
        calculation_date: &str,
        requested_output: Option<&str>,
        visited: HashSet<String>,
        depth: usize,
        trace: Option<Rc<RefCell<TraceBuilder>>>,
    ) -> Result<ArticleResult> {
        tracing::debug!(
            law_id = %self.law.id,
            article = %self.article.number,
            depth = depth,
            requested_output = ?requested_output,
            "Starting article evaluation"
        );

        // Check depth limit
        if depth > config::MAX_RESOLUTION_DEPTH {
            tracing::warn!(
                law_id = %self.law.id,
                article = %self.article.number,
                depth = depth,
                "Resolution depth exceeded"
            );
            return Err(EngineError::CircularReference(format!(
                "Resolution depth exceeded {} levels. Possible circular reference involving article '{}'",
                config::MAX_RESOLUTION_DEPTH, self.article.number
            )));
        }

        // Create execution context
        let mut context = RuleContext::new(parameters.clone(), calculation_date)?;

        // Attach trace builder if provided
        if let Some(ref tb) = trace {
            context.set_trace(Rc::clone(tb));
        }

        // Set definitions from article
        if let Some(definitions) = self.article.get_definitions() {
            context.set_definitions(definitions);
        }

        // Resolve inputs with sources (internal references)
        self.resolve_input_sources(&mut context, &parameters, calculation_date, &visited, depth)?;

        // Execute actions (with trace instrumentation)
        self.execute_actions_traced(&mut context, requested_output)?;

        // Build result
        let result = ArticleResult {
            outputs: context.outputs().clone(),
            resolved_inputs: context.resolved_inputs().clone(),
            article_number: self.article.number.clone(),
            law_id: self.law.id.clone(),
            law_uuid: self.law.uuid.clone(),
            trace: None, // Trace is extracted by the caller (service layer)
        };

        tracing::debug!(
            law_id = %self.law.id,
            article = %self.article.number,
            outputs = ?result.outputs.keys().collect::<Vec<_>>(),
            "Article evaluation completed"
        );

        Ok(result)
    }

    /// Resolve input sources (internal and external references).
    ///
    /// This processes inputs that have a `source` specification and resolves them
    /// before action execution. Internal references (same law) are resolved directly.
    /// External references require a ServiceProvider (Phase 7).
    ///
    /// # Arguments
    /// * `context` - Execution context
    /// * `parameters` - Input parameters
    /// * `calculation_date` - Date for calculations
    /// * `visited` - Set of article numbers already in the resolution chain
    /// * `depth` - Current resolution depth
    fn resolve_input_sources(
        &self,
        context: &mut RuleContext,
        parameters: &HashMap<String, Value>,
        calculation_date: &str,
        visited: &HashSet<String>,
        depth: usize,
    ) -> Result<()> {
        let inputs = self.get_inputs();

        for input in inputs {
            let source = match &input.source {
                Some(s) => s,
                None => continue, // No source, skip
            };

            // Determine the type of reference:
            // 1. Delegation: source.delegation is set (complex cross-law with select_on)
            // 2. External: source.regulation is set (simple cross-law reference)
            // 3. Internal: neither is set (same-law reference)

            if let Some(delegation) = &source.delegation {
                // Delegation reference: requires ServiceProvider to find matching regulation
                // Check if value was pre-resolved via parameters
                if parameters.contains_key(&input.name) {
                    // Already resolved, use the parameter value
                    continue;
                }

                // Format select_on criteria for error message
                let criteria_desc = delegation
                    .select_on
                    .as_ref()
                    .map(|criteria| {
                        criteria
                            .iter()
                            .map(|c| c.name.clone())
                            .collect::<Vec<_>>()
                            .join(", ")
                    })
                    .unwrap_or_else(|| "none".to_string());

                return Err(EngineError::DelegationNotResolved {
                    input_name: input.name.clone(),
                    law_id: delegation.law_id.clone(),
                    article: delegation.article.clone(),
                    select_on: criteria_desc,
                });
            } else if let Some(regulation) = &source.regulation {
                // External reference: requires ServiceProvider
                // Check if value was pre-resolved via parameters
                if parameters.contains_key(&input.name) {
                    continue;
                }

                return Err(EngineError::ExternalReferenceNotResolved {
                    input_name: input.name.clone(),
                    regulation: regulation.clone(),
                    output: source.output.clone().unwrap_or_default(),
                });
            } else if let Some(output_name) = &source.output {
                // Internal reference: resolve within the same law.
                // Check if already resolved by service layer.
                if parameters.contains_key(&input.name) {
                    continue;
                }
                let value = self.resolve_internal_reference(
                    output_name,
                    parameters,
                    calculation_date,
                    visited,
                    depth,
                )?;
                context.set_resolved_input(&input.name, value);
            } else {
                // Empty source (source: {}) — resolved by DataSourceRegistry in service layer.
                // Check if already provided as parameter.
                if parameters.contains_key(&input.name) {
                    continue;
                }
                // Otherwise leave unresolved — the service layer will handle it.
            }
        }

        Ok(())
    }

    /// Resolve an internal reference (within the same law).
    ///
    /// Finds the article that produces the given output and executes it.
    /// Tracks visited articles to detect indirect circular references (A→B→A).
    ///
    /// # Arguments
    /// * `output_name` - Name of the output to resolve
    /// * `parameters` - Input parameters
    /// * `calculation_date` - Date for calculations
    /// * `visited` - Set of article numbers already in the resolution chain
    /// * `depth` - Current resolution depth
    fn resolve_internal_reference(
        &self,
        output_name: &str,
        parameters: &HashMap<String, Value>,
        calculation_date: &str,
        visited: &HashSet<String>,
        depth: usize,
    ) -> Result<Value> {
        // Find the article that produces this output
        let article = self
            .law
            .find_article_by_output(output_name)
            .ok_or_else(|| EngineError::OutputNotFound {
                law_id: self.law.id.clone(),
                output: output_name.to_string(),
            })?;

        // Check for circular reference (direct or indirect)
        // visited already contains the current article (and all its callers)
        if visited.contains(&article.number) {
            return Err(EngineError::CircularReference(format!(
                "Circular reference detected: article '{}' references output '{}' from article '{}', \
                 which is already in the resolution chain: {:?}",
                self.article.number, output_name, article.number, visited
            )));
        }

        // Add the target article to visited set for the recursive call
        let mut new_visited = visited.clone();
        new_visited.insert(article.number.clone());

        // Execute the referenced article with updated visited set
        let engine = ArticleEngine::new(article, self.law);
        let result = engine.evaluate_internal(
            parameters.clone(),
            calculation_date,
            Some(output_name),
            new_visited,
            depth + 1,
        )?;

        // Extract the requested output
        result
            .outputs
            .get(output_name)
            .cloned()
            .ok_or_else(|| EngineError::OutputNotFound {
                law_id: self.law.id.clone(),
                output: output_name.to_string(),
            })
    }

    /// Execute all actions in order, with optional trace instrumentation.
    fn execute_actions_traced(
        &self,
        context: &mut RuleContext,
        _requested_output: Option<&str>,
    ) -> Result<()> {
        let actions = self.get_actions();
        let tracing_active = context.has_trace();

        for action in actions {
            let output_name = match &action.output {
                Some(name) => name,
                None => continue,
            };

            if tracing_active {
                context.trace_push(output_name, PathNodeType::Action);
                context.trace_set_message(format!("Computing {}", output_name));
            }

            let value = match self.evaluate_action(action, context) {
                Ok(v) => v,
                Err(e) => {
                    if tracing_active {
                        context.trace_set_message(format!("Action failed: {}", e));
                        context.trace_pop();
                    }
                    return Err(e);
                }
            };

            if tracing_active {
                context.trace_set_result(value.clone());
            }

            tracing::debug!("Output {} = {}", output_name, value);
            context.set_output(output_name, value.clone());

            if tracing_active {
                context.trace_pop();
            }
        }

        Ok(())
    }

    /// Evaluate a single action.
    ///
    /// # Arguments
    /// * `action` - Action specification
    /// * `context` - Execution context
    ///
    /// # Returns
    /// Calculated value
    fn evaluate_action(&self, action: &Action, context: &RuleContext) -> Result<Value> {
        // Check for operation at action level FIRST
        // When an action has an operation, the value/subject fields are operands, not direct results
        if let Some(operation) = &action.operation {
            let action_op = self.action_to_operation(action, operation)?;
            return execute_operation(&action_op, context, 0);
        }

        // Check for direct value (only when no operation is specified)
        if let Some(value) = &action.value {
            return evaluate_value(value, context, 0);
        }

        // Check for resolve (delegation to child regulations)
        // Resolve actions are pre-resolved by the service layer and injected as parameters.
        // If the output was pre-resolved, use that value. Otherwise, error.
        if let Some(resolve) = &action.resolve {
            // Check if pre-resolved by service layer (available via context parameters)
            if let Some(output_name) = &action.output {
                if let Ok(value) = context.resolve(output_name) {
                    return Ok(value);
                }
            }

            let match_desc = resolve
                .match_spec
                .as_ref()
                .map(|m| format!("output={}, value=<expr>", m.output))
                .unwrap_or_else(|| "none".to_string());

            return Err(EngineError::DelegationError(format!(
                "Resolve action requires ServiceProvider: type={}, output={}, match=[{}]. \
                 Use LawExecutionService to automatically resolve, or pass the value as a parameter.",
                resolve.resolve_type, resolve.output, match_desc
            )));
        }

        // No value or operation specified
        Ok(Value::Null)
    }

    /// Convert an Action to an ActionOperation for execution.
    ///
    /// This is needed because actions can have operations specified inline
    /// rather than as nested ActionValue::Operation.
    ///
    /// # Limitations
    ///
    /// SWITCH operations at the action level are NOT supported because the
    /// `Action` struct doesn't have `cases` and `default` fields. SWITCH must
    /// be nested inside `value` as an `ActionValue::Operation`. Example:
    ///
    /// ```yaml
    /// # INCORRECT - won't work:
    /// - output: result
    ///   operation: SWITCH
    ///   cases: [...]  # Action doesn't have this field
    ///
    /// # CORRECT - use value wrapper:
    /// - output: result
    ///   value:
    ///     operation: SWITCH
    ///     cases: [...]
    /// ```
    fn action_to_operation(
        &self,
        action: &Action,
        operation: &crate::types::Operation,
    ) -> Result<ActionOperation> {
        Ok(ActionOperation {
            operation: *operation, // Operation is Copy
            subject: action.subject.clone(),
            value: action.value.clone(),
            values: action.values.clone(),
            when: action.when.clone(),
            then: action.then.clone(),
            else_branch: action.else_branch.clone(),
            conditions: action.conditions.clone(),
            // SWITCH-specific fields: Action struct doesn't have these,
            // so SWITCH must be nested inside `value` to work correctly
            cases: None,
            default: None,
            // Unit for SUBTRACT_DATE
            unit: action.unit.clone(),
        })
    }

    /// Get actions from the article's execution spec.
    fn get_actions(&self) -> &[Action] {
        self.article
            .get_execution_spec()
            .and_then(|exec| exec.actions.as_deref())
            .unwrap_or(&[])
    }

    /// Get inputs from the article's execution spec.
    fn get_inputs(&self) -> &[Input] {
        self.article
            .get_execution_spec()
            .and_then(|exec| exec.input.as_deref())
            .unwrap_or(&[])
    }
}

// =============================================================================
// Delegation Helper Functions
// =============================================================================

/// Evaluate select_on criteria to concrete values.
///
/// Used by ServiceProvider to match delegation criteria against candidate regulations.
/// This function evaluates variable references in the criteria values.
///
/// # Arguments
/// * `criteria` - List of select_on criteria
/// * `context` - Execution context with variables
///
/// # Returns
/// HashMap mapping criteria names to their evaluated values.
///
/// # Example
///
/// Given:
/// ```yaml
/// select_on:
///   - name: gemeente_code
///     value: $gemeente_code
/// ```
///
/// And context with `gemeente_code = "0363"`, returns:
/// ```text
/// { "gemeente_code" => Value::String("0363") }
/// ```
pub fn evaluate_select_on_criteria(
    criteria: &[SelectOnCriteria],
    context: &RuleContext,
) -> Result<HashMap<String, Value>> {
    let mut result = HashMap::new();

    for criterion in criteria {
        let value = evaluate_value(&criterion.value, context, 0)?;
        result.insert(criterion.name.clone(), value);
    }

    Ok(result)
}

/// Check if a regulation matches delegation criteria.
///
/// Compares the evaluated select_on criteria against a regulation's metadata
/// or output values. Used by ServiceProvider to find matching regulations.
///
/// # Arguments
/// * `criteria_values` - Evaluated criteria from `evaluate_select_on_criteria`
/// * `regulation_values` - Values from the candidate regulation
///
/// # Returns
/// `true` if all criteria match, `false` otherwise.
pub fn matches_delegation_criteria(
    criteria_values: &HashMap<String, Value>,
    regulation_values: &HashMap<String, Value>,
) -> bool {
    for (name, expected_value) in criteria_values {
        match regulation_values.get(name) {
            Some(actual_value) if values_equal(expected_value, actual_value) => continue,
            _ => return false,
        }
    }
    true
}

/// Extract delegation info from a source specification.
///
/// Returns references to delegation details for efficient access.
/// Used by ServiceProvider to match delegation criteria against candidate regulations.
pub fn get_delegation_info(delegation: &Delegation) -> (&str, &str, Option<&[SelectOnCriteria]>) {
    (
        &delegation.law_id,
        &delegation.article,
        delegation.select_on.as_deref(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::article::ArticleBasedLaw;

    fn make_simple_law() -> ArticleBasedLaw {
        let yaml = r#"
$id: test_law
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Test article
    machine_readable:
      definitions:
        MAX_AGE:
          value: 67
        MIN_AGE:
          value: 18
      execution:
        parameters:
          - name: age
            type: number
            required: true
        output:
          - name: is_adult
            type: boolean
          - name: age_check_result
            type: string
        actions:
          - output: is_adult
            operation: GREATER_THAN_OR_EQUAL
            subject: $age
            value: $MIN_AGE
          - output: age_check_result
            value:
              operation: IF
              when:
                operation: GREATER_THAN_OR_EQUAL
                subject: $age
                value: $MIN_AGE
              then: "adult"
              else: "minor"
"#;
        ArticleBasedLaw::from_yaml_str(yaml).unwrap()
    }

    fn make_arithmetic_law() -> ArticleBasedLaw {
        let yaml = r#"
$id: calc_law
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Calculation article
    machine_readable:
      definitions:
        TAX_RATE:
          value: 0.21
        BASE_DEDUCTION:
          value: 1000
      execution:
        parameters:
          - name: income
            type: number
            required: true
        output:
          - name: taxable_income
            type: number
          - name: tax_amount
            type: number
        actions:
          - output: taxable_income
            operation: MAX
            values:
              - 0
              - operation: SUBTRACT
                values:
                  - $income
                  - $BASE_DEDUCTION
          - output: tax_amount
            operation: MULTIPLY
            values:
              - $taxable_income
              - $TAX_RATE
"#;
        ArticleBasedLaw::from_yaml_str(yaml).unwrap()
    }

    // -------------------------------------------------------------------------
    // Basic Execution Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_evaluate_simple_comparison() {
        let law = make_simple_law();
        let article = law.find_article_by_number("1").unwrap();
        let engine = ArticleEngine::new(article, &law);

        let mut params = HashMap::new();
        params.insert("age".to_string(), Value::Int(25));

        let result = engine.evaluate(params, "2025-01-01").unwrap();

        assert_eq!(result.article_number, "1");
        assert_eq!(result.law_id, "test_law");
        assert_eq!(result.outputs.get("is_adult"), Some(&Value::Bool(true)));
    }

    #[test]
    fn test_evaluate_with_definitions() {
        let law = make_simple_law();
        let article = law.find_article_by_number("1").unwrap();
        let engine = ArticleEngine::new(article, &law);

        // Age 15 is less than MIN_AGE (18)
        let mut params = HashMap::new();
        params.insert("age".to_string(), Value::Int(15));

        let result = engine.evaluate(params, "2025-01-01").unwrap();

        assert_eq!(result.outputs.get("is_adult"), Some(&Value::Bool(false)));
    }

    #[test]
    fn test_evaluate_nested_if() {
        let law = make_simple_law();
        let article = law.find_article_by_number("1").unwrap();
        let engine = ArticleEngine::new(article, &law);

        // Adult case
        let mut params = HashMap::new();
        params.insert("age".to_string(), Value::Int(25));
        let result = engine.evaluate(params, "2025-01-01").unwrap();
        assert_eq!(
            result.outputs.get("age_check_result"),
            Some(&Value::String("adult".to_string()))
        );

        // Minor case
        let mut params = HashMap::new();
        params.insert("age".to_string(), Value::Int(15));
        let result = engine.evaluate(params, "2025-01-01").unwrap();
        assert_eq!(
            result.outputs.get("age_check_result"),
            Some(&Value::String("minor".to_string()))
        );
    }

    // -------------------------------------------------------------------------
    // Arithmetic Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_evaluate_arithmetic_operations() {
        let law = make_arithmetic_law();
        let article = law.find_article_by_number("1").unwrap();
        let engine = ArticleEngine::new(article, &law);

        let mut params = HashMap::new();
        params.insert("income".to_string(), Value::Int(5000));

        let result = engine.evaluate(params, "2025-01-01").unwrap();

        // taxable_income = MAX(0, 5000 - 1000) = 4000
        assert_eq!(
            result.outputs.get("taxable_income"),
            Some(&Value::Int(4000))
        );

        // tax_amount = 4000 * 0.21 = 840.0
        assert_eq!(result.outputs.get("tax_amount"), Some(&Value::Float(840.0)));
    }

    #[test]
    fn test_evaluate_arithmetic_with_floor() {
        let law = make_arithmetic_law();
        let article = law.find_article_by_number("1").unwrap();
        let engine = ArticleEngine::new(article, &law);

        // Income below deduction
        let mut params = HashMap::new();
        params.insert("income".to_string(), Value::Int(500));

        let result = engine.evaluate(params, "2025-01-01").unwrap();

        // taxable_income = MAX(0, 500 - 1000) = MAX(0, -500) = 0
        assert_eq!(result.outputs.get("taxable_income"), Some(&Value::Int(0)));

        // tax_amount = 0 * 0.21 = 0.0
        assert_eq!(result.outputs.get("tax_amount"), Some(&Value::Float(0.0)));
    }

    // -------------------------------------------------------------------------
    // Selective Output Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_evaluate_specific_output() {
        let law = make_simple_law();
        let article = law.find_article_by_number("1").unwrap();
        let engine = ArticleEngine::new(article, &law);

        let mut params = HashMap::new();
        params.insert("age".to_string(), Value::Int(25));

        // Request specific output (used for article lookup)
        let result = engine
            .evaluate_with_output(params, "2025-01-01", Some("is_adult"))
            .unwrap();

        // All outputs are calculated (matches Python behavior)
        // Later actions may depend on earlier outputs
        assert!(result.outputs.contains_key("is_adult"));
        assert!(result.outputs.contains_key("age_check_result"));
    }

    // -------------------------------------------------------------------------
    // Error Handling Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_missing_parameter() {
        let law = make_simple_law();
        let article = law.find_article_by_number("1").unwrap();
        let engine = ArticleEngine::new(article, &law);

        // No parameters - age is missing
        let params = HashMap::new();
        let result = engine.evaluate(params, "2025-01-01");

        assert!(matches!(result, Err(EngineError::VariableNotFound(_))));
    }

    #[test]
    fn test_invalid_date() {
        let law = make_simple_law();
        let article = law.find_article_by_number("1").unwrap();
        let engine = ArticleEngine::new(article, &law);

        let mut params = HashMap::new();
        params.insert("age".to_string(), Value::Int(25));

        let result = engine.evaluate(params, "not-a-date");
        assert!(matches!(result, Err(EngineError::InvalidDate(_))));
    }

    // -------------------------------------------------------------------------
    // Reference Date Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_reference_date_accessible() {
        let yaml = r#"
$id: date_law
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Date test
    machine_readable:
      execution:
        output:
          - name: current_year
            type: number
        actions:
          - output: current_year
            value: $referencedate.year
"#;
        let law = ArticleBasedLaw::from_yaml_str(yaml).unwrap();
        let article = law.find_article_by_number("1").unwrap();
        let engine = ArticleEngine::new(article, &law);

        let result = engine.evaluate(HashMap::new(), "2025-06-15").unwrap();

        assert_eq!(result.outputs.get("current_year"), Some(&Value::Int(2025)));
    }

    // -------------------------------------------------------------------------
    // Internal Reference Resolution Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_internal_reference_basic() {
        // Law with two articles: article 2 references output from article 1
        let yaml = r#"
$id: internal_ref_law
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Base calculation article
    machine_readable:
      definitions:
        BASE_VALUE:
          value: 100
      execution:
        output:
          - name: base_amount
            type: number
        actions:
          - output: base_amount
            value: $BASE_VALUE
  - number: '2'
    text: Derived calculation using internal reference
    machine_readable:
      execution:
        input:
          - name: base_amount
            type: number
            source:
              output: base_amount
        output:
          - name: doubled_amount
            type: number
        actions:
          - output: doubled_amount
            operation: MULTIPLY
            values:
              - $base_amount
              - 2
"#;
        let law = ArticleBasedLaw::from_yaml_str(yaml).unwrap();
        let article = law.find_article_by_number("2").unwrap();
        let engine = ArticleEngine::new(article, &law);

        let result = engine.evaluate(HashMap::new(), "2025-01-01").unwrap();

        // doubled_amount should be 100 * 2 = 200
        assert_eq!(result.outputs.get("doubled_amount"), Some(&Value::Int(200)));
        // base_amount should be in resolved_inputs
        assert_eq!(
            result.resolved_inputs.get("base_amount"),
            Some(&Value::Int(100))
        );
    }

    #[test]
    fn test_internal_reference_chain() {
        // Law with three articles: 3 -> 2 -> 1
        let yaml = r#"
$id: chain_law
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: First article
    machine_readable:
      execution:
        output:
          - name: first_value
            type: number
        actions:
          - output: first_value
            value: 10
  - number: '2'
    text: Second article references first
    machine_readable:
      execution:
        input:
          - name: first_value
            type: number
            source:
              output: first_value
        output:
          - name: second_value
            type: number
        actions:
          - output: second_value
            operation: ADD
            values:
              - $first_value
              - 5
  - number: '3'
    text: Third article references second
    machine_readable:
      execution:
        input:
          - name: second_value
            type: number
            source:
              output: second_value
        output:
          - name: third_value
            type: number
        actions:
          - output: third_value
            operation: MULTIPLY
            values:
              - $second_value
              - 2
"#;
        let law = ArticleBasedLaw::from_yaml_str(yaml).unwrap();
        let article = law.find_article_by_number("3").unwrap();
        let engine = ArticleEngine::new(article, &law);

        let result = engine.evaluate(HashMap::new(), "2025-01-01").unwrap();

        // Chain: first_value = 10, second_value = 10 + 5 = 15, third_value = 15 * 2 = 30
        assert_eq!(result.outputs.get("third_value"), Some(&Value::Int(30)));
    }

    #[test]
    fn test_internal_reference_with_parameters() {
        // Referenced article uses parameters passed through
        let yaml = r#"
$id: param_law
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Article that uses parameter
    machine_readable:
      execution:
        parameters:
          - name: multiplier
            type: number
            required: true
        output:
          - name: base_result
            type: number
        actions:
          - output: base_result
            operation: MULTIPLY
            values:
              - 100
              - $multiplier
  - number: '2'
    text: Article that references article 1
    machine_readable:
      execution:
        parameters:
          - name: multiplier
            type: number
            required: true
        input:
          - name: base_result
            type: number
            source:
              output: base_result
        output:
          - name: final_result
            type: number
        actions:
          - output: final_result
            operation: ADD
            values:
              - $base_result
              - 50
"#;
        let law = ArticleBasedLaw::from_yaml_str(yaml).unwrap();
        let article = law.find_article_by_number("2").unwrap();
        let engine = ArticleEngine::new(article, &law);

        let mut params = HashMap::new();
        params.insert("multiplier".to_string(), Value::Int(3));

        let result = engine.evaluate(params, "2025-01-01").unwrap();

        // base_result = 100 * 3 = 300, final_result = 300 + 50 = 350
        assert_eq!(result.outputs.get("final_result"), Some(&Value::Int(350)));
    }

    #[test]
    fn test_circular_reference_detection() {
        // Article that references itself should fail
        let yaml = r#"
$id: circular_law
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Self-referencing article
    machine_readable:
      execution:
        input:
          - name: my_output
            type: number
            source:
              output: my_output
        output:
          - name: my_output
            type: number
        actions:
          - output: my_output
            value: $my_output
"#;
        let law = ArticleBasedLaw::from_yaml_str(yaml).unwrap();
        let article = law.find_article_by_number("1").unwrap();
        let engine = ArticleEngine::new(article, &law);

        let result = engine.evaluate(HashMap::new(), "2025-01-01");

        assert!(matches!(result, Err(EngineError::CircularReference(_))));
    }

    #[test]
    fn test_indirect_circular_reference_detection() {
        // Indirect circular reference: A → B → A should fail
        let yaml = r#"
$id: indirect_circular_law
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Article A references B
    machine_readable:
      execution:
        input:
          - name: value_from_b
            type: number
            source:
              output: output_b
        output:
          - name: output_a
            type: number
        actions:
          - output: output_a
            operation: ADD
            values:
              - $value_from_b
              - 1
  - number: '2'
    text: Article B references A (creates cycle)
    machine_readable:
      execution:
        input:
          - name: value_from_a
            type: number
            source:
              output: output_a
        output:
          - name: output_b
            type: number
        actions:
          - output: output_b
            operation: ADD
            values:
              - $value_from_a
              - 2
"#;
        let law = ArticleBasedLaw::from_yaml_str(yaml).unwrap();

        // Starting from article 1: A → B → A (cycle detected)
        let article = law.find_article_by_number("1").unwrap();
        let engine = ArticleEngine::new(article, &law);

        let result = engine.evaluate(HashMap::new(), "2025-01-01");

        assert!(
            matches!(result, Err(EngineError::CircularReference(_))),
            "Expected CircularReference error for A→B→A cycle, got: {:?}",
            result
        );
    }

    #[test]
    fn test_three_way_circular_reference() {
        // Three-way circular reference: A → B → C → A should fail
        let yaml = r#"
$id: three_way_circular_law
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Article A references B
    machine_readable:
      execution:
        input:
          - name: from_b
            type: number
            source:
              output: output_b
        output:
          - name: output_a
            type: number
        actions:
          - output: output_a
            value: $from_b
  - number: '2'
    text: Article B references C
    machine_readable:
      execution:
        input:
          - name: from_c
            type: number
            source:
              output: output_c
        output:
          - name: output_b
            type: number
        actions:
          - output: output_b
            value: $from_c
  - number: '3'
    text: Article C references A (completes cycle)
    machine_readable:
      execution:
        input:
          - name: from_a
            type: number
            source:
              output: output_a
        output:
          - name: output_c
            type: number
        actions:
          - output: output_c
            value: $from_a
"#;
        let law = ArticleBasedLaw::from_yaml_str(yaml).unwrap();

        // Starting from article 1: A → B → C → A (cycle detected)
        let article = law.find_article_by_number("1").unwrap();
        let engine = ArticleEngine::new(article, &law);

        let result = engine.evaluate(HashMap::new(), "2025-01-01");

        assert!(
            matches!(result, Err(EngineError::CircularReference(_))),
            "Expected CircularReference error for A→B→C→A cycle, got: {:?}",
            result
        );
    }

    #[test]
    fn test_external_reference_error() {
        // External reference (with regulation) should fail with helpful error
        let yaml = r#"
$id: external_ref_law
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Article with external reference
    machine_readable:
      execution:
        input:
          - name: external_value
            type: number
            source:
              regulation: other_law
              output: some_output
        output:
          - name: result
            type: number
        actions:
          - output: result
            value: $external_value
"#;
        let law = ArticleBasedLaw::from_yaml_str(yaml).unwrap();
        let article = law.find_article_by_number("1").unwrap();
        let engine = ArticleEngine::new(article, &law);

        let result = engine.evaluate(HashMap::new(), "2025-01-01");

        assert!(
            matches!(
                result,
                Err(EngineError::ExternalReferenceNotResolved { .. })
            ),
            "Expected ExternalReferenceNotResolved error, got: {:?}",
            result
        );
        if let Err(EngineError::ExternalReferenceNotResolved {
            input_name,
            regulation,
            output,
        }) = result
        {
            assert_eq!(input_name, "external_value");
            assert_eq!(regulation, "other_law");
            assert_eq!(output, "some_output");
        }
    }

    // -------------------------------------------------------------------------
    // Delegation Tests
    // -------------------------------------------------------------------------

    mod delegation {
        use super::*;

        #[test]
        fn test_delegation_source_error() {
            // Delegation source should fail with DelegationNotResolved error
            let yaml = r#"
$id: delegation_law
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Article with delegation source
    machine_readable:
      execution:
        parameters:
          - name: gemeente_code
            type: string
            required: true
        input:
          - name: delegated_value
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
          - name: result
            type: number
        actions:
          - output: result
            value: $delegated_value
"#;
            let law = ArticleBasedLaw::from_yaml_str(yaml).unwrap();
            let article = law.find_article_by_number("1").unwrap();
            let engine = ArticleEngine::new(article, &law);

            let mut params = HashMap::new();
            params.insert(
                "gemeente_code".to_string(),
                Value::String("0363".to_string()),
            );

            let result = engine.evaluate(params, "2025-01-01");

            assert!(
                matches!(result, Err(EngineError::DelegationNotResolved { .. })),
                "Expected DelegationNotResolved error, got: {:?}",
                result
            );

            if let Err(EngineError::DelegationNotResolved {
                input_name,
                law_id,
                article,
                select_on,
            }) = result
            {
                assert_eq!(input_name, "delegated_value");
                assert_eq!(law_id, "participatiewet");
                assert_eq!(article, "8");
                assert!(select_on.contains("gemeente_code"));
            }
        }

        #[test]
        fn test_delegation_with_preresolved_parameter() {
            // Delegation source should succeed if value is pre-resolved via parameters
            let yaml = r#"
$id: delegation_law
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Article with delegation source
    machine_readable:
      execution:
        parameters:
          - name: gemeente_code
            type: string
          - name: delegated_value
            type: number
        input:
          - name: delegated_value
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
          - name: result
            type: number
        actions:
          - output: result
            operation: MULTIPLY
            values:
              - $delegated_value
              - 2
"#;
            let law = ArticleBasedLaw::from_yaml_str(yaml).unwrap();
            let article = law.find_article_by_number("1").unwrap();
            let engine = ArticleEngine::new(article, &law);

            let mut params = HashMap::new();
            params.insert(
                "gemeente_code".to_string(),
                Value::String("0363".to_string()),
            );
            params.insert("delegated_value".to_string(), Value::Int(50)); // Pre-resolved

            let result = engine.evaluate(params, "2025-01-01").unwrap();

            // Should use pre-resolved value: 50 * 2 = 100
            assert_eq!(result.outputs.get("result"), Some(&Value::Int(100)));
        }

        #[test]
        fn test_resolve_action_error() {
            // Resolve action should fail with DelegationError
            let yaml = r#"
$id: resolve_law
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
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
"#;
            let law = ArticleBasedLaw::from_yaml_str(yaml).unwrap();
            let article = law.find_article_by_number("1").unwrap();
            let engine = ArticleEngine::new(article, &law);

            let result = engine.evaluate(HashMap::new(), "2025-01-01");

            assert!(
                matches!(result, Err(EngineError::DelegationError(_))),
                "Expected DelegationError, got: {:?}",
                result
            );

            if let Err(EngineError::DelegationError(msg)) = result {
                assert!(msg.contains("ServiceProvider"));
                assert!(msg.contains("ministeriele_regeling"));
            }
        }

        #[test]
        fn test_evaluate_select_on_criteria() {
            use crate::article::{ActionValue, SelectOnCriteria};
            use crate::types::Value;

            let criteria = vec![
                SelectOnCriteria {
                    name: "gemeente_code".to_string(),
                    value: ActionValue::Literal(Value::String("$gemeente_code".to_string())),
                },
                SelectOnCriteria {
                    name: "year".to_string(),
                    value: ActionValue::Literal(Value::Int(2025)),
                },
            ];

            let mut params = HashMap::new();
            params.insert(
                "gemeente_code".to_string(),
                Value::String("0363".to_string()),
            );

            let context = RuleContext::new(params, "2025-01-01").unwrap();
            let result = evaluate_select_on_criteria(&criteria, &context).unwrap();

            // First criterion is a variable reference, should resolve to "0363"
            assert_eq!(
                result.get("gemeente_code"),
                Some(&Value::String("0363".to_string()))
            );

            // Second criterion is a literal, should be 2025
            assert_eq!(result.get("year"), Some(&Value::Int(2025)));
        }

        #[test]
        fn test_matches_delegation_criteria() {
            let mut criteria = HashMap::new();
            criteria.insert(
                "gemeente_code".to_string(),
                Value::String("0363".to_string()),
            );
            criteria.insert("year".to_string(), Value::Int(2025));

            // Exact match
            let mut regulation_values = HashMap::new();
            regulation_values.insert(
                "gemeente_code".to_string(),
                Value::String("0363".to_string()),
            );
            regulation_values.insert("year".to_string(), Value::Int(2025));
            assert!(matches_delegation_criteria(&criteria, &regulation_values));

            // Mismatch on gemeente_code
            regulation_values.insert(
                "gemeente_code".to_string(),
                Value::String("0518".to_string()),
            );
            assert!(!matches_delegation_criteria(&criteria, &regulation_values));

            // Missing field
            regulation_values.remove("year");
            regulation_values.insert(
                "gemeente_code".to_string(),
                Value::String("0363".to_string()),
            );
            assert!(!matches_delegation_criteria(&criteria, &regulation_values));

            // Int/Float equality
            let mut float_criteria = HashMap::new();
            float_criteria.insert("amount".to_string(), Value::Int(100));
            let mut float_values = HashMap::new();
            float_values.insert("amount".to_string(), Value::Float(100.0));
            assert!(matches_delegation_criteria(&float_criteria, &float_values));
        }

        #[test]
        fn test_parse_participatiewet_ioc() {
            // Test that participatiewet uses IoC: article 8 has open_terms,
            // article 43 references article 8 via source.output
            use std::path::PathBuf;

            let manifest_dir = env!("CARGO_MANIFEST_DIR");
            let path = PathBuf::from(manifest_dir)
                .join("..")
                .join("..")
                .join("regulation")
                .join("nl/wet/participatiewet/2022-03-15.yaml");

            let law = ArticleBasedLaw::from_yaml_file(&path).unwrap();

            // Article 8 should declare open_terms
            let article8 = law.find_article_by_number("8").unwrap();
            let mr = article8.machine_readable.as_ref().unwrap();
            let open_terms = mr.open_terms.as_ref().unwrap();
            assert_eq!(open_terms.len(), 2);
            assert_eq!(open_terms[0].id, "verlaging_percentage");
            assert_eq!(open_terms[1].id, "duur_maanden");

            // Article 43 should reference article 8 via source.output
            let article43 = law.find_article_by_number("43").unwrap();
            let exec = article43.get_execution_spec().unwrap();
            let inputs = exec.input.as_ref().unwrap();
            let input = inputs
                .iter()
                .find(|i| i.name == "verlaging_percentage_uit_verordening")
                .unwrap();
            let source = input.source.as_ref().unwrap();
            assert!(source.delegation.is_none(), "Should not use delegation");
            assert_eq!(source.output.as_deref(), Some("verlaging_percentage"));
        }
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
        fn test_execute_zorgtoeslagwet_vermogen_check() {
            let path = get_regulation_path().join("nl/wet/wet_op_de_zorgtoeslag/2025-01-01.yaml");
            let law = ArticleBasedLaw::from_yaml_file(&path).unwrap();

            // Find article that calculates vermogen_onder_grens
            let article = law.find_article_by_output("vermogen_onder_grens");
            assert!(
                article.is_some(),
                "Should find vermogen_onder_grens article"
            );

            let article = article.unwrap();
            let engine = ArticleEngine::new(article, &law);

            // Test with vermogen under threshold for single person
            // The article requires: vermogen, heeft_toeslagpartner
            // Thresholds: €161.329 single, €203.643 with partner
            let mut params = HashMap::new();
            params.insert("vermogen".to_string(), Value::Int(100000)); // €1000 in cents, well under €161.329
            params.insert("heeft_toeslagpartner".to_string(), Value::Bool(false));

            let result = engine.evaluate(params, "2025-01-01").unwrap();

            // Should have vermogen_onder_grens output
            assert!(result.outputs.contains_key("vermogen_onder_grens"));
            assert_eq!(
                result.outputs.get("vermogen_onder_grens"),
                Some(&Value::Bool(true))
            );
        }

        #[test]
        fn test_execute_regeling_standaardpremie() {
            let path = get_regulation_path()
                .join("nl/ministeriele_regeling/regeling_standaardpremie/2025-01-01.yaml");
            let law = ArticleBasedLaw::from_yaml_file(&path).unwrap();

            // Find article with standaardpremie output
            let article = law.find_article_by_output("standaardpremie");
            assert!(article.is_some(), "Should find standaardpremie article");

            let article = article.unwrap();
            let engine = ArticleEngine::new(article, &law);

            // Execute with minimal params
            let result = engine.evaluate(HashMap::new(), "2025-01-01").unwrap();

            // Should have standaardpremie output (211200 eurocent = €2112 for 2025)
            assert_eq!(
                result.outputs.get("standaardpremie"),
                Some(&Value::Int(211200))
            );
        }
    }
}
