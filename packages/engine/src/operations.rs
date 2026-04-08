//! Operation execution for the RegelRecht engine
//!
//! Implements the execution logic for all operation types in the v0.5.0 schema,
//! plus engine-only operations retained for backward compatibility.
//!
//! **Schema v0.5.0 operations:**
//! - **Comparison:** EQUALS, GREATER_THAN, LESS_THAN, GREATER_THAN_OR_EQUAL, LESS_THAN_OR_EQUAL
//! - **Arithmetic:** ADD, SUBTRACT, MULTIPLY, DIVIDE
//! - **Aggregate:** MAX, MIN
//! - **Logical:** AND, OR, NOT
//! - **Conditional:** IF (multi-case with cases/default)
//! - **Collection:** IN, LIST
//! - **Date:** AGE, DATE_ADD, DATE, DAY_OF_WEEK
//!
//! **Engine-only (not in schema, accepted for backward compatibility):**
//! NOT_EQUALS, IS_NULL, NOT_NULL, NOT_IN

use crate::article::{ActionOperation, ActionValue, Case};
use crate::context::RuleContext;
use crate::error::{EngineError, Result};
use crate::types::{PathNodeType, Value};
use chrono::{Datelike, NaiveDate};

/// Maximum nesting depth for operations to prevent stack overflow
const MAX_OPERATION_DEPTH: usize = 100;

/// If any value in the slice is Untranslatable, return it (NaN-like propagation).
fn find_untranslatable(values: &[Value]) -> Option<Value> {
    values.iter().find(|v| v.is_untranslatable()).cloned()
}

/// If either of two values is Untranslatable, return it.
fn propagate_binary(a: &Value, b: &Value) -> Option<Value> {
    if a.is_untranslatable() {
        Some(a.clone())
    } else if b.is_untranslatable() {
        Some(b.clone())
    } else {
        None
    }
}

/// If any value in the slice is Null, return Some(Value::Null).
fn find_null(values: &[Value]) -> Option<Value> {
    if values.iter().any(|v| v.is_null()) {
        Some(Value::Null)
    } else {
        None
    }
}

/// If either of two values is Null, return Some(Value::Null).
fn propagate_null_binary(a: &Value, b: &Value) -> Option<Value> {
    if a.is_null() || b.is_null() {
        Some(Value::Null)
    } else {
        None
    }
}

/// Maximum integer value that can be exactly represented in f64.
/// Beyond this, precision is lost when converting i64 to f64.
/// This is 2^53 = 9007199254740992.
const MAX_SAFE_INTEGER: i64 = 9_007_199_254_740_992;

/// Minimum integer value that can be exactly represented in f64.
/// This is -2^53.
const MIN_SAFE_INTEGER: i64 = -9_007_199_254_740_992;

/// Trait for resolving variable references ($var) during operation execution.
///
/// Implementations should provide variable resolution from context (parameters,
/// inputs, outputs, definitions, etc.)
///
/// # Error Handling
///
/// - Return `Err(EngineError::VariableNotFound)` when a variable doesn't exist
/// - The error will propagate up through the operation execution chain
/// - Callers should handle missing variables appropriately (e.g., default values)
///
/// # Example Implementation
///
/// ```ignore
/// impl ValueResolver for MyContext {
///     fn resolve(&self, name: &str) -> Result<Value> {
///         self.variables.get(name)
///             .cloned()
///             .ok_or_else(|| EngineError::VariableNotFound(name.to_string()))
///     }
/// }
/// ```
pub trait ValueResolver {
    /// Resolve a variable name to its value.
    ///
    /// # Arguments
    /// * `name` - Variable name without the `$` prefix (e.g., "age" not "$age")
    ///
    /// # Returns
    /// * `Ok(Value)` - The resolved value
    /// * `Err(EngineError::VariableNotFound)` - Variable doesn't exist in context
    fn resolve(&self, name: &str) -> Result<Value>;

    /// Push a trace node. No-op by default.
    fn trace_push(&self, _name: &str, _node_type: PathNodeType) {}

    /// Pop a trace node. No-op by default.
    fn trace_pop(&self) {}

    /// Set result on current trace node. No-op by default.
    fn trace_set_result(&self, _result: Value) {}

    /// Set message on current trace node. No-op by default.
    fn trace_set_message(&self, _msg: String) {}

    /// Get message from current trace node. Returns None by default.
    fn trace_get_message(&self) -> Option<String> {
        None
    }

    /// Check if tracing is active. Returns false by default.
    fn has_trace(&self) -> bool {
        false
    }

    /// Execute a FOREACH operation if the resolver supports child scopes.
    ///
    /// Returns `None` if FOREACH is not supported (e.g., for simple test resolvers).
    /// Returns `Some(Result<Value>)` if FOREACH was executed.
    ///
    /// The default implementation returns `None`, causing the caller to fall
    /// through to an error. Implementations that support child scopes (like
    /// `RuleContext`) should override this to delegate to `execute_foreach`.
    fn execute_foreach_op(
        &self,
        _collection: &ActionValue,
        _as_name: &str,
        _body: &ActionValue,
        _filter: Option<&ActionValue>,
        _combine: Option<&str>,
        _depth: usize,
    ) -> Option<Result<Value>> {
        None
    }
}

/// Evaluate an ActionValue to a concrete Value.
///
/// Handles:
/// - Literal values (returned directly)
/// - Variable references ($name) - resolved via the resolver
/// - Nested operations - executed recursively
///
/// The depth parameter tracks recursion to prevent stack overflow.
pub fn evaluate_value<R: ValueResolver>(
    value: &ActionValue,
    resolver: &R,
    depth: usize,
) -> Result<Value> {
    if depth > MAX_OPERATION_DEPTH {
        return Err(EngineError::MaxDepthExceeded(depth));
    }

    match value {
        ActionValue::Literal(v) => {
            // Check if it's a variable reference (starts with $)
            if let Value::String(s) = v {
                if let Some(var_name) = s.strip_prefix('$') {
                    return match resolver.resolve(var_name) {
                        Ok(val) => Ok(val),
                        Err(EngineError::VariableNotFound(_)) => Ok(Value::Null),
                        Err(e) => Err(e),
                    };
                }
            }
            Ok(v.clone())
        }
        ActionValue::Operation(op) => execute_operation(op, resolver, depth + 1),
    }
}

/// Execute an operation and return the result.
///
/// Dispatches to the appropriate operation handler based on the operation type.
/// The depth parameter tracks recursion to prevent stack overflow.
pub fn execute_operation<R: ValueResolver>(
    op: &ActionOperation,
    resolver: &R,
    depth: usize,
) -> Result<Value> {
    if depth > MAX_OPERATION_DEPTH {
        return Err(EngineError::MaxDepthExceeded(depth));
    }

    let op_name = op.operation_name();
    let tracing = resolver.has_trace();
    if tracing {
        resolver.trace_push(op_name, PathNodeType::Operation);
    }

    let result = execute_operation_internal(op, resolver, depth);

    if tracing {
        match &result {
            Ok(value) => {
                resolver.trace_set_result(value.clone());
                // For IF (cases/default), execute_if already set a message
                // with case match info; incorporate it instead of overwriting.
                let existing_msg = resolver.trace_get_message();
                let msg = if matches!(op, ActionOperation::If { .. }) {
                    if let Some(case_info) = existing_msg {
                        format!("IF({}) = {}", case_info, format_value_for_trace(value))
                    } else {
                        format!(
                            "Compute {}(...) = {}",
                            op_name,
                            format_value_for_trace(value)
                        )
                    }
                } else {
                    format!(
                        "Compute {}(...) = {}",
                        op_name,
                        format_value_for_trace(value)
                    )
                };
                resolver.trace_set_message(msg);
            }
            Err(e) => {
                resolver.trace_set_message(format!("Error in {}: {}", op_name, e));
            }
        }
        resolver.trace_pop();
    }

    result
}

/// Format a value compactly for trace messages.
fn format_value_for_trace(value: &Value) -> String {
    match value {
        Value::Null => "None".to_string(),
        Value::Bool(b) => {
            if *b {
                "True".to_string()
            } else {
                "False".to_string()
            }
        }
        Value::Int(i) => i.to_string(),
        Value::Float(f) => format!("{}", f),
        Value::String(s) => format!("'{}'", s),
        Value::Array(arr) => {
            let items: Vec<String> = arr.iter().map(format_value_for_trace).collect();
            format!("[{}]", items.join(", "))
        }
        Value::Object(_) => "{...}".to_string(),
        Value::Untranslatable { article, .. } => format!("UNTRANSLATABLE(art. {})", article),
    }
}

/// Internal operation dispatch (no tracing).
fn execute_operation_internal<R: ValueResolver>(
    op: &ActionOperation,
    resolver: &R,
    depth: usize,
) -> Result<Value> {
    match op {
        // Comparison operations
        ActionOperation::Equals { subject, value } => {
            execute_equality(subject, value, resolver, depth, false)
        }
        ActionOperation::NotEquals { subject, value } => {
            execute_equality(subject, value, resolver, depth, true)
        }
        ActionOperation::GreaterThan { subject, value } => {
            execute_numeric_comparison(subject, value, resolver, depth, |a, b| a > b)
        }
        ActionOperation::LessThan { subject, value } => {
            execute_numeric_comparison(subject, value, resolver, depth, |a, b| a < b)
        }
        ActionOperation::GreaterThanOrEqual { subject, value } => {
            execute_numeric_comparison(subject, value, resolver, depth, |a, b| a >= b)
        }
        ActionOperation::LessThanOrEqual { subject, value } => {
            execute_numeric_comparison(subject, value, resolver, depth, |a, b| a <= b)
        }

        // Arithmetic
        ActionOperation::Add { values } => execute_add(values, resolver, depth),
        ActionOperation::Subtract { values } => execute_subtract(values, resolver, depth),
        ActionOperation::Multiply { values } => execute_multiply(values, resolver, depth),
        ActionOperation::Divide { values } => execute_divide(values, resolver, depth),

        // Aggregate
        ActionOperation::Max { values } => execute_aggregate(values, resolver, depth, f64::max),
        ActionOperation::Min { values } => execute_aggregate(values, resolver, depth, f64::min),

        // Logical
        ActionOperation::And { conditions } => execute_and(conditions, resolver, depth),
        ActionOperation::Or { conditions } => execute_or(conditions, resolver, depth),
        ActionOperation::Not { value } => execute_not(value, resolver, depth),

        // Conditional (multi-case with cases/default)
        ActionOperation::If { cases, default } => {
            execute_if(cases, default.as_ref(), resolver, depth)
        }

        // Null checking operations
        ActionOperation::IsNull { subject } => execute_null_check(subject, resolver, depth, false),
        ActionOperation::NotNull { subject } => execute_null_check(subject, resolver, depth, true),

        // Collection operations
        ActionOperation::In {
            subject,
            value,
            values,
        } => execute_membership(
            subject,
            value.as_ref(),
            values.as_deref(),
            resolver,
            depth,
            false,
        ),
        ActionOperation::NotIn {
            subject,
            value,
            values,
        } => execute_membership(
            subject,
            value.as_ref(),
            values.as_deref(),
            resolver,
            depth,
            true,
        ),
        ActionOperation::List { items } => execute_list(items, resolver, depth),

        // Date
        ActionOperation::Age {
            date_of_birth,
            reference_date,
        } => execute_age(date_of_birth, reference_date, resolver, depth),
        ActionOperation::DateAdd {
            date,
            years,
            months,
            weeks,
            days,
        } => execute_date_add(
            date,
            years.as_ref(),
            months.as_ref(),
            weeks.as_ref(),
            days.as_ref(),
            resolver,
            depth,
        ),
        ActionOperation::Date { year, month, day } => {
            execute_date_construct(year, month, day, resolver, depth)
        }
        ActionOperation::DayOfWeek { date } => execute_day_of_week(date, resolver, depth),

        // Collection iteration — FOREACH requires child scopes.
        // Delegate to the resolver's execute_foreach_op method, which returns
        // None for resolvers that don't support child scopes (falls through to error).
        ActionOperation::Foreach {
            collection,
            as_name,
            body,
            filter,
            combine,
        } => resolver
            .execute_foreach_op(
                collection,
                as_name,
                body,
                filter.as_ref(),
                combine.as_deref(),
                depth,
            )
            .unwrap_or_else(|| {
                Err(EngineError::InvalidOperation(
                    "FOREACH requires a resolver that supports child scopes".to_string(),
                ))
            }),
    }
}

// =============================================================================
// Comparison Operations
// =============================================================================

/// Check if two Values are equal, with Python-style numeric coercion.
///
/// This matches Python's behavior where `42 == 42.0` is `True`.
/// For non-numeric types, uses standard equality.
///
/// # NaN Handling
///
/// Unlike IEEE 754 where `NaN != NaN`, this function treats two NaN values
/// as equal. This is intentional for law execution where NaN represents
/// invalid/missing data, and comparing two missing values should be consistent.
///
/// # Precision Guard
///
/// Integers beyond ±2^53 cannot be exactly represented as f64, so
/// Int-Float comparisons involving such integers return `false` immediately
/// to avoid silent precision loss.
pub(crate) fn values_equal(a: &Value, b: &Value) -> bool {
    match (a, b) {
        // Untranslatable: two untranslatables are equal, mixed is never equal
        (Value::Untranslatable { .. }, Value::Untranslatable { .. }) => true,
        (Value::Untranslatable { .. }, _) | (_, Value::Untranslatable { .. }) => false,
        // Float-Float comparison: handle NaN specially
        (Value::Float(f1), Value::Float(f2)) => {
            if f1.is_nan() && f2.is_nan() {
                true // Both NaN are considered equal
            } else {
                f1 == f2
            }
        }
        // Int-Float comparison: handle NaN and precision guard
        (Value::Int(i), Value::Float(f)) => {
            !f.is_nan() && *i >= MIN_SAFE_INTEGER && *i <= MAX_SAFE_INTEGER && (*i as f64) == *f
        }
        (Value::Float(f), Value::Int(i)) => {
            !f.is_nan() && *i >= MIN_SAFE_INTEGER && *i <= MAX_SAFE_INTEGER && *f == (*i as f64)
        }
        // Default: use structural equality
        _ => a == b,
    }
}

/// Execute EQUALS / NOT_EQUALS with Python-style numeric coercion.
///
/// - `Int(42) == Float(42.0)` returns `true` (like Python)
/// - Non-numeric types use structural equality
/// - When `negate` is true, the result is inverted (NOT_EQUALS).
fn execute_equality<R: ValueResolver>(
    subject: &ActionValue,
    value: &ActionValue,
    resolver: &R,
    depth: usize,
    negate: bool,
) -> Result<Value> {
    let subject_val = evaluate_value(subject, resolver, depth)?;
    let value_val = evaluate_value(value, resolver, depth)?;

    if let Some(tainted) = propagate_binary(&subject_val, &value_val) {
        return Ok(tainted);
    }

    // Note: No null propagation for EQUALS/NOT_EQUALS.
    // Null == Null returns true (needed for null-checking patterns like
    // `EQUALS($var, null)` which is a common idiom in law YAML).
    // Null == non-null returns false. This matches Python semantics.

    let equal = values_equal(&subject_val, &value_val);
    Ok(Value::Bool(if negate { !equal } else { equal }))
}

/// Execute a numeric comparison (>, <, >=, <=).
///
/// Converts values to f64 for comparison to handle both Int and Float types.
fn execute_numeric_comparison<R: ValueResolver, F>(
    subject: &ActionValue,
    value: &ActionValue,
    resolver: &R,
    depth: usize,
    compare: F,
) -> Result<Value>
where
    F: Fn(f64, f64) -> bool,
{
    let subject_val = evaluate_value(subject, resolver, depth)?;
    let value_val = evaluate_value(value, resolver, depth)?;

    if let Some(tainted) = propagate_binary(&subject_val, &value_val) {
        return Ok(tainted);
    }

    // Null propagation: comparison with Null yields Null (unknown)
    if let Some(null) = propagate_null_binary(&subject_val, &value_val) {
        return Ok(null);
    }

    let subject_num = to_number(&subject_val)?;
    let value_num = to_number(&value_val)?;

    Ok(Value::Bool(compare(subject_num, value_num)))
}

// =============================================================================
// Arithmetic Operations
// =============================================================================

/// Add (sum/concatenate) a slice of already-evaluated Values.
///
/// Polymorphic: numbers are summed, arrays concatenated, strings concatenated.
/// The first element's type determines the mode. Nulls are skipped in numeric mode.
fn add_values(evaluated: &[Value]) -> Result<Value> {
    if evaluated.is_empty() {
        return Ok(Value::Int(0));
    }

    match &evaluated[0] {
        Value::Array(_) => {
            let mut result = Vec::new();
            for val in evaluated {
                match val {
                    Value::Array(arr) => result.extend(arr.iter().cloned()),
                    Value::Null => return Ok(Value::Null),
                    _ => {
                        return Err(EngineError::TypeMismatch {
                            expected: "array".to_string(),
                            actual: val.type_name().to_string(),
                        })
                    }
                }
            }
            Ok(Value::Array(result))
        }
        Value::String(_) => {
            let mut result = String::new();
            for val in evaluated {
                match val {
                    Value::String(s) => result.push_str(s),
                    Value::Null => return Ok(Value::Null),
                    _ => {
                        return Err(EngineError::TypeMismatch {
                            expected: "string".to_string(),
                            actual: val.type_name().to_string(),
                        })
                    }
                }
            }
            Ok(Value::String(result))
        }
        Value::Int(_) | Value::Float(_) | Value::Null => {
            // First pass: check if all numeric values are integers.
            // If so, use i64::checked_add to avoid f64 precision loss.
            let mut all_int = true;
            for val in evaluated {
                match val {
                    Value::Int(_) | Value::Null => {}
                    Value::Float(_) => {
                        all_int = false;
                        break;
                    }
                    _ => return Err(type_error("number", val)),
                }
            }

            if all_int {
                let mut sum: i64 = 0;
                for val in evaluated {
                    match val {
                        Value::Int(i) => {
                            sum = sum.checked_add(*i).ok_or_else(|| {
                                EngineError::ArithmeticOverflow(format!(
                                    "Integer overflow in ADD: {} + {}",
                                    sum, i
                                ))
                            })?;
                        }
                        Value::Null => {} // Skip nulls in sum
                        _ => unreachable!("checked in first pass"),
                    }
                }
                Ok(Value::Int(sum))
            } else {
                let mut sum = 0.0;
                for val in evaluated {
                    match val {
                        Value::Int(_) => sum += to_number(val)?,
                        Value::Float(f) => sum += f,
                        Value::Null => {} // Skip nulls in sum
                        _ => return Err(type_error("number", val)),
                    }
                }
                Ok(Value::Float(sum))
            }
        }
        _ => Err(type_error("number, string, or array", &evaluated[0])),
    }
}

/// Execute ADD operation: sum numbers, concatenate arrays, or concatenate strings.
///
/// The type of the first value determines the operation mode:
/// - Numbers: sum all values
/// - Arrays: concatenate all arrays
/// - Strings: concatenate all strings
fn execute_add<R: ValueResolver>(
    values: &[ActionValue],
    resolver: &R,
    depth: usize,
) -> Result<Value> {
    let evaluated = evaluate_values(values, resolver, depth)?;

    if evaluated.is_empty() {
        return Err(EngineError::InvalidOperation(
            "ADD requires at least one value".to_string(),
        ));
    }

    if let Some(tainted) = find_untranslatable(&evaluated) {
        return Ok(tainted);
    }

    add_values(&evaluated)
}

/// Execute SUBTRACT operation: first value minus all subsequent values.
///
/// Note: Uses `to_number()` which validates that integers are within the
/// safe range for f64 conversion (±2^53).
fn execute_subtract<R: ValueResolver>(
    values: &[ActionValue],
    resolver: &R,
    depth: usize,
) -> Result<Value> {
    if values.is_empty() {
        return Err(EngineError::InvalidOperation(
            "SUBTRACT requires at least one value".to_string(),
        ));
    }

    let evaluated = evaluate_values(values, resolver, depth)?;

    if let Some(tainted) = find_untranslatable(&evaluated) {
        return Ok(tainted);
    }

    // Null propagation: if any value is Null, result is Null
    if let Some(null) = find_null(&evaluated) {
        return Ok(null);
    }

    // SAFETY: values guaranteed non-empty by check above
    let Some((first, rest)) = evaluated.split_first() else {
        unreachable!("values checked non-empty above")
    };
    let mut result = to_number(first)?;
    let mut has_float = matches!(first, Value::Float(_));

    for val in rest {
        result -= to_number(val)?;
        if matches!(val, Value::Float(_)) {
            has_float = true;
        }
    }

    Ok(if has_float {
        Value::Float(result)
    } else {
        Value::Int(f64_to_i64_safe(result)?)
    })
}

/// Execute MULTIPLY operation: product of all values.
///
/// Note: Uses `to_number()` which validates that integers are within the
/// safe range for f64 conversion (±2^53).
fn execute_multiply<R: ValueResolver>(
    values: &[ActionValue],
    resolver: &R,
    depth: usize,
) -> Result<Value> {
    if values.is_empty() {
        return Err(EngineError::InvalidOperation(
            "MULTIPLY requires at least one value".to_string(),
        ));
    }

    let evaluated = evaluate_values(values, resolver, depth)?;

    if let Some(tainted) = find_untranslatable(&evaluated) {
        return Ok(tainted);
    }

    // Null propagation: if any value is Null, result is Null
    if let Some(null) = find_null(&evaluated) {
        return Ok(null);
    }

    let mut result = 1.0;
    let mut has_float = false;

    for val in &evaluated {
        match val {
            Value::Int(_) => result *= to_number(val)?,
            Value::Float(f) => {
                result *= f;
                has_float = true;
            }
            _ => return Err(type_error("number", val)),
        }
    }

    Ok(if has_float {
        Value::Float(result)
    } else {
        Value::Int(f64_to_i64_safe(result)?)
    })
}

/// Execute DIVIDE operation: first value divided by all subsequent values.
///
/// Returns `Err(DivisionByZero)` for division by zero.
/// Returns `Err(InvalidOperation)` for NaN or Infinity results.
fn execute_divide<R: ValueResolver>(
    values: &[ActionValue],
    resolver: &R,
    depth: usize,
) -> Result<Value> {
    if values.is_empty() {
        return Err(EngineError::InvalidOperation(
            "DIVIDE requires at least one value".to_string(),
        ));
    }

    let evaluated = evaluate_values(values, resolver, depth)?;

    if let Some(tainted) = find_untranslatable(&evaluated) {
        return Ok(tainted);
    }

    // Null propagation: if any value is Null, result is Null
    if let Some(null) = find_null(&evaluated) {
        return Ok(null);
    }

    // SAFETY: values guaranteed non-empty by check above
    let Some((first, rest)) = evaluated.split_first() else {
        unreachable!("values checked non-empty above")
    };
    let mut result = to_number(first)?;

    for val in rest {
        let divisor = to_number(val)?;
        if divisor == 0.0 {
            return Err(EngineError::DivisionByZero);
        }
        result /= divisor;
    }

    // Check for invalid results (NaN or Infinity)
    if result.is_nan() {
        return Err(EngineError::InvalidOperation(
            "Division resulted in NaN".to_string(),
        ));
    }
    if result.is_infinite() {
        return Err(EngineError::InvalidOperation(
            "Division resulted in infinity".to_string(),
        ));
    }

    // Division always returns a float (like Python)
    Ok(Value::Float(result))
}

// =============================================================================
// Aggregate Operations
// =============================================================================

/// Execute aggregate operation (MAX, MIN).
fn execute_aggregate<R: ValueResolver, F>(
    values: &[ActionValue],
    resolver: &R,
    depth: usize,
    combine: F,
) -> Result<Value>
where
    F: Fn(f64, f64) -> f64,
{
    if values.is_empty() {
        return Err(EngineError::InvalidOperation(
            "Aggregate operation requires at least one value".to_string(),
        ));
    }

    let evaluated = evaluate_values(values, resolver, depth)?;

    if let Some(tainted) = find_untranslatable(&evaluated) {
        return Ok(tainted);
    }

    // Skip Null values in aggregation
    let non_null: Vec<&Value> = evaluated.iter().filter(|v| !v.is_null()).collect();
    if non_null.is_empty() {
        return Ok(Value::Null);
    }

    let mut has_float = false;
    let nums: Vec<f64> = non_null
        .iter()
        .map(|v| {
            if matches!(v, Value::Float(_)) {
                has_float = true;
            }
            to_number(v)
        })
        .collect::<Result<Vec<_>>>()?;

    // SAFETY: non_null guaranteed non-empty by check above
    let Some(result) = nums.into_iter().reduce(combine) else {
        unreachable!("non_null checked non-empty above")
    };

    Ok(if has_float {
        Value::Float(result)
    } else {
        Value::Int(f64_to_i64_safe(result)?)
    })
}

// =============================================================================
// Logical Operations
// =============================================================================

/// Execute AND operation: short-circuit evaluation, returns false if any condition is false.
fn execute_and<R: ValueResolver>(
    conditions: &[ActionValue],
    resolver: &R,
    depth: usize,
) -> Result<Value> {
    let tracing = resolver.has_trace();
    let mut results: Option<Vec<Value>> = if tracing { Some(Vec::new()) } else { None };
    let mut taint: Option<Value> = None;
    for condition in conditions {
        let val = evaluate_value(condition, resolver, depth)?;
        // Definitive false wins over taint/null (AND commutativity)
        if !val.to_bool() && !val.is_untranslatable() && !val.is_null() {
            return Ok(Value::Bool(false));
        }
        if (val.is_untranslatable() || val.is_null()) && taint.is_none() {
            taint = Some(val);
            continue;
        }
        if let Some(ref mut r) = results {
            r.push(val);
        }
    }

    // If any operand was tainted/null but none was definitively false, propagate
    if let Some(t) = taint {
        return Ok(t);
    }

    if let Some(results) = results {
        let result_strs: Vec<String> = results.iter().map(format_value_for_trace).collect();
        resolver.trace_set_message(format!("Result [{}] AND: True", result_strs.join(", ")));
    }

    Ok(Value::Bool(true))
}

/// Execute OR operation: short-circuit evaluation, returns true if any condition is true.
fn execute_or<R: ValueResolver>(
    conditions: &[ActionValue],
    resolver: &R,
    depth: usize,
) -> Result<Value> {
    let mut taint: Option<Value> = None;
    for condition in conditions {
        let val = evaluate_value(condition, resolver, depth)?;
        // Definitive true wins over taint/null (OR commutativity)
        if val.to_bool() {
            return Ok(Value::Bool(true));
        }
        if (val.is_untranslatable() || val.is_null()) && taint.is_none() {
            taint = Some(val);
        }
    }

    // If any operand was tainted/null but none was definitively true, propagate
    if let Some(t) = taint {
        return Ok(t);
    }

    Ok(Value::Bool(false))
}

/// Execute NOT operation: logical negation.
///
/// Takes a single `value` field (which should be a boolean-returning operation).
fn execute_not<R: ValueResolver>(value: &ActionValue, resolver: &R, depth: usize) -> Result<Value> {
    let val = evaluate_value(value, resolver, depth)?;
    if val.is_untranslatable() {
        return Ok(val);
    }
    // Null propagation: NOT(Null) = Null (unknown)
    if val.is_null() {
        return Ok(Value::Null);
    }
    Ok(Value::Bool(!val.to_bool()))
}

// =============================================================================
// Conditional Operations
// =============================================================================

/// Execute IF operation: evaluates cases in order, returns first matching case's value.
fn execute_if<R: ValueResolver>(
    cases: &[Case],
    default: Option<&ActionValue>,
    resolver: &R,
    depth: usize,
) -> Result<Value> {
    let tracing = resolver.has_trace();

    for (i, case) in cases.iter().enumerate() {
        if tracing {
            resolver.trace_push(&format!("CASE_{}", i), PathNodeType::Operation);
        }
        let condition_result = evaluate_value(&case.when, resolver, depth)?;
        if tracing {
            resolver.trace_set_result(condition_result.clone());
            resolver.trace_set_message(format!(
                "CASE {}: {}",
                i,
                format_value_for_trace(&condition_result)
            ));
            resolver.trace_pop();
        }

        if condition_result.is_untranslatable() {
            return Ok(condition_result);
        }

        if condition_result.to_bool() {
            if tracing {
                resolver.trace_push("THEN", PathNodeType::Operation);
            }
            let result = evaluate_value(&case.then, resolver, depth)?;
            if tracing {
                resolver.trace_set_result(result.clone());
                resolver.trace_set_message(format!("THEN: {}", format_value_for_trace(&result)));
                resolver.trace_pop();
                resolver.trace_set_message(format!("case {} matched", i));
            }
            return Ok(result);
        }
    }

    // Return default if no case matched
    if let Some(default) = default {
        if tracing {
            resolver.trace_push("DEFAULT", PathNodeType::Operation);
        }
        let result = evaluate_value(default, resolver, depth)?;
        if tracing {
            resolver.trace_set_result(result.clone());
            resolver.trace_set_message(format!("DEFAULT: {}", format_value_for_trace(&result)));
            resolver.trace_pop();
            resolver.trace_set_message("took default".to_string());
        }
        Ok(result)
    } else {
        if tracing {
            resolver.trace_set_message("no case matched, no default".to_string());
        }
        Ok(Value::Null)
    }
}

// =============================================================================
// Null Checking Operations
// =============================================================================

/// Execute IS_NULL / NOT_NULL operation.
///
/// When `negate` is true, returns true if the subject is *not* null (NOT_NULL).
fn execute_null_check<R: ValueResolver>(
    subject: &ActionValue,
    resolver: &R,
    depth: usize,
    negate: bool,
) -> Result<Value> {
    let subject_val = evaluate_value(subject, resolver, depth)?;
    if subject_val.is_untranslatable() {
        return Ok(subject_val);
    }
    let is_null = subject_val.is_null();
    Ok(Value::Bool(if negate { !is_null } else { is_null }))
}

// =============================================================================
// Collection Operations
// =============================================================================

/// Execute IN / NOT_IN operation.
///
/// Uses Python-style numeric coercion for equality comparison.
/// When `negate` is true, returns true if subject is *not* in the list (NOT_IN).
///
/// Supports both `values: [...]` (inline list) and `value: $list_ref` (reference to a
/// definition list). When `value` resolves to a non-array, it is wrapped in a single-element vec.
fn execute_membership<R: ValueResolver>(
    subject: &ActionValue,
    value: Option<&ActionValue>,
    values: Option<&[ActionValue]>,
    resolver: &R,
    depth: usize,
    negate: bool,
) -> Result<Value> {
    let subject_val = evaluate_value(subject, resolver, depth)?;
    if subject_val.is_untranslatable() {
        return Ok(subject_val);
    }
    // Null propagation: if subject is Null, return Null (unknown membership)
    if subject_val.is_null() {
        return Ok(Value::Null);
    }

    let check_values = if let Some(values) = values {
        evaluate_values(values, resolver, depth)?
    } else if let Some(value) = value {
        let resolved = evaluate_value(value, resolver, depth)?;
        match resolved {
            Value::Array(items) => items,
            other => vec![other],
        }
    } else {
        let op_name = if negate { "NOT_IN" } else { "IN" };
        return Err(EngineError::InvalidOperation(format!(
            "{op_name} requires 'values' or 'value'"
        )));
    };

    let found = check_values
        .iter()
        .any(|val| values_equal(&subject_val, val));
    Ok(Value::Bool(if negate { !found } else { found }))
}

/// Execute LIST operation: construct an array from items.
fn execute_list<R: ValueResolver>(
    items: &[ActionValue],
    resolver: &R,
    depth: usize,
) -> Result<Value> {
    let values: Vec<Value> = items
        .iter()
        .map(|item| evaluate_value(item, resolver, depth))
        .collect::<Result<Vec<_>>>()?;

    Ok(Value::Array(values))
}

// =============================================================================
// Date Operations
// =============================================================================

/// Execute AGE operation: calculate age in whole years from date_of_birth to reference_date.
///
/// # Arguments
/// - `date_of_birth`: Birth date (ISO 8601 YYYY-MM-DD)
/// - `reference_date`: Date to calculate age at (ISO 8601 YYYY-MM-DD)
fn execute_age<R: ValueResolver>(
    date_of_birth: &ActionValue,
    reference_date: &ActionValue,
    resolver: &R,
    depth: usize,
) -> Result<Value> {
    let dob_val = evaluate_value(date_of_birth, resolver, depth)?;
    let ref_val = evaluate_value(reference_date, resolver, depth)?;

    if let Some(tainted) = propagate_binary(&dob_val, &ref_val) {
        return Ok(tainted);
    }

    // Null propagation: if either input is null, return null
    if matches!(dob_val, Value::Null) || matches!(ref_val, Value::Null) {
        return Ok(Value::Null);
    }

    let dob_date = match parse_date(&dob_val)? {
        Some(d) => d,
        None => return Ok(Value::Null),
    };
    let ref_date_parsed = match parse_date(&ref_val)? {
        Some(d) => d,
        None => return Ok(Value::Null),
    };

    let age = calculate_years_difference(ref_date_parsed, dob_date);
    Ok(Value::Int(age))
}

/// Execute DATE_ADD operation: add years, months, weeks, and/or days to a date.
///
/// Applied in order: years → months → weeks → days (coarsest to finest).
///
/// For months and years, uses standard calendar arithmetic: the result lands on
/// the same day number in the target month, clamped to the last day of that month
/// when the day doesn't exist. E.g., Jan 31 + 1 month = Feb 28 (or 29 in leap year).
///
/// This is not domain knowledge in the engine — it is pure calendar math. The Hoge
/// Raad confirmed that Dutch legal termijnberekening follows standard calendar
/// arithmetic (HR 1 September 2017, ECLI:NL:HR:2017:2225).
fn execute_date_add<R: ValueResolver>(
    date: &ActionValue,
    years: Option<&ActionValue>,
    months: Option<&ActionValue>,
    weeks: Option<&ActionValue>,
    days: Option<&ActionValue>,
    resolver: &R,
    depth: usize,
) -> Result<Value> {
    let date_val = evaluate_value(date, resolver, depth)?;
    if date_val.is_untranslatable() {
        return Ok(date_val);
    }
    // Null propagation: if date input is Null, return Null
    if date_val.is_null() {
        return Ok(Value::Null);
    }
    let mut result_date = match parse_date(&date_val)? {
        Some(d) => d,
        None => return Ok(Value::Null),
    };

    // Years: add to year component, clamp day to last day of target month
    if let Some(years) = years {
        let years_val = evaluate_value(years, resolver, depth)?;
        if years_val.is_null() {
            return Ok(Value::Null);
        }
        let years_i64 = years_val.as_int().ok_or_else(|| {
            EngineError::InvalidOperation("DATE_ADD 'years' must be a number".to_string())
        })?;
        let years_int = i32::try_from(years_i64).map_err(|_| {
            EngineError::InvalidOperation(format!(
                "DATE_ADD 'years' value {} exceeds supported range",
                years_i64
            ))
        })?;
        let target_year = result_date.year() + years_int;
        let clamped_day = result_date
            .day()
            .min(days_in_month(target_year, result_date.month()));
        result_date = NaiveDate::from_ymd_opt(target_year, result_date.month(), clamped_day)
            .ok_or_else(|| {
                EngineError::InvalidOperation(format!(
                    "DATE_ADD: invalid date after adding {} years",
                    years_int
                ))
            })?;
    }

    // Months: add to month component, clamp day to last day of target month
    if let Some(months) = months {
        let months_val = evaluate_value(months, resolver, depth)?;
        if months_val.is_null() {
            return Ok(Value::Null);
        }
        let months_int = months_val.as_int().ok_or_else(|| {
            EngineError::InvalidOperation("DATE_ADD 'months' must be a number".to_string())
        })?;
        result_date = add_months(result_date, months_int)?;
    }

    // Weeks
    if let Some(weeks) = weeks {
        let weeks_val = evaluate_value(weeks, resolver, depth)?;
        if weeks_val.is_null() {
            return Ok(Value::Null);
        }
        let weeks_int = weeks_val.as_int().ok_or_else(|| {
            EngineError::InvalidOperation("DATE_ADD 'weeks' must be a number".to_string())
        })?;
        result_date = result_date
            .checked_add_signed(chrono::Duration::weeks(weeks_int))
            .ok_or_else(|| {
                EngineError::InvalidOperation(
                    "DATE_ADD: date out of range after adding weeks".to_string(),
                )
            })?;
    }

    // Days
    if let Some(days) = days {
        let days_val = evaluate_value(days, resolver, depth)?;
        if days_val.is_null() {
            return Ok(Value::Null);
        }
        let days_int = days_val.as_int().ok_or_else(|| {
            EngineError::InvalidOperation("DATE_ADD 'days' must be a number".to_string())
        })?;
        result_date = result_date
            .checked_add_signed(chrono::Duration::days(days_int))
            .ok_or_else(|| {
                EngineError::InvalidOperation(
                    "DATE_ADD: date out of range after adding days".to_string(),
                )
            })?;
    }

    Ok(Value::String(result_date.format("%Y-%m-%d").to_string()))
}

/// Add months to a date using standard calendar arithmetic.
///
/// Clamps the day to the last day of the target month when the original day
/// doesn't exist in that month (e.g., Jan 31 + 1 month = Feb 28).
fn add_months(date: NaiveDate, months: i64) -> Result<NaiveDate> {
    let total_months = date.year() as i64 * 12 + (date.month() as i64 - 1) + months;
    let target_year = i32::try_from(total_months.div_euclid(12)).map_err(|_| {
        EngineError::InvalidOperation(format!(
            "DATE_ADD: year out of range after adding {} months",
            months
        ))
    })?;
    let target_month = (total_months.rem_euclid(12) + 1) as u32;
    let clamped_day = date.day().min(days_in_month(target_year, target_month));

    NaiveDate::from_ymd_opt(target_year, target_month, clamped_day).ok_or_else(|| {
        EngineError::InvalidOperation(format!(
            "DATE_ADD: invalid date after adding {} months",
            months
        ))
    })
}

/// Execute DATE operation: construct a date from year, month, day components.
///
/// # Arguments
/// - `year`: Year component (integer)
/// - `month`: Month component (integer, 1-12)
/// - `day`: Day component (integer, 1-31)
fn execute_date_construct<R: ValueResolver>(
    year: &ActionValue,
    month: &ActionValue,
    day: &ActionValue,
    resolver: &R,
    depth: usize,
) -> Result<Value> {
    let year_val = evaluate_value(year, resolver, depth)?;
    let month_val = evaluate_value(month, resolver, depth)?;
    let day_val = evaluate_value(day, resolver, depth)?;

    if let Some(tainted) =
        find_untranslatable(&[year_val.clone(), month_val.clone(), day_val.clone()])
    {
        return Ok(tainted);
    }

    // Null propagation: if any component is Null, return Null
    if let Some(null) = find_null(&[year_val.clone(), month_val.clone(), day_val.clone()]) {
        return Ok(null);
    }

    let y_i64 = year_val
        .as_int()
        .ok_or_else(|| EngineError::InvalidOperation("DATE 'year' must be a number".to_string()))?;
    let y = i32::try_from(y_i64).map_err(|_| {
        EngineError::InvalidOperation(format!(
            "DATE 'year' value {} exceeds supported range",
            y_i64
        ))
    })?;
    let m = month_val
        .as_int()
        .ok_or_else(|| EngineError::InvalidOperation("DATE 'month' must be a number".to_string()))?
        as u32;
    let d = day_val
        .as_int()
        .ok_or_else(|| EngineError::InvalidOperation("DATE 'day' must be a number".to_string()))?
        as u32;

    let date = NaiveDate::from_ymd_opt(y, m, d).ok_or_else(|| {
        EngineError::InvalidOperation(format!("DATE: invalid date {}-{}-{}", y, m, d))
    })?;

    Ok(Value::String(date.format("%Y-%m-%d").to_string()))
}

/// Execute DAY_OF_WEEK operation: get the weekday number for a date.
///
/// Returns an integer where 0=Monday, 6=Sunday.
///
/// # Arguments
/// - `date`: Date to get weekday for (ISO 8601 YYYY-MM-DD)
fn execute_day_of_week<R: ValueResolver>(
    date_value: &ActionValue,
    resolver: &R,
    depth: usize,
) -> Result<Value> {
    let val = evaluate_value(date_value, resolver, depth)?;
    if val.is_untranslatable() {
        return Ok(val);
    }
    // Null propagation: if date is Null, return Null
    if val.is_null() {
        return Ok(Value::Null);
    }
    let parsed = match parse_date(&val)? {
        Some(d) => d,
        None => return Ok(Value::Null),
    };
    Ok(Value::Int(parsed.weekday().num_days_from_monday() as i64))
}

/// Parse a date from a Value.
///
/// Expects the value to be a string in ISO 8601 format (YYYY-MM-DD).
fn parse_date(value: &Value) -> Result<Option<NaiveDate>> {
    match value {
        // Null propagation: Null date input returns None (caller produces Value::Null)
        Value::Null => Ok(None),
        Value::String(s) => NaiveDate::parse_from_str(s, "%Y-%m-%d")
            .map(Some)
            .map_err(|e| {
                EngineError::InvalidOperation(format!(
                    "Failed to parse date '{}': {}. Expected format: YYYY-MM-DD",
                    s, e
                ))
            }),
        // Handle referencedate objects with {iso, year, month, day}
        Value::Object(obj) => {
            if let Some(Value::String(iso)) = obj.get("iso") {
                NaiveDate::parse_from_str(iso, "%Y-%m-%d")
                    .map(Some)
                    .map_err(|e| {
                        EngineError::InvalidOperation(format!(
                            "Failed to parse date '{}': {}. Expected format: YYYY-MM-DD",
                            iso, e
                        ))
                    })
            } else {
                Err(EngineError::TypeMismatch {
                    expected: "date string (YYYY-MM-DD) or object with 'iso' field".to_string(),
                    actual: format!("{:?}", value),
                })
            }
        }
        _ => Err(EngineError::TypeMismatch {
            expected: "date string (YYYY-MM-DD)".to_string(),
            actual: format!("{:?}", value),
        }),
    }
}

/// Calculate the difference in complete months between two dates.
///
/// Uses proper calendar arithmetic. A month is counted as complete when
/// the same day-of-month (or end of month if day doesn't exist) is reached.
/// For end-of-month edge cases (e.g., Jan 31 -> Feb 28), if `earlier.day()`
/// exceeds the number of days in `later`'s month, it is capped to the last
/// day of that month so the month is correctly counted as complete.
#[allow(dead_code)] // Retained for potential future date-difference operations
fn calculate_months_difference(date1: NaiveDate, date2: NaiveDate) -> i64 {
    let (earlier, later, sign) = if date1 >= date2 {
        (date2, date1, 1)
    } else {
        (date1, date2, -1)
    };

    let years_diff = later.year() - earlier.year();
    let months_diff = later.month() as i32 - earlier.month() as i32;
    let mut total_months = years_diff * 12 + months_diff;

    // Cap earlier.day() to the max days in later's month so that
    // Jan 31 → Feb 28 counts as 1 month (28 is the last day of Feb).
    let max_day_in_later_month = days_in_month(later.year(), later.month());
    let earlier_day_capped = earlier.day().min(max_day_in_later_month);

    // Adjust if we haven't reached the (capped) day in the month
    if later.day() < earlier_day_capped {
        total_months -= 1;
    }

    (total_months as i64) * sign
}

/// Return the number of days in a given month.
fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if NaiveDate::from_ymd_opt(year, 2, 29).is_some() {
                29
            } else {
                28
            }
        }
        _ => unreachable!("Invalid month: {month}"),
    }
}

/// Calculate the difference in complete years between two dates.
///
/// Uses proper calendar arithmetic. A year is counted as complete when
/// the anniversary date (or Feb 28 for leap year births on Feb 29) is reached.
fn calculate_years_difference(date1: NaiveDate, date2: NaiveDate) -> i64 {
    let (earlier, later, sign) = if date1 >= date2 {
        (date2, date1, 1)
    } else {
        (date1, date2, -1)
    };

    let mut years = later.year() - earlier.year();

    // Check if we've reached the anniversary this year.
    // For Feb 29 birthdays in non-leap years, the anniversary falls on Feb 28
    // (per Dutch law: BW art. 1:2, Algemene Termijnenwet).
    let anniversary_month = earlier.month();
    let anniversary_day = {
        let day = earlier.day();
        if anniversary_month == 2 && day == 29 && days_in_month(later.year(), 2) < 29 {
            28
        } else {
            day
        }
    };

    if later.month() < anniversary_month
        || (later.month() == anniversary_month && later.day() < anniversary_day)
    {
        years -= 1;
    }

    (years as i64) * sign
}

// =============================================================================
// Helper Functions
// =============================================================================

/// Safely convert f64 to i64, returning error on overflow/NaN/Infinity
pub(crate) fn f64_to_i64_safe(f: f64) -> Result<i64> {
    if f.is_nan() {
        return Err(EngineError::ArithmeticOverflow("Result is NaN".to_string()));
    }
    if f.is_infinite() {
        return Err(EngineError::ArithmeticOverflow(
            "Result is infinite".to_string(),
        ));
    }

    // i64::MIN as f64 is exact (-9223372036854775808.0).
    // i64::MAX as f64 rounds UP to 9223372036854775808.0, which is i64::MAX + 1.
    // Using strict < on the upper bound prevents accepting that rounded-up value,
    // which would saturate to i64::MAX on `as i64` (semantically wrong).
    const I64_MIN_F64: f64 = i64::MIN as f64;
    const I64_MAX_F64: f64 = i64::MAX as f64;

    if !(I64_MIN_F64..I64_MAX_F64).contains(&f) {
        return Err(EngineError::ArithmeticOverflow(format!(
            "Value {} exceeds i64 range",
            f
        )));
    }

    Ok(f as i64)
}

/// Evaluate a slice of ActionValues to concrete Values.
fn evaluate_values<R: ValueResolver>(
    values: &[ActionValue],
    resolver: &R,
    depth: usize,
) -> Result<Vec<Value>> {
    values
        .iter()
        .map(|v| evaluate_value(v, resolver, depth))
        .collect()
}

/// Convert a Value to a number (f64).
///
/// # Precision
///
/// For integers larger than 2^53 or smaller than -2^53, precision is lost
/// when converting to f64. This function returns an error for such values
/// to prevent silent precision loss in financial/legal calculations.
fn to_number(val: &Value) -> Result<f64> {
    match val {
        Value::Int(i) => {
            // Check if integer is within safe range for f64
            if *i > MAX_SAFE_INTEGER || *i < MIN_SAFE_INTEGER {
                return Err(EngineError::ArithmeticOverflow(format!(
                    "Integer {} exceeds safe range for floating-point conversion (±2^53)",
                    i
                )));
            }
            Ok(*i as f64)
        }
        Value::Float(f) => Ok(*f),
        Value::String(s) => Ok(s.parse::<f64>().unwrap_or(0.0)),
        Value::Bool(b) => Ok(if *b { 1.0 } else { 0.0 }),
        Value::Null => Ok(0.0),
        Value::Untranslatable { .. } => Ok(0.0),
        _ => Ok(0.0), // Lenient: non-numeric types → 0
    }
}

/// Create a TypeMismatch error.
fn type_error(expected: &str, actual: &Value) -> EngineError {
    EngineError::TypeMismatch {
        expected: expected.to_string(),
        actual: actual.type_name().to_string(),
    }
}

// =============================================================================
// Collection Iteration (RFC-016)
// =============================================================================

/// Execute FOREACH operation: iterate over a collection, evaluate body per element.
///
/// FOREACH requires a `RuleContext` (not generic `ValueResolver`) because it needs
/// to create child scopes with local variable bindings. This is the only operation
/// that introduces a new variable name into scope (RFC-016).
///
/// The `collection` is evaluated in the current scope. For each element, a child
/// context is created with the element bound to the `as_name` variable. The `filter`
/// and `body` are evaluated in the child scope.
pub fn execute_foreach(
    collection: &ActionValue,
    as_name: &str,
    body: &ActionValue,
    filter: Option<&ActionValue>,
    combine: Option<&str>,
    context: &RuleContext,
    depth: usize,
) -> Result<Value> {
    // Validate as_name: must be lowercase identifier per RFC-016 schema
    if !as_name.is_empty()
        && !as_name
            .chars()
            .enumerate()
            .all(|(i, c)| match (i, c) {
                (0, 'a'..='z' | '_') => true,
                (_, 'a'..='z' | '0'..='9' | '_') => true,
                _ => false,
            })
    {
        return Err(EngineError::InvalidOperation(format!(
            "FOREACH 'as' must be a lowercase identifier (got '{}')",
            as_name
        )));
    }

    // Evaluate collection in current (outer) scope
    let collection_value = evaluate_value(collection, context, depth)?;

    // Propagate untranslatable
    if collection_value.is_untranslatable() {
        return Ok(collection_value);
    }

    // Get array to iterate over (wrap non-arrays in single-element array, null → empty)
    let items = match &collection_value {
        Value::Array(arr) => arr.clone(),
        Value::Null => Vec::new(),
        other => vec![other.clone()],
    };

    // Security: check array size limit
    if items.len() > crate::config::MAX_ARRAY_SIZE {
        return Err(EngineError::InvalidOperation(format!(
            "FOREACH collection size {} exceeds limit {}",
            items.len(),
            crate::config::MAX_ARRAY_SIZE,
        )));
    }

    // Iterate and collect results
    let mut results: Vec<Value> = Vec::with_capacity(items.len());

    for item in &items {
        // Create child context with the element bound to as_name
        let mut child = context.create_child();
        child.set_local(as_name.to_string(), item.clone());

        // Evaluate filter if present (in child scope)
        if let Some(filter_expr) = filter {
            let filter_result = evaluate_value(filter_expr, &child, depth + 1)?;
            if filter_result.is_untranslatable() {
                return Ok(filter_result);
            }
            if !filter_result.to_bool() {
                continue; // Skip this element
            }
        }

        // Evaluate body (in child scope)
        let result = evaluate_value(body, &child, depth + 1)?;

        // Propagate untranslatable immediately
        if result.is_untranslatable() {
            return Ok(result);
        }

        results.push(result);
    }

    // Apply combine aggregation or return array
    match combine {
        Some("ADD") => combine_add(&results),
        Some("OR") => Ok(Value::Bool(results.iter().any(|v| v.to_bool()))),
        // Vacuous truth: AND over an empty collection returns true, matching standard
        // logical convention (the universal quantifier over an empty domain is true).
        // This aligns with the legal reading "all conditions are met" being trivially
        // satisfied when there are no conditions to check.
        Some("AND") => Ok(Value::Bool(
            results.is_empty() || results.iter().all(|v| v.to_bool()),
        )),
        Some("MIN") => combine_min_max(&results, true),
        Some("MAX") => combine_min_max(&results, false),
        Some(other) => Err(EngineError::InvalidOperation(format!(
            "Unknown FOREACH combine operation: {}",
            other
        ))),
        None => Ok(Value::Array(results)),
    }
}

/// Combine FOREACH results with ADD. Delegates to the shared `add_values` helper.
fn combine_add(results: &[Value]) -> Result<Value> {
    add_values(results)
}

/// Combine results with MIN or MAX.
/// Uses i64 comparison directly when all values are integers to avoid f64 precision loss.
fn combine_min_max(results: &[Value], is_min: bool) -> Result<Value> {
    if results.is_empty() {
        return Ok(Value::Null);
    }

    let mut all_int = true;
    let mut best_int: Option<i64> = None;
    let mut best_float: f64 = if is_min {
        f64::INFINITY
    } else {
        f64::NEG_INFINITY
    };
    let mut has_any_number = false;

    for v in results {
        match v {
            Value::Int(i) => {
                has_any_number = true;
                best_int = Some(match best_int {
                    None => *i,
                    Some(prev) => {
                        if is_min {
                            prev.min(*i)
                        } else {
                            prev.max(*i)
                        }
                    }
                });
                // Also track f64 in case we later encounter a Float
                let n = *i as f64;
                best_float = if is_min {
                    best_float.min(n)
                } else {
                    best_float.max(n)
                };
            }
            Value::Float(f) => {
                has_any_number = true;
                all_int = false;
                best_float = if is_min {
                    best_float.min(*f)
                } else {
                    best_float.max(*f)
                };
            }
            Value::Null => continue,
            _ => {
                return Err(EngineError::TypeMismatch {
                    expected: "number".to_string(),
                    actual: v.type_name().to_string(),
                })
            }
        }
    }

    match (has_any_number, all_int, best_int) {
        (false, _, _) => Ok(Value::Null), // All values were null
        (true, true, Some(i)) => Ok(Value::Int(i)),
        (true, false, _) => Ok(Value::Float(best_float)),
        // Unreachable: has_any_number && all_int implies best_int is Some
        (true, true, None) => Ok(Value::Null),
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{BTreeMap, HashMap};

    /// Simple resolver for testing that uses a HashMap
    struct TestResolver {
        vars: HashMap<String, Value>,
    }

    impl TestResolver {
        fn new() -> Self {
            Self {
                vars: HashMap::new(),
            }
        }

        fn with_var(mut self, name: &str, value: impl Into<Value>) -> Self {
            self.vars.insert(name.to_string(), value.into());
            self
        }
    }

    impl ValueResolver for TestResolver {
        fn resolve(&self, name: &str) -> Result<Value> {
            self.vars
                .get(name)
                .cloned()
                .ok_or_else(|| EngineError::VariableNotFound(name.to_string()))
        }
    }

    /// Helper to create a literal ActionValue
    fn lit(v: impl Into<Value>) -> ActionValue {
        ActionValue::Literal(v.into())
    }

    /// Helper to create a variable reference
    fn var(name: &str) -> ActionValue {
        ActionValue::Literal(Value::String(format!("${}", name)))
    }

    // -------------------------------------------------------------------------
    // Comparison Operations Tests
    // -------------------------------------------------------------------------

    mod comparison {
        use super::*;

        #[test]
        fn test_equals_integers() {
            let resolver = TestResolver::new();
            let op = ActionOperation::Equals {
                subject: lit(42i64),
                value: lit(42i64),
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Bool(true));
        }

        #[test]
        fn test_equals_different_values() {
            let resolver = TestResolver::new();
            let op = ActionOperation::Equals {
                subject: lit(42i64),
                value: lit(43i64),
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Bool(false));
        }

        #[test]
        fn test_greater_than() {
            let resolver = TestResolver::new();
            let op = ActionOperation::GreaterThan {
                subject: lit(50i64),
                value: lit(42i64),
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Bool(true));
        }

        #[test]
        fn test_less_than() {
            let resolver = TestResolver::new();
            let op = ActionOperation::LessThan {
                subject: lit(30i64),
                value: lit(42i64),
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Bool(true));
        }

        #[test]
        fn test_greater_than_or_equal() {
            let resolver = TestResolver::new();

            let op = ActionOperation::GreaterThanOrEqual {
                subject: lit(42i64),
                value: lit(42i64),
            };
            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::Bool(true)
            );

            let op2 = ActionOperation::GreaterThanOrEqual {
                subject: lit(50i64),
                value: lit(42i64),
            };
            assert_eq!(
                execute_operation(&op2, &resolver, 0).unwrap(),
                Value::Bool(true)
            );
        }

        #[test]
        fn test_less_than_or_equal() {
            let resolver = TestResolver::new();

            let op = ActionOperation::LessThanOrEqual {
                subject: lit(42i64),
                value: lit(42i64),
            };
            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::Bool(true)
            );

            let op2 = ActionOperation::LessThanOrEqual {
                subject: lit(30i64),
                value: lit(42i64),
            };
            assert_eq!(
                execute_operation(&op2, &resolver, 0).unwrap(),
                Value::Bool(true)
            );
        }

        #[test]
        fn test_comparison_with_variables() {
            let resolver = TestResolver::new()
                .with_var("age", 25i64)
                .with_var("min_age", 18i64);

            let op = ActionOperation::GreaterThanOrEqual {
                subject: var("age"),
                value: var("min_age"),
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Bool(true));
        }

        #[test]
        fn test_comparison_mixed_int_float() {
            let resolver = TestResolver::new();

            let op = ActionOperation::Equals {
                subject: lit(42i64),
                value: lit(42.0f64),
            };
            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::Bool(true)
            );

            let op2 = ActionOperation::Equals {
                subject: lit(42.0f64),
                value: lit(42i64),
            };
            assert_eq!(
                execute_operation(&op2, &resolver, 0).unwrap(),
                Value::Bool(true)
            );
        }
    }

    // -------------------------------------------------------------------------
    // Arithmetic Operations Tests
    // -------------------------------------------------------------------------

    mod arithmetic {
        use super::*;

        #[test]
        fn test_add_integers() {
            let resolver = TestResolver::new();
            let op = ActionOperation::Add {
                values: vec![lit(10i64), lit(20i64), lit(30i64)],
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Int(60));
        }

        #[test]
        fn test_add_with_floats() {
            let resolver = TestResolver::new();
            let op = ActionOperation::Add {
                values: vec![lit(10i64), lit(20.5f64), lit(30i64)],
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Float(60.5));
        }

        #[test]
        fn test_subtract() {
            let resolver = TestResolver::new();
            let op = ActionOperation::Subtract {
                values: vec![lit(100i64), lit(30i64), lit(20i64)],
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Int(50));
        }

        #[test]
        fn test_multiply() {
            let resolver = TestResolver::new();
            let op = ActionOperation::Multiply {
                values: vec![lit(2i64), lit(3i64), lit(4i64)],
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Int(24));
        }

        #[test]
        fn test_divide() {
            let resolver = TestResolver::new();
            let op = ActionOperation::Divide {
                values: vec![lit(100i64), lit(2i64)],
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Float(50.0));
        }

        #[test]
        fn test_divide_by_zero() {
            let resolver = TestResolver::new();
            let op = ActionOperation::Divide {
                values: vec![lit(100i64), lit(0i64)],
            };

            let result = execute_operation(&op, &resolver, 0);
            assert!(matches!(result, Err(EngineError::DivisionByZero)));
        }

        #[test]
        fn test_arithmetic_with_variables() {
            let resolver = TestResolver::new()
                .with_var("base", 1000i64)
                .with_var("rate", 0.05f64);

            let op = ActionOperation::Multiply {
                values: vec![var("base"), var("rate")],
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Float(50.0));
        }

        #[test]
        fn test_add_arrays() {
            let resolver = TestResolver::new();
            let op = ActionOperation::Add {
                values: vec![
                    lit(Value::Array(vec![Value::Int(1), Value::Int(2)])),
                    lit(Value::Array(vec![Value::Int(3), Value::Int(4)])),
                ],
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(
                result,
                Value::Array(vec![
                    Value::Int(1),
                    Value::Int(2),
                    Value::Int(3),
                    Value::Int(4)
                ])
            );
        }

        #[test]
        fn test_add_strings() {
            let resolver = TestResolver::new();
            let op = ActionOperation::Add {
                values: vec![lit("hello"), lit(" "), lit("world")],
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::String("hello world".to_string()));
        }
    }

    // -------------------------------------------------------------------------
    // Aggregate Operations Tests
    // -------------------------------------------------------------------------

    mod aggregate {
        use super::*;

        #[test]
        fn test_max() {
            let resolver = TestResolver::new();
            let op = ActionOperation::Max {
                values: vec![lit(10i64), lit(50i64), lit(30i64)],
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Int(50));
        }

        #[test]
        fn test_min() {
            let resolver = TestResolver::new();
            let op = ActionOperation::Min {
                values: vec![lit(10i64), lit(50i64), lit(30i64)],
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Int(10));
        }

        #[test]
        fn test_max_with_floats() {
            let resolver = TestResolver::new();
            let op = ActionOperation::Max {
                values: vec![lit(10.5f64), lit(50.3f64), lit(30.7f64)],
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Float(50.3));
        }

        #[test]
        fn test_max_with_zero() {
            let resolver = TestResolver::new();
            let op = ActionOperation::Max {
                values: vec![lit(0i64), lit(-10i64)],
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Int(0));
        }
    }

    // -------------------------------------------------------------------------
    // Logical Operations Tests
    // -------------------------------------------------------------------------

    mod logical {
        use super::*;

        #[test]
        fn test_and_all_true() {
            let resolver = TestResolver::new();
            let op = ActionOperation::And {
                conditions: vec![lit(true), lit(true), lit(true)],
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Bool(true));
        }

        #[test]
        fn test_and_one_false() {
            let resolver = TestResolver::new();
            let op = ActionOperation::And {
                conditions: vec![lit(true), lit(false), lit(true)],
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Bool(false));
        }

        #[test]
        fn test_or_one_true() {
            let resolver = TestResolver::new();
            let op = ActionOperation::Or {
                conditions: vec![lit(false), lit(true), lit(false)],
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Bool(true));
        }

        #[test]
        fn test_or_all_false() {
            let resolver = TestResolver::new();
            let op = ActionOperation::Or {
                conditions: vec![lit(false), lit(false), lit(false)],
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Bool(false));
        }

        #[test]
        fn test_and_with_nested_comparison() {
            let resolver = TestResolver::new()
                .with_var("age", 25i64)
                .with_var("has_insurance", true);

            let age_check_op = ActionOperation::GreaterThanOrEqual {
                subject: var("age"),
                value: lit(18i64),
            };
            let age_check = ActionValue::Operation(Box::new(age_check_op));

            let op = ActionOperation::And {
                conditions: vec![age_check, var("has_insurance")],
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Bool(true));
        }

        #[test]
        fn test_not_negates_true() {
            let resolver = TestResolver::new();
            let op = ActionOperation::Not { value: lit(true) };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Bool(false));
        }

        #[test]
        fn test_not_negates_false() {
            let resolver = TestResolver::new();
            let op = ActionOperation::Not { value: lit(false) };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Bool(true));
        }

        #[test]
        fn test_not_wrapping_equals() {
            let resolver = TestResolver::new();

            // NOT(EQUALS(42, 42)) should be false
            let eq_op = ActionOperation::Equals {
                subject: lit(42i64),
                value: lit(42i64),
            };

            let op = ActionOperation::Not {
                value: ActionValue::Operation(Box::new(eq_op)),
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Bool(false));

            // NOT(EQUALS(42, 43)) should be true
            let eq_op2 = ActionOperation::Equals {
                subject: lit(42i64),
                value: lit(43i64),
            };

            let op2 = ActionOperation::Not {
                value: ActionValue::Operation(Box::new(eq_op2)),
            };

            let result2 = execute_operation(&op2, &resolver, 0).unwrap();
            assert_eq!(result2, Value::Bool(true));
        }
    }

    // -------------------------------------------------------------------------
    // Conditional Operations Tests (IF with cases/default)
    // -------------------------------------------------------------------------

    mod conditional {
        use super::*;

        #[test]
        fn test_if_first_match() {
            let resolver = TestResolver::new();
            let op = ActionOperation::If {
                cases: vec![
                    Case {
                        when: lit(true),
                        then: lit(100i64),
                    },
                    Case {
                        when: lit(true),
                        then: lit(200i64),
                    },
                ],
                default: Some(lit(0i64)),
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Int(100));
        }

        #[test]
        fn test_if_second_match() {
            let resolver = TestResolver::new();
            let op = ActionOperation::If {
                cases: vec![
                    Case {
                        when: lit(false),
                        then: lit(100i64),
                    },
                    Case {
                        when: lit(true),
                        then: lit(200i64),
                    },
                ],
                default: Some(lit(0i64)),
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Int(200));
        }

        #[test]
        fn test_if_default() {
            let resolver = TestResolver::new();
            let op = ActionOperation::If {
                cases: vec![
                    Case {
                        when: lit(false),
                        then: lit(100i64),
                    },
                    Case {
                        when: lit(false),
                        then: lit(200i64),
                    },
                ],
                default: Some(lit(0i64)),
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Int(0));
        }

        #[test]
        fn test_if_no_default_returns_null() {
            let resolver = TestResolver::new();
            let op = ActionOperation::If {
                cases: vec![Case {
                    when: lit(false),
                    then: lit(100i64),
                }],
                default: None,
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Null);
        }

        #[test]
        fn test_if_with_nested_conditions() {
            let resolver = TestResolver::new().with_var("status", "active");

            let pending_check = ActionOperation::Equals {
                subject: var("status"),
                value: lit("pending"),
            };

            let active_check = ActionOperation::Equals {
                subject: var("status"),
                value: lit("active"),
            };

            let op = ActionOperation::If {
                cases: vec![
                    Case {
                        when: ActionValue::Operation(Box::new(pending_check)),
                        then: lit(10i64),
                    },
                    Case {
                        when: ActionValue::Operation(Box::new(active_check)),
                        then: lit(20i64),
                    },
                ],
                default: Some(lit(0i64)),
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Int(20));
        }
    }

    // -------------------------------------------------------------------------
    // Nested Operations Tests
    // -------------------------------------------------------------------------

    mod nested {
        use super::*;

        #[test]
        fn test_nested_arithmetic_in_max() {
            // MAX(0, 100 - 50) = MAX(0, 50) = 50
            let resolver = TestResolver::new();

            let sub_op = ActionOperation::Subtract {
                values: vec![lit(100i64), lit(50i64)],
            };
            let subtract_val = ActionValue::Operation(Box::new(sub_op));

            let op = ActionOperation::Max {
                values: vec![lit(0i64), subtract_val],
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Int(50));
        }

        #[test]
        fn test_deeply_nested_operations() {
            // IF (10 > 5) THEN (2 * 3) ELSE (1 + 1)
            // Expressed as IF with cases: [case(10>5 -> 2*3)], default: 1+1
            let resolver = TestResolver::new();

            let gt_op = ActionOperation::GreaterThan {
                subject: lit(10i64),
                value: lit(5i64),
            };
            let condition = ActionValue::Operation(Box::new(gt_op));

            let mul_op = ActionOperation::Multiply {
                values: vec![lit(2i64), lit(3i64)],
            };
            let then_branch = ActionValue::Operation(Box::new(mul_op));

            let add_op = ActionOperation::Add {
                values: vec![lit(1i64), lit(1i64)],
            };
            let else_branch = ActionValue::Operation(Box::new(add_op));

            let op = ActionOperation::If {
                cases: vec![Case {
                    when: condition,
                    then: then_branch,
                }],
                default: Some(else_branch),
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Int(6));
        }
    }

    // -------------------------------------------------------------------------
    // Error Handling Tests
    // -------------------------------------------------------------------------

    mod errors {
        use super::*;

        #[test]
        fn test_type_mismatch_in_arithmetic() {
            let resolver = TestResolver::new();
            let op = ActionOperation::Add {
                values: vec![lit(10i64), lit("not a number")],
            };

            let result = execute_operation(&op, &resolver, 0);
            assert!(matches!(result, Err(EngineError::TypeMismatch { .. })));
        }

        #[test]
        fn test_variable_not_found_returns_null() {
            let resolver = TestResolver::new();
            // Unresolved variables return Null (lenient mode)
            let val = var("nonexistent");
            let result = evaluate_value(&val, &resolver, 0);
            assert!(matches!(result, Ok(Value::Null)));
        }

        #[test]
        fn test_overflow_detection() {
            let resolver = TestResolver::new();
            let op = ActionOperation::Multiply {
                values: vec![lit(i64::MAX), lit(2i64)],
            };

            let result = execute_operation(&op, &resolver, 0);
            assert!(matches!(result, Err(EngineError::ArithmeticOverflow(_))));
        }

        #[test]
        fn test_max_depth_exceeded() {
            let resolver = TestResolver::new();

            let mut nested: ActionValue = lit(42i64);
            for _ in 0..=MAX_OPERATION_DEPTH + 1 {
                let if_op = ActionOperation::If {
                    cases: vec![Case {
                        when: lit(true),
                        then: nested,
                    }],
                    default: None,
                };
                nested = ActionValue::Operation(Box::new(if_op));
            }

            if let ActionValue::Operation(op) = nested {
                let result = execute_operation(&op, &resolver, 0);
                assert!(
                    matches!(result, Err(EngineError::MaxDepthExceeded(_))),
                    "Expected MaxDepthExceeded error"
                );
            }
        }

        #[test]
        fn test_nan_detection_in_f64_to_i64() {
            let result = f64_to_i64_safe(f64::NAN);
            assert!(matches!(result, Err(EngineError::ArithmeticOverflow(_))));
            if let Err(EngineError::ArithmeticOverflow(msg)) = result {
                assert!(msg.contains("NaN"));
            }
        }

        #[test]
        fn test_infinity_detection_in_f64_to_i64() {
            let result_pos = f64_to_i64_safe(f64::INFINITY);
            assert!(matches!(
                result_pos,
                Err(EngineError::ArithmeticOverflow(_))
            ));

            let result_neg = f64_to_i64_safe(f64::NEG_INFINITY);
            assert!(matches!(
                result_neg,
                Err(EngineError::ArithmeticOverflow(_))
            ));
        }

        #[test]
        fn test_i64_range_overflow_detection() {
            let result_high = f64_to_i64_safe(1e20);
            assert!(matches!(
                result_high,
                Err(EngineError::ArithmeticOverflow(_))
            ));

            let result_low = f64_to_i64_safe(-1e20);
            assert!(matches!(
                result_low,
                Err(EngineError::ArithmeticOverflow(_))
            ));
        }

        #[test]
        fn test_valid_f64_to_i64_conversion() {
            assert_eq!(f64_to_i64_safe(42.0).unwrap(), 42);
            assert_eq!(f64_to_i64_safe(-42.0).unwrap(), -42);
            assert_eq!(f64_to_i64_safe(0.0).unwrap(), 0);
            assert_eq!(f64_to_i64_safe(0.9).unwrap(), 0);
            assert_eq!(f64_to_i64_safe(-0.9).unwrap(), 0);
        }

        #[test]
        fn test_f64_to_i64_rejects_i64_max_as_f64() {
            let result = f64_to_i64_safe(i64::MAX as f64);
            assert!(
                matches!(result, Err(EngineError::ArithmeticOverflow(_))),
                "Expected ArithmeticOverflow for i64::MAX as f64, got: {:?}",
                result
            );
        }

        #[test]
        fn test_nan_equality() {
            let nan1 = Value::Float(f64::NAN);
            let nan2 = Value::Float(f64::NAN);
            assert!(
                values_equal(&nan1, &nan2),
                "Two NaN values should be considered equal"
            );

            assert!(!values_equal(&Value::Float(f64::NAN), &Value::Int(42)));
            assert!(!values_equal(&Value::Int(42), &Value::Float(f64::NAN)));
            assert!(!values_equal(&Value::Float(f64::NAN), &Value::Float(42.0)));
        }

        #[test]
        fn test_large_integer_precision_error() {
            let large_int = Value::Int(MAX_SAFE_INTEGER + 1);
            assert!(matches!(
                to_number(&large_int),
                Err(EngineError::ArithmeticOverflow(_))
            ));

            let small_int = Value::Int(MIN_SAFE_INTEGER - 1);
            assert!(matches!(
                to_number(&small_int),
                Err(EngineError::ArithmeticOverflow(_))
            ));

            let safe_int = Value::Int(MAX_SAFE_INTEGER);
            assert_eq!(to_number(&safe_int).unwrap(), MAX_SAFE_INTEGER as f64);

            let safe_neg = Value::Int(MIN_SAFE_INTEGER);
            assert_eq!(to_number(&safe_neg).unwrap(), MIN_SAFE_INTEGER as f64);
        }

        #[test]
        fn test_arithmetic_with_large_integer() {
            // Values beyond f64's safe integer range (2^53) are fine with i64::checked_add
            let large_value: i64 = 9_007_199_254_740_993; // MAX_SAFE_INTEGER + 1
            let resolver = TestResolver::new();
            let op = ActionOperation::Add {
                values: vec![lit(large_value), lit(1i64)],
            };
            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Int(9_007_199_254_740_994));

            // Actual i64 overflow should still produce an error
            let op = ActionOperation::Add {
                values: vec![lit(i64::MAX), lit(1i64)],
            };
            let result = execute_operation(&op, &resolver, 0);
            assert!(
                matches!(result, Err(EngineError::ArithmeticOverflow(_))),
                "i64 overflow in ADD should cause arithmetic overflow error"
            );
        }
    }

    // -------------------------------------------------------------------------
    // Collection Operations Tests
    // -------------------------------------------------------------------------

    mod collection {
        use super::*;

        #[test]
        fn test_in_found() {
            let resolver = TestResolver::new();
            let op = ActionOperation::In {
                subject: lit(42i64),
                value: None,
                values: Some(vec![lit(10i64), lit(20i64), lit(42i64), lit(50i64)]),
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Bool(true));
        }

        #[test]
        fn test_in_not_found() {
            let resolver = TestResolver::new();
            let op = ActionOperation::In {
                subject: lit(99i64),
                value: None,
                values: Some(vec![lit(10i64), lit(20i64), lit(42i64), lit(50i64)]),
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Bool(false));
        }

        #[test]
        fn test_in_with_strings() {
            let resolver = TestResolver::new();
            let op = ActionOperation::In {
                subject: lit("apple"),
                value: None,
                values: Some(vec![lit("banana"), lit("apple"), lit("orange")]),
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Bool(true));
        }

        #[test]
        fn test_in_with_mixed_int_float() {
            let resolver = TestResolver::new();
            let op = ActionOperation::In {
                subject: lit(42i64),
                value: None,
                values: Some(vec![lit(10i64), lit(42.0f64), lit(50i64)]),
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Bool(true));
        }

        #[test]
        fn test_in_with_variables() {
            let resolver = TestResolver::new().with_var("status", "active");

            let op = ActionOperation::In {
                subject: var("status"),
                value: None,
                values: Some(vec![lit("active"), lit("pending"), lit("inactive")]),
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Bool(true));
        }

        #[test]
        fn test_in_missing_values() {
            let resolver = TestResolver::new();
            let op = ActionOperation::In {
                subject: lit(42i64),
                value: None,
                values: None,
            };

            let result = execute_operation(&op, &resolver, 0);
            assert!(matches!(result, Err(EngineError::InvalidOperation(_))));
        }

        #[test]
        fn test_list_construct() {
            let resolver = TestResolver::new();
            let op = ActionOperation::List {
                items: vec![lit(1i64), lit(2i64), lit(3i64)],
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(
                result,
                Value::Array(vec![Value::Int(1), Value::Int(2), Value::Int(3)])
            );
        }

        #[test]
        fn test_list_with_mixed_types() {
            let resolver = TestResolver::new();
            let op = ActionOperation::List {
                items: vec![lit(1i64), lit("two"), lit(true)],
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(
                result,
                Value::Array(vec![
                    Value::Int(1),
                    Value::String("two".to_string()),
                    Value::Bool(true),
                ])
            );
        }
    }

    // -------------------------------------------------------------------------
    // Date Operations Tests
    // -------------------------------------------------------------------------

    mod date_operations {
        use super::*;

        #[test]
        fn test_age_exact_birthday() {
            let resolver = TestResolver::new();
            let op = ActionOperation::Age {
                date_of_birth: lit("1990-03-15"),
                reference_date: lit("2025-03-15"),
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Int(35));
        }

        #[test]
        fn test_age_before_birthday() {
            let resolver = TestResolver::new();
            let op = ActionOperation::Age {
                date_of_birth: lit("1990-03-15"),
                reference_date: lit("2025-03-14"),
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Int(34));
        }

        #[test]
        fn test_age_feb29_birthday_on_non_leap_year() {
            // Per Dutch law (BW art. 1:2): Feb 28 counts as birthday
            let resolver = TestResolver::new();
            let op = ActionOperation::Age {
                date_of_birth: lit("2000-02-29"),
                reference_date: lit("2001-02-28"),
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Int(1));
        }

        #[test]
        fn test_age_feb29_birthday_before_feb28() {
            let resolver = TestResolver::new();
            let op = ActionOperation::Age {
                date_of_birth: lit("2000-02-29"),
                reference_date: lit("2001-02-27"),
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Int(0));
        }

        #[test]
        fn test_age_feb29_birthday_on_leap_year() {
            let resolver = TestResolver::new();
            let op = ActionOperation::Age {
                date_of_birth: lit("2000-02-29"),
                reference_date: lit("2004-02-29"),
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Int(4));
        }

        #[test]
        fn test_age_with_variables() {
            let resolver = TestResolver::new()
                .with_var("birth_date", "1990-03-15")
                .with_var("ref_date", "2025-03-15");

            let op = ActionOperation::Age {
                date_of_birth: var("birth_date"),
                reference_date: var("ref_date"),
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Int(35));
        }

        #[test]
        fn test_age_with_object_date() {
            let mut date_obj = BTreeMap::new();
            date_obj.insert("iso".to_string(), Value::String("2025-01-01".to_string()));
            date_obj.insert("year".to_string(), Value::Int(2025));
            date_obj.insert("month".to_string(), Value::Int(1));
            date_obj.insert("day".to_string(), Value::Int(1));

            let resolver = TestResolver::new()
                .with_var("referencedate", Value::Object(date_obj))
                .with_var("geboortedatum", Value::String("2005-01-01".to_string()));

            let op = ActionOperation::Age {
                date_of_birth: var("geboortedatum"),
                reference_date: var("referencedate"),
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Int(20));
        }

        #[test]
        fn test_date_add_days() {
            let resolver = TestResolver::new();
            let op = ActionOperation::DateAdd {
                date: lit("2025-01-10"),
                years: None,
                months: None,
                days: Some(lit(5i64)),
                weeks: None,
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::String("2025-01-15".to_string()));
        }

        #[test]
        fn test_date_add_weeks() {
            let resolver = TestResolver::new();
            let op = ActionOperation::DateAdd {
                date: lit("2025-01-01"),
                years: None,
                months: None,
                days: None,
                weeks: Some(lit(2i64)),
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::String("2025-01-15".to_string()));
        }

        #[test]
        fn test_date_add_days_and_weeks() {
            let resolver = TestResolver::new();
            let op = ActionOperation::DateAdd {
                date: lit("2025-01-01"),
                years: None,
                months: None,
                days: Some(lit(3i64)),
                weeks: Some(lit(1i64)),
            };

            // 1 week + 3 days = 10 days from Jan 1 = Jan 11
            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::String("2025-01-11".to_string()));
        }

        #[test]
        fn test_date_add_negative_days() {
            let resolver = TestResolver::new();
            let op = ActionOperation::DateAdd {
                date: lit("2025-01-15"),
                years: None,
                months: None,
                days: Some(lit(-5i64)),
                weeks: None,
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::String("2025-01-10".to_string()));
        }

        #[test]
        fn test_date_add_months() {
            let resolver = TestResolver::new();
            let op = ActionOperation::DateAdd {
                date: lit("2025-03-15"),
                years: None,
                months: Some(lit(2i64)),
                weeks: None,
                days: None,
            };
            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::String("2025-05-15".to_string()));
        }

        #[test]
        fn test_date_add_months_end_of_month_clamping() {
            // Jan 31 + 1 month = Feb 28 (not March 1)
            let resolver = TestResolver::new();
            let op = ActionOperation::DateAdd {
                date: lit("2025-01-31"),
                years: None,
                months: Some(lit(1i64)),
                weeks: None,
                days: None,
            };
            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::String("2025-02-28".to_string()));
        }

        #[test]
        fn test_date_add_months_end_of_month_leap_year() {
            // Jan 31 + 1 month in leap year = Feb 29
            let resolver = TestResolver::new();
            let op = ActionOperation::DateAdd {
                date: lit("2024-01-31"),
                years: None,
                months: Some(lit(1i64)),
                weeks: None,
                days: None,
            };
            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::String("2024-02-29".to_string()));
        }

        #[test]
        fn test_date_add_months_negative() {
            let resolver = TestResolver::new();
            let op = ActionOperation::DateAdd {
                date: lit("2025-03-31"),
                years: None,
                months: Some(lit(-1i64)),
                weeks: None,
                days: None,
            };
            // Mar 31 - 1 month = Feb 28
            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::String("2025-02-28".to_string()));
        }

        #[test]
        fn test_date_add_months_six_months() {
            // "binnen zes maanden" from Aug 31
            let resolver = TestResolver::new();
            let op = ActionOperation::DateAdd {
                date: lit("2025-08-31"),
                years: None,
                months: Some(lit(6i64)),
                weeks: None,
                days: None,
            };
            // Aug 31 + 6 months = Feb 28
            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::String("2026-02-28".to_string()));
        }

        #[test]
        fn test_date_add_years() {
            let resolver = TestResolver::new();
            let op = ActionOperation::DateAdd {
                date: lit("2025-06-15"),
                years: Some(lit(2i64)),
                months: None,
                weeks: None,
                days: None,
            };
            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::String("2027-06-15".to_string()));
        }

        #[test]
        fn test_date_add_years_leap_day() {
            // Feb 29 + 1 year = Feb 28 (non-leap year)
            let resolver = TestResolver::new();
            let op = ActionOperation::DateAdd {
                date: lit("2024-02-29"),
                years: Some(lit(1i64)),
                months: None,
                weeks: None,
                days: None,
            };
            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::String("2025-02-28".to_string()));
        }

        #[test]
        fn test_date_add_years_leap_day_to_leap_year() {
            // Feb 29 + 4 years = Feb 29 (another leap year)
            let resolver = TestResolver::new();
            let op = ActionOperation::DateAdd {
                date: lit("2024-02-29"),
                years: Some(lit(4i64)),
                months: None,
                weeks: None,
                days: None,
            };
            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::String("2028-02-29".to_string()));
        }

        #[test]
        fn test_date_add_combined_years_months_weeks_days() {
            // 2025-01-15 + 1 year + 2 months + 1 week + 3 days
            let resolver = TestResolver::new();
            let op = ActionOperation::DateAdd {
                date: lit("2025-01-15"),
                years: Some(lit(1i64)),
                months: Some(lit(2i64)),
                weeks: Some(lit(1i64)),
                days: Some(lit(3i64)),
            };
            // +1y = 2026-01-15, +2m = 2026-03-15, +1w = 2026-03-22, +3d = 2026-03-25
            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::String("2026-03-25".to_string()));
        }

        #[test]
        fn test_date_construct() {
            let resolver = TestResolver::new();
            let op = ActionOperation::Date {
                year: lit(2025i64),
                month: lit(3i64),
                day: lit(15i64),
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::String("2025-03-15".to_string()));
        }

        #[test]
        fn test_date_construct_leap_year() {
            let resolver = TestResolver::new();
            let op = ActionOperation::Date {
                year: lit(2024i64),
                month: lit(2i64),
                day: lit(29i64),
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::String("2024-02-29".to_string()));
        }

        #[test]
        fn test_date_construct_invalid() {
            let resolver = TestResolver::new();
            let op = ActionOperation::Date {
                year: lit(2025i64),
                month: lit(2i64),
                day: lit(30i64), // Feb 30 doesn't exist
            };

            let result = execute_operation(&op, &resolver, 0);
            assert!(matches!(result, Err(EngineError::InvalidOperation(_))));
        }

        #[test]
        fn test_day_of_week() {
            let resolver = TestResolver::new();

            // 2025-01-06 is a Monday (weekday 0)
            let op = ActionOperation::DayOfWeek {
                date: lit("2025-01-06"),
            };
            assert_eq!(execute_operation(&op, &resolver, 0).unwrap(), Value::Int(0));

            // 2025-01-12 is a Sunday (weekday 6)
            let op2 = ActionOperation::DayOfWeek {
                date: lit("2025-01-12"),
            };
            assert_eq!(
                execute_operation(&op2, &resolver, 0).unwrap(),
                Value::Int(6)
            );
        }
    }

    // -------------------------------------------------------------------------
    // parse_date Tests (Object input)
    // -------------------------------------------------------------------------

    #[test]
    fn test_parse_date_with_object() {
        let mut date_obj = BTreeMap::new();
        date_obj.insert("iso".to_string(), Value::String("2025-01-01".to_string()));
        date_obj.insert("year".to_string(), Value::Int(2025));

        let result = parse_date(&Value::Object(date_obj)).unwrap().unwrap();
        assert_eq!(result.to_string(), "2025-01-01");
    }

    #[test]
    fn test_parse_date_object_without_iso_field() {
        let mut date_obj = BTreeMap::new();
        date_obj.insert("year".to_string(), Value::Int(2025));

        let result = parse_date(&Value::Object(date_obj));
        assert!(matches!(result, Err(EngineError::TypeMismatch { .. })));
    }

    // -------------------------------------------------------------------------
    // values_equal Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_values_equal_precision_guard() {
        let large_int = MAX_SAFE_INTEGER + 1;
        let large_neg = MIN_SAFE_INTEGER - 1;

        assert!(!values_equal(
            &Value::Int(large_int),
            &Value::Float(large_int as f64)
        ));
        assert!(!values_equal(
            &Value::Float(large_neg as f64),
            &Value::Int(large_neg)
        ));

        assert!(values_equal(&Value::Int(42), &Value::Float(42.0)));
        assert!(values_equal(&Value::Float(42.0), &Value::Int(42)));
        assert!(values_equal(
            &Value::Int(MAX_SAFE_INTEGER),
            &Value::Float(MAX_SAFE_INTEGER as f64)
        ));

        assert!(values_equal(
            &Value::Float(f64::NAN),
            &Value::Float(f64::NAN)
        ));
        assert!(!values_equal(&Value::Int(0), &Value::Float(f64::NAN)));
        assert!(!values_equal(&Value::Float(f64::NAN), &Value::Int(0)));
    }

    // -------------------------------------------------------------------------
    // FOREACH Operations Tests (using RuleContext for child scope support)
    // -------------------------------------------------------------------------

    mod foreach_ops {
        use super::*;
        use crate::context::RuleContext;

        /// Helper to create a RuleContext with given parameters.
        fn make_ctx(params: Vec<(&str, Value)>) -> RuleContext {
            let p: BTreeMap<String, Value> = params
                .into_iter()
                .map(|(k, v)| (k.to_string(), v))
                .collect();
            RuleContext::new(p, "2025-06-15").unwrap()
        }

        #[test]
        fn test_foreach_basic_combine_add() {
            let ctx = make_ctx(vec![(
                "items",
                Value::Array(vec![Value::Int(10), Value::Int(20), Value::Int(30)]),
            )]);

            let op = ActionOperation::Foreach {
                collection: var("items"),
                as_name: "x".to_string(),
                body: var("x"),
                filter: None,
                combine: Some("ADD".to_string()),
            };

            let result = execute_operation(&op, &ctx, 0).unwrap();
            assert_eq!(result, Value::Int(60));
        }

        #[test]
        fn test_foreach_body_with_operation() {
            // FOREACH items as x: MULTIPLY(x, 2), combine ADD
            let ctx = make_ctx(vec![(
                "items",
                Value::Array(vec![Value::Int(1), Value::Int(2), Value::Int(3)]),
            )]);

            let mul_op = ActionOperation::Multiply {
                values: vec![var("x"), lit(2i64)],
            };

            let op = ActionOperation::Foreach {
                collection: var("items"),
                as_name: "x".to_string(),
                body: ActionValue::Operation(Box::new(mul_op)),
                filter: None,
                combine: Some("ADD".to_string()),
            };

            // 1*2 + 2*2 + 3*2 = 2+4+6 = 12
            let result = execute_operation(&op, &ctx, 0).unwrap();
            assert_eq!(result, Value::Int(12));
        }

        #[test]
        fn test_foreach_filter_skip_elements() {
            // FOREACH [1, 2, 3, 4, 5] as x: x, filter x > 2, combine ADD
            let ctx = make_ctx(vec![(
                "nums",
                Value::Array(vec![
                    Value::Int(1),
                    Value::Int(2),
                    Value::Int(3),
                    Value::Int(4),
                    Value::Int(5),
                ]),
            )]);

            let filter_op = ActionOperation::GreaterThan {
                subject: var("x"),
                value: lit(2i64),
            };

            let op = ActionOperation::Foreach {
                collection: var("nums"),
                as_name: "x".to_string(),
                body: var("x"),
                filter: Some(ActionValue::Operation(Box::new(filter_op))),
                combine: Some("ADD".to_string()),
            };

            // Only 3+4+5 = 12
            let result = execute_operation(&op, &ctx, 0).unwrap();
            assert_eq!(result, Value::Int(12));
        }

        #[test]
        fn test_foreach_empty_collection() {
            let ctx = make_ctx(vec![("items", Value::Array(vec![]))]);

            let op = ActionOperation::Foreach {
                collection: var("items"),
                as_name: "x".to_string(),
                body: var("x"),
                filter: None,
                combine: Some("ADD".to_string()),
            };

            // Empty collection with ADD combine returns 0
            let result = execute_operation(&op, &ctx, 0).unwrap();
            assert_eq!(result, Value::Int(0));
        }

        #[test]
        fn test_foreach_null_collection() {
            let ctx = make_ctx(vec![("items", Value::Null)]);

            let op = ActionOperation::Foreach {
                collection: var("items"),
                as_name: "x".to_string(),
                body: var("x"),
                filter: None,
                combine: Some("ADD".to_string()),
            };

            // Null collection treated as empty array, ADD of empty = 0
            let result = execute_operation(&op, &ctx, 0).unwrap();
            assert_eq!(result, Value::Int(0));
        }

        #[test]
        fn test_foreach_untranslatable_propagation() {
            let untranslatable = Value::Untranslatable {
                article: "42".to_string(),
                construct: "not applicable".to_string(),
            };

            let ctx = make_ctx(vec![("items", untranslatable.clone())]);

            let op = ActionOperation::Foreach {
                collection: var("items"),
                as_name: "x".to_string(),
                body: var("x"),
                filter: None,
                combine: Some("ADD".to_string()),
            };

            // Untranslatable collection should propagate
            let result = execute_operation(&op, &ctx, 0).unwrap();
            assert!(result.is_untranslatable());
        }

        #[test]
        fn test_foreach_untranslatable_in_body() {
            let untranslatable = Value::Untranslatable {
                article: "7".to_string(),
                construct: "open norm".to_string(),
            };

            let ctx = make_ctx(vec![(
                "items",
                Value::Array(vec![Value::Int(1), untranslatable.clone(), Value::Int(3)]),
            )]);

            let op = ActionOperation::Foreach {
                collection: var("items"),
                as_name: "x".to_string(),
                body: var("x"),
                filter: None,
                combine: Some("ADD".to_string()),
            };

            // Untranslatable in body should propagate immediately
            let result = execute_operation(&op, &ctx, 0).unwrap();
            assert!(result.is_untranslatable());
        }

        #[test]
        fn test_foreach_single_value_wrapping() {
            // Non-array collection is wrapped in a single-element array
            let ctx = make_ctx(vec![("item", Value::Int(42))]);

            let op = ActionOperation::Foreach {
                collection: var("item"),
                as_name: "x".to_string(),
                body: var("x"),
                filter: None,
                combine: Some("ADD".to_string()),
            };

            let result = execute_operation(&op, &ctx, 0).unwrap();
            assert_eq!(result, Value::Int(42));
        }

        #[test]
        fn test_foreach_no_combine_returns_array() {
            let ctx = make_ctx(vec![(
                "items",
                Value::Array(vec![Value::Int(10), Value::Int(20)]),
            )]);

            let op = ActionOperation::Foreach {
                collection: var("items"),
                as_name: "x".to_string(),
                body: var("x"),
                filter: None,
                combine: None,
            };

            let result = execute_operation(&op, &ctx, 0).unwrap();
            assert_eq!(result, Value::Array(vec![Value::Int(10), Value::Int(20)]));
        }

        #[test]
        fn test_foreach_combine_or() {
            let ctx = make_ctx(vec![(
                "flags",
                Value::Array(vec![
                    Value::Bool(false),
                    Value::Bool(true),
                    Value::Bool(false),
                ]),
            )]);

            let op = ActionOperation::Foreach {
                collection: var("flags"),
                as_name: "f".to_string(),
                body: var("f"),
                filter: None,
                combine: Some("OR".to_string()),
            };

            let result = execute_operation(&op, &ctx, 0).unwrap();
            assert_eq!(result, Value::Bool(true));
        }

        #[test]
        fn test_foreach_combine_and() {
            let ctx = make_ctx(vec![(
                "flags",
                Value::Array(vec![Value::Bool(true), Value::Bool(true)]),
            )]);

            let op = ActionOperation::Foreach {
                collection: var("flags"),
                as_name: "f".to_string(),
                body: var("f"),
                filter: None,
                combine: Some("AND".to_string()),
            };

            let result = execute_operation(&op, &ctx, 0).unwrap();
            assert_eq!(result, Value::Bool(true));
        }

        #[test]
        fn test_foreach_nested_in_add() {
            // ADD(FOREACH([1,2,3] as x: x, combine ADD), 100)
            // This tests the key fix: FOREACH as a nested operation inside ADD.
            let ctx = make_ctx(vec![(
                "items",
                Value::Array(vec![Value::Int(1), Value::Int(2), Value::Int(3)]),
            )]);

            let foreach_op = ActionOperation::Foreach {
                collection: var("items"),
                as_name: "x".to_string(),
                body: var("x"),
                filter: None,
                combine: Some("ADD".to_string()),
            };

            let add_op = ActionOperation::Add {
                values: vec![ActionValue::Operation(Box::new(foreach_op)), lit(100i64)],
            };

            // FOREACH sum = 1+2+3 = 6, then ADD(6, 100) = 106
            let result = execute_operation(&add_op, &ctx, 0).unwrap();
            assert_eq!(result, Value::Int(106));
        }

        #[test]
        fn test_foreach_without_child_scope_support() {
            // TestResolver does NOT implement execute_foreach_op,
            // so FOREACH should return an appropriate error.
            let resolver = TestResolver::new()
                .with_var("items", Value::Array(vec![Value::Int(1), Value::Int(2)]));

            let op = ActionOperation::Foreach {
                collection: var("items"),
                as_name: "x".to_string(),
                body: var("x"),
                filter: None,
                combine: None,
            };

            let result = execute_operation(&op, &resolver, 0);
            assert!(matches!(result, Err(EngineError::InvalidOperation(ref msg))
                if msg.contains("child scopes")));
        }
    }
}
