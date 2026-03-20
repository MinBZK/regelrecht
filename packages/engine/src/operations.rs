//! Operation execution for the RegelRecht engine
//!
//! This module implements the execution logic for all 21 operation types:
//! - **Comparison (6):** EQUALS, NOT_EQUALS, GREATER_THAN, LESS_THAN, GREATER_THAN_OR_EQUAL, LESS_THAN_OR_EQUAL
//! - **Arithmetic (4):** ADD, SUBTRACT, MULTIPLY, DIVIDE
//! - **Aggregate (2):** MAX, MIN
//! - **Logical (2):** AND, OR
//! - **Conditional (2):** IF, SWITCH
//! - **Null checking (2):** IS_NULL, NOT_NULL
//! - **Membership testing (2):** IN, NOT_IN
//! - **Date operations (1):** SUBTRACT_DATE

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
                // For IF, execute_if already set a message with branch info;
                // incorporate it instead of overwriting.
                let existing_msg = resolver.trace_get_message();
                let msg = if op.operation == Operation::If {
                    if let Some(branch) = existing_msg {
                        format!("IF({}) = {}", branch, format_value_for_trace(value))
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
        Operation::Equals => execute_equals(op, resolver, depth),
        Operation::NotEquals => execute_not_equals(op, resolver, depth),
        Operation::GreaterThan => execute_numeric_comparison(op, resolver, depth, |a, b| a > b),
        Operation::LessThan => execute_numeric_comparison(op, resolver, depth, |a, b| a < b),
        Operation::GreaterThanOrEqual => {
            execute_numeric_comparison(op, resolver, depth, |a, b| a >= b)
        }
        Operation::LessThanOrEqual => {
            execute_numeric_comparison(op, resolver, depth, |a, b| a <= b)
        }

        // Arithmetic operations
        Operation::Add => execute_add(op, resolver, depth),
        Operation::Subtract => execute_subtract(op, resolver, depth),
        Operation::Multiply => execute_multiply(op, resolver, depth),
        Operation::Divide => execute_divide(op, resolver, depth),

        // Aggregate operations
        Operation::Max => execute_aggregate(op, resolver, depth, f64::max),
        Operation::Min => execute_aggregate(op, resolver, depth, f64::min),

        // Logical operations
        Operation::And => execute_and(op, resolver, depth),
        Operation::Or => execute_or(op, resolver, depth),

        // Conditional operations
        Operation::If => execute_if(op, resolver, depth),
        Operation::Switch => execute_switch(op, resolver, depth),

        // Null checking operations
        Operation::IsNull => execute_is_null(op, resolver, depth),
        Operation::NotNull => execute_not_null(op, resolver, depth),

        // Membership testing operations
        Operation::In => execute_in(op, resolver, depth),
        Operation::NotIn => execute_not_in(op, resolver, depth),

        // Date operations
        Operation::SubtractDate => execute_subtract_date(op, resolver, depth),
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

/// Execute EQUALS operation with Python-style numeric coercion.
///
/// - `Int(42) == Float(42.0)` returns `true` (like Python)
/// - Non-numeric types use structural equality
fn execute_equals<R: ValueResolver>(
    op: &ActionOperation,
    resolver: &R,
    depth: usize,
) -> Result<Value> {
    let subject = op.subject.as_ref().ok_or_else(|| {
        EngineError::InvalidOperation("Comparison requires 'subject'".to_string())
    })?;
    let value = op
        .value
        .as_ref()
        .ok_or_else(|| EngineError::InvalidOperation("Comparison requires 'value'".to_string()))?;

    let subject_val = evaluate_value(subject, resolver, depth)?;
    let value_val = evaluate_value(value, resolver, depth)?;

    Ok(Value::Bool(values_equal(&subject_val, &value_val)))
}

/// Execute NOT_EQUALS operation with Python-style numeric coercion.
fn execute_not_equals<R: ValueResolver>(
    op: &ActionOperation,
    resolver: &R,
    depth: usize,
) -> Result<Value> {
    let subject = op.subject.as_ref().ok_or_else(|| {
        EngineError::InvalidOperation("Comparison requires 'subject'".to_string())
    })?;
    let value = op
        .value
        .as_ref()
        .ok_or_else(|| EngineError::InvalidOperation("Comparison requires 'value'".to_string()))?;

    let subject_val = evaluate_value(subject, resolver, depth)?;
    let value_val = evaluate_value(value, resolver, depth)?;

    Ok(Value::Bool(!values_equal(&subject_val, &value_val)))
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
    let subject = op.subject.as_ref().ok_or_else(|| {
        EngineError::InvalidOperation("Comparison requires 'subject'".to_string())
    })?;
    let value = op
        .value
        .as_ref()
        .ok_or_else(|| EngineError::InvalidOperation("Comparison requires 'value'".to_string()))?;

    let subject_val = evaluate_value(subject, resolver, depth)?;
    let value_val = evaluate_value(value, resolver, depth)?;

    let subject_num = to_number(&subject_val)?;
    let value_num = to_number(&value_val)?;

    Ok(Value::Bool(compare(subject_num, value_num)))
}

// =============================================================================
// Arithmetic Operations
// =============================================================================

/// Execute ADD operation: sum of all values.
fn execute_add<R: ValueResolver>(
    op: &ActionOperation,
    resolver: &R,
    depth: usize,
) -> Result<Value> {
    let values = get_values(op)?;
    let evaluated = evaluate_values(values, resolver, depth)?;

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

// =============================================================================
// Conditional Operations
// =============================================================================

/// Execute IF operation: evaluates condition, returns then or else branch.
fn execute_if<R: ValueResolver>(op: &ActionOperation, resolver: &R, depth: usize) -> Result<Value> {
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
    let condition_result = evaluate_value(when, resolver, depth)?;
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
        let result = evaluate_value(then, resolver, depth)?;
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
        let result = evaluate_value(else_branch, resolver, depth)?;
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

/// Execute SWITCH operation: evaluates cases in order, returns first matching case.
fn execute_switch<R: ValueResolver>(
    op: &ActionOperation,
    resolver: &R,
    depth: usize,
) -> Result<Value> {
    let cases = op
        .cases
        .as_ref()
        .ok_or_else(|| EngineError::InvalidOperation("SWITCH requires 'cases'".to_string()))?;

    for case in cases {
        let condition_result = evaluate_value(&case.when, resolver, depth)?;
        if condition_result.to_bool() {
            return evaluate_value(&case.then, resolver, depth);
        }
    }

    // Return default if no case matched
    if let Some(default) = &op.default {
        evaluate_value(default, resolver, depth)
    } else {
        Ok(Value::Null)
    }
}

// =============================================================================
// Null Checking Operations
// =============================================================================

/// Execute IS_NULL operation: returns true if the subject is null.
fn execute_is_null<R: ValueResolver>(
    op: &ActionOperation,
    resolver: &R,
    depth: usize,
) -> Result<Value> {
    let subject = op
        .subject
        .as_ref()
        .ok_or_else(|| EngineError::InvalidOperation("IS_NULL requires 'subject'".to_string()))?;

    let subject_val = evaluate_value(subject, resolver, depth)?;
    Ok(Value::Bool(subject_val.is_null()))
}

/// Execute NOT_NULL operation: returns true if the subject is not null.
fn execute_not_null<R: ValueResolver>(
    op: &ActionOperation,
    resolver: &R,
    depth: usize,
) -> Result<Value> {
    let subject = op
        .subject
        .as_ref()
        .ok_or_else(|| EngineError::InvalidOperation("NOT_NULL requires 'subject'".to_string()))?;

    let subject_val = evaluate_value(subject, resolver, depth)?;
    Ok(Value::Bool(!subject_val.is_null()))
}

// =============================================================================
// Membership Testing Operations
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

/// Execute IN operation: returns true if subject is in the values list.
///
/// Uses Python-style numeric coercion for equality comparison.
fn execute_in<R: ValueResolver>(op: &ActionOperation, resolver: &R, depth: usize) -> Result<Value> {
    let subject = op
        .subject
        .as_ref()
        .ok_or_else(|| EngineError::InvalidOperation("IN requires 'subject'".to_string()))?;

    let subject_val = evaluate_value(subject, resolver, depth)?;
    let check_values = resolve_membership_values(op, "IN", resolver, depth)?;

    for val in &check_values {
        if values_equal(&subject_val, val) {
            return Ok(Value::Bool(true));
        }
    }

    Ok(Value::Bool(false))
}

/// Execute NOT_IN operation: returns true if subject is not in the values list.
///
/// Uses Python-style numeric coercion for equality comparison.
fn execute_not_in<R: ValueResolver>(
    op: &ActionOperation,
    resolver: &R,
    depth: usize,
) -> Result<Value> {
    let subject = op
        .subject
        .as_ref()
        .ok_or_else(|| EngineError::InvalidOperation("NOT_IN requires 'subject'".to_string()))?;

    let subject_val = evaluate_value(subject, resolver, depth)?;
    let check_values = resolve_membership_values(op, "NOT_IN", resolver, depth)?;

    for val in &check_values {
        if values_equal(&subject_val, val) {
            return Ok(Value::Bool(false));
        }
    }

    Ok(Value::Bool(true))
}

// =============================================================================
// Date Operations
// =============================================================================

/// Execute SUBTRACT_DATE operation: calculate the difference between two dates.
///
/// Calculates the difference between the subject date and the value date.
/// Returns a positive number if subject is after value, negative if before.
///
/// # Arguments
/// - `subject`: The first date (minuend)
/// - `value`: The second date (subtrahend)
/// - `unit`: The unit of measurement ("days", "months", "years")
///
/// # Date Parsing
/// Dates should be in ISO 8601 format (YYYY-MM-DD).
///
/// # Calculation Details
/// - **days**: Uses exact calendar day difference
/// - **months**: Uses proper calendar arithmetic, not days/30 approximation
/// - **years**: Uses proper calendar arithmetic, not days/365 approximation
///
/// For months and years, the calculation counts complete units between the dates.
/// A month is counted as complete when the same day-of-month is reached (or end of month).
fn execute_subtract_date<R: ValueResolver>(
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

    // Parse dates from string values
    let subject_date = parse_date(&subject_val)?;
    let value_date = parse_date(&value_val)?;

    // Get unit, defaulting to "days"
    let unit = op.unit.as_deref().unwrap_or("days");

    let result = match unit {
        "days" => calculate_days_difference(subject_date, value_date),
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
fn calculate_days_difference(date1: NaiveDate, date2: NaiveDate) -> i64 {
    (date1 - date2).num_days()
}

/// Calculate the difference in complete months between two dates.
///
/// Uses proper calendar arithmetic. A month is counted as complete when
/// the same day-of-month (or end of month if day doesn't exist) is reached.
/// For end-of-month edge cases (e.g., Jan 31 → Feb 28), if `earlier.day()`
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
    // Month lengths are well-known; use a match for safety and clarity
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 {
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
    let actual_type = match actual {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Int(_) => "integer",
        Value::Float(_) => "float",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    };
    EngineError::TypeMismatch {
        expected: expected.to_string(),
        actual: actual_type.to_string(),
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

    // -------------------------------------------------------------------------
    // Comparison Operations Tests
    // -------------------------------------------------------------------------

    mod comparison {
        use super::*;

        #[test]
        fn test_equals_integers() {
            let resolver = TestResolver::new();
            let op = ActionOperation {
                operation: Operation::Equals,
                subject: Some(lit(42i64)),
                value: Some(lit(42i64)),
                values: None,
                when: None,
                then: None,
                else_branch: None,
                conditions: None,
                cases: None,
                default: None,
                unit: None,
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Bool(true));
        }

        #[test]
        fn test_equals_different_values() {
            let resolver = TestResolver::new();
            let op = ActionOperation {
                operation: Operation::Equals,
                subject: Some(lit(42i64)),
                value: Some(lit(43i64)),
                values: None,
                when: None,
                then: None,
                else_branch: None,
                conditions: None,
                cases: None,
                default: None,
                unit: None,
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Bool(false));
        }

        #[test]
        fn test_not_equals() {
            let resolver = TestResolver::new();
            let op = ActionOperation {
                operation: Operation::NotEquals,
                subject: Some(lit(42i64)),
                value: Some(lit(43i64)),
                values: None,
                when: None,
                then: None,
                else_branch: None,
                conditions: None,
                cases: None,
                default: None,
                unit: None,
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Bool(true));
        }

        #[test]
        fn test_greater_than() {
            let resolver = TestResolver::new();
            let op = ActionOperation {
                operation: Operation::GreaterThan,
                subject: Some(lit(50i64)),
                value: Some(lit(42i64)),
                values: None,
                when: None,
                then: None,
                else_branch: None,
                conditions: None,
                cases: None,
                default: None,
                unit: None,
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Bool(true));
        }

        #[test]
        fn test_less_than() {
            let resolver = TestResolver::new();
            let op = ActionOperation {
                operation: Operation::LessThan,
                subject: Some(lit(30i64)),
                value: Some(lit(42i64)),
                values: None,
                when: None,
                then: None,
                else_branch: None,
                conditions: None,
                cases: None,
                default: None,
                unit: None,
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Bool(true));
        }

        #[test]
        fn test_greater_than_or_equal() {
            let resolver = TestResolver::new();

            // Test equal case
            let op = ActionOperation {
                operation: Operation::GreaterThanOrEqual,
                subject: Some(lit(42i64)),
                value: Some(lit(42i64)),
                values: None,
                when: None,
                then: None,
                else_branch: None,
                conditions: None,
                cases: None,
                default: None,
                unit: None,
            };
            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::Bool(true)
            );

            // Test greater case
            let op2 = ActionOperation {
                operation: Operation::GreaterThanOrEqual,
                subject: Some(lit(50i64)),
                value: Some(lit(42i64)),
                values: None,
                when: None,
                then: None,
                else_branch: None,
                conditions: None,
                cases: None,
                default: None,
                unit: None,
            };
            assert_eq!(
                execute_operation(&op2, &resolver, 0).unwrap(),
                Value::Bool(true)
            );
        }

        #[test]
        fn test_less_than_or_equal() {
            let resolver = TestResolver::new();

            // Test equal case
            let op = ActionOperation {
                operation: Operation::LessThanOrEqual,
                subject: Some(lit(42i64)),
                value: Some(lit(42i64)),
                values: None,
                when: None,
                then: None,
                else_branch: None,
                conditions: None,
                cases: None,
                default: None,
                unit: None,
            };
            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::Bool(true)
            );

            // Test less case
            let op2 = ActionOperation {
                operation: Operation::LessThanOrEqual,
                subject: Some(lit(30i64)),
                value: Some(lit(42i64)),
                values: None,
                when: None,
                then: None,
                else_branch: None,
                conditions: None,
                cases: None,
                default: None,
                unit: None,
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

            let op = ActionOperation {
                operation: Operation::GreaterThanOrEqual,
                subject: Some(var("age")),
                value: Some(var("min_age")),
                values: None,
                when: None,
                then: None,
                else_branch: None,
                conditions: None,
                cases: None,
                default: None,
                unit: None,
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Bool(true));
        }

        #[test]
        fn test_comparison_mixed_int_float() {
            let resolver = TestResolver::new();

            // Int(42) == Float(42.0) should be true (Python-style numeric coercion)
            let op = ActionOperation {
                operation: Operation::Equals,
                subject: Some(lit(42i64)),
                value: Some(lit(42.0f64)),
                values: None,
                when: None,
                then: None,
                else_branch: None,
                conditions: None,
                cases: None,
                default: None,
                unit: None,
            };
            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Bool(true));

            // Float(42.0) == Int(42) should also be true (symmetric)
            let op2 = ActionOperation {
                operation: Operation::Equals,
                subject: Some(lit(42.0f64)),
                value: Some(lit(42i64)),
                values: None,
                when: None,
                then: None,
                else_branch: None,
                conditions: None,
                cases: None,
                default: None,
                unit: None,
            };
            let result2 = execute_operation(&op2, &resolver, 0).unwrap();
            assert_eq!(result2, Value::Bool(true));

            // Int(42) != Float(42.5) should be true
            let op3 = ActionOperation {
                operation: Operation::NotEquals,
                subject: Some(lit(42i64)),
                value: Some(lit(42.5f64)),
                values: None,
                when: None,
                then: None,
                else_branch: None,
                conditions: None,
                cases: None,
                default: None,
                unit: None,
            };
            let result3 = execute_operation(&op3, &resolver, 0).unwrap();
            assert_eq!(result3, Value::Bool(true));
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
            let op = ActionOperation {
                operation: Operation::Add,
                subject: None,
                value: None,
                values: Some(vec![lit(10i64), lit(20i64), lit(30i64)]),
                when: None,
                then: None,
                else_branch: None,
                conditions: None,
                cases: None,
                default: None,
                unit: None,
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Int(60));
        }

        #[test]
        fn test_add_with_floats() {
            let resolver = TestResolver::new();
            let op = ActionOperation {
                operation: Operation::Add,
                subject: None,
                value: None,
                values: Some(vec![lit(10i64), lit(20.5f64), lit(30i64)]),
                when: None,
                then: None,
                else_branch: None,
                conditions: None,
                cases: None,
                default: None,
                unit: None,
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Float(60.5));
        }

        #[test]
        fn test_subtract() {
            let resolver = TestResolver::new();
            let op = ActionOperation {
                operation: Operation::Subtract,
                subject: None,
                value: None,
                values: Some(vec![lit(100i64), lit(30i64), lit(20i64)]),
                when: None,
                then: None,
                else_branch: None,
                conditions: None,
                cases: None,
                default: None,
                unit: None,
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Int(50));
        }

        #[test]
        fn test_multiply() {
            let resolver = TestResolver::new();
            let op = ActionOperation {
                operation: Operation::Multiply,
                subject: None,
                value: None,
                values: Some(vec![lit(2i64), lit(3i64), lit(4i64)]),
                when: None,
                then: None,
                else_branch: None,
                conditions: None,
                cases: None,
                default: None,
                unit: None,
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Int(24));
        }

        #[test]
        fn test_divide() {
            let resolver = TestResolver::new();
            let op = ActionOperation {
                operation: Operation::Divide,
                subject: None,
                value: None,
                values: Some(vec![lit(100i64), lit(2i64)]),
                when: None,
                then: None,
                else_branch: None,
                conditions: None,
                cases: None,
                default: None,
                unit: None,
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Float(50.0));
        }

        #[test]
        fn test_divide_by_zero() {
            let resolver = TestResolver::new();
            let op = ActionOperation {
                operation: Operation::Divide,
                subject: None,
                value: None,
                values: Some(vec![lit(100i64), lit(0i64)]),
                when: None,
                then: None,
                else_branch: None,
                conditions: None,
                cases: None,
                default: None,
                unit: None,
            };

            let result = execute_operation(&op, &resolver, 0);
            assert!(matches!(result, Err(EngineError::DivisionByZero)));
        }

        #[test]
        fn test_arithmetic_with_variables() {
            let resolver = TestResolver::new()
                .with_var("base", 1000i64)
                .with_var("rate", 0.05f64);

            let op = ActionOperation {
                operation: Operation::Multiply,
                subject: None,
                value: None,
                values: Some(vec![var("base"), var("rate")]),
                when: None,
                then: None,
                else_branch: None,
                conditions: None,
                cases: None,
                default: None,
                unit: None,
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Float(50.0));
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
            let op = ActionOperation {
                operation: Operation::Max,
                subject: None,
                value: None,
                values: Some(vec![lit(10i64), lit(50i64), lit(30i64)]),
                when: None,
                then: None,
                else_branch: None,
                conditions: None,
                cases: None,
                default: None,
                unit: None,
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Int(50));
        }

        #[test]
        fn test_min() {
            let resolver = TestResolver::new();
            let op = ActionOperation {
                operation: Operation::Min,
                subject: None,
                value: None,
                values: Some(vec![lit(10i64), lit(50i64), lit(30i64)]),
                when: None,
                then: None,
                else_branch: None,
                conditions: None,
                cases: None,
                default: None,
                unit: None,
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Int(10));
        }

        #[test]
        fn test_max_with_floats() {
            let resolver = TestResolver::new();
            let op = ActionOperation {
                operation: Operation::Max,
                subject: None,
                value: None,
                values: Some(vec![lit(10.5f64), lit(50.3f64), lit(30.7f64)]),
                when: None,
                then: None,
                else_branch: None,
                conditions: None,
                cases: None,
                default: None,
                unit: None,
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Float(50.3));
        }

        #[test]
        fn test_max_with_zero() {
            let resolver = TestResolver::new();
            let op = ActionOperation {
                operation: Operation::Max,
                subject: None,
                value: None,
                values: Some(vec![lit(0i64), lit(-10i64)]),
                when: None,
                then: None,
                else_branch: None,
                conditions: None,
                cases: None,
                default: None,
                unit: None,
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
            let op = ActionOperation {
                operation: Operation::And,
                subject: None,
                value: None,
                values: None,
                when: None,
                then: None,
                else_branch: None,
                conditions: Some(vec![lit(true), lit(true), lit(true)]),
                cases: None,
                default: None,
                unit: None,
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Bool(true));
        }

        #[test]
        fn test_and_one_false() {
            let resolver = TestResolver::new();
            let op = ActionOperation {
                operation: Operation::And,
                subject: None,
                value: None,
                values: None,
                when: None,
                then: None,
                else_branch: None,
                conditions: Some(vec![lit(true), lit(false), lit(true)]),
                cases: None,
                default: None,
                unit: None,
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Bool(false));
        }

        #[test]
        fn test_or_one_true() {
            let resolver = TestResolver::new();
            let op = ActionOperation {
                operation: Operation::Or,
                subject: None,
                value: None,
                values: None,
                when: None,
                then: None,
                else_branch: None,
                conditions: Some(vec![lit(false), lit(true), lit(false)]),
                cases: None,
                default: None,
                unit: None,
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Bool(true));
        }

        #[test]
        fn test_or_all_false() {
            let resolver = TestResolver::new();
            let op = ActionOperation {
                operation: Operation::Or,
                subject: None,
                value: None,
                values: None,
                when: None,
                then: None,
                else_branch: None,
                conditions: Some(vec![lit(false), lit(false), lit(false)]),
                cases: None,
                default: None,
                unit: None,
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Bool(false));
        }

        #[test]
        fn test_and_with_nested_comparison() {
            let resolver = TestResolver::new()
                .with_var("age", 25i64)
                .with_var("has_insurance", true);

            // AND with nested operations: age >= 18 AND has_insurance
            let age_check = ActionValue::Operation(Box::new(ActionOperation {
                operation: Operation::GreaterThanOrEqual,
                subject: Some(var("age")),
                value: Some(lit(18i64)),
                values: None,
                when: None,
                then: None,
                else_branch: None,
                conditions: None,
                cases: None,
                default: None,
                unit: None,
            }));

            let op = ActionOperation {
                operation: Operation::And,
                subject: None,
                value: None,
                values: None,
                when: None,
                then: None,
                else_branch: None,
                conditions: Some(vec![age_check, var("has_insurance")]),
                cases: None,
                default: None,
                unit: None,
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Bool(true));
        }
    }

    // -------------------------------------------------------------------------
    // Conditional Operations Tests
    // -------------------------------------------------------------------------

    mod conditional {
        use super::*;
        use crate::article::SwitchCase;

        #[test]
        fn test_if_true_branch() {
            let resolver = TestResolver::new();
            let op = ActionOperation {
                operation: Operation::If,
                subject: None,
                value: None,
                values: None,
                when: Some(lit(true)),
                then: Some(lit(100i64)),
                else_branch: Some(lit(50i64)),
                conditions: None,
                cases: None,
                default: None,
                unit: None,
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Int(100));
        }

        #[test]
        fn test_if_false_branch() {
            let resolver = TestResolver::new();
            let op = ActionOperation {
                operation: Operation::If,
                subject: None,
                value: None,
                values: None,
                when: Some(lit(false)),
                then: Some(lit(100i64)),
                else_branch: Some(lit(50i64)),
                conditions: None,
                cases: None,
                default: None,
                unit: None,
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Int(50));
        }

        #[test]
        fn test_if_no_else_returns_null() {
            let resolver = TestResolver::new();
            let op = ActionOperation {
                operation: Operation::If,
                subject: None,
                value: None,
                values: None,
                when: Some(lit(false)),
                then: Some(lit(100i64)),
                else_branch: None,
                conditions: None,
                cases: None,
                default: None,
                unit: None,
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Null);
        }

        #[test]
        fn test_if_with_nested_condition() {
            let resolver = TestResolver::new().with_var("has_partner", true);

            let condition = ActionValue::Operation(Box::new(ActionOperation {
                operation: Operation::Equals,
                subject: Some(var("has_partner")),
                value: Some(lit(true)),
                values: None,
                when: None,
                then: None,
                else_branch: None,
                conditions: None,
                cases: None,
                default: None,
                unit: None,
            }));

            let op = ActionOperation {
                operation: Operation::If,
                subject: None,
                value: None,
                values: None,
                when: Some(condition),
                then: Some(lit(200i64)),
                else_branch: Some(lit(100i64)),
                conditions: None,
                cases: None,
                default: None,
                unit: None,
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Int(200));
        }

        #[test]
        fn test_switch_first_match() {
            let resolver = TestResolver::new();
            let op = ActionOperation {
                operation: Operation::Switch,
                subject: None,
                value: None,
                values: None,
                when: None,
                then: None,
                else_branch: None,
                conditions: None,
                cases: Some(vec![
                    SwitchCase {
                        when: lit(true),
                        then: lit(100i64),
                    },
                    SwitchCase {
                        when: lit(true),
                        then: lit(200i64),
                    },
                ]),
                default: Some(lit(0i64)),
                unit: None,
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Int(100));
        }

        #[test]
        fn test_switch_second_match() {
            let resolver = TestResolver::new();
            let op = ActionOperation {
                operation: Operation::Switch,
                subject: None,
                value: None,
                values: None,
                when: None,
                then: None,
                else_branch: None,
                conditions: None,
                cases: Some(vec![
                    SwitchCase {
                        when: lit(false),
                        then: lit(100i64),
                    },
                    SwitchCase {
                        when: lit(true),
                        then: lit(200i64),
                    },
                ]),
                default: Some(lit(0i64)),
                unit: None,
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Int(200));
        }

        #[test]
        fn test_switch_default() {
            let resolver = TestResolver::new();
            let op = ActionOperation {
                operation: Operation::Switch,
                subject: None,
                value: None,
                values: None,
                when: None,
                then: None,
                else_branch: None,
                conditions: None,
                cases: Some(vec![
                    SwitchCase {
                        when: lit(false),
                        then: lit(100i64),
                    },
                    SwitchCase {
                        when: lit(false),
                        then: lit(200i64),
                    },
                ]),
                default: Some(lit(0i64)),
                unit: None,
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Int(0));
        }

        #[test]
        fn test_switch_no_default_returns_null() {
            let resolver = TestResolver::new();
            let op = ActionOperation {
                operation: Operation::Switch,
                subject: None,
                value: None,
                values: None,
                when: None,
                then: None,
                else_branch: None,
                conditions: None,
                cases: Some(vec![SwitchCase {
                    when: lit(false),
                    then: lit(100i64),
                }]),
                default: None,
                unit: None,
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Null);
        }

        #[test]
        fn test_switch_with_nested_conditions() {
            let resolver = TestResolver::new().with_var("status", "active");

            let op = ActionOperation {
                operation: Operation::Switch,
                subject: None,
                value: None,
                values: None,
                when: None,
                then: None,
                else_branch: None,
                conditions: None,
                cases: Some(vec![
                    SwitchCase {
                        when: ActionValue::Operation(Box::new(ActionOperation {
                            operation: Operation::Equals,
                            subject: Some(var("status")),
                            value: Some(lit("pending")),
                            values: None,
                            when: None,
                            then: None,
                            else_branch: None,
                            conditions: None,
                            cases: None,
                            default: None,
                            unit: None,
                        })),
                        then: lit(10i64),
                    },
                    SwitchCase {
                        when: ActionValue::Operation(Box::new(ActionOperation {
                            operation: Operation::Equals,
                            subject: Some(var("status")),
                            value: Some(lit("active")),
                            values: None,
                            when: None,
                            then: None,
                            else_branch: None,
                            conditions: None,
                            cases: None,
                            default: None,
                            unit: None,
                        })),
                        then: lit(20i64),
                    },
                ]),
                default: Some(lit(0i64)),
                unit: None,
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

            let subtract_op = ActionValue::Operation(Box::new(ActionOperation {
                operation: Operation::Subtract,
                subject: None,
                value: None,
                values: Some(vec![lit(100i64), lit(50i64)]),
                when: None,
                then: None,
                else_branch: None,
                conditions: None,
                cases: None,
                default: None,
                unit: None,
            }));

            let op = ActionOperation {
                operation: Operation::Max,
                subject: None,
                value: None,
                values: Some(vec![lit(0i64), subtract_op]),
                when: None,
                then: None,
                else_branch: None,
                conditions: None,
                cases: None,
                default: None,
                unit: None,
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Int(50));
        }

        #[test]
        fn test_deeply_nested_operations() {
            // IF (10 > 5) THEN (2 * 3) ELSE (1 + 1)
            let resolver = TestResolver::new();

            let condition = ActionValue::Operation(Box::new(ActionOperation {
                operation: Operation::GreaterThan,
                subject: Some(lit(10i64)),
                value: Some(lit(5i64)),
                values: None,
                when: None,
                then: None,
                else_branch: None,
                conditions: None,
                cases: None,
                default: None,
                unit: None,
            }));

            let then_branch = ActionValue::Operation(Box::new(ActionOperation {
                operation: Operation::Multiply,
                subject: None,
                value: None,
                values: Some(vec![lit(2i64), lit(3i64)]),
                when: None,
                then: None,
                else_branch: None,
                conditions: None,
                cases: None,
                default: None,
                unit: None,
            }));

            let else_branch = ActionValue::Operation(Box::new(ActionOperation {
                operation: Operation::Add,
                subject: None,
                value: None,
                values: Some(vec![lit(1i64), lit(1i64)]),
                when: None,
                then: None,
                else_branch: None,
                conditions: None,
                cases: None,
                default: None,
                unit: None,
            }));

            let op = ActionOperation {
                operation: Operation::If,
                subject: None,
                value: None,
                values: None,
                when: Some(condition),
                then: Some(then_branch),
                else_branch: Some(else_branch),
                conditions: None,
                cases: None,
                default: None,
                unit: None,
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
        fn test_missing_subject_in_comparison() {
            let resolver = TestResolver::new();
            let op = ActionOperation {
                operation: Operation::Equals,
                subject: None,
                value: Some(lit(42i64)),
                values: None,
                when: None,
                then: None,
                else_branch: None,
                conditions: None,
                cases: None,
                default: None,
                unit: None,
            };

            let result = execute_operation(&op, &resolver, 0);
            assert!(matches!(result, Err(EngineError::InvalidOperation(_))));
        }

        #[test]
        fn test_missing_values_in_add() {
            let resolver = TestResolver::new();
            let op = ActionOperation {
                operation: Operation::Add,
                subject: None,
                value: None,
                values: None,
                when: None,
                then: None,
                else_branch: None,
                conditions: None,
                cases: None,
                default: None,
                unit: None,
            };

            let result = execute_operation(&op, &resolver, 0);
            assert!(matches!(result, Err(EngineError::InvalidOperation(_))));
        }

        #[test]
        fn test_type_mismatch_in_arithmetic() {
            let resolver = TestResolver::new();
            let op = ActionOperation {
                operation: Operation::Add,
                subject: None,
                value: None,
                values: Some(vec![lit(10i64), lit("not a number")]),
                when: None,
                then: None,
                else_branch: None,
                conditions: None,
                cases: None,
                default: None,
                unit: None,
            };

            let result = execute_operation(&op, &resolver, 0);
            assert!(matches!(result, Err(EngineError::TypeMismatch { .. })));
        }

        #[test]
        fn test_variable_not_found() {
            let resolver = TestResolver::new();
            let op = ActionOperation {
                operation: Operation::Equals,
                subject: Some(var("nonexistent")),
                value: Some(lit(42i64)),
                values: None,
                when: None,
                then: None,
                else_branch: None,
                conditions: None,
                cases: None,
                default: None,
                unit: None,
            };

            let result = execute_operation(&op, &resolver, 0);
            assert!(matches!(result, Err(EngineError::VariableNotFound(_))));
        }

        #[test]
        fn test_missing_when_in_if() {
            let resolver = TestResolver::new();
            let op = ActionOperation {
                operation: Operation::If,
                subject: None,
                value: None,
                values: None,
                when: None,
                then: Some(lit(100i64)),
                else_branch: None,
                conditions: None,
                cases: None,
                default: None,
                unit: None,
            };

            let result = execute_operation(&op, &resolver, 0);
            assert!(matches!(result, Err(EngineError::InvalidOperation(_))));
        }

        #[test]
        fn test_missing_cases_in_switch() {
            let resolver = TestResolver::new();
            let op = ActionOperation {
                operation: Operation::Switch,
                subject: None,
                value: None,
                values: None,
                when: None,
                then: None,
                else_branch: None,
                conditions: None,
                cases: None,
                default: Some(lit(0i64)),
                unit: None,
            };

            let result = execute_operation(&op, &resolver, 0);
            assert!(matches!(result, Err(EngineError::InvalidOperation(_))));
        }

        #[test]
        fn test_overflow_detection() {
            let resolver = TestResolver::new();
            // Use values that will overflow when multiplied
            let op = ActionOperation {
                operation: Operation::Multiply,
                subject: None,
                value: None,
                values: Some(vec![lit(i64::MAX), lit(2i64)]),
                when: None,
                then: None,
                else_branch: None,
                conditions: None,
                cases: None,
                default: None,
                unit: None,
            };

            let result = execute_operation(&op, &resolver, 0);
            assert!(matches!(result, Err(EngineError::ArithmeticOverflow(_))));
        }

        #[test]
        fn test_max_depth_exceeded() {
            // Create a deeply nested operation that exceeds MAX_OPERATION_DEPTH
            let resolver = TestResolver::new();

            // Build nested IF operations
            let mut nested: ActionValue = lit(42i64);
            for _ in 0..=MAX_OPERATION_DEPTH + 1 {
                nested = ActionValue::Operation(Box::new(ActionOperation {
                    operation: Operation::If,
                    subject: None,
                    value: None,
                    values: None,
                    when: Some(lit(true)),
                    then: Some(nested),
                    else_branch: None,
                    conditions: None,
                    cases: None,
                    default: None,
                    unit: None,
                }));
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
            // Test that NaN is properly detected
            let result = f64_to_i64_safe(f64::NAN);
            assert!(matches!(result, Err(EngineError::ArithmeticOverflow(_))));
            if let Err(EngineError::ArithmeticOverflow(msg)) = result {
                assert!(msg.contains("NaN"));
            }
        }

        #[test]
        fn test_infinity_detection_in_f64_to_i64() {
            // Test that infinity is properly detected
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
            // Test values beyond i64 range
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
            // Test valid conversions work correctly
            assert_eq!(f64_to_i64_safe(42.0).unwrap(), 42);
            assert_eq!(f64_to_i64_safe(-42.0).unwrap(), -42);
            assert_eq!(f64_to_i64_safe(0.0).unwrap(), 0);
            assert_eq!(f64_to_i64_safe(0.9).unwrap(), 0); // truncates
            assert_eq!(f64_to_i64_safe(-0.9).unwrap(), 0); // truncates towards zero
        }

        #[test]
        fn test_f64_to_i64_rejects_i64_max_as_f64() {
            // i64::MAX as f64 rounds up to 9223372036854775808.0 (i64::MAX + 1).
            // `f as i64` would saturate to i64::MAX, which is semantically wrong.
            let result = f64_to_i64_safe(i64::MAX as f64);
            assert!(
                matches!(result, Err(EngineError::ArithmeticOverflow(_))),
                "Expected ArithmeticOverflow for i64::MAX as f64, got: {:?}",
                result
            );
        }

        #[test]
        fn test_nan_equality() {
            // NaN == NaN should be true in our implementation (unlike IEEE 754)
            let nan1 = Value::Float(f64::NAN);
            let nan2 = Value::Float(f64::NAN);
            assert!(
                values_equal(&nan1, &nan2),
                "Two NaN values should be considered equal"
            );

            // NaN != any integer
            let nan = Value::Float(f64::NAN);
            let int = Value::Int(42);
            assert!(
                !values_equal(&nan, &int),
                "NaN should not equal any integer"
            );
            assert!(
                !values_equal(&int, &nan),
                "Any integer should not equal NaN"
            );

            // NaN != regular float
            let nan = Value::Float(f64::NAN);
            let float = Value::Float(42.0);
            assert!(
                !values_equal(&nan, &float),
                "NaN should not equal regular float"
            );
        }

        #[test]
        fn test_large_integer_precision_error() {
            // Integers beyond 2^53 should error when converted to f64
            let large_int = Value::Int(MAX_SAFE_INTEGER + 1);
            let result = to_number(&large_int);
            assert!(
                matches!(result, Err(EngineError::ArithmeticOverflow(_))),
                "Large integer should cause overflow error, got: {:?}",
                result
            );

            let small_int = Value::Int(MIN_SAFE_INTEGER - 1);
            let result = to_number(&small_int);
            assert!(
                matches!(result, Err(EngineError::ArithmeticOverflow(_))),
                "Small integer should cause overflow error, got: {:?}",
                result
            );

            // Within safe range should work
            let safe_int = Value::Int(MAX_SAFE_INTEGER);
            let result = to_number(&safe_int);
            assert!(result.is_ok(), "Safe integer should convert successfully");
            assert_eq!(result.unwrap(), MAX_SAFE_INTEGER as f64);

            let safe_neg = Value::Int(MIN_SAFE_INTEGER);
            let result = to_number(&safe_neg);
            assert!(result.is_ok(), "Safe negative should convert successfully");
            assert_eq!(result.unwrap(), MIN_SAFE_INTEGER as f64);
        }

        #[test]
        fn test_arithmetic_with_large_integer() {
            // Addition with integers beyond safe range (>2^53) should fail
            let large_value: i64 = 9_007_199_254_740_993; // MAX_SAFE_INTEGER + 1

            let resolver = TestResolver::new();
            let op = ActionOperation {
                operation: Operation::Add,
                subject: None,
                value: None,
                values: Some(vec![lit(large_value), lit(1i64)]),
                when: None,
                then: None,
                else_branch: None,
                conditions: None,
                cases: None,
                default: None,
                unit: None,
            };

            let result = execute_operation(&op, &resolver, 0);
            assert!(
                matches!(result, Err(EngineError::ArithmeticOverflow(_))),
                "Large integer in arithmetic should cause overflow error, got: {:?}",
                result
            );
        }
    }

    // -------------------------------------------------------------------------
    // Null Checking Operations Tests
    // -------------------------------------------------------------------------

    mod null_checking {
        use super::*;

        #[test]
        fn test_is_null_with_null_value() {
            let resolver = TestResolver::new().with_var("nullable", Value::Null);
            let op = ActionOperation {
                operation: Operation::IsNull,
                subject: Some(var("nullable")),
                value: None,
                values: None,
                when: None,
                then: None,
                else_branch: None,
                conditions: None,
                cases: None,
                default: None,
                unit: None,
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Bool(true));
        }

        #[test]
        fn test_is_null_with_non_null_value() {
            let resolver = TestResolver::new().with_var("value", 42i64);
            let op = ActionOperation {
                operation: Operation::IsNull,
                subject: Some(var("value")),
                value: None,
                values: None,
                when: None,
                then: None,
                else_branch: None,
                conditions: None,
                cases: None,
                default: None,
                unit: None,
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Bool(false));
        }

        #[test]
        fn test_is_null_with_literal_null() {
            let resolver = TestResolver::new();
            let op = ActionOperation {
                operation: Operation::IsNull,
                subject: Some(ActionValue::Literal(Value::Null)),
                value: None,
                values: None,
                when: None,
                then: None,
                else_branch: None,
                conditions: None,
                cases: None,
                default: None,
                unit: None,
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Bool(true));
        }

        #[test]
        fn test_not_null_with_null_value() {
            let resolver = TestResolver::new().with_var("nullable", Value::Null);
            let op = ActionOperation {
                operation: Operation::NotNull,
                subject: Some(var("nullable")),
                value: None,
                values: None,
                when: None,
                then: None,
                else_branch: None,
                conditions: None,
                cases: None,
                default: None,
                unit: None,
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Bool(false));
        }

        #[test]
        fn test_not_null_with_non_null_value() {
            let resolver = TestResolver::new().with_var("value", "hello");
            let op = ActionOperation {
                operation: Operation::NotNull,
                subject: Some(var("value")),
                value: None,
                values: None,
                when: None,
                then: None,
                else_branch: None,
                conditions: None,
                cases: None,
                default: None,
                unit: None,
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Bool(true));
        }

        #[test]
        fn test_is_null_missing_subject() {
            let resolver = TestResolver::new();
            let op = ActionOperation {
                operation: Operation::IsNull,
                subject: None,
                value: None,
                values: None,
                when: None,
                then: None,
                else_branch: None,
                conditions: None,
                cases: None,
                default: None,
                unit: None,
            };

            let result = execute_operation(&op, &resolver, 0);
            assert!(matches!(result, Err(EngineError::InvalidOperation(_))));
        }
    }

    // -------------------------------------------------------------------------
    // Membership Testing Operations Tests
    // -------------------------------------------------------------------------

    mod membership {
        use super::*;

        #[test]
        fn test_in_found() {
            let resolver = TestResolver::new();
            let op = ActionOperation {
                operation: Operation::In,
                subject: Some(lit(42i64)),
                value: None,
                values: Some(vec![lit(10i64), lit(20i64), lit(42i64), lit(50i64)]),
                when: None,
                then: None,
                else_branch: None,
                conditions: None,
                cases: None,
                default: None,
                unit: None,
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Bool(true));
        }

        #[test]
        fn test_in_not_found() {
            let resolver = TestResolver::new();
            let op = ActionOperation {
                operation: Operation::In,
                subject: Some(lit(99i64)),
                value: None,
                values: Some(vec![lit(10i64), lit(20i64), lit(42i64), lit(50i64)]),
                when: None,
                then: None,
                else_branch: None,
                conditions: None,
                cases: None,
                default: None,
                unit: None,
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Bool(false));
        }

        #[test]
        fn test_in_with_strings() {
            let resolver = TestResolver::new();
            let op = ActionOperation {
                operation: Operation::In,
                subject: Some(lit("apple")),
                value: None,
                values: Some(vec![lit("banana"), lit("apple"), lit("orange")]),
                when: None,
                then: None,
                else_branch: None,
                conditions: None,
                cases: None,
                default: None,
                unit: None,
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Bool(true));
        }

        #[test]
        fn test_in_with_mixed_int_float() {
            // 42 should be found in [10, 42.0, 50] due to numeric coercion
            let resolver = TestResolver::new();
            let op = ActionOperation {
                operation: Operation::In,
                subject: Some(lit(42i64)),
                value: None,
                values: Some(vec![lit(10i64), lit(42.0f64), lit(50i64)]),
                when: None,
                then: None,
                else_branch: None,
                conditions: None,
                cases: None,
                default: None,
                unit: None,
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Bool(true));
        }

        #[test]
        fn test_not_in_found() {
            let resolver = TestResolver::new();
            let op = ActionOperation {
                operation: Operation::NotIn,
                subject: Some(lit(42i64)),
                value: None,
                values: Some(vec![lit(10i64), lit(20i64), lit(42i64), lit(50i64)]),
                when: None,
                then: None,
                else_branch: None,
                conditions: None,
                cases: None,
                default: None,
                unit: None,
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Bool(false));
        }

        #[test]
        fn test_not_in_not_found() {
            let resolver = TestResolver::new();
            let op = ActionOperation {
                operation: Operation::NotIn,
                subject: Some(lit(99i64)),
                value: None,
                values: Some(vec![lit(10i64), lit(20i64), lit(42i64), lit(50i64)]),
                when: None,
                then: None,
                else_branch: None,
                conditions: None,
                cases: None,
                default: None,
                unit: None,
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Bool(true));
        }

        #[test]
        fn test_in_with_variables() {
            let resolver = TestResolver::new().with_var("status", "active").with_var(
                "valid_statuses",
                Value::Array(vec![
                    Value::String("active".to_string()),
                    Value::String("pending".to_string()),
                ]),
            );

            let op = ActionOperation {
                operation: Operation::In,
                subject: Some(var("status")),
                value: None,
                values: Some(vec![lit("active"), lit("pending"), lit("inactive")]),
                when: None,
                then: None,
                else_branch: None,
                conditions: None,
                cases: None,
                default: None,
                unit: None,
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Bool(true));
        }

        #[test]
        fn test_in_missing_subject() {
            let resolver = TestResolver::new();
            let op = ActionOperation {
                operation: Operation::In,
                subject: None,
                value: None,
                values: Some(vec![lit(1i64), lit(2i64)]),
                when: None,
                then: None,
                else_branch: None,
                conditions: None,
                cases: None,
                default: None,
                unit: None,
            };

            let result = execute_operation(&op, &resolver, 0);
            assert!(matches!(result, Err(EngineError::InvalidOperation(_))));
        }

        #[test]
        fn test_in_missing_values() {
            let resolver = TestResolver::new();
            let op = ActionOperation {
                operation: Operation::In,
                subject: Some(lit(42i64)),
                value: None,
                values: None,
                when: None,
                then: None,
                else_branch: None,
                conditions: None,
                cases: None,
                default: None,
                unit: None,
            };

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
        fn test_subtract_date_days_positive() {
            let resolver = TestResolver::new();
            let op = ActionOperation {
                operation: Operation::SubtractDate,
                subject: Some(lit("2025-01-15")),
                value: Some(lit("2025-01-10")),
                values: None,
                when: None,
                then: None,
                else_branch: None,
                conditions: None,
                cases: None,
                default: None,
                unit: Some("days".to_string()),
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Int(5));
        }

        #[test]
        fn test_subtract_date_days_negative() {
            let resolver = TestResolver::new();
            let op = ActionOperation {
                operation: Operation::SubtractDate,
                subject: Some(lit("2025-01-10")),
                value: Some(lit("2025-01-15")),
                values: None,
                when: None,
                then: None,
                else_branch: None,
                conditions: None,
                cases: None,
                default: None,
                unit: Some("days".to_string()),
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Int(-5));
        }

        #[test]
        fn test_subtract_date_days_default_unit() {
            // When unit is not specified, should default to days
            let resolver = TestResolver::new();
            let op = ActionOperation {
                operation: Operation::SubtractDate,
                subject: Some(lit("2025-01-20")),
                value: Some(lit("2025-01-01")),
                values: None,
                when: None,
                then: None,
                else_branch: None,
                conditions: None,
                cases: None,
                default: None,
                unit: None,
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Int(19));
        }

        #[test]
        fn test_subtract_date_months_same_day() {
            let resolver = TestResolver::new();
            let op = ActionOperation {
                operation: Operation::SubtractDate,
                subject: Some(lit("2025-05-15")),
                value: Some(lit("2025-01-15")),
                values: None,
                when: None,
                then: None,
                else_branch: None,
                conditions: None,
                cases: None,
                default: None,
                unit: Some("months".to_string()),
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Int(4));
        }

        #[test]
        fn test_subtract_date_months_incomplete() {
            // From Jan 15 to May 14 = 3 complete months (not 4)
            let resolver = TestResolver::new();
            let op = ActionOperation {
                operation: Operation::SubtractDate,
                subject: Some(lit("2025-05-14")),
                value: Some(lit("2025-01-15")),
                values: None,
                when: None,
                then: None,
                else_branch: None,
                conditions: None,
                cases: None,
                default: None,
                unit: Some("months".to_string()),
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Int(3));
        }

        #[test]
        fn test_subtract_date_months_end_of_month() {
            let resolver = TestResolver::new();

            // Jan 31 → Feb 28: should be 1 month (Feb 28 is last day of Feb)
            let op = ActionOperation {
                operation: Operation::SubtractDate,
                subject: Some(lit("2025-02-28")),
                value: Some(lit("2025-01-31")),
                values: None,
                when: None,
                then: None,
                else_branch: None,
                conditions: None,
                cases: None,
                default: None,
                unit: Some("months".to_string()),
            };
            assert_eq!(execute_operation(&op, &resolver, 0).unwrap(), Value::Int(1));

            // Jan 29 → Feb 28: should be 1 month (28 < 29, but 28 is max for Feb)
            let op = ActionOperation {
                operation: Operation::SubtractDate,
                subject: Some(lit("2025-02-28")),
                value: Some(lit("2025-01-29")),
                values: None,
                when: None,
                then: None,
                else_branch: None,
                conditions: None,
                cases: None,
                default: None,
                unit: Some("months".to_string()),
            };
            assert_eq!(execute_operation(&op, &resolver, 0).unwrap(), Value::Int(1));

            // Jan 30 → Feb 28: should be 1 month
            let op = ActionOperation {
                operation: Operation::SubtractDate,
                subject: Some(lit("2025-02-28")),
                value: Some(lit("2025-01-30")),
                values: None,
                when: None,
                then: None,
                else_branch: None,
                conditions: None,
                cases: None,
                default: None,
                unit: Some("months".to_string()),
            };
            assert_eq!(execute_operation(&op, &resolver, 0).unwrap(), Value::Int(1));

            // Jan 31 → Feb 15: should be 0 months (haven't reached day 31 or end of Feb)
            let op = ActionOperation {
                operation: Operation::SubtractDate,
                subject: Some(lit("2025-02-15")),
                value: Some(lit("2025-01-31")),
                values: None,
                when: None,
                then: None,
                else_branch: None,
                conditions: None,
                cases: None,
                default: None,
                unit: Some("months".to_string()),
            };
            assert_eq!(execute_operation(&op, &resolver, 0).unwrap(), Value::Int(0));

            // Mar 31 → Apr 30: should be 1 month (30 is last day of Apr)
            let op = ActionOperation {
                operation: Operation::SubtractDate,
                subject: Some(lit("2025-04-30")),
                value: Some(lit("2025-03-31")),
                values: None,
                when: None,
                then: None,
                else_branch: None,
                conditions: None,
                cases: None,
                default: None,
                unit: Some("months".to_string()),
            };
            assert_eq!(execute_operation(&op, &resolver, 0).unwrap(), Value::Int(1));

            // Leap year: Jan 31 → Feb 29: should be 1 month
            let op = ActionOperation {
                operation: Operation::SubtractDate,
                subject: Some(lit("2024-02-29")),
                value: Some(lit("2024-01-31")),
                values: None,
                when: None,
                then: None,
                else_branch: None,
                conditions: None,
                cases: None,
                default: None,
                unit: Some("months".to_string()),
            };
            assert_eq!(execute_operation(&op, &resolver, 0).unwrap(), Value::Int(1));
        }

        #[test]
        fn test_subtract_date_years() {
            let resolver = TestResolver::new();
            let op = ActionOperation {
                operation: Operation::SubtractDate,
                subject: Some(lit("2025-06-15")),
                value: Some(lit("2020-06-15")),
                values: None,
                when: None,
                then: None,
                else_branch: None,
                conditions: None,
                cases: None,
                default: None,
                unit: Some("years".to_string()),
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Int(5));
        }

        #[test]
        fn test_subtract_date_years_incomplete() {
            // From 2020-06-15 to 2025-06-14 = 4 complete years (not 5)
            let resolver = TestResolver::new();
            let op = ActionOperation {
                operation: Operation::SubtractDate,
                subject: Some(lit("2025-06-14")),
                value: Some(lit("2020-06-15")),
                values: None,
                when: None,
                then: None,
                else_branch: None,
                conditions: None,
                cases: None,
                default: None,
                unit: Some("years".to_string()),
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Int(4));
        }

        #[test]
        fn test_subtract_date_years_negative() {
            let resolver = TestResolver::new();
            let op = ActionOperation {
                operation: Operation::SubtractDate,
                subject: Some(lit("2020-01-01")),
                value: Some(lit("2025-01-01")),
                values: None,
                when: None,
                then: None,
                else_branch: None,
                conditions: None,
                cases: None,
                default: None,
                unit: Some("years".to_string()),
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Int(-5));
        }

        #[test]
        fn test_subtract_date_with_variables() {
            let resolver = TestResolver::new()
                .with_var("birth_date", "1990-03-15")
                .with_var("reference_date", "2025-03-15");

            let op = ActionOperation {
                operation: Operation::SubtractDate,
                subject: Some(var("reference_date")),
                value: Some(var("birth_date")),
                values: None,
                when: None,
                then: None,
                else_branch: None,
                conditions: None,
                cases: None,
                default: None,
                unit: Some("years".to_string()),
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Int(35));
        }

        #[test]
        fn test_subtract_date_invalid_format() {
            let resolver = TestResolver::new();
            let op = ActionOperation {
                operation: Operation::SubtractDate,
                subject: Some(lit("15-01-2025")), // Wrong format
                value: Some(lit("2025-01-10")),
                values: None,
                when: None,
                then: None,
                else_branch: None,
                conditions: None,
                cases: None,
                default: None,
                unit: Some("days".to_string()),
            };

            let result = execute_operation(&op, &resolver, 0);
            assert!(matches!(result, Err(EngineError::InvalidOperation(_))));
        }

        #[test]
        fn test_subtract_date_invalid_unit() {
            let resolver = TestResolver::new();
            let op = ActionOperation {
                operation: Operation::SubtractDate,
                subject: Some(lit("2025-01-15")),
                value: Some(lit("2025-01-10")),
                values: None,
                when: None,
                then: None,
                else_branch: None,
                conditions: None,
                cases: None,
                default: None,
                unit: Some("weeks".to_string()), // Unsupported unit
            };

            let result = execute_operation(&op, &resolver, 0);
            assert!(matches!(result, Err(EngineError::InvalidOperation(_))));
        }

        #[test]
        fn test_subtract_date_missing_subject() {
            let resolver = TestResolver::new();
            let op = ActionOperation {
                operation: Operation::SubtractDate,
                subject: None,
                value: Some(lit("2025-01-10")),
                values: None,
                when: None,
                then: None,
                else_branch: None,
                conditions: None,
                cases: None,
                default: None,
                unit: Some("days".to_string()),
            };

            let result = execute_operation(&op, &resolver, 0);
            assert!(matches!(result, Err(EngineError::InvalidOperation(_))));
        }

        #[test]
        fn test_subtract_date_non_string_value() {
            let resolver = TestResolver::new();
            let op = ActionOperation {
                operation: Operation::SubtractDate,
                subject: Some(lit(12345i64)), // Not a date string
                value: Some(lit("2025-01-10")),
                values: None,
                when: None,
                then: None,
                else_branch: None,
                conditions: None,
                cases: None,
                default: None,
                unit: Some("days".to_string()),
            };

            let result = execute_operation(&op, &resolver, 0);
            assert!(matches!(result, Err(EngineError::TypeMismatch { .. })));
        }

        #[test]
        fn test_subtract_date_leap_year() {
            // Test Feb 29 on a leap year
            let resolver = TestResolver::new();
            let op = ActionOperation {
                operation: Operation::SubtractDate,
                subject: Some(lit("2024-03-01")),
                value: Some(lit("2024-02-28")),
                values: None,
                when: None,
                then: None,
                else_branch: None,
                conditions: None,
                cases: None,
                default: None,
                unit: Some("days".to_string()),
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Int(2)); // Feb 29 exists in 2024
        }

        #[test]
        fn test_subtract_date_same_date() {
            let resolver = TestResolver::new();
            let op = ActionOperation {
                operation: Operation::SubtractDate,
                subject: Some(lit("2025-01-15")),
                value: Some(lit("2025-01-15")),
                values: None,
                when: None,
                then: None,
                else_branch: None,
                conditions: None,
                cases: None,
                default: None,
                unit: Some("days".to_string()),
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Int(0));
        }

        #[test]
        fn test_subtract_date_years_feb29_birthday_on_feb28() {
            // Born Feb 29 (leap year). On Feb 28 of a non-leap year, the person
            // should have turned their age (per Dutch law: BW art. 1:2).
            let resolver = TestResolver::new();
            let op = ActionOperation {
                operation: Operation::SubtractDate,
                subject: Some(lit("2001-02-28")),
                value: Some(lit("2000-02-29")),
                values: None,
                when: None,
                then: None,
                else_branch: None,
                conditions: None,
                cases: None,
                default: None,
                unit: Some("years".to_string()),
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Int(1));
        }

        #[test]
        fn test_subtract_date_years_feb29_birthday_before_feb28() {
            // Born Feb 29. On Feb 27 of a non-leap year, the birthday hasn't
            // happened yet — should be 0 years.
            let resolver = TestResolver::new();
            let op = ActionOperation {
                operation: Operation::SubtractDate,
                subject: Some(lit("2001-02-27")),
                value: Some(lit("2000-02-29")),
                values: None,
                when: None,
                then: None,
                else_branch: None,
                conditions: None,
                cases: None,
                default: None,
                unit: Some("years".to_string()),
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Int(0));
        }

        #[test]
        fn test_subtract_date_years_feb29_birthday_on_leap_year() {
            // Born Feb 29. On Feb 29 of the next leap year, exactly 4 years.
            let resolver = TestResolver::new();
            let op = ActionOperation {
                operation: Operation::SubtractDate,
                subject: Some(lit("2004-02-29")),
                value: Some(lit("2000-02-29")),
                values: None,
                when: None,
                then: None,
                else_branch: None,
                conditions: None,
                cases: None,
                default: None,
                unit: Some("years".to_string()),
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Int(4));
        }
    }

    // -------------------------------------------------------------------------
    // parse_date Tests (Object input)
    // -------------------------------------------------------------------------

    #[test]
    fn test_subtract_date_with_object_subject() {
        // Simulates $referencedate resolving to {iso: "2025-01-01", year: 2025, month: 1, day: 1}
        let mut date_obj = HashMap::new();
        date_obj.insert("iso".to_string(), Value::String("2025-01-01".to_string()));
        date_obj.insert("year".to_string(), Value::Int(2025));
        date_obj.insert("month".to_string(), Value::Int(1));
        date_obj.insert("day".to_string(), Value::Int(1));

        let resolver = TestResolver::new()
            .with_var("referencedate", Value::Object(date_obj))
            .with_var("geboortedatum", Value::String("2005-01-01".to_string()));

        let op = ActionOperation {
            operation: Operation::SubtractDate,
            subject: Some(var("referencedate")),
            value: Some(var("geboortedatum")),
            values: None,
            when: None,
            then: None,
            else_branch: None,
            conditions: None,
            cases: None,
            default: None,
            unit: Some("years".to_string()),
        };

        let result = execute_operation(&op, &resolver, 0).unwrap();
        assert_eq!(result, Value::Int(20));
    }

    #[test]
    fn test_subtract_date_object_without_iso_field() {
        let mut date_obj = HashMap::new();
        date_obj.insert("year".to_string(), Value::Int(2025));

        let resolver = TestResolver::new().with_var("bad_date", Value::Object(date_obj));

        let op = ActionOperation {
            operation: Operation::SubtractDate,
            subject: Some(var("bad_date")),
            value: Some(lit("2020-01-01")),
            values: None,
            when: None,
            then: None,
            else_branch: None,
            conditions: None,
            cases: None,
            default: None,
            unit: Some("years".to_string()),
        };

        let result = execute_operation(&op, &resolver, 0);
        assert!(matches!(result, Err(EngineError::TypeMismatch { .. })));
    }

    // -------------------------------------------------------------------------
    // values_equal Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_values_equal_precision_guard() {
        // Integers beyond ±2^53 can't be exactly represented as f64
        let large_int = MAX_SAFE_INTEGER + 1; // 2^53 + 1
        let large_neg = MIN_SAFE_INTEGER - 1; // -(2^53) - 1

        // These should return false because the integer can't be exactly represented
        assert!(!values_equal(
            &Value::Int(large_int),
            &Value::Float(large_int as f64)
        ));
        assert!(!values_equal(
            &Value::Float(large_neg as f64),
            &Value::Int(large_neg)
        ));

        // Integers within safe range should still work
        assert!(values_equal(&Value::Int(42), &Value::Float(42.0)));
        assert!(values_equal(&Value::Float(42.0), &Value::Int(42)));
        assert!(values_equal(
            &Value::Int(MAX_SAFE_INTEGER),
            &Value::Float(MAX_SAFE_INTEGER as f64)
        ));

        // NaN handling
        assert!(values_equal(
            &Value::Float(f64::NAN),
            &Value::Float(f64::NAN)
        ));
        assert!(!values_equal(&Value::Int(0), &Value::Float(f64::NAN)));
        assert!(!values_equal(&Value::Float(f64::NAN), &Value::Int(0)));
    }
}
