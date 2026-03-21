//! Operation execution for the RegelRecht engine
//!
//! This module implements the execution logic for all 21 operation types:
//! - **Comparison (5):** EQUALS, GREATER_THAN, LESS_THAN, GREATER_THAN_OR_EQUAL, LESS_THAN_OR_EQUAL
//! - **Arithmetic (4):** ADD, SUBTRACT, MULTIPLY, DIVIDE
//! - **Aggregate (2):** MAX, MIN
//! - **Logical (3):** AND, OR, NOT
//! - **Conditional (1):** IF (multi-case with cases/default)
//! - **Collection (2):** IN, LIST
//! - **Date (4):** AGE, DATE_ADD, DATE, DAY_OF_WEEK

use crate::article::{ActionOperation, ActionValue};
use crate::error::{EngineError, Result};
use crate::types::{Operation, PathNodeType, Value};
use chrono::{Datelike, NaiveDate};

/// Maximum nesting depth for operations to prevent stack overflow
const MAX_OPERATION_DEPTH: usize = 100;

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
                    return resolver.resolve(var_name);
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

    let tracing = resolver.has_trace();
    if tracing {
        resolver.trace_push(op.operation.name(), PathNodeType::Operation);
    }

    let result = execute_operation_internal(op, resolver, depth);

    if tracing {
        let op_name = op.operation.name();
        match &result {
            Ok(value) => {
                resolver.trace_set_result(value.clone());
                // For IF (cases/default), execute_if already set a message
                // with case match info; incorporate it instead of overwriting.
                let existing_msg = resolver.trace_get_message();
                let msg = if op.operation == Operation::If {
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
    }
}

/// Internal operation dispatch (no tracing).
fn execute_operation_internal<R: ValueResolver>(
    op: &ActionOperation,
    resolver: &R,
    depth: usize,
) -> Result<Value> {
    match op.operation {
        // Comparison operations
        Operation::Equals => execute_equality(op, resolver, depth, false),
        Operation::NotEquals => execute_equality(op, resolver, depth, true),
        Operation::GreaterThan => execute_numeric_comparison(op, resolver, depth, |a, b| a > b),
        Operation::LessThan => execute_numeric_comparison(op, resolver, depth, |a, b| a < b),
        Operation::GreaterThanOrEqual => {
            execute_numeric_comparison(op, resolver, depth, |a, b| a >= b)
        }
        Operation::LessThanOrEqual => {
            execute_numeric_comparison(op, resolver, depth, |a, b| a <= b)
        }

        // Arithmetic
        Operation::Add => execute_add(op, resolver, depth),
        Operation::Subtract => execute_subtract(op, resolver, depth),
        Operation::Multiply => execute_multiply(op, resolver, depth),
        Operation::Divide => execute_divide(op, resolver, depth),

        // Aggregate
        Operation::Max => execute_aggregate(op, resolver, depth, f64::max),
        Operation::Min => execute_aggregate(op, resolver, depth, f64::min),

        // Logical
        Operation::And => execute_and(op, resolver, depth),
        Operation::Or => execute_or(op, resolver, depth),
        Operation::Not => execute_not(op, resolver, depth),

        // Conditional (multi-case with cases/default)
        Operation::If => execute_if(op, resolver, depth),

        // Null checking operations
        Operation::IsNull => execute_null_check(op, resolver, depth, false),
        Operation::NotNull => execute_null_check(op, resolver, depth, true),

        // Collection operations
        Operation::In => execute_membership(op, resolver, depth, false),
        Operation::NotIn => execute_membership(op, resolver, depth, true),
        Operation::List => execute_list(op, resolver, depth),

        // Date
        Operation::Age => execute_age(op, resolver, depth),
        Operation::DateAdd => execute_date_add(op, resolver, depth),
        Operation::Date => execute_date_construct(op, resolver, depth),
        Operation::DayOfWeek => execute_day_of_week(op, resolver, depth),

        // Backward compatibility (v0.4.0 and earlier)
        Operation::SubtractDate => execute_compat_subtract_date(op, resolver, depth),
        Operation::Concat => execute_add(op, resolver, depth),
    }
}

// =============================================================================
// Field Extraction Helpers
// =============================================================================

/// Extract the `subject` field from an operation, or return a descriptive error.
fn require_subject(op: &ActionOperation) -> Result<&ActionValue> {
    op.subject.as_ref().ok_or_else(|| {
        EngineError::InvalidOperation(format!("{} requires 'subject'", op.operation.name()))
    })
}

/// Extract both `subject` and `value` fields from an operation, or return a descriptive error.
fn require_subject_value(op: &ActionOperation) -> Result<(&ActionValue, &ActionValue)> {
    let subject = require_subject(op)?;
    let value = op.value.as_ref().ok_or_else(|| {
        EngineError::InvalidOperation(format!("{} requires 'value'", op.operation.name()))
    })?;
    Ok((subject, value))
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
    op: &ActionOperation,
    resolver: &R,
    depth: usize,
    negate: bool,
) -> Result<Value> {
    let (subject, value) = require_subject_value(op)?;

    let subject_val = evaluate_value(subject, resolver, depth)?;
    let value_val = evaluate_value(value, resolver, depth)?;

    let equal = values_equal(&subject_val, &value_val);
    Ok(Value::Bool(if negate { !equal } else { equal }))
}

/// Execute a numeric comparison (>, <, >=, <=).
///
/// Converts values to f64 for comparison to handle both Int and Float types.
fn execute_numeric_comparison<R: ValueResolver, F>(
    op: &ActionOperation,
    resolver: &R,
    depth: usize,
    compare: F,
) -> Result<Value>
where
    F: Fn(f64, f64) -> bool,
{
    let (subject, value) = require_subject_value(op)?;

    let subject_val = evaluate_value(subject, resolver, depth)?;
    let value_val = evaluate_value(value, resolver, depth)?;

    let subject_num = to_number(&subject_val)?;
    let value_num = to_number(&value_val)?;

    Ok(Value::Bool(compare(subject_num, value_num)))
}

// =============================================================================
// Arithmetic Operations
// =============================================================================

/// Execute ADD operation: sum numbers, concatenate arrays, or concatenate strings.
///
/// The type of the first value determines the operation mode:
/// - Numbers: sum all values
/// - Arrays: concatenate all arrays
/// - Strings: concatenate all strings
fn execute_add<R: ValueResolver>(
    op: &ActionOperation,
    resolver: &R,
    depth: usize,
) -> Result<Value> {
    let values = get_values(op)?;
    let evaluated = evaluate_values(values, resolver, depth)?;

    if evaluated.is_empty() {
        return Err(EngineError::InvalidOperation(
            "ADD requires at least one value".to_string(),
        ));
    }

    // Determine type from first value
    match &evaluated[0] {
        Value::Array(_) => {
            // Concatenate arrays
            let mut result = Vec::new();
            for val in &evaluated {
                match val {
                    Value::Array(arr) => result.extend(arr.iter().cloned()),
                    _ => {
                        return Err(EngineError::TypeMismatch {
                            expected: "array".to_string(),
                            actual: format!("{:?}", val),
                        })
                    }
                }
            }
            Ok(Value::Array(result))
        }
        Value::String(_) => {
            // Concatenate strings
            let mut result = String::new();
            for val in &evaluated {
                match val {
                    Value::String(s) => result.push_str(s),
                    _ => {
                        return Err(EngineError::TypeMismatch {
                            expected: "string".to_string(),
                            actual: format!("{:?}", val),
                        })
                    }
                }
            }
            Ok(Value::String(result))
        }
        Value::Int(_) | Value::Float(_) => {
            // Original numeric addition
            let mut sum = 0.0;
            let mut has_float = false;
            for val in &evaluated {
                match val {
                    Value::Int(_) => sum += to_number(val)?,
                    Value::Float(f) => {
                        sum += f;
                        has_float = true;
                    }
                    _ => return Err(type_error("number", val)),
                }
            }
            Ok(if has_float {
                Value::Float(sum)
            } else {
                Value::Int(f64_to_i64_safe(sum)?)
            })
        }
        _ => Err(type_error("number, string, or array", &evaluated[0])),
    }
}

/// Execute SUBTRACT operation: first value minus all subsequent values.
///
/// Note: Uses `to_number()` which validates that integers are within the
/// safe range for f64 conversion (±2^53).
fn execute_subtract<R: ValueResolver>(
    op: &ActionOperation,
    resolver: &R,
    depth: usize,
) -> Result<Value> {
    let values = get_values(op)?;
    if values.is_empty() {
        return Err(EngineError::InvalidOperation(
            "SUBTRACT requires at least one value".to_string(),
        ));
    }

    let evaluated = evaluate_values(values, resolver, depth)?;

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
    op: &ActionOperation,
    resolver: &R,
    depth: usize,
) -> Result<Value> {
    let values = get_values(op)?;
    if values.is_empty() {
        return Err(EngineError::InvalidOperation(
            "MULTIPLY requires at least one value".to_string(),
        ));
    }

    let evaluated = evaluate_values(values, resolver, depth)?;

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
    op: &ActionOperation,
    resolver: &R,
    depth: usize,
) -> Result<Value> {
    let values = get_values(op)?;
    if values.is_empty() {
        return Err(EngineError::InvalidOperation(
            "DIVIDE requires at least one value".to_string(),
        ));
    }

    let evaluated = evaluate_values(values, resolver, depth)?;

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
    op: &ActionOperation,
    resolver: &R,
    depth: usize,
    combine: F,
) -> Result<Value>
where
    F: Fn(f64, f64) -> f64,
{
    let values = get_values(op)?;
    if values.is_empty() {
        return Err(EngineError::InvalidOperation(
            "Aggregate operation requires at least one value".to_string(),
        ));
    }

    let evaluated = evaluate_values(values, resolver, depth)?;

    let mut has_float = false;
    let nums: Vec<f64> = evaluated
        .iter()
        .map(|v| {
            if matches!(v, Value::Float(_)) {
                has_float = true;
            }
            to_number(v)
        })
        .collect::<Result<Vec<_>>>()?;

    // SAFETY: values guaranteed non-empty by check above
    let Some(result) = nums.into_iter().reduce(combine) else {
        unreachable!("values checked non-empty above")
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
    op: &ActionOperation,
    resolver: &R,
    depth: usize,
) -> Result<Value> {
    let conditions = op
        .conditions
        .as_ref()
        .ok_or_else(|| EngineError::InvalidOperation("AND requires 'conditions'".to_string()))?;

    let tracing = resolver.has_trace();
    let mut results: Option<Vec<Value>> = if tracing { Some(Vec::new()) } else { None };
    for condition in conditions {
        let val = evaluate_value(condition, resolver, depth)?;
        if !val.to_bool() {
            return Ok(Value::Bool(false));
        }
        if let Some(ref mut r) = results {
            r.push(val);
        }
    }

    if let Some(results) = results {
        let result_strs: Vec<String> = results.iter().map(format_value_for_trace).collect();
        resolver.trace_set_message(format!("Result [{}] AND: True", result_strs.join(", ")));
    }

    Ok(Value::Bool(true))
}

/// Execute OR operation: short-circuit evaluation, returns true if any condition is true.
fn execute_or<R: ValueResolver>(op: &ActionOperation, resolver: &R, depth: usize) -> Result<Value> {
    let conditions = op
        .conditions
        .as_ref()
        .ok_or_else(|| EngineError::InvalidOperation("OR requires 'conditions'".to_string()))?;

    for condition in conditions {
        let val = evaluate_value(condition, resolver, depth)?;
        if val.to_bool() {
            return Ok(Value::Bool(true));
        }
    }

    Ok(Value::Bool(false))
}

/// Execute NOT operation: logical negation.
///
/// Takes a single `value` field (which should be a boolean-returning operation).
fn execute_not<R: ValueResolver>(
    op: &ActionOperation,
    resolver: &R,
    depth: usize,
) -> Result<Value> {
    let value = op
        .value
        .as_ref()
        .ok_or_else(|| EngineError::InvalidOperation("NOT requires 'value'".to_string()))?;
    let val = evaluate_value(value, resolver, depth)?;
    Ok(Value::Bool(!val.to_bool()))
}

// =============================================================================
// Conditional Operations
// =============================================================================

/// Execute IF operation: evaluates cases in order, returns first matching case's value.
///
/// Supports two syntaxes:
/// - **v0.5.0 (cases/default):** Each case has a `when` condition and `then` value.
/// - **v0.4.0 compat (when/then/else):** Single condition with then/else branches.
fn execute_if<R: ValueResolver>(op: &ActionOperation, resolver: &R, depth: usize) -> Result<Value> {
    // Backward compat: if `when` field is present (old IF syntax), use when/then/else
    if op.when.is_some() {
        return execute_if_legacy(op, resolver, depth);
    }

    let cases = op
        .cases
        .as_ref()
        .ok_or_else(|| EngineError::InvalidOperation("IF requires 'cases'".to_string()))?;

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
    if let Some(default) = &op.default {
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

/// Legacy IF with when/then/else (v0.4.0 backward compatibility).
fn execute_if_legacy<R: ValueResolver>(
    op: &ActionOperation,
    resolver: &R,
    depth: usize,
) -> Result<Value> {
    let when = op
        .when
        .as_ref()
        .ok_or_else(|| EngineError::InvalidOperation("IF requires 'when'".to_string()))?;
    let then = op
        .then
        .as_ref()
        .ok_or_else(|| EngineError::InvalidOperation("IF requires 'then'".to_string()))?;

    let tracing = resolver.has_trace();

    // Wrap condition evaluation in a WHEN label node
    if tracing {
        resolver.trace_push("WHEN", PathNodeType::Operation);
    }
    let condition_result = evaluate_value(when, resolver, depth).inspect_err(|_| {
        if tracing {
            resolver.trace_pop();
        }
    })?;
    if tracing {
        resolver.trace_set_result(condition_result.clone());
        resolver.trace_set_message(format!(
            "WHEN: {}",
            format_value_for_trace(&condition_result)
        ));
        resolver.trace_pop();
    }

    if condition_result.to_bool() {
        if tracing {
            resolver.trace_push("THEN", PathNodeType::Operation);
        }
        let result = evaluate_value(then, resolver, depth).inspect_err(|_| {
            if tracing {
                resolver.trace_pop();
            }
        })?;
        if tracing {
            resolver.trace_set_result(result.clone());
            resolver.trace_set_message(format!("THEN: {}", format_value_for_trace(&result)));
            resolver.trace_pop();
            resolver.trace_set_message("took THEN".to_string());
        }
        Ok(result)
    } else if let Some(else_branch) = &op.else_branch {
        if tracing {
            resolver.trace_push("ELSE", PathNodeType::Operation);
        }
        let result = evaluate_value(else_branch, resolver, depth).inspect_err(|_| {
            if tracing {
                resolver.trace_pop();
            }
        })?;
        if tracing {
            resolver.trace_set_result(result.clone());
            resolver.trace_set_message(format!("ELSE: {}", format_value_for_trace(&result)));
            resolver.trace_pop();
            resolver.trace_set_message("took ELSE".to_string());
        }
        Ok(result)
    } else {
        Ok(Value::Null)
    }
}

// =============================================================================
// Backward Compatibility Operations (v0.4.0 and earlier)
// =============================================================================

// =============================================================================
// Null Checking Operations
// =============================================================================

/// Execute IS_NULL / NOT_NULL operation.
///
/// When `negate` is true, returns true if the subject is *not* null (NOT_NULL).
fn execute_null_check<R: ValueResolver>(
    op: &ActionOperation,
    resolver: &R,
    depth: usize,
    negate: bool,
) -> Result<Value> {
    let subject = require_subject(op)?;

    let subject_val = evaluate_value(subject, resolver, depth)?;
    let is_null = subject_val.is_null();
    Ok(Value::Bool(if negate { !is_null } else { is_null }))
}

fn execute_compat_subtract_date<R: ValueResolver>(
    op: &ActionOperation,
    resolver: &R,
    depth: usize,
) -> Result<Value> {
    let subject = op.subject.as_ref().ok_or_else(|| {
        EngineError::InvalidOperation("SUBTRACT_DATE requires 'subject'".to_string())
    })?;
    let value = op.value.as_ref().ok_or_else(|| {
        EngineError::InvalidOperation("SUBTRACT_DATE requires 'value'".to_string())
    })?;

    let subject_val = evaluate_value(subject, resolver, depth)?;
    let value_val = evaluate_value(value, resolver, depth)?;
    let subject_date = parse_date(&subject_val)?;
    let value_date = parse_date(&value_val)?;

    let unit = op.unit.as_deref().unwrap_or("days");
    let result = match unit {
        "days" => (subject_date - value_date).num_days(),
        "months" => calculate_months_difference(subject_date, value_date),
        "years" => calculate_years_difference(subject_date, value_date),
        other => {
            return Err(EngineError::InvalidOperation(format!(
                "SUBTRACT_DATE: unsupported unit '{}'. Expected 'days', 'months', or 'years'",
                other
            )));
        }
    };

    Ok(Value::Int(result))
}

// =============================================================================
// Collection Operations
// =============================================================================

/// Resolve the membership check values from an operation.
///
/// Supports both `values: [...]` (inline list) and `value: $list_ref` (reference to a
/// definition list). When `value` resolves to a non-array, it is wrapped in a single-element vec.
fn resolve_membership_values<R: ValueResolver>(
    op: &ActionOperation,
    op_name: &str,
    resolver: &R,
    depth: usize,
) -> Result<Vec<Value>> {
    if let Some(values) = &op.values {
        evaluate_values(values, resolver, depth)
    } else if let Some(value) = &op.value {
        let resolved = evaluate_value(value, resolver, depth)?;
        match resolved {
            Value::Array(items) => Ok(items),
            other => Ok(vec![other]),
        }
    } else {
        Err(EngineError::InvalidOperation(format!(
            "{op_name} requires 'values' or 'value'"
        )))
    }
}

/// Execute IN / NOT_IN operation.
///
/// Uses Python-style numeric coercion for equality comparison.
/// When `negate` is true, returns true if subject is *not* in the list (NOT_IN).
fn execute_membership<R: ValueResolver>(
    op: &ActionOperation,
    resolver: &R,
    depth: usize,
    negate: bool,
) -> Result<Value> {
    let subject = require_subject(op)?;

    let subject_val = evaluate_value(subject, resolver, depth)?;
    let op_name = op.operation.name();
    let check_values = resolve_membership_values(op, op_name, resolver, depth)?;

    let found = check_values
        .iter()
        .any(|val| values_equal(&subject_val, val));
    Ok(Value::Bool(if negate { !found } else { found }))
}

/// Execute LIST operation: construct an array from items.
fn execute_list<R: ValueResolver>(
    op: &ActionOperation,
    resolver: &R,
    depth: usize,
) -> Result<Value> {
    let items = op
        .items
        .as_ref()
        .ok_or_else(|| EngineError::InvalidOperation("LIST requires 'items'".to_string()))?;

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
    op: &ActionOperation,
    resolver: &R,
    depth: usize,
) -> Result<Value> {
    let dob = op
        .date_of_birth
        .as_ref()
        .ok_or_else(|| EngineError::InvalidOperation("AGE requires 'date_of_birth'".to_string()))?;
    let ref_date = op.reference_date.as_ref().ok_or_else(|| {
        EngineError::InvalidOperation("AGE requires 'reference_date'".to_string())
    })?;

    let dob_val = evaluate_value(dob, resolver, depth)?;
    let ref_val = evaluate_value(ref_date, resolver, depth)?;

    let dob_date = parse_date(&dob_val)?;
    let ref_date_parsed = parse_date(&ref_val)?;

    let age = calculate_years_difference(ref_date_parsed, dob_date);
    Ok(Value::Int(age))
}

/// Execute DATE_ADD operation: add days and/or weeks to a date.
///
/// # Arguments
/// - `date`: Base date (ISO 8601 YYYY-MM-DD)
/// - `days`: Number of days to add (optional)
/// - `weeks`: Number of weeks to add (optional)
fn execute_date_add<R: ValueResolver>(
    op: &ActionOperation,
    resolver: &R,
    depth: usize,
) -> Result<Value> {
    let date = op
        .date
        .as_ref()
        .ok_or_else(|| EngineError::InvalidOperation("DATE_ADD requires 'date'".to_string()))?;

    let date_val = evaluate_value(date, resolver, depth)?;
    let mut result_date = parse_date(&date_val)?;

    if let Some(days) = &op.days {
        let days_val = evaluate_value(days, resolver, depth)?;
        let days_int = days_val.as_int().ok_or_else(|| {
            EngineError::InvalidOperation("DATE_ADD 'days' must be a number".to_string())
        })?;
        result_date += chrono::Duration::days(days_int);
    }

    if let Some(weeks) = &op.weeks {
        let weeks_val = evaluate_value(weeks, resolver, depth)?;
        let weeks_int = weeks_val.as_int().ok_or_else(|| {
            EngineError::InvalidOperation("DATE_ADD 'weeks' must be a number".to_string())
        })?;
        result_date += chrono::Duration::weeks(weeks_int);
    }

    Ok(Value::String(result_date.format("%Y-%m-%d").to_string()))
}

/// Execute DATE operation: construct a date from year, month, day components.
///
/// # Arguments
/// - `year`: Year component (integer)
/// - `month`: Month component (integer, 1-12)
/// - `day`: Day component (integer, 1-31)
fn execute_date_construct<R: ValueResolver>(
    op: &ActionOperation,
    resolver: &R,
    depth: usize,
) -> Result<Value> {
    let year = op
        .year
        .as_ref()
        .ok_or_else(|| EngineError::InvalidOperation("DATE requires 'year'".to_string()))?;
    let month = op
        .month
        .as_ref()
        .ok_or_else(|| EngineError::InvalidOperation("DATE requires 'month'".to_string()))?;
    let day = op
        .day
        .as_ref()
        .ok_or_else(|| EngineError::InvalidOperation("DATE requires 'day'".to_string()))?;

    let year_val = evaluate_value(year, resolver, depth)?;
    let month_val = evaluate_value(month, resolver, depth)?;
    let day_val = evaluate_value(day, resolver, depth)?;

    let y = year_val
        .as_int()
        .ok_or_else(|| EngineError::InvalidOperation("DATE 'year' must be a number".to_string()))?
        as i32;
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
    op: &ActionOperation,
    resolver: &R,
    depth: usize,
) -> Result<Value> {
    let date_val = op
        .date
        .as_ref()
        .ok_or_else(|| EngineError::InvalidOperation("DAY_OF_WEEK requires 'date'".to_string()))?;
    let date = parse_date(&evaluate_value(date_val, resolver, depth)?)?;
    Ok(Value::Int(date.weekday().num_days_from_monday() as i64))
}

/// Parse a date from a Value.
///
/// Expects the value to be a string in ISO 8601 format (YYYY-MM-DD).
fn parse_date(value: &Value) -> Result<NaiveDate> {
    match value {
        Value::String(s) => NaiveDate::parse_from_str(s, "%Y-%m-%d").map_err(|e| {
            EngineError::InvalidOperation(format!(
                "Failed to parse date '{}': {}. Expected format: YYYY-MM-DD",
                s, e
            ))
        }),
        // Handle referencedate objects with {iso, year, month, day}
        Value::Object(obj) => {
            if let Some(Value::String(iso)) = obj.get("iso") {
                NaiveDate::parse_from_str(iso, "%Y-%m-%d").map_err(|e| {
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

/// Calculate the difference in days between two dates.
#[cfg(test)]
fn calculate_days_difference(date1: NaiveDate, date2: NaiveDate) -> i64 {
    (date1 - date2).num_days()
}

/// Calculate the difference in complete months between two dates.
///
/// Uses proper calendar arithmetic. A month is counted as complete when
/// the same day-of-month (or end of month if day doesn't exist) is reached.
/// For end-of-month edge cases (e.g., Jan 31 -> Feb 28), if `earlier.day()`
/// exceeds the number of days in `later`'s month, it is capped to the last
/// day of that month so the month is correctly counted as complete.
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

/// Get the 'values' array from an operation, or return an error.
fn get_values(op: &ActionOperation) -> Result<&Vec<ActionValue>> {
    op.values.as_ref().ok_or_else(|| {
        EngineError::InvalidOperation(format!("{:?} requires 'values'", op.operation))
    })
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
        _ => Err(type_error("number", val)),
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
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

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

    /// Shorthand to build an ActionOperation with all optional fields set to None.
    fn op_new(operation: Operation) -> ActionOperation {
        ActionOperation {
            operation,
            subject: None,
            value: None,
            values: None,
            conditions: None,
            cases: None,
            default: None,
            date: None,
            days: None,
            weeks: None,
            year: None,
            month: None,
            day: None,
            date_of_birth: None,
            reference_date: None,
            items: None,
            // Backward compatibility fields
            when: None,
            then: None,
            else_branch: None,
            unit: None,
        }
    }

    // -------------------------------------------------------------------------
    // Comparison Operations Tests
    // -------------------------------------------------------------------------

    mod comparison {
        use super::*;

        #[test]
        fn test_equals_integers() {
            let resolver = TestResolver::new();
            let mut op = op_new(Operation::Equals);
            op.subject = Some(lit(42i64));
            op.value = Some(lit(42i64));

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Bool(true));
        }

        #[test]
        fn test_equals_different_values() {
            let resolver = TestResolver::new();
            let mut op = op_new(Operation::Equals);
            op.subject = Some(lit(42i64));
            op.value = Some(lit(43i64));

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Bool(false));
        }

        #[test]
        fn test_greater_than() {
            let resolver = TestResolver::new();
            let mut op = op_new(Operation::GreaterThan);
            op.subject = Some(lit(50i64));
            op.value = Some(lit(42i64));

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Bool(true));
        }

        #[test]
        fn test_less_than() {
            let resolver = TestResolver::new();
            let mut op = op_new(Operation::LessThan);
            op.subject = Some(lit(30i64));
            op.value = Some(lit(42i64));

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Bool(true));
        }

        #[test]
        fn test_greater_than_or_equal() {
            let resolver = TestResolver::new();

            let mut op = op_new(Operation::GreaterThanOrEqual);
            op.subject = Some(lit(42i64));
            op.value = Some(lit(42i64));
            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::Bool(true)
            );

            let mut op2 = op_new(Operation::GreaterThanOrEqual);
            op2.subject = Some(lit(50i64));
            op2.value = Some(lit(42i64));
            assert_eq!(
                execute_operation(&op2, &resolver, 0).unwrap(),
                Value::Bool(true)
            );
        }

        #[test]
        fn test_less_than_or_equal() {
            let resolver = TestResolver::new();

            let mut op = op_new(Operation::LessThanOrEqual);
            op.subject = Some(lit(42i64));
            op.value = Some(lit(42i64));
            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::Bool(true)
            );

            let mut op2 = op_new(Operation::LessThanOrEqual);
            op2.subject = Some(lit(30i64));
            op2.value = Some(lit(42i64));
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

            let mut op = op_new(Operation::GreaterThanOrEqual);
            op.subject = Some(var("age"));
            op.value = Some(var("min_age"));

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Bool(true));
        }

        #[test]
        fn test_comparison_mixed_int_float() {
            let resolver = TestResolver::new();

            let mut op = op_new(Operation::Equals);
            op.subject = Some(lit(42i64));
            op.value = Some(lit(42.0f64));
            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::Bool(true)
            );

            let mut op2 = op_new(Operation::Equals);
            op2.subject = Some(lit(42.0f64));
            op2.value = Some(lit(42i64));
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
            let mut op = op_new(Operation::Add);
            op.values = Some(vec![lit(10i64), lit(20i64), lit(30i64)]);

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Int(60));
        }

        #[test]
        fn test_add_with_floats() {
            let resolver = TestResolver::new();
            let mut op = op_new(Operation::Add);
            op.values = Some(vec![lit(10i64), lit(20.5f64), lit(30i64)]);

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Float(60.5));
        }

        #[test]
        fn test_subtract() {
            let resolver = TestResolver::new();
            let mut op = op_new(Operation::Subtract);
            op.values = Some(vec![lit(100i64), lit(30i64), lit(20i64)]);

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Int(50));
        }

        #[test]
        fn test_multiply() {
            let resolver = TestResolver::new();
            let mut op = op_new(Operation::Multiply);
            op.values = Some(vec![lit(2i64), lit(3i64), lit(4i64)]);

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Int(24));
        }

        #[test]
        fn test_divide() {
            let resolver = TestResolver::new();
            let mut op = op_new(Operation::Divide);
            op.values = Some(vec![lit(100i64), lit(2i64)]);

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Float(50.0));
        }

        #[test]
        fn test_divide_by_zero() {
            let resolver = TestResolver::new();
            let mut op = op_new(Operation::Divide);
            op.values = Some(vec![lit(100i64), lit(0i64)]);

            let result = execute_operation(&op, &resolver, 0);
            assert!(matches!(result, Err(EngineError::DivisionByZero)));
        }

        #[test]
        fn test_arithmetic_with_variables() {
            let resolver = TestResolver::new()
                .with_var("base", 1000i64)
                .with_var("rate", 0.05f64);

            let mut op = op_new(Operation::Multiply);
            op.values = Some(vec![var("base"), var("rate")]);

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Float(50.0));
        }

        #[test]
        fn test_add_arrays() {
            let resolver = TestResolver::new();
            let mut op = op_new(Operation::Add);
            op.values = Some(vec![
                lit(Value::Array(vec![Value::Int(1), Value::Int(2)])),
                lit(Value::Array(vec![Value::Int(3), Value::Int(4)])),
            ]);

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
            let mut op = op_new(Operation::Add);
            op.values = Some(vec![lit("hello"), lit(" "), lit("world")]);

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
            let mut op = op_new(Operation::Max);
            op.values = Some(vec![lit(10i64), lit(50i64), lit(30i64)]);

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Int(50));
        }

        #[test]
        fn test_min() {
            let resolver = TestResolver::new();
            let mut op = op_new(Operation::Min);
            op.values = Some(vec![lit(10i64), lit(50i64), lit(30i64)]);

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Int(10));
        }

        #[test]
        fn test_max_with_floats() {
            let resolver = TestResolver::new();
            let mut op = op_new(Operation::Max);
            op.values = Some(vec![lit(10.5f64), lit(50.3f64), lit(30.7f64)]);

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Float(50.3));
        }

        #[test]
        fn test_max_with_zero() {
            let resolver = TestResolver::new();
            let mut op = op_new(Operation::Max);
            op.values = Some(vec![lit(0i64), lit(-10i64)]);

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
            let mut op = op_new(Operation::And);
            op.conditions = Some(vec![lit(true), lit(true), lit(true)]);

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Bool(true));
        }

        #[test]
        fn test_and_one_false() {
            let resolver = TestResolver::new();
            let mut op = op_new(Operation::And);
            op.conditions = Some(vec![lit(true), lit(false), lit(true)]);

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Bool(false));
        }

        #[test]
        fn test_or_one_true() {
            let resolver = TestResolver::new();
            let mut op = op_new(Operation::Or);
            op.conditions = Some(vec![lit(false), lit(true), lit(false)]);

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Bool(true));
        }

        #[test]
        fn test_or_all_false() {
            let resolver = TestResolver::new();
            let mut op = op_new(Operation::Or);
            op.conditions = Some(vec![lit(false), lit(false), lit(false)]);

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Bool(false));
        }

        #[test]
        fn test_and_with_nested_comparison() {
            let resolver = TestResolver::new()
                .with_var("age", 25i64)
                .with_var("has_insurance", true);

            let mut age_check_op = op_new(Operation::GreaterThanOrEqual);
            age_check_op.subject = Some(var("age"));
            age_check_op.value = Some(lit(18i64));
            let age_check = ActionValue::Operation(Box::new(age_check_op));

            let mut op = op_new(Operation::And);
            op.conditions = Some(vec![age_check, var("has_insurance")]);

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Bool(true));
        }

        #[test]
        fn test_not_negates_true() {
            let resolver = TestResolver::new();
            let mut op = op_new(Operation::Not);
            op.value = Some(lit(true));

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Bool(false));
        }

        #[test]
        fn test_not_negates_false() {
            let resolver = TestResolver::new();
            let mut op = op_new(Operation::Not);
            op.value = Some(lit(false));

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Bool(true));
        }

        #[test]
        fn test_not_wrapping_equals() {
            let resolver = TestResolver::new();

            // NOT(EQUALS(42, 42)) should be false
            let mut eq_op = op_new(Operation::Equals);
            eq_op.subject = Some(lit(42i64));
            eq_op.value = Some(lit(42i64));

            let mut op = op_new(Operation::Not);
            op.value = Some(ActionValue::Operation(Box::new(eq_op)));

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Bool(false));

            // NOT(EQUALS(42, 43)) should be true
            let mut eq_op2 = op_new(Operation::Equals);
            eq_op2.subject = Some(lit(42i64));
            eq_op2.value = Some(lit(43i64));

            let mut op2 = op_new(Operation::Not);
            op2.value = Some(ActionValue::Operation(Box::new(eq_op2)));

            let result2 = execute_operation(&op2, &resolver, 0).unwrap();
            assert_eq!(result2, Value::Bool(true));
        }
    }

    // -------------------------------------------------------------------------
    // Conditional Operations Tests (IF with cases/default)
    // -------------------------------------------------------------------------

    mod conditional {
        use super::*;
        use crate::article::Case;

        #[test]
        fn test_if_first_match() {
            let resolver = TestResolver::new();
            let mut op = op_new(Operation::If);
            op.cases = Some(vec![
                Case {
                    when: lit(true),
                    then: lit(100i64),
                },
                Case {
                    when: lit(true),
                    then: lit(200i64),
                },
            ]);
            op.default = Some(lit(0i64));

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Int(100));
        }

        #[test]
        fn test_if_second_match() {
            let resolver = TestResolver::new();
            let mut op = op_new(Operation::If);
            op.cases = Some(vec![
                Case {
                    when: lit(false),
                    then: lit(100i64),
                },
                Case {
                    when: lit(true),
                    then: lit(200i64),
                },
            ]);
            op.default = Some(lit(0i64));

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Int(200));
        }

        #[test]
        fn test_if_default() {
            let resolver = TestResolver::new();
            let mut op = op_new(Operation::If);
            op.cases = Some(vec![
                Case {
                    when: lit(false),
                    then: lit(100i64),
                },
                Case {
                    when: lit(false),
                    then: lit(200i64),
                },
            ]);
            op.default = Some(lit(0i64));

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Int(0));
        }

        #[test]
        fn test_if_no_default_returns_null() {
            let resolver = TestResolver::new();
            let mut op = op_new(Operation::If);
            op.cases = Some(vec![Case {
                when: lit(false),
                then: lit(100i64),
            }]);

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Null);
        }

        #[test]
        fn test_if_with_nested_conditions() {
            let resolver = TestResolver::new().with_var("status", "active");

            let mut pending_check = op_new(Operation::Equals);
            pending_check.subject = Some(var("status"));
            pending_check.value = Some(lit("pending"));

            let mut active_check = op_new(Operation::Equals);
            active_check.subject = Some(var("status"));
            active_check.value = Some(lit("active"));

            let mut op = op_new(Operation::If);
            op.cases = Some(vec![
                Case {
                    when: ActionValue::Operation(Box::new(pending_check)),
                    then: lit(10i64),
                },
                Case {
                    when: ActionValue::Operation(Box::new(active_check)),
                    then: lit(20i64),
                },
            ]);
            op.default = Some(lit(0i64));

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Int(20));
        }
    }

    // -------------------------------------------------------------------------
    // Nested Operations Tests
    // -------------------------------------------------------------------------

    mod nested {
        use super::*;
        use crate::article::Case;

        #[test]
        fn test_nested_arithmetic_in_max() {
            // MAX(0, 100 - 50) = MAX(0, 50) = 50
            let resolver = TestResolver::new();

            let mut sub_op = op_new(Operation::Subtract);
            sub_op.values = Some(vec![lit(100i64), lit(50i64)]);
            let subtract_val = ActionValue::Operation(Box::new(sub_op));

            let mut op = op_new(Operation::Max);
            op.values = Some(vec![lit(0i64), subtract_val]);

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Int(50));
        }

        #[test]
        fn test_deeply_nested_operations() {
            // IF (10 > 5) THEN (2 * 3) ELSE (1 + 1)
            // Expressed as IF with cases: [case(10>5 -> 2*3)], default: 1+1
            let resolver = TestResolver::new();

            let mut gt_op = op_new(Operation::GreaterThan);
            gt_op.subject = Some(lit(10i64));
            gt_op.value = Some(lit(5i64));
            let condition = ActionValue::Operation(Box::new(gt_op));

            let mut mul_op = op_new(Operation::Multiply);
            mul_op.values = Some(vec![lit(2i64), lit(3i64)]);
            let then_branch = ActionValue::Operation(Box::new(mul_op));

            let mut add_op = op_new(Operation::Add);
            add_op.values = Some(vec![lit(1i64), lit(1i64)]);
            let else_branch = ActionValue::Operation(Box::new(add_op));

            let mut op = op_new(Operation::If);
            op.cases = Some(vec![Case {
                when: condition,
                then: then_branch,
            }]);
            op.default = Some(else_branch);

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Int(6));
        }
    }

    // -------------------------------------------------------------------------
    // Error Handling Tests
    // -------------------------------------------------------------------------

    mod errors {
        use super::*;
        use crate::article::Case;

        #[test]
        fn test_missing_subject_in_comparison() {
            let resolver = TestResolver::new();
            let mut op = op_new(Operation::Equals);
            op.value = Some(lit(42i64));

            let result = execute_operation(&op, &resolver, 0);
            assert!(matches!(result, Err(EngineError::InvalidOperation(_))));
        }

        #[test]
        fn test_missing_values_in_add() {
            let resolver = TestResolver::new();
            let op = op_new(Operation::Add);

            let result = execute_operation(&op, &resolver, 0);
            assert!(matches!(result, Err(EngineError::InvalidOperation(_))));
        }

        #[test]
        fn test_type_mismatch_in_arithmetic() {
            let resolver = TestResolver::new();
            let mut op = op_new(Operation::Add);
            op.values = Some(vec![lit(10i64), lit("not a number")]);

            let result = execute_operation(&op, &resolver, 0);
            assert!(matches!(result, Err(EngineError::TypeMismatch { .. })));
        }

        #[test]
        fn test_variable_not_found() {
            let resolver = TestResolver::new();
            let mut op = op_new(Operation::Equals);
            op.subject = Some(var("nonexistent"));
            op.value = Some(lit(42i64));

            let result = execute_operation(&op, &resolver, 0);
            assert!(matches!(result, Err(EngineError::VariableNotFound(_))));
        }

        #[test]
        fn test_missing_cases_in_if() {
            let resolver = TestResolver::new();
            let mut op = op_new(Operation::If);
            op.default = Some(lit(0i64));

            let result = execute_operation(&op, &resolver, 0);
            assert!(matches!(result, Err(EngineError::InvalidOperation(_))));
        }

        #[test]
        fn test_overflow_detection() {
            let resolver = TestResolver::new();
            let mut op = op_new(Operation::Multiply);
            op.values = Some(vec![lit(i64::MAX), lit(2i64)]);

            let result = execute_operation(&op, &resolver, 0);
            assert!(matches!(result, Err(EngineError::ArithmeticOverflow(_))));
        }

        #[test]
        fn test_max_depth_exceeded() {
            let resolver = TestResolver::new();

            let mut nested: ActionValue = lit(42i64);
            for _ in 0..=MAX_OPERATION_DEPTH + 1 {
                let mut if_op = op_new(Operation::If);
                if_op.cases = Some(vec![Case {
                    when: lit(true),
                    then: nested,
                }]);
                nested = ActionValue::Operation(Box::new(if_op));
            }

            if let ActionValue::Operation(op) = nested {
                let result = execute_operation(&op, &resolver, 0);
                assert!(
                    matches!(result, Err(EngineError::MaxDepthExceeded(_))),
                    "Expected MaxDepthExceeded but got {:?}",
                    result
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
            let large_value: i64 = 9_007_199_254_740_993; // MAX_SAFE_INTEGER + 1

            let resolver = TestResolver::new();
            let mut op = op_new(Operation::Add);
            op.values = Some(vec![lit(large_value), lit(1i64)]);

            let result = execute_operation(&op, &resolver, 0);
            assert!(
                matches!(result, Err(EngineError::ArithmeticOverflow(_))),
                "Large integer in arithmetic should cause overflow error, got: {:?}",
                result
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
            let mut op = op_new(Operation::In);
            op.subject = Some(lit(42i64));
            op.values = Some(vec![lit(10i64), lit(20i64), lit(42i64), lit(50i64)]);

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Bool(true));
        }

        #[test]
        fn test_in_not_found() {
            let resolver = TestResolver::new();
            let mut op = op_new(Operation::In);
            op.subject = Some(lit(99i64));
            op.values = Some(vec![lit(10i64), lit(20i64), lit(42i64), lit(50i64)]);

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Bool(false));
        }

        #[test]
        fn test_in_with_strings() {
            let resolver = TestResolver::new();
            let mut op = op_new(Operation::In);
            op.subject = Some(lit("apple"));
            op.values = Some(vec![lit("banana"), lit("apple"), lit("orange")]);

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Bool(true));
        }

        #[test]
        fn test_in_with_mixed_int_float() {
            let resolver = TestResolver::new();
            let mut op = op_new(Operation::In);
            op.subject = Some(lit(42i64));
            op.values = Some(vec![lit(10i64), lit(42.0f64), lit(50i64)]);

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Bool(true));
        }

        #[test]
        fn test_in_with_variables() {
            let resolver = TestResolver::new().with_var("status", "active");

            let mut op = op_new(Operation::In);
            op.subject = Some(var("status"));
            op.values = Some(vec![lit("active"), lit("pending"), lit("inactive")]);

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Bool(true));
        }

        #[test]
        fn test_in_missing_subject() {
            let resolver = TestResolver::new();
            let mut op = op_new(Operation::In);
            op.values = Some(vec![lit(1i64), lit(2i64)]);

            let result = execute_operation(&op, &resolver, 0);
            assert!(matches!(result, Err(EngineError::InvalidOperation(_))));
        }

        #[test]
        fn test_in_missing_values() {
            let resolver = TestResolver::new();
            let mut op = op_new(Operation::In);
            op.subject = Some(lit(42i64));

            let result = execute_operation(&op, &resolver, 0);
            assert!(matches!(result, Err(EngineError::InvalidOperation(_))));
        }

        #[test]
        fn test_list_construct() {
            let resolver = TestResolver::new();
            let mut op = op_new(Operation::List);
            op.items = Some(vec![lit(1i64), lit(2i64), lit(3i64)]);

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(
                result,
                Value::Array(vec![Value::Int(1), Value::Int(2), Value::Int(3)])
            );
        }

        #[test]
        fn test_list_with_mixed_types() {
            let resolver = TestResolver::new();
            let mut op = op_new(Operation::List);
            op.items = Some(vec![lit(1i64), lit("two"), lit(true)]);

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

        #[test]
        fn test_list_missing_items() {
            let resolver = TestResolver::new();
            let op = op_new(Operation::List);

            let result = execute_operation(&op, &resolver, 0);
            assert!(matches!(result, Err(EngineError::InvalidOperation(_))));
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
            let mut op = op_new(Operation::Age);
            op.date_of_birth = Some(lit("1990-03-15"));
            op.reference_date = Some(lit("2025-03-15"));

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Int(35));
        }

        #[test]
        fn test_age_before_birthday() {
            let resolver = TestResolver::new();
            let mut op = op_new(Operation::Age);
            op.date_of_birth = Some(lit("1990-03-15"));
            op.reference_date = Some(lit("2025-03-14"));

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Int(34));
        }

        #[test]
        fn test_age_feb29_birthday_on_non_leap_year() {
            // Per Dutch law (BW art. 1:2): Feb 28 counts as birthday
            let resolver = TestResolver::new();
            let mut op = op_new(Operation::Age);
            op.date_of_birth = Some(lit("2000-02-29"));
            op.reference_date = Some(lit("2001-02-28"));

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Int(1));
        }

        #[test]
        fn test_age_feb29_birthday_before_feb28() {
            let resolver = TestResolver::new();
            let mut op = op_new(Operation::Age);
            op.date_of_birth = Some(lit("2000-02-29"));
            op.reference_date = Some(lit("2001-02-27"));

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Int(0));
        }

        #[test]
        fn test_age_feb29_birthday_on_leap_year() {
            let resolver = TestResolver::new();
            let mut op = op_new(Operation::Age);
            op.date_of_birth = Some(lit("2000-02-29"));
            op.reference_date = Some(lit("2004-02-29"));

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Int(4));
        }

        #[test]
        fn test_age_with_variables() {
            let resolver = TestResolver::new()
                .with_var("birth_date", "1990-03-15")
                .with_var("ref_date", "2025-03-15");

            let mut op = op_new(Operation::Age);
            op.date_of_birth = Some(var("birth_date"));
            op.reference_date = Some(var("ref_date"));

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Int(35));
        }

        #[test]
        fn test_age_with_object_date() {
            let mut date_obj = HashMap::new();
            date_obj.insert("iso".to_string(), Value::String("2025-01-01".to_string()));
            date_obj.insert("year".to_string(), Value::Int(2025));
            date_obj.insert("month".to_string(), Value::Int(1));
            date_obj.insert("day".to_string(), Value::Int(1));

            let resolver = TestResolver::new()
                .with_var("referencedate", Value::Object(date_obj))
                .with_var("geboortedatum", Value::String("2005-01-01".to_string()));

            let mut op = op_new(Operation::Age);
            op.date_of_birth = Some(var("geboortedatum"));
            op.reference_date = Some(var("referencedate"));

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Int(20));
        }

        #[test]
        fn test_date_add_days() {
            let resolver = TestResolver::new();
            let mut op = op_new(Operation::DateAdd);
            op.date = Some(lit("2025-01-10"));
            op.days = Some(lit(5i64));

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::String("2025-01-15".to_string()));
        }

        #[test]
        fn test_date_add_weeks() {
            let resolver = TestResolver::new();
            let mut op = op_new(Operation::DateAdd);
            op.date = Some(lit("2025-01-01"));
            op.weeks = Some(lit(2i64));

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::String("2025-01-15".to_string()));
        }

        #[test]
        fn test_date_add_days_and_weeks() {
            let resolver = TestResolver::new();
            let mut op = op_new(Operation::DateAdd);
            op.date = Some(lit("2025-01-01"));
            op.days = Some(lit(3i64));
            op.weeks = Some(lit(1i64));

            // 1 week + 3 days = 10 days from Jan 1 = Jan 11
            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::String("2025-01-11".to_string()));
        }

        #[test]
        fn test_date_add_negative_days() {
            let resolver = TestResolver::new();
            let mut op = op_new(Operation::DateAdd);
            op.date = Some(lit("2025-01-15"));
            op.days = Some(lit(-5i64));

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::String("2025-01-10".to_string()));
        }

        #[test]
        fn test_date_construct() {
            let resolver = TestResolver::new();
            let mut op = op_new(Operation::Date);
            op.year = Some(lit(2025i64));
            op.month = Some(lit(3i64));
            op.day = Some(lit(15i64));

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::String("2025-03-15".to_string()));
        }

        #[test]
        fn test_date_construct_leap_year() {
            let resolver = TestResolver::new();
            let mut op = op_new(Operation::Date);
            op.year = Some(lit(2024i64));
            op.month = Some(lit(2i64));
            op.day = Some(lit(29i64));

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::String("2024-02-29".to_string()));
        }

        #[test]
        fn test_date_construct_invalid() {
            let resolver = TestResolver::new();
            let mut op = op_new(Operation::Date);
            op.year = Some(lit(2025i64));
            op.month = Some(lit(2i64));
            op.day = Some(lit(30i64)); // Feb 30 doesn't exist

            let result = execute_operation(&op, &resolver, 0);
            assert!(matches!(result, Err(EngineError::InvalidOperation(_))));
        }

        #[test]
        fn test_day_of_week() {
            let resolver = TestResolver::new();

            // 2025-01-06 is a Monday (weekday 0)
            let mut op = op_new(Operation::DayOfWeek);
            op.date = Some(lit("2025-01-06"));
            assert_eq!(execute_operation(&op, &resolver, 0).unwrap(), Value::Int(0));

            // 2025-01-12 is a Sunday (weekday 6)
            let mut op2 = op_new(Operation::DayOfWeek);
            op2.date = Some(lit("2025-01-12"));
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
        let mut date_obj = HashMap::new();
        date_obj.insert("iso".to_string(), Value::String("2025-01-01".to_string()));
        date_obj.insert("year".to_string(), Value::Int(2025));

        let result = parse_date(&Value::Object(date_obj)).unwrap();
        assert_eq!(result.to_string(), "2025-01-01");
    }

    #[test]
    fn test_parse_date_object_without_iso_field() {
        let mut date_obj = HashMap::new();
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
}
