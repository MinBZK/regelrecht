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
use crate::context::RuleContext;
use crate::error::{EngineError, Result};
use crate::operations::{evaluate_value, execute_operation};
use crate::trace::{PathNode, TraceBuilder};
use crate::types::{PathNodeType, Value};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

/// Provenance of an output value: how it was produced during execution.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "type")]
pub enum OutputProvenance {
    /// Produced by the article's own actions.
    Direct { law_id: String, article: String },
    /// Produced by a hook firing on a lifecycle event (e.g., AWB on BESCHIKKING).
    Reactive {
        law_id: String,
        article: String,
        hook_point: String,
    },
    /// Produced by a lex specialis override (RFC-007).
    Override { law_id: String, article: String },
}

/// Result of article execution
#[derive(Debug, Clone)]
pub struct ArticleResult {
    /// Calculated output values
    pub outputs: BTreeMap<String, Value>,
    /// Per-output provenance: which law/article/mechanism produced each output.
    pub output_provenance: BTreeMap<String, OutputProvenance>,
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
///
/// The engine evaluates **one article in isolation**: it expects every input
/// it needs to already be present in `parameters`. It does **not** resolve
/// cross-article (`source.output`) or cross-law (`source.regulation`)
/// references itself. An unresolved internal (`source.output`) input is left
/// untouched (a consuming action then surfaces a `VariableNotFound`), while an
/// unresolved external (`source.regulation`) input — which this engine can
/// never resolve on its own — yields [`EngineError::ExternalReferenceNotResolved`].
/// All reference resolution, cycle detection and depth limiting live in
/// [`crate::LawExecutionService`], which is the single resolution authority and
/// the entry point every production caller uses.
pub struct ArticleEngine<'a> {
    /// Article to execute
    article: &'a Article,
    /// Law containing the article
    law: &'a ArticleBasedLaw,
    /// Declared units for this article's symbols (RFC-023). Empty/all-unknown
    /// for un-annotated articles, which then skip unit checking entirely.
    symbols: crate::units::SymbolUnits,
}

impl<'a> ArticleEngine<'a> {
    /// Create a new article engine.
    ///
    /// # Arguments
    /// * `article` - Article to execute
    /// * `law` - Law containing the article
    pub fn new(article: &'a Article, law: &'a ArticleBasedLaw) -> Self {
        let symbols = crate::units::SymbolUnits::from_article(article);
        Self {
            article,
            law,
            symbols,
        }
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
        self.evaluate_internal_traced(parameters, calculation_date, requested_output, None)
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
        self.evaluate_internal_traced(parameters, calculation_date, requested_output, Some(trace))
    }

    /// Internal evaluation, optionally tracing.
    ///
    /// `parameters` must already contain every value this article needs;
    /// cross-article/cross-law resolution is [`crate::LawExecutionService`]'s job.
    fn evaluate_internal_traced(
        &self,
        parameters: BTreeMap<String, Value>,
        calculation_date: &str,
        requested_output: Option<&str>,
        trace: Option<Rc<RefCell<TraceBuilder>>>,
    ) -> Result<ArticleResult> {
        tracing::debug!(
            law_id = %self.law.id,
            article = %self.article.number,
            requested_output = ?requested_output,
            "Starting article evaluation"
        );

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

        // Guard against any input that still carries an unresolved external source.
        self.check_input_sources(&parameters)?;

        // Execute actions (with trace instrumentation)
        self.execute_actions_traced(&mut context, requested_output)?;

        // Build result
        // Tag all outputs as Direct (hooks/overrides are tagged by the service layer)
        let output_provenance: BTreeMap<String, OutputProvenance> = context
            .outputs()
            .keys()
            .map(|name| {
                (
                    name.clone(),
                    OutputProvenance::Direct {
                        law_id: self.law.id.clone(),
                        article: self.article.number.clone(),
                    },
                )
            })
            .collect();

        let result = ArticleResult {
            outputs: context.outputs().clone(),
            output_provenance,
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

    /// Validate that every input carrying an external `source` was pre-resolved.
    ///
    /// Reference resolution is owned by [`crate::LawExecutionService`], which
    /// resolves inputs (data sources, cross-law and cross-article references)
    /// and passes the values in via `parameters`. This single-article engine
    /// only checks that a `source.regulation` input it cannot resolve itself is
    /// already present; internal (`source.output`) and empty (`source: {}`)
    /// inputs are left untouched — they are either already in `parameters` or
    /// stay unresolved (a consuming action then surfaces a `VariableNotFound`).
    ///
    /// # Arguments
    /// * `parameters` - Input parameters (already-resolved values)
    fn check_input_sources(&self, parameters: &BTreeMap<String, Value>) -> Result<()> {
        for input in self.article.get_inputs() {
            let Some(source) = &input.source else {
                continue; // No source, nothing to check
            };

            // An external (cross-law) reference can only be resolved by the
            // service layer. If it was not pre-resolved into parameters, this
            // standalone engine cannot proceed.
            if let Some(regulation) = &source.regulation {
                if !parameters.contains_key(&input.name) {
                    return Err(EngineError::ExternalReferenceNotResolved {
                        input_name: input.name.clone(),
                        regulation: regulation.clone(),
                        output: source.output.clone().unwrap_or_default(),
                    });
                }
            }
            // Internal (`source.output`) and empty (`source: {}`) inputs need no
            // action here: resolved by the service when present in parameters,
            // otherwise intentionally left unresolved.
        }

        Ok(())
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
            // RFC-023: reject incompatible units (e.g. eurocent + days). Gated on
            // the article declaring any unit, so un-annotated laws are untouched.
            if self.symbols.has_any_unit() {
                crate::units::infer_operation_unit(&action_op, &self.symbols)?;
            }
            return execute_operation(&action_op, context, 0);
        }

        // Check for direct value (only when no operation is specified)
        if let Some(value) = &action.value {
            if self.symbols.has_any_unit() {
                crate::units::infer_unit(value, &self.symbols)?;
            }
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

        let require_subject = |op: &Operation| {
            action.subject.clone().ok_or_else(|| {
                EngineError::InvalidOperation(format!(
                    "{} requires 'subject' at action level",
                    op.name()
                ))
            })
        };
        let require_value = |op: &Operation| {
            action.value.clone().ok_or_else(|| {
                EngineError::InvalidOperation(format!(
                    "{} requires 'value' at action level",
                    op.name()
                ))
            })
        };
        let require_values = |op: &Operation| {
            action.values.clone().ok_or_else(|| {
                EngineError::InvalidOperation(format!(
                    "{} requires 'values' at action level",
                    op.name()
                ))
            })
        };
        let require_conditions = |op: &Operation| {
            action.conditions.clone().ok_or_else(|| {
                EngineError::InvalidOperation(format!(
                    "{} requires 'conditions' at action level",
                    op.name()
                ))
            })
        };
        let require_precision = |op: &Operation| {
            action.precision.ok_or_else(|| {
                EngineError::InvalidOperation(format!(
                    "{} requires 'precision' at action level",
                    op.name()
                ))
            })
        };

        match operation {
            // Comparison operations (subject + value)
            Operation::Equals => Ok(ActionOperation::Equals {
                subject: require_subject(operation)?,
                value: require_value(operation)?,
            }),
            Operation::NotEquals => Ok(ActionOperation::NotEquals {
                subject: require_subject(operation)?,
                value: require_value(operation)?,
            }),
            Operation::GreaterThan => Ok(ActionOperation::GreaterThan {
                subject: require_subject(operation)?,
                value: require_value(operation)?,
            }),
            Operation::LessThan => Ok(ActionOperation::LessThan {
                subject: require_subject(operation)?,
                value: require_value(operation)?,
            }),
            Operation::GreaterThanOrEqual => Ok(ActionOperation::GreaterThanOrEqual {
                subject: require_subject(operation)?,
                value: require_value(operation)?,
            }),
            Operation::LessThanOrEqual => Ok(ActionOperation::LessThanOrEqual {
                subject: require_subject(operation)?,
                value: require_value(operation)?,
            }),

            // Arithmetic operations (values)
            Operation::Add => Ok(ActionOperation::Add {
                values: require_values(operation)?,
            }),
            Operation::Subtract => Ok(ActionOperation::Subtract {
                values: require_values(operation)?,
            }),
            Operation::Multiply => Ok(ActionOperation::Multiply {
                values: require_values(operation)?,
            }),
            Operation::Divide => Ok(ActionOperation::Divide {
                values: require_values(operation)?,
            }),

            // Aggregate operations (values)
            Operation::Max => Ok(ActionOperation::Max {
                values: require_values(operation)?,
            }),
            Operation::Min => Ok(ActionOperation::Min {
                values: require_values(operation)?,
            }),

            // Rounding operations (unary value + precision; RFC-024)
            Operation::Round => Ok(ActionOperation::Round {
                value: require_value(operation)?,
                precision: require_precision(operation)?,
            }),
            Operation::Ceil => Ok(ActionOperation::Ceil {
                value: require_value(operation)?,
                precision: require_precision(operation)?,
            }),
            Operation::Floor => Ok(ActionOperation::Floor {
                value: require_value(operation)?,
                precision: require_precision(operation)?,
            }),

            // Logical operations
            Operation::And => Ok(ActionOperation::And {
                conditions: require_conditions(operation)?,
            }),
            Operation::Or => Ok(ActionOperation::Or {
                conditions: require_conditions(operation)?,
            }),
            Operation::Not => Ok(ActionOperation::Not {
                value: require_value(operation)?,
            }),

            // Null check operations (subject only)
            Operation::IsNull => Ok(ActionOperation::IsNull {
                subject: require_subject(operation)?,
            }),
            Operation::NotNull => Ok(ActionOperation::NotNull {
                subject: require_subject(operation)?,
            }),

            // Collection: IN/NOT_IN (subject + value/values)
            Operation::In => Ok(ActionOperation::In {
                subject: require_subject(operation)?,
                value: action.value.clone(),
                values: action.values.clone(),
            }),
            Operation::NotIn => Ok(ActionOperation::NotIn {
                subject: require_subject(operation)?,
                value: action.value.clone(),
                values: action.values.clone(),
            }),

            // Operations not supported at action level
            Operation::If
            | Operation::List
            | Operation::Age
            | Operation::DateAdd
            | Operation::Date
            | Operation::DayOfWeek
            | Operation::DateDiff => Err(EngineError::InvalidOperation(format!(
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
    use crate::article::{ArticleBasedLaw, LawLoad};
    use rust_decimal_macros::dec;

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

    /// A law that adds a eurocent amount to a duration in days — a unit mismatch
    /// the engine must reject at runtime (RFC-023).
    fn make_unit_mismatch_law() -> ArticleBasedLaw {
        let yaml = r#"
$id: unit_mismatch_law
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Unit mismatch article
    machine_readable:
      execution:
        input:
          - name: bedrag
            type: amount
            type_spec:
              unit: eurocent
          - name: duur
            type: number
            type_spec:
              unit: days
        output:
          - name: onzin
            type: amount
            type_spec:
              unit: eurocent
        actions:
          - output: onzin
            operation: ADD
            values:
              - $bedrag
              - $duur
"#;
        ArticleBasedLaw::from_yaml_str(yaml).unwrap()
    }

    // -------------------------------------------------------------------------
    // Basic Execution Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_unit_mismatch_rejected_at_runtime() {
        let law = make_unit_mismatch_law();
        let article = law.find_article_by_number("1").unwrap();
        let engine = ArticleEngine::new(article, &law);

        let mut params = BTreeMap::new();
        params.insert("bedrag".to_string(), Value::Int(100));
        params.insert("duur".to_string(), Value::Int(5));

        let result = engine.evaluate(params, "2025-01-01");
        assert!(
            matches!(result, Err(crate::error::EngineError::UnitMismatch { .. })),
            "Expected UnitMismatch (eurocent + days), got {:?}",
            result
        );
    }

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
        assert_eq!(
            result.outputs.get("tax_amount"),
            Some(&Value::Decimal(dec!(840)))
        );
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
        assert_eq!(
            result.outputs.get("tax_amount"),
            Some(&Value::Decimal(dec!(0)))
        );
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
        fn test_execute_wet_op_de_zorgtoeslag_vermogen_check() {
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
