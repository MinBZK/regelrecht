//! Core types for the RegelRecht engine

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

/// Represents any value in the engine (similar to Python's Any)
///
/// Note: `PartialEq` is implemented manually so that `Float(NaN) == Float(NaN)`
/// returns `true`. In the law execution domain, NaN represents invalid/missing
/// data and two missing values are considered equal for comparison purposes.
/// This matches the behavior of [`crate::operations::values_equal`].
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(untagged)]
pub enum Value {
    /// Null/None value
    #[default]
    Null,
    /// Boolean value
    Bool(bool),
    /// Integer value
    Int(i64),
    /// Floating point value
    Float(f64),
    /// String value
    String(String),
    /// Array of values
    Array(Vec<Value>),
    /// Object/Map of values
    Object(HashMap<String, Value>),
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Null, Value::Null) => true,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Int(a), Value::Int(b)) => a == b,
            (Value::Float(a), Value::Float(b)) => {
                // NaN == NaN is true in the law execution domain
                if a.is_nan() && b.is_nan() {
                    return true;
                }
                a == b
            }
            (Value::String(a), Value::String(b)) => a == b,
            (Value::Array(a), Value::Array(b)) => a == b,
            (Value::Object(a), Value::Object(b)) => a == b,
            _ => false,
        }
    }
}

impl Value {
    /// Check if value is null
    pub fn is_null(&self) -> bool {
        matches!(self, Value::Null)
    }

    /// Try to get value as boolean
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// Try to get value as i64
    ///
    /// For floats, truncates toward zero (like Python's `int()`).
    /// For example: `1.9` becomes `1`, `-1.9` becomes `-1`.
    pub fn as_int(&self) -> Option<i64> {
        match self {
            Value::Int(i) => Some(*i),
            Value::Float(f) => Some(*f as i64),
            _ => None,
        }
    }

    /// Try to get value as f64
    pub fn as_float(&self) -> Option<f64> {
        match self {
            Value::Float(f) => Some(*f),
            Value::Int(i) => Some(*i as f64),
            _ => None,
        }
    }

    /// Try to get value as string reference
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::String(s) => Some(s),
            _ => None,
        }
    }

    /// Try to get value as array reference
    pub fn as_array(&self) -> Option<&Vec<Value>> {
        match self {
            Value::Array(a) => Some(a),
            _ => None,
        }
    }

    /// Try to get value as object reference
    pub fn as_object(&self) -> Option<&HashMap<String, Value>> {
        match self {
            Value::Object(o) => Some(o),
            _ => None,
        }
    }

    /// Convert value to boolean (Python-style truthiness)
    ///
    /// Note: NaN is treated as falsy (unlike Python where `bool(float('nan'))` is True).
    /// This is intentional for law execution where NaN represents invalid/missing data.
    pub fn to_bool(&self) -> bool {
        match self {
            Value::Null => false,
            Value::Bool(b) => *b,
            Value::Int(i) => *i != 0,
            Value::Float(f) => *f != 0.0 && !f.is_nan(),
            Value::String(s) => !s.is_empty(),
            Value::Array(a) => !a.is_empty(),
            Value::Object(o) => !o.is_empty(),
        }
    }
}

impl From<bool> for Value {
    fn from(b: bool) -> Self {
        Value::Bool(b)
    }
}

impl From<i64> for Value {
    fn from(i: i64) -> Self {
        Value::Int(i)
    }
}

impl From<i32> for Value {
    fn from(i: i32) -> Self {
        Value::Int(i as i64)
    }
}

impl From<f64> for Value {
    fn from(f: f64) -> Self {
        Value::Float(f)
    }
}

impl From<String> for Value {
    fn from(s: String) -> Self {
        Value::String(s)
    }
}

impl From<&str> for Value {
    fn from(s: &str) -> Self {
        Value::String(s.to_string())
    }
}

impl<T: Into<Value>> From<Vec<T>> for Value {
    fn from(v: Vec<T>) -> Self {
        Value::Array(v.into_iter().map(Into::into).collect())
    }
}

impl<T: Into<Value>> From<Option<T>> for Value {
    fn from(opt: Option<T>) -> Self {
        match opt {
            Some(v) => v.into(),
            None => Value::Null,
        }
    }
}

impl From<serde_json::Value> for Value {
    fn from(v: serde_json::Value) -> Self {
        match v {
            serde_json::Value::Null => Value::Null,
            serde_json::Value::Bool(b) => Value::Bool(b),
            serde_json::Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    Value::Int(i)
                } else if let Some(f) = n.as_f64() {
                    // Coerce whole-number floats to Int for consistency
                    if f.fract() == 0.0 && f >= i64::MIN as f64 && f <= i64::MAX as f64 {
                        Value::Int(f as i64)
                    } else {
                        Value::Float(f)
                    }
                } else {
                    Value::Null
                }
            }
            serde_json::Value::String(s) => Value::String(s),
            serde_json::Value::Array(arr) => {
                Value::Array(arr.into_iter().map(Value::from).collect())
            }
            serde_json::Value::Object(obj) => {
                let map: HashMap<String, Value> =
                    obj.into_iter().map(|(k, v)| (k, Value::from(v))).collect();
                Value::Object(map)
            }
        }
    }
}

impl From<&serde_json::Value> for Value {
    fn from(v: &serde_json::Value) -> Self {
        match v {
            serde_json::Value::Null => Value::Null,
            serde_json::Value::Bool(b) => Value::Bool(*b),
            serde_json::Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    Value::Int(i)
                } else if let Some(f) = n.as_f64() {
                    if f.fract() == 0.0 && f >= i64::MIN as f64 && f <= i64::MAX as f64 {
                        Value::Int(f as i64)
                    } else {
                        Value::Float(f)
                    }
                } else {
                    Value::Null
                }
            }
            serde_json::Value::String(s) => Value::String(s.clone()),
            serde_json::Value::Array(arr) => Value::Array(arr.iter().map(Value::from).collect()),
            serde_json::Value::Object(obj) => {
                let map: HashMap<String, Value> = obj
                    .iter()
                    .map(|(k, v)| (k.clone(), Value::from(v)))
                    .collect();
                Value::Object(map)
            }
        }
    }
}

impl From<&Value> for serde_json::Value {
    fn from(v: &Value) -> Self {
        match v {
            Value::Null => serde_json::Value::Null,
            Value::Bool(b) => serde_json::Value::Bool(*b),
            Value::Int(i) => serde_json::json!(*i),
            Value::Float(f) => serde_json::json!(*f),
            Value::String(s) => serde_json::Value::String(s.clone()),
            Value::Array(arr) => {
                serde_json::Value::Array(arr.iter().map(serde_json::Value::from).collect())
            }
            Value::Object(map) => {
                let obj: serde_json::Map<String, serde_json::Value> = map
                    .iter()
                    .map(|(k, v)| (k.clone(), serde_json::Value::from(v)))
                    .collect();
                serde_json::Value::Object(obj)
            }
        }
    }
}

impl From<Value> for serde_json::Value {
    fn from(v: Value) -> Self {
        match v {
            Value::Null => serde_json::Value::Null,
            Value::Bool(b) => serde_json::Value::Bool(b),
            Value::Int(i) => serde_json::json!(i),
            Value::Float(f) => serde_json::json!(f),
            Value::String(s) => serde_json::Value::String(s),
            Value::Array(arr) => {
                serde_json::Value::Array(arr.into_iter().map(Into::into).collect())
            }
            Value::Object(map) => {
                serde_json::Value::Object(map.into_iter().map(|(k, v)| (k, v.into())).collect())
            }
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Null => write!(f, "null"),
            Value::Bool(b) => write!(f, "{}", b),
            Value::Int(i) => write!(f, "{}", i),
            Value::Float(fl) => write!(f, "{}", fl),
            Value::String(s) => write!(f, "{}", s),
            Value::Array(arr) => {
                write!(f, "[")?;
                for (i, v) in arr.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", v)?;
                }
                write!(f, "]")
            }
            Value::Object(obj) => {
                write!(f, "{{")?;
                for (i, (k, v)) in obj.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}: {}", k, v)?;
                }
                write!(f, "}}")
            }
        }
    }
}

/// Operation types supported by the engine
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Operation {
    // Comparison operations (6)
    Equals,
    NotEquals,
    GreaterThan,
    LessThan,
    GreaterThanOrEqual,
    LessThanOrEqual,

    // Arithmetic operations (4)
    Add,
    Subtract,
    Multiply,
    Divide,

    // Aggregate operations (2)
    Max,
    Min,

    // Logical operations (2)
    And,
    Or,

    // Conditional operations (2)
    If,
    Switch,

    // Null checking operations (2)
    IsNull,
    NotNull,

    // Membership testing operations (2)
    In,
    NotIn,

    // Date operations (1)
    SubtractDate,
}

impl Operation {
    /// Check if this is a comparison operation
    pub fn is_comparison(&self) -> bool {
        matches!(
            self,
            Operation::Equals
                | Operation::NotEquals
                | Operation::GreaterThan
                | Operation::LessThan
                | Operation::GreaterThanOrEqual
                | Operation::LessThanOrEqual
        )
    }

    /// Check if this is an arithmetic operation
    pub fn is_arithmetic(&self) -> bool {
        matches!(
            self,
            Operation::Add | Operation::Subtract | Operation::Multiply | Operation::Divide
        )
    }

    /// Check if this is an aggregate operation
    pub fn is_aggregate(&self) -> bool {
        matches!(self, Operation::Max | Operation::Min)
    }

    /// Check if this is a logical operation
    pub fn is_logical(&self) -> bool {
        matches!(self, Operation::And | Operation::Or)
    }

    /// Check if this is a conditional operation
    pub fn is_conditional(&self) -> bool {
        matches!(self, Operation::If | Operation::Switch)
    }

    /// Check if this is a null checking operation
    pub fn is_null_check(&self) -> bool {
        matches!(self, Operation::IsNull | Operation::NotNull)
    }

    /// Check if this is a membership testing operation
    pub fn is_membership(&self) -> bool {
        matches!(self, Operation::In | Operation::NotIn)
    }

    /// Check if this is a date operation
    pub fn is_date(&self) -> bool {
        matches!(self, Operation::SubtractDate)
    }

    /// Get the operation name as a static uppercase string.
    ///
    /// Avoids per-invocation `format!("{:?}", op).to_uppercase()` allocations.
    pub fn name(&self) -> &'static str {
        match self {
            Operation::Equals => "EQUALS",
            Operation::NotEquals => "NOTEQUALS",
            Operation::GreaterThan => "GREATERTHAN",
            Operation::LessThan => "LESSTHAN",
            Operation::GreaterThanOrEqual => "GREATERTHANOREQUAL",
            Operation::LessThanOrEqual => "LESSTHANOREQUAL",
            Operation::Add => "ADD",
            Operation::Subtract => "SUBTRACT",
            Operation::Multiply => "MULTIPLY",
            Operation::Divide => "DIVIDE",
            Operation::Max => "MAX",
            Operation::Min => "MIN",
            Operation::And => "AND",
            Operation::Or => "OR",
            Operation::If => "IF",
            Operation::Switch => "SWITCH",
            Operation::IsNull => "ISNULL",
            Operation::NotNull => "NOTNULL",
            Operation::In => "IN",
            Operation::NotIn => "NOTIN",
            Operation::SubtractDate => "SUBTRACTDATE",
        }
    }
}

/// Re-export the canonical regulatory layer types from the shared crate.
pub use regelrecht_shared::RegulatoryLayer;

/// Parameter type specification
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ParameterType {
    #[default]
    String,
    Number,
    Boolean,
    Date,
    Array,
    Object,
}

/// Node type in execution trace
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PathNodeType {
    /// Variable/value resolution step
    Resolve,
    /// Operation execution (e.g., ADD, EQUALS)
    Operation,
    /// Action execution within an article
    Action,
    /// Requirement check
    Requirement,
    /// Cross-law URI call
    UriCall,
    /// Article-level execution
    Article,
    /// Delegation to another regulation
    Delegation,
    /// Cached cross-law result (memoized)
    Cached,
    /// Open term resolution via IoC (implements lookup)
    OpenTermResolution,
}

/// Resolve type for variable resolution
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ResolveType {
    /// Value resolved from a regelrecht:// URI
    Uri,
    /// Value resolved from input parameters
    Parameter,
    /// Value resolved from article definitions (constants)
    Definition,
    /// Value resolved from calculated outputs
    Output,
    /// Value resolved from input specification
    Input,
    /// Value resolved from local scope (loop variables)
    Local,
    /// Value resolved from context variables (referencedate)
    Context,
    /// Value resolved from cached cross-law results
    ResolvedInput,
    /// Value resolved from external data source
    DataSource,
    /// Value resolved via open term implementation (IoC)
    OpenTerm,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_value_bool_conversion() {
        assert!(Value::Bool(true).to_bool());
        assert!(!Value::Bool(false).to_bool());
        assert!(!Value::Null.to_bool());
        assert!(Value::Int(1).to_bool());
        assert!(!Value::Int(0).to_bool());
        assert!(Value::String("hello".to_string()).to_bool());
        assert!(!Value::String("".to_string()).to_bool());
        // NaN is falsy (intentional deviation from Python)
        assert!(!Value::Float(f64::NAN).to_bool());
        assert!(Value::Float(1.0).to_bool());
        assert!(!Value::Float(0.0).to_bool());
    }

    #[test]
    fn test_value_from_primitives() {
        assert_eq!(Value::from(true), Value::Bool(true));
        assert_eq!(Value::from(42i64), Value::Int(42));
        assert_eq!(Value::from(3.14f64), Value::Float(3.14));
        assert_eq!(Value::from("test"), Value::String("test".to_string()));
    }

    #[test]
    fn test_value_as_methods() {
        let bool_val = Value::Bool(true);
        assert_eq!(bool_val.as_bool(), Some(true));
        assert_eq!(bool_val.as_int(), None);

        let int_val = Value::Int(42);
        assert_eq!(int_val.as_int(), Some(42));
        assert_eq!(int_val.as_float(), Some(42.0));

        let str_val = Value::String("hello".to_string());
        assert_eq!(str_val.as_str(), Some("hello"));
    }

    #[test]
    fn test_operation_categories() {
        assert!(Operation::Equals.is_comparison());
        assert!(Operation::Add.is_arithmetic());
        assert!(Operation::Max.is_aggregate());
        assert!(Operation::And.is_logical());
        assert!(Operation::If.is_conditional());
    }

    #[test]
    fn test_value_nan_equality() {
        // NaN == NaN should be true (intentional domain decision for law execution)
        assert_eq!(Value::Float(f64::NAN), Value::Float(f64::NAN));
        // NaN != non-NaN
        assert_ne!(Value::Float(f64::NAN), Value::Float(0.0));
        assert_ne!(Value::Float(0.0), Value::Float(f64::NAN));
        // Normal floats still work
        assert_eq!(Value::Float(1.0), Value::Float(1.0));
        assert_ne!(Value::Float(1.0), Value::Float(2.0));
    }

    #[test]
    fn test_value_serde_roundtrip() {
        let values = vec![
            Value::Null,
            Value::Bool(true),
            Value::Int(42),
            Value::Float(3.14),
            Value::String("test".to_string()),
            Value::Array(vec![Value::Int(1), Value::Int(2)]),
        ];

        for value in values {
            let json = serde_json::to_string(&value).unwrap();
            let parsed: Value = serde_json::from_str(&json).unwrap();
            assert_eq!(value, parsed);
        }
    }
}
