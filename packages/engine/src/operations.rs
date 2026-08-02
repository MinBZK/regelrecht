//! Operation execution for the RegelRecht engine
//!
//! Implements the execution logic for all operation types in the regulation
//! schema, plus engine-only operations retained for backward compatibility.
//!
//! **Schema operations:**
//! - **Comparison:** EQUALS, GREATER_THAN, LESS_THAN, GREATER_THAN_OR_EQUAL, LESS_THAN_OR_EQUAL
//! - **Arithmetic:** ADD, SUBTRACT, MULTIPLY, DIVIDE
//! - **Aggregate:** MAX, MIN
//! - **Logical:** AND, OR, NOT
//! - **Conditional:** IF (multi-case with cases/default)
//! - **Collection:** IN, LIST
//! - **Date:** AGE, DATE_ADD, DATE, DAY_OF_WEEK, DATE_DIFF, DATE_PART, START_OF
//!
//! **Engine-only (not in schema, accepted for backward compatibility):**
//! NOT_EQUALS, IS_NULL, NOT_NULL, NOT_IN

use crate::article::{ActionOperation, ActionValue, Case};
use crate::error::{EngineError, Result};
use crate::types::{PathNodeType, Value};
use chrono::{Datelike, NaiveDate};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::{Decimal, MathematicalOps, RoundingStrategy};
use std::cmp::Ordering;

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
        Value::Decimal(d) => format!("{}", d),
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
            execute_ordered_comparison(subject, value, resolver, depth, |ord| {
                ord == Ordering::Greater
            })
        }
        ActionOperation::LessThan { subject, value } => {
            execute_ordered_comparison(subject, value, resolver, depth, |ord| ord == Ordering::Less)
        }
        ActionOperation::GreaterThanOrEqual { subject, value } => {
            execute_ordered_comparison(subject, value, resolver, depth, |ord| ord != Ordering::Less)
        }
        ActionOperation::LessThanOrEqual { subject, value } => {
            execute_ordered_comparison(subject, value, resolver, depth, |ord| {
                ord != Ordering::Greater
            })
        }

        // Arithmetic
        ActionOperation::Add { values } => execute_add(values, resolver, depth),
        ActionOperation::Subtract { values } => execute_subtract(values, resolver, depth),
        ActionOperation::Multiply { values } => execute_multiply(values, resolver, depth),
        ActionOperation::Divide { values } => execute_divide(values, resolver, depth),

        // Aggregate
        ActionOperation::Max { values } => execute_aggregate(values, resolver, depth, Decimal::max),
        ActionOperation::Min { values } => execute_aggregate(values, resolver, depth, Decimal::min),

        // Rounding (RFC-024)
        ActionOperation::Round { value, precision } => {
            execute_rounding(value, *precision, RoundMode::Round, resolver, depth)
        }
        ActionOperation::Ceil { value, precision } => {
            execute_rounding(value, *precision, RoundMode::Ceil, resolver, depth)
        }
        ActionOperation::Floor { value, precision } => {
            execute_rounding(value, *precision, RoundMode::Floor, resolver, depth)
        }

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
        ActionOperation::DateDiff { from, to, unit } => {
            execute_date_diff(from, to, unit, resolver, depth)
        }
        ActionOperation::DatePart { date, part } => execute_date_part(date, part, resolver, depth),
        ActionOperation::StartOf { date, unit } => execute_start_of(date, unit, resolver, depth),
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
/// Numbers compare exactly: `Decimal` carries full precision and `Int`/`Decimal`
/// are compared as decimals (e.g. `42 == 42.0`), with no float rounding or
/// ±2^53 precision loss.
pub(crate) fn values_equal(a: &Value, b: &Value) -> bool {
    match (a, b) {
        // Untranslatable: two untranslatables are equal, mixed is never equal
        (Value::Untranslatable { .. }, Value::Untranslatable { .. }) => true,
        (Value::Untranslatable { .. }, _) | (_, Value::Untranslatable { .. }) => false,
        // Decimal-Decimal comparison
        (Value::Decimal(d1), Value::Decimal(d2)) => d1 == d2,
        // Int-Decimal comparison (numeric, exact)
        (Value::Int(i), Value::Decimal(d)) | (Value::Decimal(d), Value::Int(i)) => {
            Decimal::from(*i) == *d
        }
        // Default: use structural equality
        _ => a == b,
    }
}

/// Execute EQUALS / NOT_EQUALS with Python-style numeric coercion.
///
/// - `Int(42) == Float(42.0)` returns `true` (like Python)
/// - The mixed date forms compare chronologically: the string form `"2025-01-01"`
///   equals the reference-date object form `{iso: "2025-01-01", ...}` (RFC-021).
///   This keeps EQUALS consistent with the ordering operators, which also accept
///   both date forms via `parse_date`. Two strings or two objects keep structural
///   equality, so objects that merely share an `iso` field are not equal.
/// - Other non-numeric types use structural equality
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

    let mut equal = values_equal(&subject_val, &value_val);

    // Date-aware fallback, scoped to the mixed form (a string against an `{iso}`
    // object): the one date pairing structural equality can never match. Two
    // canonical date strings are equal structurally exactly when they are equal
    // chronologically, and object↔object stays structural so arbitrary objects
    // sharing an `iso` field do not become equal (RFC-021). A side that fails to
    // parse as a date leaves the structural verdict untouched. Scoped to EQUALS so
    // membership and other structural comparisons keep their existing semantics.
    if !equal && is_mixed_date_pair(&subject_val, &value_val) {
        if let (Ok(subject_date), Ok(value_date)) =
            (parse_date(&subject_val), parse_date(&value_val))
        {
            equal = subject_date == value_date;
        }
    }

    Ok(Value::Bool(if negate { !equal } else { equal }))
}

/// Whether the pair is the mixed date form: one string against one
/// reference-date `{iso}` object, in either order.
///
/// Used to scope date-aware equality to the single pairing that structural
/// equality cannot already handle.
fn is_mixed_date_pair(a: &Value, b: &Value) -> bool {
    fn iso_object(v: &Value) -> bool {
        matches!(v, Value::Object(obj) if matches!(obj.get("iso"), Some(Value::String(_))))
    }
    (matches!(a, Value::String(_)) && iso_object(b))
        || (iso_object(a) && matches!(b, Value::String(_)))
}

/// Execute an ordered comparison (>, <, >=, <=) on numbers or dates.
///
/// Operands are type-safe and dispatch on their resolved type:
/// - both numeric (Int/Decimal) → exact decimal comparison
/// - both ISO dates (YYYY-MM-DD strings, or `{iso, ...}` objects) → date comparison
/// - mixed or unsupported types → `TypeMismatch` error
///
/// This lets `$peildatum > $grensdatum` work the same way `$bedrag > 1000` does,
/// without introducing date-specific operators (RFC-021, route A).
///
/// The operator is a single predicate over [`Ordering`], shared by the numeric
/// and the date path so the two can never drift apart. `Decimal` is totally
/// ordered, so every numeric pair compares.
fn execute_ordered_comparison<R: ValueResolver, F>(
    subject: &ActionValue,
    value: &ActionValue,
    resolver: &R,
    depth: usize,
    satisfies: F,
) -> Result<Value>
where
    F: Fn(Ordering) -> bool,
{
    let subject_val = evaluate_value(subject, resolver, depth)?;
    let value_val = evaluate_value(value, resolver, depth)?;

    if let Some(tainted) = propagate_binary(&subject_val, &value_val) {
        return Ok(tainted);
    }

    // Numbers first: the common case and the historical behavior.
    if is_numeric(&subject_val) && is_numeric(&value_val) {
        let subject_num = to_decimal(&subject_val)?;
        let value_num = to_decimal(&value_val)?;
        return Ok(Value::Bool(satisfies(subject_num.cmp(&value_num))));
    }

    // Dates: only when BOTH operands parse as ISO dates, so a mismatch
    // (e.g. number vs date) surfaces as a clear error rather than a silent coercion.
    let subject_date = parse_date(&subject_val);
    let value_date = parse_date(&value_val);
    if let (Ok(subject_date), Ok(value_date)) = (&subject_date, &value_date) {
        return Ok(Value::Bool(satisfies(subject_date.cmp(value_date))));
    }

    // Neither a numeric pair nor a date pair. Point the error at what actually
    // went wrong: a number against a date is a mixed pair (each operand is fine
    // on its own), otherwise blame the operand that is neither number nor date.
    let subject_kind = comparable_kind(&subject_val, subject_date.is_ok());
    let value_kind = comparable_kind(&value_val, value_date.is_ok());
    match (subject_kind, value_kind) {
        (Some(subject_kind), Some(value_kind)) => Err(EngineError::TypeMismatch {
            expected: "two numbers or two dates".to_string(),
            actual: format!("a {subject_kind} compared against a {value_kind}"),
        }),
        (Some(_), None) => Err(type_error("number or date", &value_val)),
        _ => Err(type_error("number or date", &subject_val)),
    }
}

/// The comparable kind of an operand, for error reporting: `number`, `date`,
/// or `None` when it is neither.
fn comparable_kind(val: &Value, parses_as_date: bool) -> Option<&'static str> {
    if is_numeric(val) {
        Some("number")
    } else if parses_as_date {
        Some("date")
    } else {
        None
    }
}

/// Whether a value is a number (Int or Decimal) for comparison dispatch.
fn is_numeric(val: &Value) -> bool {
    matches!(val, Value::Int(_) | Value::Decimal(_))
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
        Value::Int(_) | Value::Decimal(_) => {
            // Original numeric addition
            let mut sum = Decimal::ZERO;
            let mut has_decimal = false;
            for val in &evaluated {
                match val {
                    Value::Int(i) => sum += Decimal::from(*i),
                    Value::Decimal(d) => {
                        sum += d;
                        has_decimal = true;
                    }
                    _ => return Err(type_error("number", val)),
                }
            }
            int_or_decimal_result(sum, has_decimal)
        }
        _ => Err(type_error("number, string, or array", &evaluated[0])),
    }
}

/// Wrap a numeric arithmetic result so all-int inputs return `Int` and any
/// `Decimal` input promotes the result to `Decimal`. Centralises the
/// `decimal_to_i64_safe` overflow guard shared by ADD/SUBTRACT/MULTIPLY.
fn int_or_decimal_result(result: Decimal, has_decimal: bool) -> Result<Value> {
    Ok(if has_decimal {
        Value::Decimal(result)
    } else {
        Value::Int(decimal_to_i64_safe(result)?)
    })
}

/// Execute SUBTRACT operation: first value minus all subsequent values.
///
/// Computed in exact decimal arithmetic; all-integer inputs return `Int`.
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

    // SAFETY: values guaranteed non-empty by check above
    let Some((first, rest)) = evaluated.split_first() else {
        unreachable!("values checked non-empty above")
    };
    let mut result = to_decimal(first)?;
    let mut has_decimal = matches!(first, Value::Decimal(_));

    for val in rest {
        result -= to_decimal(val)?;
        if matches!(val, Value::Decimal(_)) {
            has_decimal = true;
        }
    }

    int_or_decimal_result(result, has_decimal)
}

/// Execute MULTIPLY operation: product of all values.
///
/// Computed in exact decimal arithmetic; all-integer inputs return `Int`.
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

    let mut result = Decimal::ONE;
    let mut has_decimal = false;

    for val in &evaluated {
        match val {
            Value::Int(i) => result *= Decimal::from(*i),
            Value::Decimal(d) => {
                result *= d;
                has_decimal = true;
            }
            _ => return Err(type_error("number", val)),
        }
    }

    int_or_decimal_result(result, has_decimal)
}

/// Execute DIVIDE operation: first value divided by all subsequent values.
///
/// Returns `Err(DivisionByZero)` for division by zero. Division is computed in
/// exact decimal arithmetic and always returns a `Decimal` (like Python's `/`).
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

    // SAFETY: values guaranteed non-empty by check above
    let Some((first, rest)) = evaluated.split_first() else {
        unreachable!("values checked non-empty above")
    };
    let mut result = to_decimal(first)?;

    for val in rest {
        let divisor = to_decimal(val)?;
        if divisor.is_zero() {
            return Err(EngineError::DivisionByZero);
        }
        result = result
            .checked_div(divisor)
            .ok_or_else(|| EngineError::ArithmeticOverflow("Division overflowed".to_string()))?;
    }

    // Division always returns a decimal (like Python's `/`)
    Ok(Value::Decimal(result))
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
    F: Fn(Decimal, Decimal) -> Decimal,
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

    let mut has_decimal = false;
    let nums: Vec<Decimal> = evaluated
        .iter()
        .map(|v| {
            if matches!(v, Value::Decimal(_)) {
                has_decimal = true;
            }
            to_decimal(v)
        })
        .collect::<Result<Vec<_>>>()?;

    // SAFETY: values guaranteed non-empty by check above
    let Some(result) = nums.into_iter().reduce(combine) else {
        unreachable!("values checked non-empty above")
    };

    int_or_decimal_result(result, has_decimal)
}

// =============================================================================
// Rounding Operations (RFC-024)
// =============================================================================

/// Rounding direction.
#[derive(Clone, Copy)]
enum RoundMode {
    /// Round to nearest, ties away from zero (rekenkundig; the Hoge Raad default).
    Round,
    /// Round toward +∞ ("naar boven").
    Ceil,
    /// Round toward -∞ ("naar beneden" / "afkapping").
    Floor,
}

/// Maximum absolute `precision` a rounding operation accepts (Decimal holds at
/// most 28 fractional digits, so larger magnitudes are out of range).
const MAX_ROUND_PRECISION: i64 = 28;

/// Execute a rounding operation (ROUND/CEIL/FLOOR): round the single
/// operand to `precision` decimal places.
///
/// `precision` is in the value's own unit (RFC-023): `0` rounds to whole units,
/// `2` to two decimals, and a negative precision rounds to tens/hundreds (e.g.
/// `-2` rounds a eurocent value to whole euros). Rounding never happens
/// implicitly — only here, where the law asks for it (RFC-024).
///
/// `ROUND` is half-up in the legal sense (rekenkundig — ties away from zero, the
/// Hoge Raad 2009 default). The money domain is non-negative, where "half away
/// from zero" and "half toward +∞" coincide.
fn execute_rounding<R: ValueResolver>(
    value: &ActionValue,
    precision: i64,
    mode: RoundMode,
    resolver: &R,
    depth: usize,
) -> Result<Value> {
    let evaluated = evaluate_value(value, resolver, depth)?;
    if evaluated.is_untranslatable() {
        return Ok(evaluated);
    }
    let operand = to_decimal(&evaluated)?;
    let rounded = round_decimal(operand, precision, mode)?;
    // Mirror the arithmetic ops: an integral result returns `Int`, otherwise `Decimal`.
    Ok(match decimal_to_i64_safe(rounded) {
        Ok(i) => Value::Int(i),
        Err(_) => Value::Decimal(rounded),
    })
}

/// Round `value` to `precision` decimal places using `mode`.
fn round_decimal(value: Decimal, precision: i64, mode: RoundMode) -> Result<Decimal> {
    let strategy = match mode {
        RoundMode::Round => RoundingStrategy::MidpointAwayFromZero,
        RoundMode::Ceil => RoundingStrategy::ToPositiveInfinity,
        RoundMode::Floor => RoundingStrategy::ToNegativeInfinity,
    };

    if !(-MAX_ROUND_PRECISION..=MAX_ROUND_PRECISION).contains(&precision) {
        return Err(EngineError::InvalidOperation(format!(
            "rounding precision {precision} out of range [-{MAX_ROUND_PRECISION}, {MAX_ROUND_PRECISION}]"
        )));
    }

    if precision >= 0 {
        Ok(value.round_dp_with_strategy(precision as u32, strategy))
    } else {
        // Negative precision: round to a power of ten (e.g. -2 → whole hundreds).
        // Use checked arithmetic so an extreme magnitude errors rather than panics.
        let overflow = || EngineError::ArithmeticOverflow("rounding overflowed".to_string());
        let scale = Decimal::TEN.checked_powi(-precision).ok_or_else(overflow)?;
        let scaled = value
            .checked_div(scale)
            .ok_or_else(overflow)?
            .round_dp_with_strategy(0, strategy);
        scaled.checked_mul(scale).ok_or_else(overflow)
    }
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
        // Definitive false wins over taint (AND commutativity)
        if !val.to_bool() && !val.is_untranslatable() {
            return Ok(Value::Bool(false));
        }
        if val.is_untranslatable() && taint.is_none() {
            taint = Some(val);
            continue;
        }
        if let Some(ref mut r) = results {
            r.push(val);
        }
    }

    // If any operand was tainted but none was definitively false, propagate
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
        // Definitive true wins over taint (OR commutativity)
        if val.to_bool() {
            return Ok(Value::Bool(true));
        }
        if val.is_untranslatable() && taint.is_none() {
            taint = Some(val);
        }
    }

    // If any operand was tainted but none was definitively true, propagate
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

    let dob_date = parse_date(&dob_val)?;
    let ref_date_parsed = parse_date(&ref_val)?;

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
    let mut result_date = parse_date(&date_val)?;

    // Years: add to year component, clamp day to last day of target month
    if let Some(years) = years {
        let years_val = evaluate_value(years, resolver, depth)?;
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
        let months_int = months_val.as_int().ok_or_else(|| {
            EngineError::InvalidOperation("DATE_ADD 'months' must be a number".to_string())
        })?;
        result_date = add_months(result_date, months_int)?;
    }

    // Weeks
    if let Some(weeks) = weeks {
        let weeks_val = evaluate_value(weeks, resolver, depth)?;
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
    let parsed = parse_date(&val)?;
    Ok(Value::Int(parsed.weekday().num_days_from_monday() as i64))
}

/// Execute DATE_DIFF: the signed difference between two dates, expressed in a unit.
///
/// ```yaml
/// operation: DATE_DIFF
/// from: $informatie_datum
/// to: $referencedate.iso
/// in: days        # one of: days | months | years
/// ```
///
/// Returns an `Int`. The result is **positive when `to >= from`** and negative
/// otherwise, so `DATE_DIFF(from, to)` reads as "how far `to` is ahead of `from`".
///
/// - `days` — exact calendar days between the dates.
/// - `months` / `years` — *complete* calendar units, reusing the same arithmetic
///   as `AGE` (BW art. 1:2): e.g. Jan 31 → Feb 28 counts as one whole month.
///
/// The unit is an explicit argument (not inferred) because the difference between
/// two dates is genuinely ambiguous: 400 days is also "1 year" and "13 months".
fn execute_date_diff<R: ValueResolver>(
    from: &ActionValue,
    to: &ActionValue,
    unit: &ActionValue,
    resolver: &R,
    depth: usize,
) -> Result<Value> {
    let from_val = evaluate_value(from, resolver, depth)?;
    let to_val = evaluate_value(to, resolver, depth)?;

    if let Some(tainted) = propagate_binary(&from_val, &to_val) {
        return Ok(tainted);
    }

    let from_date = parse_date(&from_val)?;
    let to_date = parse_date(&to_val)?;

    let unit_val = evaluate_value(unit, resolver, depth)?;
    // The unit participates in taint propagation like the date operands: an
    // Untranslatable unit flows through as a value (RFC-012), not as an error.
    if unit_val.is_untranslatable() {
        return Ok(unit_val);
    }
    let unit_str = match &unit_val {
        Value::String(s) => s.as_str(),
        _ => {
            return Err(type_error(
                "string ('days', 'months', or 'years')",
                &unit_val,
            ))
        }
    };

    // calculate_*_difference are positive when their first arg >= second arg.
    // Passing (to, from) makes the result positive when `to >= from`.
    let diff = match unit_str {
        "days" => (to_date - from_date).num_days(),
        "months" => calculate_months_difference(to_date, from_date),
        "years" => calculate_years_difference(to_date, from_date),
        other => {
            return Err(EngineError::InvalidOperation(format!(
                "DATE_DIFF 'in' must be one of 'days', 'months', 'years', got '{}'",
                other
            )));
        }
    };

    Ok(Value::Int(diff))
}

/// Execute DATE_PART: read one calendar component out of a date.
///
/// ```yaml
/// operation: DATE_PART
/// date: $referencedate.iso
/// in: year        # one of: year | month | day
/// ```
///
/// Returns an `Int`: the year number, the month number (1-12) or the day of the
/// month (1-31). It is the inverse of `DATE`, and `in` ranges over exactly the
/// components `DATE` takes — which is why `month` exists even though no legal
/// text has asked for it yet, and why the weekday does not (that is
/// `DAY_OF_WEEK`, a projection outside the year-month-day decomposition).
///
/// The unit is singular where `DATE_DIFF` is plural, because this selects a
/// component instead of counting units. An author who copies `in: months` from
/// `DATE_DIFF` gets a named error rather than a silent answer.
fn execute_date_part<R: ValueResolver>(
    date_value: &ActionValue,
    part: &str,
    resolver: &R,
    depth: usize,
) -> Result<Value> {
    let val = evaluate_value(date_value, resolver, depth)?;
    if val.is_untranslatable() {
        return Ok(val);
    }
    let parsed = parse_date(&val)?;

    let component = match part {
        "year" => i64::from(parsed.year()),
        "month" => i64::from(parsed.month()),
        "day" => i64::from(parsed.day()),
        other => {
            return Err(EngineError::InvalidOperation(format!(
                "DATE_PART 'in' must be one of 'year', 'month', 'day', got '{}'",
                other
            )));
        }
    };

    Ok(Value::Int(component))
}

/// Execute START_OF: truncate a date down to the start of the calendar unit it
/// falls in.
///
/// ```yaml
/// operation: START_OF
/// date: $peildatum
/// in: month       # one of: year | month
/// ```
///
/// Returns a date string in canonical `%Y-%m-%d` form: 1 January of the year, or
/// the first day of the month. `day` is the identity and `week` would need a
/// convention (ISO Monday) that no legal text imposes, so neither is offered.
///
/// **Order matters for "the first day of the month following".** Truncate first,
/// add afterwards:
///
/// ```yaml
/// operation: DATE_ADD
/// date:
///   operation: START_OF
///   date: $peildatum
///   in: month
/// months: 1
/// ```
///
/// That is exact for every input date, including the first of the month and
/// including 31 January, because the day-clamping of `DATE_ADD` never comes into
/// play: the truncated date is always day 1, which exists in every month. The
/// reverse order happens to give the same answer, but reaches it through the
/// clamp — a step a reviewer has to re-check each time.
///
/// Truncation is an explicit operation and never an implicit property of a date
/// (RFC-024): the cut stands in the law-YAML where the legal text prescribes it,
/// and nowhere else.
fn execute_start_of<R: ValueResolver>(
    date_value: &ActionValue,
    unit: &str,
    resolver: &R,
    depth: usize,
) -> Result<Value> {
    let val = evaluate_value(date_value, resolver, depth)?;
    if val.is_untranslatable() {
        return Ok(val);
    }
    let parsed = parse_date(&val)?;

    let truncated = match unit {
        "year" => NaiveDate::from_ymd_opt(parsed.year(), 1, 1),
        "month" => NaiveDate::from_ymd_opt(parsed.year(), parsed.month(), 1),
        other => {
            return Err(EngineError::InvalidOperation(format!(
                "START_OF 'in' must be one of 'year', 'month', got '{}'",
                other
            )));
        }
    }
    .ok_or_else(|| {
        EngineError::InvalidOperation(format!(
            "START_OF: invalid date after truncating '{}' to {}",
            parsed, unit
        ))
    })?;

    Ok(Value::String(truncated.format("%Y-%m-%d").to_string()))
}

/// Parse a date from a Value.
///
/// Expects the value to be a string in ISO 8601 format (YYYY-MM-DD).
fn parse_date(value: &Value) -> Result<NaiveDate> {
    match value {
        Value::String(s) => parse_iso_date(s),
        // Handle referencedate objects with {iso, year, month, day}
        Value::Object(obj) => {
            if let Some(Value::String(iso)) = obj.get("iso") {
                parse_iso_date(iso)
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

/// Parse a date string in canonical ISO 8601 `YYYY-MM-DD` form.
///
/// chrono's `%Y-%m-%d` parser is lenient and accepts non-zero-padded input like
/// `2025-1-1`. We reject those: a non-canonical literal would compare chronologically
/// under `>`/`<` but inequal under `EQUALS` (which uses string equality), so the two
/// must never disagree. Requiring canonical form keeps every operator consistent and
/// matches the format the engine itself produces for `$referencedate.iso`.
fn parse_iso_date(s: &str) -> Result<NaiveDate> {
    let parsed = NaiveDate::parse_from_str(s, "%Y-%m-%d").map_err(|e| {
        EngineError::InvalidOperation(format!(
            "Failed to parse date '{}': {}. Expected format: YYYY-MM-DD",
            s, e
        ))
    })?;
    // Reject lenient (non-zero-padded) input by round-tripping through the canonical form.
    if parsed.format("%Y-%m-%d").to_string() != s {
        return Err(EngineError::InvalidOperation(format!(
            "Date '{}' is not in canonical YYYY-MM-DD form (use zero-padded components, e.g. '{}')",
            s,
            parsed.format("%Y-%m-%d")
        )));
    }
    Ok(parsed)
}

/// Calculate the difference in complete months between two dates.
///
/// Uses proper calendar arithmetic. A month is counted as complete when
/// the same day-of-month (or end of month if day doesn't exist) is reached.
/// For end-of-month edge cases (e.g., Jan 31 -> Feb 28), if `earlier.day()`
/// exceeds the number of days in `later`'s month, it is capped to the last
/// day of that month so the month is correctly counted as complete.
///
/// Used by DATE_DIFF with `in: months`.
fn calculate_months_difference(date1: NaiveDate, date2: NaiveDate) -> i64 {
    // Het teken als tak in plaats van als vermenigvuldiging: `total * sign` met
    // sign in {1, -1} levert een mutant op die per constructie hetzelfde
    // antwoord geeft (delen door -1 is vermenigvuldigen met -1), en die mutant
    // deelt zijn omschrijving met `years_diff * 12` een paar regels lager. Eén
    // uitsluiting zou daarmee ook echte rekenkunde stilzwijgend afdekken.
    let (earlier, later, negate) = if date1 >= date2 {
        (date2, date1, false)
    } else {
        (date1, date2, true)
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

    let total = total_months as i64;
    if negate {
        -total
    } else {
        total
    }
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
    // Zie calculate_months_difference: het teken staat als tak, niet als
    // vermenigvuldiging, zodat er geen gedragsequivalente mutant ontstaat.
    let (earlier, later, negate) = if date1 >= date2 {
        (date2, date1, false)
    } else {
        (date1, date2, true)
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

    let years = years as i64;
    if negate {
        -years
    } else {
        years
    }
}

// =============================================================================
// Helper Functions
// =============================================================================

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

/// Convert a Value to an exact [`Decimal`].
///
/// Integers widen to `Decimal` losslessly (no ±2^53 limit), so financial/legal
/// calculations carry full precision.
fn to_decimal(val: &Value) -> Result<Decimal> {
    match val {
        Value::Int(i) => Ok(Decimal::from(*i)),
        Value::Decimal(d) => Ok(*d),
        // Untranslatable should be caught by the caller before reaching to_decimal,
        // but handle it gracefully.
        _ => Err(type_error("number", val)),
    }
}

/// Convert an exact [`Decimal`] result to `i64`, erroring if it is non-integral
/// or out of `i64` range. Used to keep all-integer arithmetic results as `Int`.
pub(crate) fn decimal_to_i64_safe(d: Decimal) -> Result<i64> {
    let truncated = d.trunc();
    if truncated != d {
        // Not an overflow — the value simply has a fractional part.
        return Err(EngineError::InvalidOperation(format!(
            "Value {} is not an integer",
            d
        )));
    }
    truncated
        .to_i64()
        .ok_or_else(|| EngineError::ArithmeticOverflow(format!("Value {} exceeds i64 range", d)))
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
    use rust_decimal_macros::dec;
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

    /// Helper to build a reference-date `{iso, year, month, day}` object Value,
    /// matching how the engine injects `$referencedate`.
    fn date_obj(iso: &str) -> Value {
        let date = NaiveDate::parse_from_str(iso, "%Y-%m-%d").expect("valid test date");
        let mut obj = BTreeMap::new();
        obj.insert("iso".to_string(), Value::String(iso.to_string()));
        obj.insert("year".to_string(), Value::Int(i64::from(date.year())));
        obj.insert("month".to_string(), Value::Int(i64::from(date.month())));
        obj.insert("day".to_string(), Value::Int(i64::from(date.day())));
        Value::Object(obj)
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
            assert_eq!(result, Value::Decimal(dec!(60.5)));
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
            assert_eq!(result, Value::Decimal(dec!(50)));
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
            assert_eq!(result, Value::Decimal(dec!(50)));
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

        #[test]
        fn test_add_propagates_an_untranslatable_operand() {
            // RFC-012: a sum in which one term could not be translated has no
            // number as its answer. The taint travels on as a value; it is not a
            // type error and not a silent zero.
            let tainted = Value::Untranslatable {
                article: "12".to_string(),
                construct: "naar redelijkheid".to_string(),
            };
            let resolver = TestResolver::new().with_var("onvertaalbaar", tainted.clone());
            let op = ActionOperation::Add {
                values: vec![lit(100i64), var("onvertaalbaar"), lit(1i64)],
            };

            assert_eq!(execute_operation(&op, &resolver, 0).unwrap(), tainted);
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
            assert_eq!(result, Value::Decimal(dec!(50.3)));
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

        #[test]
        fn test_max_propagates_an_untranslatable_operand() {
            // RFC-012: the highest of a set that contains an untranslatable is
            // itself untranslatable — picking the largest of the rest would
            // silently drop the value that could not be determined.
            let tainted = Value::Untranslatable {
                article: "12".to_string(),
                construct: "naar redelijkheid".to_string(),
            };
            let resolver = TestResolver::new().with_var("onvertaalbaar", tainted.clone());
            let op = ActionOperation::Max {
                values: vec![lit(10i64), var("onvertaalbaar")],
            };

            assert_eq!(execute_operation(&op, &resolver, 0).unwrap(), tainted);
        }
    }

    // -------------------------------------------------------------------------
    // Rounding Operations Tests (RFC-024)
    // -------------------------------------------------------------------------

    mod rounding {
        use super::*;

        fn round_op(value: ActionValue, precision: i64) -> ActionOperation {
            ActionOperation::Round { value, precision }
        }

        #[test]
        fn round_to_whole_units_half_up() {
            let resolver = TestResolver::new();
            // 2.5 -> 3 (rekenkundig, ties away from zero)
            let op = round_op(lit(2.5f64), 0);
            assert_eq!(execute_operation(&op, &resolver, 0).unwrap(), Value::Int(3));
            // 2.4 -> 2
            let op = round_op(lit(2.4f64), 0);
            assert_eq!(execute_operation(&op, &resolver, 0).unwrap(), Value::Int(2));
        }

        #[test]
        fn round_to_two_decimals() {
            let resolver = TestResolver::new();
            // 1.005 -> 1.01 (exact in Decimal; the classic f64 failure case where
            // 1.005 is stored as 1.00499... and would wrongly round down to 1.00).
            let op = round_op(lit(Value::Decimal(dec!(1.005))), 2);
            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::Decimal(dec!(1.01))
            );
        }

        #[test]
        fn round_to_five_decimals_subcent_tariff() {
            let resolver = TestResolver::new();
            let op = round_op(lit(0.846789f64), 5);
            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::Decimal(dec!(0.84679))
            );
        }

        #[test]
        fn round_negative_precision_eurocent_to_whole_euros() {
            let resolver = TestResolver::new();
            // 209691.78888 eurocent rounded to whole euros (precision -2) -> 209700
            let op = round_op(lit(209691.78888f64), -2);
            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::Int(209700)
            );
        }

        #[test]
        fn ceil_rounds_up() {
            let resolver = TestResolver::new();
            let op = ActionOperation::Ceil {
                value: lit(1.01f64),
                precision: 0,
            };
            assert_eq!(execute_operation(&op, &resolver, 0).unwrap(), Value::Int(2));
        }

        #[test]
        fn floor_rounds_toward_negative_infinity() {
            let resolver = TestResolver::new();
            let floor = ActionOperation::Floor {
                value: lit(-1.5f64),
                precision: 0,
            };
            // FLOOR rounds toward -inf, so -1.5 → -2.
            assert_eq!(
                execute_operation(&floor, &resolver, 0).unwrap(),
                Value::Int(-2)
            );
        }

        #[test]
        fn rounding_propagates_untranslatable() {
            let resolver = TestResolver::new();
            let tainted = Value::Untranslatable {
                article: "1".into(),
                construct: "afronden".into(),
            };
            let op = round_op(lit(tainted), 0);
            assert!(execute_operation(&op, &resolver, 0)
                .unwrap()
                .is_untranslatable());
        }

        #[test]
        fn rounding_rejects_non_number() {
            let resolver = TestResolver::new();
            let op = round_op(lit("not a number"), 0);
            assert!(matches!(
                execute_operation(&op, &resolver, 0),
                Err(EngineError::TypeMismatch { .. })
            ));
        }

        #[test]
        fn rounding_rejects_out_of_range_precision() {
            let resolver = TestResolver::new();
            let op = round_op(lit(1.5f64), 100);
            assert!(matches!(
                execute_operation(&op, &resolver, 0),
                Err(EngineError::InvalidOperation(_))
            ));
        }

        #[test]
        fn rounding_extreme_magnitude_errors_not_panics() {
            // A near-max Decimal with large negative precision must error gracefully
            // (checked arithmetic), never panic mid-evaluation.
            let resolver = TestResolver::new();
            let huge = Value::Decimal(dec!(79000000000000000000000000000));
            let op = round_op(lit(huge), -28);
            assert!(matches!(
                execute_operation(&op, &resolver, 0),
                Err(EngineError::ArithmeticOverflow(_))
            ));
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

        /// Build an Untranslatable value for the taint tests.
        fn untranslatable(article: &str) -> Value {
            Value::Untranslatable {
                article: article.to_string(),
                construct: "open norm".to_string(),
            }
        }

        /// The article an untranslatable result points at. Asserted explicitly
        /// because `Value`'s equality treats *any* two untranslatables as equal,
        /// so `assert_eq!` alone would not see which one came back.
        fn untranslatable_article(value: &Value) -> &str {
            match value {
                Value::Untranslatable { article, .. } => article,
                other => panic!("expected an untranslatable value, got {:?}", other),
            }
        }

        #[test]
        fn test_and_reports_the_first_untranslatable_condition() {
            // RFC-012: when nothing is definitively false, AND hands back the
            // taint. Which one matters for the explanation shown to the citizen:
            // the first untranslatable condition is the one that blocked the
            // evaluation, so its article is the one that gets reported.
            let resolver = TestResolver::new()
                .with_var("eerste", untranslatable("5"))
                .with_var("tweede", untranslatable("9"));
            let op = ActionOperation::And {
                conditions: vec![lit(true), var("eerste"), var("tweede")],
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(untranslatable_article(&result), "5");
        }

        #[test]
        fn test_and_definitive_false_beats_a_taint() {
            // A condition that is certainly false decides the whole AND, whatever
            // order it appears in — an untranslatable elsewhere cannot revive it.
            let resolver = TestResolver::new().with_var("onvertaalbaar", untranslatable("5"));
            let op = ActionOperation::And {
                conditions: vec![var("onvertaalbaar"), lit(false)],
            };

            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::Bool(false)
            );
        }

        #[test]
        fn test_or_propagates_a_taint_over_a_plain_false() {
            // A false condition does not settle an OR: as long as another
            // condition could not be evaluated, the outcome is untranslatable
            // rather than a hard "no".
            let resolver = TestResolver::new().with_var("onvertaalbaar", untranslatable("7"));
            let op = ActionOperation::Or {
                conditions: vec![lit(false), var("onvertaalbaar")],
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(untranslatable_article(&result), "7");
        }

        #[test]
        fn test_or_reports_the_first_untranslatable_condition() {
            // Same provenance rule as AND: the explanation points at the first
            // condition that could not be evaluated, not at the last one seen.
            let resolver = TestResolver::new()
                .with_var("eerste", untranslatable("5"))
                .with_var("tweede", untranslatable("9"));
            let op = ActionOperation::Or {
                conditions: vec![lit(false), var("eerste"), var("tweede")],
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(untranslatable_article(&result), "5");
        }

        #[test]
        fn test_or_definitive_true_beats_a_taint() {
            let resolver = TestResolver::new().with_var("onvertaalbaar", untranslatable("7"));
            let op = ActionOperation::Or {
                conditions: vec![var("onvertaalbaar"), lit(true)],
            };

            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::Bool(true)
            );
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
        fn test_variable_not_found() {
            let resolver = TestResolver::new();
            let op = ActionOperation::Equals {
                subject: var("nonexistent"),
                value: lit(42i64),
            };

            let result = execute_operation(&op, &resolver, 0);
            assert!(matches!(result, Err(EngineError::VariableNotFound(_))));
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
        fn test_depth_guard_allows_the_maximum_depth() {
            // The guard rejects nesting *deeper* than MAX_OPERATION_DEPTH, so a
            // law that nests exactly that deep still evaluates. Pinning the
            // boundary keeps the limit from silently shifting by one.
            let resolver = TestResolver::new();

            assert_eq!(
                evaluate_value(&lit(42i64), &resolver, MAX_OPERATION_DEPTH).unwrap(),
                Value::Int(42)
            );

            let op = ActionOperation::List { items: vec![] };
            assert_eq!(
                execute_operation(&op, &resolver, MAX_OPERATION_DEPTH).unwrap(),
                Value::Array(vec![])
            );
        }

        #[test]
        fn test_depth_guard_rejects_beyond_the_maximum_depth() {
            // One step past the limit is refused, and so is anything deeper —
            // the guard is a threshold, not an equality check on one depth.
            let resolver = TestResolver::new();

            for depth in [MAX_OPERATION_DEPTH + 1, MAX_OPERATION_DEPTH + 2] {
                assert!(
                    matches!(
                        evaluate_value(&lit(42i64), &resolver, depth),
                        Err(EngineError::MaxDepthExceeded(_))
                    ),
                    "evaluate_value accepted depth {depth}"
                );

                let op = ActionOperation::List { items: vec![] };
                assert!(
                    matches!(
                        execute_operation(&op, &resolver, depth),
                        Err(EngineError::MaxDepthExceeded(_))
                    ),
                    "execute_operation accepted depth {depth}"
                );
            }
        }

        #[test]
        fn test_decimal_to_i64_safe_rejects_non_integer() {
            // A fractional value is "not an integer" (InvalidOperation), not an overflow.
            assert!(matches!(
                decimal_to_i64_safe(dec!(42.5)),
                Err(EngineError::InvalidOperation(_))
            ));
            assert_eq!(decimal_to_i64_safe(dec!(42)).unwrap(), 42);
            assert_eq!(decimal_to_i64_safe(dec!(-42)).unwrap(), -42);
        }

        #[test]
        fn test_values_equal_numeric() {
            // Int and Decimal compare numerically and exactly (no float rounding).
            assert!(values_equal(&Value::Int(42), &Value::Decimal(dec!(42))));
            assert!(values_equal(&Value::Decimal(dec!(42)), &Value::Int(42)));
            assert!(values_equal(
                &Value::Decimal(dec!(1.50)),
                &Value::Decimal(dec!(1.5))
            ));
            assert!(!values_equal(&Value::Decimal(dec!(42.5)), &Value::Int(42)));
        }

        #[test]
        fn test_values_equal_untranslatable() {
            // RFC-012: untranslatables compare like NaN in this domain. Two of
            // them count as equal (both are "could not be determined"), and one
            // never equals a concrete value — so a membership test against a
            // list can never accidentally match on a taint.
            let unknown = Value::Untranslatable {
                article: "5".to_string(),
                construct: "naar het oordeel van".to_string(),
            };
            let other_unknown = Value::Untranslatable {
                article: "9".to_string(),
                construct: "in redelijkheid".to_string(),
            };

            assert!(values_equal(&unknown, &other_unknown));
            assert!(!values_equal(&unknown, &Value::Int(42)));
            assert!(!values_equal(&Value::String("ja".to_string()), &unknown));
        }

        #[test]
        fn test_arithmetic_with_large_integer() {
            // Beyond 2^53 arithmetic is now exact (Decimal), no precision loss.
            let large_value: i64 = 9_007_199_254_740_993; // 2^53 + 1

            let resolver = TestResolver::new();
            let op = ActionOperation::Add {
                values: vec![lit(large_value), lit(1i64)],
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Int(9_007_199_254_740_994));
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
        fn test_not_in_true_when_absent() {
            // NOT_IN is the complement of IN: absent subject → true.
            let resolver = TestResolver::new();
            let op = ActionOperation::NotIn {
                subject: lit("student"),
                value: None,
                values: Some(vec![lit("gehuwd"), lit("samenwonend")]),
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Bool(true));
        }

        #[test]
        fn test_not_in_false_when_present() {
            // The exclusion fires: a subject that IS in the list is not "not in" it.
            let resolver = TestResolver::new();
            let op = ActionOperation::NotIn {
                subject: lit("gehuwd"),
                value: None,
                values: Some(vec![lit("gehuwd"), lit("samenwonend")]),
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Bool(false));
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
    // Null Checking Tests (IS_NULL / NOT_NULL)
    // -------------------------------------------------------------------------

    mod null_checks {
        use super::*;

        #[test]
        fn test_is_null_on_missing_value() {
            // A parameter that resolves to Null: "niet opgegeven" is true.
            let resolver = TestResolver::new().with_var("inkomen", Value::Null);
            let op = ActionOperation::IsNull {
                subject: var("inkomen"),
            };

            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::Bool(true)
            );
        }

        #[test]
        fn test_is_null_false_on_present_value() {
            // Zero is a value, not a missing value: IS_NULL must not confuse the two.
            let resolver = TestResolver::new().with_var("inkomen", 0i64);
            let op = ActionOperation::IsNull {
                subject: var("inkomen"),
            };

            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::Bool(false)
            );
        }

        #[test]
        fn test_not_null_is_the_complement() {
            // NOT_NULL answers the opposite question on the same two inputs.
            let resolver = TestResolver::new()
                .with_var("leeg", Value::Null)
                .with_var("gevuld", 0i64);

            let on_null = ActionOperation::NotNull {
                subject: var("leeg"),
            };
            assert_eq!(
                execute_operation(&on_null, &resolver, 0).unwrap(),
                Value::Bool(false)
            );

            let on_value = ActionOperation::NotNull {
                subject: var("gevuld"),
            };
            assert_eq!(
                execute_operation(&on_value, &resolver, 0).unwrap(),
                Value::Bool(true)
            );
        }

        #[test]
        fn test_null_check_propagates_untranslatable() {
            // RFC-012: an untranslatable subject is neither null nor not-null;
            // the taint flows through instead of collapsing to a boolean.
            let tainted = Value::Untranslatable {
                article: "3".to_string(),
                construct: "naar het oordeel van".to_string(),
            };
            let resolver = TestResolver::new().with_var("onvertaalbaar", tainted.clone());
            let op = ActionOperation::IsNull {
                subject: var("onvertaalbaar"),
            };

            assert_eq!(execute_operation(&op, &resolver, 0).unwrap(), tainted);
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
    // Date comparison Tests (RFC-021 route A: type-safe ordered comparison)
    // -------------------------------------------------------------------------

    mod date_comparison {
        use super::*;

        #[test]
        fn test_greater_than_dates() {
            let resolver = TestResolver::new();
            let op = ActionOperation::GreaterThan {
                subject: lit("2025-01-15"),
                value: lit("2025-01-10"),
            };
            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::Bool(true)
            );
        }

        #[test]
        fn test_less_than_dates() {
            let resolver = TestResolver::new();
            let op = ActionOperation::LessThan {
                subject: lit("2024-12-31"),
                value: lit("2025-01-01"),
            };
            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::Bool(true)
            );
        }

        #[test]
        fn test_greater_than_or_equal_dates_equal() {
            let resolver = TestResolver::new();
            let op = ActionOperation::GreaterThanOrEqual {
                subject: lit("2025-06-01"),
                value: lit("2025-06-01"),
            };
            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::Bool(true)
            );
        }

        #[test]
        fn test_less_than_or_equal_dates() {
            let resolver = TestResolver::new();
            let op = ActionOperation::LessThanOrEqual {
                subject: lit("2025-06-02"),
                value: lit("2025-06-01"),
            };
            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::Bool(false)
            );
        }

        #[test]
        fn test_equals_dates() {
            // ISO date string equality is equivalent to date equality (canonical format).
            let resolver = TestResolver::new();
            let op = ActionOperation::Equals {
                subject: lit("2025-03-01"),
                value: lit("2025-03-01"),
            };
            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::Bool(true)
            );
        }

        #[test]
        fn test_compare_peildatum_against_threshold() {
            // The thread's motivating case: peildatum (referencedate.iso) vs a date literal.
            let resolver = TestResolver::new().with_var("peildatum", "2025-07-01");
            let op = ActionOperation::GreaterThan {
                subject: var("peildatum"),
                value: lit("2025-01-01"),
            };
            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::Bool(true)
            );
        }

        #[test]
        fn test_numeric_comparison_still_works() {
            // Route A must not regress integer/float comparison.
            let resolver = TestResolver::new();
            let op = ActionOperation::GreaterThan {
                subject: lit(50i64),
                value: lit(42i64),
            };
            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::Bool(true)
            );
        }

        #[test]
        fn test_mixed_number_and_date_errors() {
            // A number vs a date is a type error, not a silent coercion.
            let resolver = TestResolver::new();
            let op = ActionOperation::GreaterThan {
                subject: lit(2025i64),
                value: lit("2025-01-01"),
            };
            assert!(execute_operation(&op, &resolver, 0).is_err());
        }

        #[test]
        fn test_non_date_string_errors() {
            let resolver = TestResolver::new();
            let op = ActionOperation::GreaterThan {
                subject: lit("hello"),
                value: lit("world"),
            };
            let err = execute_operation(&op, &resolver, 0).unwrap_err();
            let msg = err.to_string();
            assert!(msg.contains("number or date"), "unexpected message: {msg}");
        }

        #[test]
        fn test_compare_referencedate_object_against_string() {
            // $referencedate resolves to an object; comparing it against a date string
            // must work via parse_date's object support.
            let resolver = TestResolver::new().with_var("referencedate", date_obj("2025-07-01"));
            let op = ActionOperation::GreaterThan {
                subject: var("referencedate"),
                value: lit("2025-01-01"),
            };
            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::Bool(true)
            );
        }

        #[test]
        fn test_equals_referencedate_object_equals_date_string() {
            // RFC-021: the object form and the string form of the same date are equal.
            let resolver = TestResolver::new().with_var("referencedate", date_obj("2025-03-01"));
            let op = ActionOperation::Equals {
                subject: var("referencedate"),
                value: lit("2025-03-01"),
            };
            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::Bool(true)
            );
        }

        #[test]
        fn test_equals_referencedate_object_differs_from_other_date() {
            let resolver = TestResolver::new().with_var("referencedate", date_obj("2025-03-01"));
            let op = ActionOperation::Equals {
                subject: var("referencedate"),
                value: lit("2025-03-02"),
            };
            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::Bool(false)
            );
        }

        #[test]
        fn test_equals_non_date_strings_unaffected() {
            // The date-aware fallback must not change plain string equality.
            let resolver = TestResolver::new();
            let same = ActionOperation::Equals {
                subject: lit("aanvraag"),
                value: lit("aanvraag"),
            };
            let diff = ActionOperation::Equals {
                subject: lit("aanvraag"),
                value: lit("bezwaar"),
            };
            assert_eq!(
                execute_operation(&same, &resolver, 0).unwrap(),
                Value::Bool(true)
            );
            assert_eq!(
                execute_operation(&diff, &resolver, 0).unwrap(),
                Value::Bool(false)
            );
        }

        #[test]
        fn test_strict_greater_than_is_false_on_equal_dates() {
            // Boundary day: a deadline check must not flip on the cutoff date
            // itself. Pins the strictness of the date path.
            let resolver = TestResolver::new();
            let op = ActionOperation::GreaterThan {
                subject: lit("2025-06-01"),
                value: lit("2025-06-01"),
            };
            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::Bool(false)
            );
        }

        #[test]
        fn test_strict_less_than_is_false_on_equal_dates() {
            let resolver = TestResolver::new();
            let op = ActionOperation::LessThan {
                subject: lit("2025-06-01"),
                value: lit("2025-06-01"),
            };
            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::Bool(false)
            );
        }

        #[test]
        fn test_not_equals_dates_mixed_forms() {
            // NOT_EQUALS inverts the date-aware fallback for the mixed form.
            let resolver = TestResolver::new().with_var("referencedate", date_obj("2025-03-01"));
            let same = ActionOperation::NotEquals {
                subject: var("referencedate"),
                value: lit("2025-03-01"),
            };
            let diff = ActionOperation::NotEquals {
                subject: var("referencedate"),
                value: lit("2025-03-02"),
            };
            assert_eq!(
                execute_operation(&same, &resolver, 0).unwrap(),
                Value::Bool(false)
            );
            assert_eq!(
                execute_operation(&diff, &resolver, 0).unwrap(),
                Value::Bool(true)
            );
        }

        #[test]
        fn test_equals_objects_sharing_iso_stay_structural() {
            // Object↔object keeps structural equality: two objects that merely
            // share an `iso` field are NOT equal (the date fallback is scoped to
            // the mixed string/object form).
            let mut left = BTreeMap::new();
            left.insert("iso".to_string(), Value::String("2025-01-01".to_string()));
            left.insert("type".to_string(), Value::String("aanvraag".to_string()));
            let mut right = BTreeMap::new();
            right.insert("iso".to_string(), Value::String("2025-01-01".to_string()));
            right.insert("type".to_string(), Value::String("bezwaar".to_string()));

            let resolver = TestResolver::new()
                .with_var("left", Value::Object(left))
                .with_var("right", Value::Object(right));
            let op = ActionOperation::Equals {
                subject: var("left"),
                value: var("right"),
            };
            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::Bool(false)
            );
        }

        #[test]
        fn test_equals_identical_referencedate_objects_still_equal() {
            // Two structurally identical reference-date objects stay equal via
            // plain structural equality; the fallback is not needed for them.
            let resolver = TestResolver::new()
                .with_var("a", date_obj("2025-03-01"))
                .with_var("b", date_obj("2025-03-01"));
            let op = ActionOperation::Equals {
                subject: var("a"),
                value: var("b"),
            };
            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::Bool(true)
            );
        }

        #[test]
        fn test_mixed_error_names_both_kinds() {
            // A number against a date reports the mixed pair, not a misleading
            // complaint about one (individually valid) operand.
            let resolver = TestResolver::new();
            let op = ActionOperation::GreaterThan {
                subject: lit(2025i64),
                value: lit("2025-01-01"),
            };
            let err = execute_operation(&op, &resolver, 0).unwrap_err();
            let msg = err.to_string();
            assert!(
                msg.contains("two numbers or two dates")
                    && msg.contains("a number compared against a date"),
                "unexpected message: {msg}"
            );
        }

        #[test]
        fn test_error_blames_the_invalid_operand() {
            // A valid date compared against a bool must blame the bool, not the
            // (perfectly fine) date subject.
            let resolver = TestResolver::new().with_var("flag", Value::Bool(true));
            let op = ActionOperation::GreaterThan {
                subject: lit("2025-01-01"),
                value: var("flag"),
            };
            let err = execute_operation(&op, &resolver, 0).unwrap_err();
            let msg = err.to_string();
            assert!(
                msg.contains("number or date") && msg.contains("bool"),
                "unexpected message: {msg}"
            );
        }

        #[test]
        fn test_tainted_date_operand_propagates() {
            // RFC-012: an Untranslatable operand flows through the ordered
            // comparison as a value instead of failing the calculation.
            let tainted = Value::Untranslatable {
                article: "1".to_string(),
                construct: "test".to_string(),
            };
            let resolver = TestResolver::new().with_var("tainted", tainted.clone());
            let op = ActionOperation::GreaterThan {
                subject: var("tainted"),
                value: lit("2025-01-01"),
            };
            assert_eq!(execute_operation(&op, &resolver, 0).unwrap(), tainted);
        }
    }

    // -------------------------------------------------------------------------
    // DATE_DIFF Tests (RFC-021 route B: explicit unit argument)
    // -------------------------------------------------------------------------

    mod date_diff {
        use super::*;

        #[test]
        fn test_diff_in_days() {
            let resolver = TestResolver::new();
            let op = ActionOperation::DateDiff {
                from: lit("2025-01-01"),
                to: lit("2025-01-11"),
                unit: lit("days"),
            };
            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::Int(10)
            );
        }

        #[test]
        fn test_diff_in_days_negative_when_to_before_from() {
            // Sign convention: positive when `to >= from`, negative otherwise.
            let resolver = TestResolver::new();
            let op = ActionOperation::DateDiff {
                from: lit("2025-01-11"),
                to: lit("2025-01-01"),
                unit: lit("days"),
            };
            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::Int(-10)
            );
        }

        #[test]
        fn test_diff_in_years() {
            let resolver = TestResolver::new();
            let op = ActionOperation::DateDiff {
                from: lit("2000-06-01"),
                to: lit("2025-06-01"),
                unit: lit("years"),
            };
            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::Int(25)
            );
        }

        #[test]
        fn test_diff_in_years_before_anniversary() {
            // 2025-05-31 has not yet reached the June 1 anniversary → 24 complete years.
            let resolver = TestResolver::new();
            let op = ActionOperation::DateDiff {
                from: lit("2000-06-01"),
                to: lit("2025-05-31"),
                unit: lit("years"),
            };
            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::Int(24)
            );
        }

        #[test]
        fn test_diff_in_months() {
            let resolver = TestResolver::new();
            let op = ActionOperation::DateDiff {
                from: lit("2025-01-01"),
                to: lit("2025-04-01"),
                unit: lit("months"),
            };
            assert_eq!(execute_operation(&op, &resolver, 0).unwrap(), Value::Int(3));
        }

        #[test]
        fn test_diff_in_months_end_of_month_clamp() {
            // Jan 31 → Feb 28 counts as one complete month.
            let resolver = TestResolver::new();
            let op = ActionOperation::DateDiff {
                from: lit("2025-01-31"),
                to: lit("2025-02-28"),
                unit: lit("months"),
            };
            assert_eq!(execute_operation(&op, &resolver, 0).unwrap(), Value::Int(1));
        }

        #[test]
        fn test_diff_in_months_counts_only_complete_months() {
            // 15 January → 10 April is two whole months plus a bit: the day of
            // the month has not been reached again, so the third month does not
            // count. Termijnen in maanden lopen tot dezelfde dag in de maand.
            let resolver = TestResolver::new();
            let op = ActionOperation::DateDiff {
                from: lit("2025-01-15"),
                to: lit("2025-04-10"),
                unit: lit("months"),
            };
            assert_eq!(execute_operation(&op, &resolver, 0).unwrap(), Value::Int(2));
        }

        #[test]
        fn test_diff_in_months_spans_calendar_years() {
            // Months keep counting across the year boundary: 2 years and 3
            // months is 27 months, not 3.
            let resolver = TestResolver::new();
            let op = ActionOperation::DateDiff {
                from: lit("2023-01-01"),
                to: lit("2025-04-01"),
                unit: lit("months"),
            };
            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::Int(27)
            );
        }

        #[test]
        fn test_diff_in_years_february_start_is_not_treated_as_feb29() {
            // The Feb-29 rule is for Feb 29 only. A 10 February start reaches its
            // anniversary on 10 February, so 15 February is past it: 25 whole
            // years, not 24.
            let resolver = TestResolver::new();
            let op = ActionOperation::DateDiff {
                from: lit("2000-02-10"),
                to: lit("2025-02-15"),
                unit: lit("years"),
            };
            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::Int(25)
            );
        }

        #[test]
        fn test_diff_in_years_feb29_anniversary_falls_on_feb29_in_a_leap_year() {
            // The fallback to 28 February exists only for years that have no 29
            // February. 2024 has one, so on 28 February 2024 the anniversary has
            // not been reached yet: 23 whole years.
            let resolver = TestResolver::new();
            let op = ActionOperation::DateDiff {
                from: lit("2000-02-29"),
                to: lit("2024-02-28"),
                unit: lit("years"),
            };
            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::Int(23)
            );
        }

        #[test]
        fn test_diff_with_variables() {
            let resolver = TestResolver::new()
                .with_var("start", "2025-01-01")
                .with_var("eind", "2025-12-31");
            let op = ActionOperation::DateDiff {
                from: var("start"),
                to: var("eind"),
                unit: lit("days"),
            };
            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::Int(364)
            );
        }

        #[test]
        fn test_diff_invalid_unit_errors() {
            let resolver = TestResolver::new();
            let op = ActionOperation::DateDiff {
                from: lit("2025-01-01"),
                to: lit("2025-02-01"),
                unit: lit("weeks"),
            };
            assert!(execute_operation(&op, &resolver, 0).is_err());
        }

        #[test]
        fn test_diff_invalid_date_errors() {
            let resolver = TestResolver::new();
            let op = ActionOperation::DateDiff {
                from: lit("not-a-date"),
                to: lit("2025-02-01"),
                unit: lit("days"),
            };
            assert!(execute_operation(&op, &resolver, 0).is_err());
        }

        #[test]
        fn test_diff_non_canonical_date_errors() {
            // Non-zero-padded dates are rejected so EQUALS (string equality) and the
            // ordering operators can never disagree on the same literal.
            let resolver = TestResolver::new();
            let op = ActionOperation::DateDiff {
                from: lit("2025-1-1"),
                to: lit("2025-02-01"),
                unit: lit("days"),
            };
            assert!(execute_operation(&op, &resolver, 0).is_err());
        }

        #[test]
        fn test_diff_with_referencedate_object() {
            // `to` is the $referencedate object form, `from` a date string.
            let resolver = TestResolver::new().with_var("referencedate", date_obj("2025-01-11"));
            let op = ActionOperation::DateDiff {
                from: lit("2025-01-01"),
                to: var("referencedate"),
                unit: lit("days"),
            };
            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::Int(10)
            );
        }

        #[test]
        fn test_diff_in_days_across_leap_day() {
            // 2024 is a leap year, so Feb has 29 days: Feb 28 → Mar 1 is 2 days.
            let resolver = TestResolver::new();
            let op = ActionOperation::DateDiff {
                from: lit("2024-02-28"),
                to: lit("2024-03-01"),
                unit: lit("days"),
            };
            assert_eq!(execute_operation(&op, &resolver, 0).unwrap(), Value::Int(2));
        }

        #[test]
        fn test_diff_in_years_feb29_birthday_non_leap_year() {
            // A Feb 29 start reaches its anniversary on Feb 28 in a non-leap year
            // (BW art. 1:2), so 2000-02-29 → 2025-02-28 is 25 whole years.
            let resolver = TestResolver::new();
            let op = ActionOperation::DateDiff {
                from: lit("2000-02-29"),
                to: lit("2025-02-28"),
                unit: lit("years"),
            };
            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::Int(25)
            );
        }

        #[test]
        fn test_diff_in_months_negative_when_to_before_from() {
            // The sign convention holds for calendar units too, including the
            // end-of-month clamp in the reverse direction.
            let resolver = TestResolver::new();
            let op = ActionOperation::DateDiff {
                from: lit("2025-03-31"),
                to: lit("2025-02-28"),
                unit: lit("months"),
            };
            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::Int(-1)
            );
        }

        #[test]
        fn test_diff_in_years_negative_when_to_before_from() {
            let resolver = TestResolver::new();
            let op = ActionOperation::DateDiff {
                from: lit("2025-06-01"),
                to: lit("2020-06-01"),
                unit: lit("years"),
            };
            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::Int(-5)
            );
        }

        #[test]
        fn test_tainted_date_operand_propagates() {
            // RFC-012: an Untranslatable `from` flows through as a value.
            let tainted = Value::Untranslatable {
                article: "1".to_string(),
                construct: "test".to_string(),
            };
            let resolver = TestResolver::new().with_var("tainted", tainted.clone());
            let op = ActionOperation::DateDiff {
                from: var("tainted"),
                to: lit("2025-01-01"),
                unit: lit("days"),
            };
            assert_eq!(execute_operation(&op, &resolver, 0).unwrap(), tainted);
        }

        #[test]
        fn test_tainted_unit_propagates() {
            // The schema allows `in: $variable`; when that variable resolves to
            // an Untranslatable, the taint propagates instead of erroring.
            let tainted = Value::Untranslatable {
                article: "1".to_string(),
                construct: "test".to_string(),
            };
            let resolver = TestResolver::new().with_var("tainted_unit", tainted.clone());
            let op = ActionOperation::DateDiff {
                from: lit("2025-01-01"),
                to: lit("2025-01-11"),
                unit: var("tainted_unit"),
            };
            assert_eq!(execute_operation(&op, &resolver, 0).unwrap(), tainted);
        }
    }

    // -------------------------------------------------------------------------
    // DATE_PART Tests (the inverse of DATE; the unit is a component, not a count)
    // -------------------------------------------------------------------------

    mod date_part {
        use super::*;

        fn date_part(date: ActionValue, part: &str) -> ActionOperation {
            ActionOperation::DatePart {
                date,
                part: part.to_string(),
            }
        }

        #[test]
        fn test_part_year() {
            let resolver = TestResolver::new();
            let op = date_part(lit("2025-03-14"), "year");
            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::Int(2025)
            );
        }

        #[test]
        fn test_part_month() {
            let resolver = TestResolver::new();
            let op = date_part(lit("2025-03-14"), "month");
            assert_eq!(execute_operation(&op, &resolver, 0).unwrap(), Value::Int(3));
        }

        #[test]
        fn test_part_day() {
            let resolver = TestResolver::new();
            let op = date_part(lit("2025-03-14"), "day");
            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::Int(14)
            );
        }

        #[test]
        fn test_part_day_on_first_of_month() {
            // Awir art. 49 asks whether a change fell *after* the first of the
            // month, so day 1 has to come back as 1 and not as 0.
            let resolver = TestResolver::new();
            let op = date_part(lit("2025-03-01"), "day");
            assert_eq!(execute_operation(&op, &resolver, 0).unwrap(), Value::Int(1));
        }

        #[test]
        fn test_part_of_leap_day() {
            let resolver = TestResolver::new();
            let op = date_part(lit("2024-02-29"), "day");
            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::Int(29)
            );
        }

        #[test]
        fn test_part_reads_referencedate_object() {
            // `$referencedate` resolves to an object with an `iso` field; the
            // shared date parser has to accept it here too.
            let resolver = TestResolver::new().with_var("referencedate", date_obj("2025-07-04"));
            let op = date_part(var("referencedate"), "year");
            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::Int(2025)
            );
        }

        #[test]
        fn test_part_of_nested_operation() {
            // The canonical form nests inside `date:`, so a computed date has to
            // work as the operand.
            let resolver = TestResolver::new();
            let op = date_part(
                ActionValue::Operation(Box::new(ActionOperation::DateAdd {
                    date: lit("2007-11-30"),
                    years: Some(lit(18)),
                    months: None,
                    weeks: None,
                    days: None,
                })),
                "year",
            );
            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::Int(2025)
            );
        }

        #[test]
        fn test_plural_unit_from_date_diff_is_rejected_by_name() {
            // The weak spot of the design: singular here, plural in DATE_DIFF.
            // An author who copies the wrong one gets told which words are valid.
            let resolver = TestResolver::new();
            let op = date_part(lit("2025-03-14"), "months");
            let err = execute_operation(&op, &resolver, 0).unwrap_err();
            let msg = err.to_string();
            assert!(
                msg.contains("DATE_PART"),
                "message names the operation: {msg}"
            );
            assert!(msg.contains("'months'"), "message quotes the input: {msg}");
            assert!(
                msg.contains("'month'"),
                "message lists the valid words: {msg}"
            );
        }

        #[test]
        fn test_weekday_is_not_a_date_part() {
            // A weekday is no part of the year-month-day decomposition, so it is
            // out of scope here and stays DAY_OF_WEEK.
            let resolver = TestResolver::new();
            let op = date_part(lit("2025-03-14"), "weekday");
            assert!(execute_operation(&op, &resolver, 0).is_err());
        }

        #[test]
        fn test_non_canonical_date_rejected() {
            let resolver = TestResolver::new();
            let op = date_part(lit("2025-3-14"), "day");
            assert!(execute_operation(&op, &resolver, 0).is_err());
        }

        #[test]
        fn test_tainted_date_propagates() {
            // RFC-012: an Untranslatable date flows through as a value.
            let tainted = Value::Untranslatable {
                article: "1".to_string(),
                construct: "test".to_string(),
            };
            let resolver = TestResolver::new().with_var("tainted", tainted.clone());
            let op = date_part(var("tainted"), "year");
            assert_eq!(execute_operation(&op, &resolver, 0).unwrap(), tainted);
        }
    }

    // -------------------------------------------------------------------------
    // START_OF Tests (RFC-024: truncation is a named operation, never implicit)
    // -------------------------------------------------------------------------

    mod start_of {
        use super::*;

        fn start_of(date: ActionValue, unit: &str) -> ActionOperation {
            ActionOperation::StartOf {
                date,
                unit: unit.to_string(),
            }
        }

        fn nested(op: ActionOperation) -> ActionValue {
            ActionValue::Operation(Box::new(op))
        }

        /// The canonical "first day of the month following" form: truncate first,
        /// add afterwards.
        fn first_of_next_month(date: ActionValue) -> ActionOperation {
            ActionOperation::DateAdd {
                date: nested(start_of(date, "month")),
                years: None,
                months: Some(lit(1)),
                weeks: None,
                days: None,
            }
        }

        #[test]
        fn test_start_of_month() {
            let resolver = TestResolver::new();
            let op = start_of(lit("2025-03-14"), "month");
            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::String("2025-03-01".to_string())
            );
        }

        #[test]
        fn test_start_of_month_is_idempotent_on_the_first() {
            let resolver = TestResolver::new();
            let op = start_of(lit("2025-03-01"), "month");
            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::String("2025-03-01".to_string())
            );
        }

        #[test]
        fn test_start_of_month_on_leap_day() {
            let resolver = TestResolver::new();
            let op = start_of(lit("2024-02-29"), "month");
            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::String("2024-02-01".to_string())
            );
        }

        #[test]
        fn test_start_of_year() {
            let resolver = TestResolver::new();
            let op = start_of(lit("2025-12-31"), "year");
            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::String("2025-01-01".to_string())
            );
        }

        #[test]
        fn test_start_of_year_is_idempotent_on_new_year() {
            let resolver = TestResolver::new();
            let op = start_of(lit("2025-01-01"), "year");
            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::String("2025-01-01".to_string())
            );
        }

        #[test]
        fn test_start_of_returns_canonical_form() {
            // Single-digit months and days come back zero-padded, so the result
            // compares equal under EQUALS as well as under ordering (RFC-021).
            let resolver = TestResolver::new();
            let op = start_of(lit("2025-01-09"), "month");
            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::String("2025-01-01".to_string())
            );
        }

        #[test]
        fn test_day_is_not_a_truncation_unit() {
            // Truncating to a day is the identity; offering it would invite the
            // reading that START_OF normalizes dates.
            let resolver = TestResolver::new();
            let op = start_of(lit("2025-03-14"), "day");
            assert!(execute_operation(&op, &resolver, 0).is_err());
        }

        #[test]
        fn test_plural_unit_from_date_diff_is_rejected_by_name() {
            let resolver = TestResolver::new();
            let op = start_of(lit("2025-03-14"), "months");
            let err = execute_operation(&op, &resolver, 0).unwrap_err();
            let msg = err.to_string();
            assert!(
                msg.contains("START_OF"),
                "message names the operation: {msg}"
            );
            assert!(msg.contains("'months'"), "message quotes the input: {msg}");
            assert!(
                msg.contains("'month'"),
                "message lists the valid words: {msg}"
            );
        }

        #[test]
        fn test_tainted_date_propagates() {
            let tainted = Value::Untranslatable {
                article: "1".to_string(),
                construct: "test".to_string(),
            };
            let resolver = TestResolver::new().with_var("tainted", tainted.clone());
            let op = start_of(var("tainted"), "month");
            assert_eq!(execute_operation(&op, &resolver, 0).unwrap(), tainted);
        }

        // --- the canonical "first day of the month following" pattern ---

        #[test]
        fn test_first_of_next_month_midmonth() {
            let resolver = TestResolver::new();
            let op = first_of_next_month(lit("2025-03-15"));
            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::String("2025-04-01".to_string())
            );
        }

        #[test]
        fn test_first_of_next_month_on_the_first() {
            // Awir art. 49 default branch: a change on the first counts in that
            // month itself, so this form is only reached from the other branch —
            // and it still has to move exactly one month.
            let resolver = TestResolver::new();
            let op = first_of_next_month(lit("2025-03-01"));
            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::String("2025-04-01".to_string())
            );
        }

        #[test]
        fn test_first_of_next_month_on_the_last_day() {
            let resolver = TestResolver::new();
            let op = first_of_next_month(lit("2025-03-31"));
            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::String("2025-04-01".to_string())
            );
        }

        #[test]
        fn test_first_of_next_month_on_31_january_never_touches_the_clamp() {
            // Truncate-then-add gives 1 February. Add-then-truncate would route
            // through the DATE_ADD clamp (31 Jan + 1 month = 28 Feb) and land on
            // the same day by coincidence; this test pins the order that does not
            // depend on that coincidence.
            let resolver = TestResolver::new();
            let op = first_of_next_month(lit("2025-01-31"));
            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::String("2025-02-01".to_string())
            );
        }

        #[test]
        fn test_first_of_next_month_crosses_the_year() {
            let resolver = TestResolver::new();
            let op = first_of_next_month(lit("2025-12-17"));
            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::String("2026-01-01".to_string())
            );
        }

        #[test]
        fn test_premieplicht_from_the_month_after_turning_eighteen() {
            // Zvw art. 16 lid 2 on a leap-day birth: 18 years later is 28 February
            // 2026 (the DATE_ADD clamp, BW art. 1:2), truncated 1 February 2026,
            // plus one month 1 March 2026.
            let resolver = TestResolver::new();
            let op = first_of_next_month(nested(ActionOperation::DateAdd {
                date: lit("2008-02-29"),
                years: Some(lit(18)),
                months: None,
                weeks: None,
                days: None,
            }));
            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::String("2026-03-01".to_string())
            );
        }

        #[test]
        fn test_start_of_year_feeds_date_part() {
            // START_OF is derivable from DATE_PART + DATE (RFC-024 rejects that
            // as the way to write it); the two still have to agree.
            let resolver = TestResolver::new();
            let op = ActionOperation::DatePart {
                date: nested(start_of(lit("2025-12-31"), "year")),
                part: "month".to_string(),
            };
            assert_eq!(execute_operation(&op, &resolver, 0).unwrap(), Value::Int(1));
        }
    }

    // -------------------------------------------------------------------------
    // Canonical date-form Tests (RFC-021: keep EQUALS and ordering consistent)
    // -------------------------------------------------------------------------

    mod canonical_dates {
        use super::*;

        #[test]
        fn test_non_canonical_date_rejected_by_comparison() {
            let resolver = TestResolver::new();
            let op = ActionOperation::GreaterThan {
                subject: lit("2025-1-1"),
                value: lit("2025-01-01"),
            };
            assert!(execute_operation(&op, &resolver, 0).is_err());
        }

        #[test]
        fn test_equals_non_canonical_string_stays_structural() {
            // The EQUALS date fallback does not fire when a side fails canonical
            // parsing: "2025-1-1" against the object form of 2025-01-01 is simply
            // unequal, without an error (RFC-021 implementation notes). Pins the
            // one place where a non-canonical date is not rejected loudly.
            let resolver = TestResolver::new().with_var("referencedate", date_obj("2025-01-01"));
            let op = ActionOperation::Equals {
                subject: lit("2025-1-1"),
                value: var("referencedate"),
            };
            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::Bool(false)
            );
        }

        #[test]
        fn test_equals_and_ordering_agree_on_canonical_dates() {
            // For canonical (zero-padded) ISO dates, "a == b" iff neither "a > b" nor "a < b".
            let resolver = TestResolver::new();
            let a = "2025-03-01";
            let b = "2025-03-01";

            let eq = ActionOperation::Equals {
                subject: lit(a),
                value: lit(b),
            };
            let gt = ActionOperation::GreaterThan {
                subject: lit(a),
                value: lit(b),
            };
            let lt = ActionOperation::LessThan {
                subject: lit(a),
                value: lit(b),
            };
            assert_eq!(
                execute_operation(&eq, &resolver, 0).unwrap(),
                Value::Bool(true)
            );
            assert_eq!(
                execute_operation(&gt, &resolver, 0).unwrap(),
                Value::Bool(false)
            );
            assert_eq!(
                execute_operation(&lt, &resolver, 0).unwrap(),
                Value::Bool(false)
            );
        }
    }

    // -------------------------------------------------------------------------
    // Tracing opt-in Tests
    // -------------------------------------------------------------------------

    mod tracing {
        use super::*;
        use std::cell::RefCell;

        /// A resolver that offers a trace hook but never switches tracing on:
        /// it keeps `has_trace` at the trait's default.
        #[derive(Default)]
        struct RecordingResolver {
            pushed: RefCell<Vec<String>>,
        }

        impl ValueResolver for RecordingResolver {
            fn resolve(&self, name: &str) -> Result<Value> {
                Err(EngineError::VariableNotFound(name.to_string()))
            }

            fn trace_push(&self, name: &str, _node_type: PathNodeType) {
                self.pushed.borrow_mut().push(name.to_string());
            }
        }

        #[test]
        fn test_no_tracing_unless_the_resolver_opts_in() {
            // Tracing is opt-in: a resolver that implements the hooks but leaves
            // `has_trace` at its default gets none of the trace bookkeeping, and
            // the operation result is the same either way.
            let resolver = RecordingResolver::default();
            let op = ActionOperation::Add {
                values: vec![lit(1i64), lit(2i64)],
            };

            assert_eq!(execute_operation(&op, &resolver, 0).unwrap(), Value::Int(3));
            assert!(
                resolver.pushed.borrow().is_empty(),
                "expected no trace nodes, got {:?}",
                resolver.pushed.borrow()
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

        let result = parse_date(&Value::Object(date_obj)).unwrap();
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
    fn test_values_equal_exact_large_integers() {
        // Decimal carries integers exactly, so large-integer Int/Decimal pairs
        // compare without the ±2^53 precision loss f64 would introduce.
        let large: i64 = 9_007_199_254_740_993; // 2^53 + 1
        assert!(values_equal(
            &Value::Int(large),
            &Value::Decimal(Decimal::from(large))
        ));
        assert!(!values_equal(
            &Value::Int(large),
            &Value::Decimal(Decimal::from(large + 1))
        ));

        assert!(values_equal(&Value::Int(42), &Value::Decimal(dec!(42))));
        assert!(values_equal(&Value::Decimal(dec!(42)), &Value::Int(42)));
    }
}
