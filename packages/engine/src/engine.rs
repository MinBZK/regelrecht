//! Article execution engine
//!
//! Core engine for evaluating article-level machine_readable.execution sections.
//!
//! # Example
//!
//! ```ignore
//! use regelrecht_engine::{ArticleEngine, ArticleBasedLaw, Value};
//! use std::collections::BTreeMap;
//!
//! let law = ArticleBasedLaw::from_yaml_file("path/to/law.yaml")?;
//! let article = law.find_article_by_output("some_output").unwrap();
//!
//! let engine = ArticleEngine::new(article, &law);
//! let mut params = BTreeMap::new();
//! params.insert("BSN".to_string(), Value::String("123456789".to_string()));
//!
//! let result = engine.evaluate(params, "2025-01-01")?;
//! println!("Output: {:?}", result.outputs);
//! ```

use crate::article::{Action, ActionOperation, Article, ArticleBasedLaw};
use crate::config;
use crate::context::RuleContext;
use crate::error::{EngineError, Result};
use crate::operations::{evaluate_value, execute_operation};
use crate::trace::{PathNode, TraceBuilder};
use crate::types::{PathNodeType, Value};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

/// Result of article execution
#[derive(Debug, Clone)]
pub struct ArticleResult {
    /// Calculated output values
    pub outputs: BTreeMap<String, Value>,
    /// Resolved input values (from cross-law references)
    pub resolved_inputs: BTreeMap<String, Value>,
    /// Article number that was executed
    pub article_number: String,
    /// Law ID containing the article
    pub law_id: String,
    /// Law UUID if available
    pub law_uuid: Option<String>,
    /// Execution trace tree (only populated when tracing is enabled)
    pub trace: Option<PathNode>,
    /// Engine version that produced this result (RFC-013)
    pub engine_version: String,
    /// Schema version of the regulation (RFC-013)
    pub schema_version: Option<String>,
    /// SHA-256 hash of the regulation YAML content (RFC-013)
    pub regulation_hash: Option<String>,
    /// valid_from date of the regulation version that was evaluated (RFC-013)
    pub regulation_valid_from: Option<String>,
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
    #[cfg_attr(feature = "otel", tracing::instrument(skip(self, parameters), fields(law_id = %self.law.id, article = %self.article.number)))]
    pub fn evaluate(
        &self,
        parameters: BTreeMap<String, Value>,
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
        parameters: BTreeMap<String, Value>,
        calculation_date: &str,
        requested_output: Option<&str>,
    ) -> Result<ArticleResult> {
        // Initialize visited set with current article to detect circular references
        let visited = vec![self.article.number.clone()];

        self.evaluate_internal(parameters, calculation_date, requested_output, visited, 0)
    }

    /// Execute this article's logic with trace support.
    ///
    /// Same as `evaluate_with_output` but accepts a shared trace builder.
    pub fn evaluate_with_trace(
        &self,
        parameters: BTreeMap<String, Value>,
        calculation_date: &str,
        requested_output: Option<&str>,
        trace: Rc<RefCell<TraceBuilder>>,
    ) -> Result<ArticleResult> {
        let visited = vec![self.article.number.clone()];

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
        parameters: BTreeMap<String, Value>,
        calculation_date: &str,
        requested_output: Option<&str>,
        visited: Vec<String>,
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
        parameters: BTreeMap<String, Value>,
        calculation_date: &str,
        requested_output: Option<&str>,
        visited: Vec<String>,
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
            engine_version: crate::VERSION.to_string(),
            schema_version: self.law.schema_version().map(String::from),
            regulation_hash: self.law.content_hash.clone(),
            regulation_valid_from: self.law.valid_from.clone(),
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
        parameters: &BTreeMap<String, Value>,
        calculation_date: &str,
        visited: &[String],
        depth: usize,
    ) -> Result<()> {
        let inputs = self.article.get_inputs();

        for input in inputs {
            let source = match &input.source {
                Some(s) => s,
                None => continue, // No source, skip
            };

            // Determine the type of reference:
            // 1. External: source.regulation is set (simple cross-law reference)
            // 2. Internal: source.output is set (same-law reference)
            // 3. Empty source: resolved by DataSourceRegistry in service layer

            if let Some(regulation) = &source.regulation {
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
        parameters: &BTreeMap<String, Value>,
        calculation_date: &str,
        visited: &[String],
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
        let mut new_visited = visited.to_vec();
        new_visited.push(article.number.clone());

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

        // No value or operation specified
        Ok(Value::Null)
    }

    /// Convert an Action to an ActionOperation for execution.
    ///
    /// This is needed because actions can have operations specified inline
    /// rather than as nested ActionValue::Operation.
    ///
    /// Only comparison, arithmetic, aggregate, and logical operations are supported
    /// at the action level because the `Action` struct only has `subject`, `value`,
    /// `values`, and `conditions` fields. IF, date operations, and LIST must be
    /// nested inside `value` as an `ActionValue::Operation`.
    fn action_to_operation(
        &self,
        action: &Action,
        operation: &crate::types::Operation,
    ) -> Result<ActionOperation> {
        use crate::types::Operation;

        match operation {
            // Comparison operations (subject + value)
            Operation::Equals
            | Operation::NotEquals
            | Operation::GreaterThan
            | Operation::LessThan
            | Operation::GreaterThanOrEqual
            | Operation::LessThanOrEqual => {
                let subject = action.subject.clone().ok_or_else(|| {
                    EngineError::InvalidOperation(format!(
                        "{} requires 'subject' at action level",
                        operation.name()
                    ))
                })?;
                let value = action.value.clone().ok_or_else(|| {
                    EngineError::InvalidOperation(format!(
                        "{} requires 'value' at action level",
                        operation.name()
                    ))
                })?;
                Ok(match operation {
                    Operation::Equals => ActionOperation::Equals { subject, value },
                    Operation::NotEquals => ActionOperation::NotEquals { subject, value },
                    Operation::GreaterThan => ActionOperation::GreaterThan { subject, value },
                    Operation::LessThan => ActionOperation::LessThan { subject, value },
                    Operation::GreaterThanOrEqual => {
                        ActionOperation::GreaterThanOrEqual { subject, value }
                    }
                    Operation::LessThanOrEqual => {
                        ActionOperation::LessThanOrEqual { subject, value }
                    }
                    _ => unreachable!(),
                })
            }

            // Arithmetic operations (values)
            Operation::Add | Operation::Subtract | Operation::Multiply | Operation::Divide => {
                let values = action.values.clone().ok_or_else(|| {
                    EngineError::InvalidOperation(format!(
                        "{} requires 'values' at action level",
                        operation.name()
                    ))
                })?;
                Ok(match operation {
                    Operation::Add => ActionOperation::Add { values },
                    Operation::Subtract => ActionOperation::Subtract { values },
                    Operation::Multiply => ActionOperation::Multiply { values },
                    Operation::Divide => ActionOperation::Divide { values },
                    _ => unreachable!(),
                })
            }

            // Aggregate operations (values)
            Operation::Max | Operation::Min => {
                let values = action.values.clone().ok_or_else(|| {
                    EngineError::InvalidOperation(format!(
                        "{} requires 'values' at action level",
                        operation.name()
                    ))
                })?;
                Ok(match operation {
                    Operation::Max => ActionOperation::Max { values },
                    Operation::Min => ActionOperation::Min { values },
                    _ => unreachable!(),
                })
            }

            // Logical operations
            Operation::And | Operation::Or => {
                let conditions = action.conditions.clone().ok_or_else(|| {
                    EngineError::InvalidOperation(format!(
                        "{} requires 'conditions' at action level",
                        operation.name()
                    ))
                })?;
                Ok(match operation {
                    Operation::And => ActionOperation::And { conditions },
                    Operation::Or => ActionOperation::Or { conditions },
                    _ => unreachable!(),
                })
            }

            Operation::Not => {
                let value = action.value.clone().ok_or_else(|| {
                    EngineError::InvalidOperation(
                        "NOT requires 'value' at action level".to_string(),
                    )
                })?;
                Ok(ActionOperation::Not { value })
            }

            // Null check operations (subject only)
            Operation::IsNull => {
                let subject = action.subject.clone().ok_or_else(|| {
                    EngineError::InvalidOperation(
                        "IS_NULL requires 'subject' at action level".to_string(),
                    )
                })?;
                Ok(ActionOperation::IsNull { subject })
            }
            Operation::NotNull => {
                let subject = action.subject.clone().ok_or_else(|| {
                    EngineError::InvalidOperation(
                        "NOT_NULL requires 'subject' at action level".to_string(),
                    )
                })?;
                Ok(ActionOperation::NotNull { subject })
            }

            // Collection: IN/NOT_IN (subject + value/values)
            Operation::In => {
                let subject = action.subject.clone().ok_or_else(|| {
                    EngineError::InvalidOperation(
                        "IN requires 'subject' at action level".to_string(),
                    )
                })?;
                Ok(ActionOperation::In {
                    subject,
                    value: action.value.clone(),
                    values: action.values.clone(),
                })
            }
            Operation::NotIn => {
                let subject = action.subject.clone().ok_or_else(|| {
                    EngineError::InvalidOperation(
                        "NOT_IN requires 'subject' at action level".to_string(),
                    )
                })?;
                Ok(ActionOperation::NotIn {
                    subject,
                    value: action.value.clone(),
                    values: action.values.clone(),
                })
            }

            // Operations not supported at action level
            Operation::If
            | Operation::List
            | Operation::Age
            | Operation::DateAdd
            | Operation::Date
            | Operation::DayOfWeek => Err(EngineError::InvalidOperation(format!(
                "{} must be nested inside 'value', not used directly at action level",
                operation.name()
            ))),
        }
    }

    /// Get actions from the article's execution spec.
    fn get_actions(&self) -> &[Action] {
        self.article
            .get_execution_spec()
            .and_then(|exec| exec.actions.as_deref())
            .unwrap_or(&[])
    }
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
              cases:
                - when:
                    operation: GREATER_THAN_OR_EQUAL
                    subject: $age
                    value: $MIN_AGE
                  then: "adult"
              default: "minor"
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

        let mut params = BTreeMap::new();
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
        let mut params = BTreeMap::new();
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
        let mut params = BTreeMap::new();
        params.insert("age".to_string(), Value::Int(25));
        let result = engine.evaluate(params, "2025-01-01").unwrap();
        assert_eq!(
            result.outputs.get("age_check_result"),
            Some(&Value::String("adult".to_string()))
        );

        // Minor case
        let mut params = BTreeMap::new();
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

        let mut params = BTreeMap::new();
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
        let mut params = BTreeMap::new();
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

        let mut params = BTreeMap::new();
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
        let params = BTreeMap::new();
        let result = engine.evaluate(params, "2025-01-01");

        assert!(matches!(result, Err(EngineError::VariableNotFound(_))));
    }

    #[test]
    fn test_invalid_date() {
        let law = make_simple_law();
        let article = law.find_article_by_number("1").unwrap();
        let engine = ArticleEngine::new(article, &law);

        let mut params = BTreeMap::new();
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

        let result = engine.evaluate(BTreeMap::new(), "2025-06-15").unwrap();

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

        let result = engine.evaluate(BTreeMap::new(), "2025-01-01").unwrap();

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

        let result = engine.evaluate(BTreeMap::new(), "2025-01-01").unwrap();

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

        let mut params = BTreeMap::new();
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

        let result = engine.evaluate(BTreeMap::new(), "2025-01-01");

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

        let result = engine.evaluate(BTreeMap::new(), "2025-01-01");

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

        let result = engine.evaluate(BTreeMap::new(), "2025-01-01");

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

        let result = engine.evaluate(BTreeMap::new(), "2025-01-01");

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

    fn get_regulation_path() -> std::path::PathBuf {
        std::env::var("REGULATION_PATH")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| {
                std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("..")
                    .join("..")
                    .join("corpus")
                    .join("regulation")
            })
    }

    // -------------------------------------------------------------------------
    // IoC Integration Tests
    // -------------------------------------------------------------------------

    mod ioc {
        use super::*;

        #[test]
        fn test_parse_participatiewet_ioc() {
            // Test that participatiewet uses IoC: article 8 has open_terms,
            // article 43 references article 8 via source.output
            let path = get_regulation_path().join("nl/wet/participatiewet/2022-03-15.yaml");

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
                .find(|i| i.name == "verlaging_percentage")
                .unwrap();
            let source = input.source.as_ref().unwrap();
            assert_eq!(source.output.as_deref(), Some("verlaging_percentage"));
        }
    }

    // -------------------------------------------------------------------------
    // Integration Tests with Real Regulation Files
    // -------------------------------------------------------------------------

    mod integration {
        use super::*;

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
            let mut params = BTreeMap::new();
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
            let result = engine.evaluate(BTreeMap::new(), "2025-01-01").unwrap();

            // Should have standaardpremie output (211200 eurocent = €2112 for 2025)
            assert_eq!(
                result.outputs.get("standaardpremie"),
                Some(&Value::Int(211200))
            );
        }
    }
}
