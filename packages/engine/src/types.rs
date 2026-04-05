//! Core types for the RegelRecht engine

use serde::de::{self, MapAccess, SeqAccess, Visitor};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::BTreeMap;
use std::fmt;

/// Represents any value in the engine (similar to Python's Any)
///
/// Note: `PartialEq` is implemented manually so that `Float(NaN) == Float(NaN)`
/// returns `true`. In the law execution domain, NaN represents invalid/missing
/// data and two missing values are considered equal for comparison purposes.
/// This matches the behavior of [`crate::operations::values_equal`].
///
/// The `Untranslatable` variant (RFC-012 Layer 3) represents a value that
/// originates from an article with untranslatable constructs. It propagates
/// through operations like NaN in floating point: any operation involving an
/// Untranslatable input produces an Untranslatable output.
#[derive(Debug, Clone, Default)]
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
    Object(BTreeMap<String, Value>),
    /// Untranslatable taint marker (RFC-012 Layer 3).
    /// Carries origin info: (article_number, construct description).
    Untranslatable {
        /// Article number where the untranslatable originated
        article: String,
        /// The construct that could not be translated
        construct: String,
    },
}

/// Sentinel key used to identify serialized Untranslatable values.
const UNTRANSLATABLE_KEY: &str = "__untranslatable";

impl Serialize for Value {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        match self {
            Value::Null => serializer.serialize_none(),
            Value::Bool(b) => serializer.serialize_bool(*b),
            Value::Int(i) => serializer.serialize_i64(*i),
            Value::Float(f) => serializer.serialize_f64(*f),
            Value::String(s) => serializer.serialize_str(s),
            Value::Array(arr) => arr.serialize(serializer),
            Value::Object(map) => map.serialize(serializer),
            Value::Untranslatable { article, construct } => {
                let mut map = serializer.serialize_map(Some(3))?;
                map.serialize_entry(UNTRANSLATABLE_KEY, &true)?;
                map.serialize_entry("article", article)?;
                map.serialize_entry("construct", construct)?;
                map.end()
            }
        }
    }
}

impl<'de> Deserialize<'de> for Value {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        deserializer.deserialize_any(ValueVisitor)
    }
}

struct ValueVisitor;

impl<'de> Visitor<'de> for ValueVisitor {
    type Value = Value;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a valid value")
    }

    fn visit_bool<E: de::Error>(self, v: bool) -> std::result::Result<Value, E> {
        Ok(Value::Bool(v))
    }

    fn visit_i64<E: de::Error>(self, v: i64) -> std::result::Result<Value, E> {
        Ok(Value::Int(v))
    }

    fn visit_u64<E: de::Error>(self, v: u64) -> std::result::Result<Value, E> {
        i64::try_from(v)
            .map(Value::Int)
            .map_err(|_| E::custom(format!("u64 value {v} overflows i64")))
    }

    fn visit_f64<E: de::Error>(self, v: f64) -> std::result::Result<Value, E> {
        // Coerce whole-number floats to Int for consistency
        if v.fract() == 0.0 && v >= i64::MIN as f64 && v <= i64::MAX as f64 {
            Ok(Value::Int(v as i64))
        } else {
            Ok(Value::Float(v))
        }
    }

    fn visit_str<E: de::Error>(self, v: &str) -> std::result::Result<Value, E> {
        Ok(Value::String(v.to_string()))
    }

    fn visit_string<E: de::Error>(self, v: String) -> std::result::Result<Value, E> {
        Ok(Value::String(v))
    }

    fn visit_none<E: de::Error>(self) -> std::result::Result<Value, E> {
        Ok(Value::Null)
    }

    fn visit_unit<E: de::Error>(self) -> std::result::Result<Value, E> {
        Ok(Value::Null)
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> std::result::Result<Value, A::Error> {
        let mut arr = Vec::new();
        while let Some(elem) = seq.next_element()? {
            arr.push(elem);
        }
        Ok(Value::Array(arr))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> std::result::Result<Value, A::Error> {
        let mut obj = BTreeMap::new();
        while let Some((key, value)) = map.next_entry::<String, Value>()? {
            obj.insert(key, value);
        }
        // Check if this is a serialized Untranslatable
        if obj.get(UNTRANSLATABLE_KEY) == Some(&Value::Bool(true)) {
            let article = match obj.get("article") {
                Some(Value::String(s)) => s.clone(),
                _ => return Err(de::Error::missing_field("article")),
            };
            let construct = match obj.get("construct") {
                Some(Value::String(s)) => s.clone(),
                _ => return Err(de::Error::missing_field("construct")),
            };
            return Ok(Value::Untranslatable { article, construct });
        }
        Ok(Value::Object(obj))
    }
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
            // Two Untranslatable values are equal (like NaN == NaN in this domain)
            (Value::Untranslatable { .. }, Value::Untranslatable { .. }) => true,
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
    pub fn as_array(&self) -> Option<&[Value]> {
        match self {
            Value::Array(a) => Some(a),
            _ => None,
        }
    }

    /// Try to get value as object reference
    pub fn as_object(&self) -> Option<&BTreeMap<String, Value>> {
        match self {
            Value::Object(o) => Some(o),
            _ => None,
        }
    }

    /// Check if value is untranslatable (RFC-012 taint).
    pub fn is_untranslatable(&self) -> bool {
        matches!(self, Value::Untranslatable { .. })
    }

    /// Get the type name as a static string (for error messages).
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Null => "null",
            Value::Bool(_) => "boolean",
            Value::Int(_) => "integer",
            Value::Float(_) => "float",
            Value::String(_) => "string",
            Value::Array(_) => "array",
            Value::Object(_) => "object",
            Value::Untranslatable { .. } => "untranslatable",
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
            Value::Untranslatable { .. } => false,
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
                // Check for Untranslatable marker
                if obj.get(UNTRANSLATABLE_KEY) == Some(&serde_json::Value::Bool(true)) {
                    let article = obj
                        .get("article")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let construct = obj
                        .get("construct")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    return Value::Untranslatable { article, construct };
                }
                let map: BTreeMap<String, Value> =
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
                // Check for Untranslatable marker
                if obj.get(UNTRANSLATABLE_KEY) == Some(&serde_json::Value::Bool(true)) {
                    let article = obj
                        .get("article")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let construct = obj
                        .get("construct")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    return Value::Untranslatable { article, construct };
                }
                let map: BTreeMap<String, Value> = obj
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
            Value::Array(arr) => serde_json::Value::Array(arr.iter().map(Into::into).collect()),
            Value::Object(map) => {
                serde_json::Value::Object(map.iter().map(|(k, v)| (k.clone(), v.into())).collect())
            }
            Value::Untranslatable { article, construct } => {
                serde_json::json!({
                    UNTRANSLATABLE_KEY: true,
                    "article": article,
                    "construct": construct,
                })
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
            Value::Untranslatable { article, construct } => {
                serde_json::json!({
                    UNTRANSLATABLE_KEY: true,
                    "article": article,
                    "construct": construct,
                })
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
            Value::Untranslatable { article, construct } => {
                write!(f, "UNTRANSLATABLE(art. {}: {})", article, construct)
            }
        }
    }
}

/// Operation types supported by the engine
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Operation {
    // Comparison operations (5)
    Equals,
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

    // Logical operations (3)
    And,
    Or,
    Not,

    // Conditional operations (1)
    /// IF with cases/default syntax (formerly SWITCH)
    #[serde(alias = "SWITCH")]
    If,

    // Collection operations (2)
    In,
    List,

    // Date operations (4)
    Age,
    DateAdd,
    Date,
    DayOfWeek,

    // Engine-only compat aliases — accepted during deserialization but NOT in the
    // v0.5.0 schema operationType enum. YAML using these will execute correctly but
    // fail schema validation. New laws should use NOT + the positive operation instead.
    #[serde(rename = "NOT_EQUALS")]
    NotEquals,
    #[serde(rename = "IS_NULL")]
    IsNull,
    #[serde(rename = "NOT_NULL")]
    NotNull,
    #[serde(rename = "NOT_IN")]
    NotIn,
}

impl Operation {
    /// All operations that are part of the schema specification.
    /// Compat aliases (NOT_EQUALS, IS_NULL, NOT_NULL, NOT_IN) are excluded.
    ///
    /// When adding a new operation: add it here AND to a conformance level
    /// in `conformance/<version>/manifest.json`. CI will fail otherwise.
    pub const SCHEMA_OPERATIONS: &[Operation] = &[
        Operation::Equals,
        Operation::GreaterThan,
        Operation::LessThan,
        Operation::GreaterThanOrEqual,
        Operation::LessThanOrEqual,
        Operation::Add,
        Operation::Subtract,
        Operation::Multiply,
        Operation::Divide,
        Operation::Max,
        Operation::Min,
        Operation::And,
        Operation::Or,
        Operation::Not,
        Operation::If,
        Operation::In,
        Operation::List,
        Operation::Age,
        Operation::DateAdd,
        Operation::Date,
        Operation::DayOfWeek,
    ];

    /// Compat aliases accepted by the engine but not in the schema.
    /// These exist for backward compatibility with older YAML files.
    pub const COMPAT_ALIASES: &[Operation] = &[
        Operation::NotEquals,
        Operation::IsNull,
        Operation::NotNull,
        Operation::NotIn,
    ];

    /// All variants of the enum. This is a manually maintained list;
    /// forgetting to add a new variant here compiles fine, but the
    /// `operation_lists_are_exhaustive` test catches it by cross-checking
    /// this list against SCHEMA_OPERATIONS + COMPAT_ALIASES.
    ///
    /// When adding a new operation: add it here AND to SCHEMA_OPERATIONS
    /// or COMPAT_ALIASES.
    pub const ALL_VARIANTS: &[Operation] = &[
        Operation::Equals,
        Operation::GreaterThan,
        Operation::LessThan,
        Operation::GreaterThanOrEqual,
        Operation::LessThanOrEqual,
        Operation::Add,
        Operation::Subtract,
        Operation::Multiply,
        Operation::Divide,
        Operation::Max,
        Operation::Min,
        Operation::And,
        Operation::Or,
        Operation::Not,
        Operation::If,
        Operation::In,
        Operation::List,
        Operation::Age,
        Operation::DateAdd,
        Operation::Date,
        Operation::DayOfWeek,
        Operation::NotEquals,
        Operation::IsNull,
        Operation::NotNull,
        Operation::NotIn,
    ];

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
        matches!(self, Operation::And | Operation::Or | Operation::Not)
    }

    /// Check if this is a conditional operation
    pub fn is_conditional(&self) -> bool {
        matches!(self, Operation::If)
    }

    /// Check if this is a collection operation
    pub fn is_collection(&self) -> bool {
        matches!(self, Operation::In | Operation::List)
    }

    /// Check if this is a null-check operation
    pub fn is_null_check(&self) -> bool {
        matches!(self, Operation::IsNull | Operation::NotNull)
    }

    /// Get the operation name as a static uppercase string.
    ///
    /// Avoids per-invocation `format!("{:?}", op).to_uppercase()` allocations.
    pub fn name(&self) -> &'static str {
        match self {
            Operation::Equals => "EQUALS",
            Operation::GreaterThan => "GREATER_THAN",
            Operation::LessThan => "LESS_THAN",
            Operation::GreaterThanOrEqual => "GREATER_THAN_OR_EQUAL",
            Operation::LessThanOrEqual => "LESS_THAN_OR_EQUAL",
            Operation::Add => "ADD",
            Operation::Subtract => "SUBTRACT",
            Operation::Multiply => "MULTIPLY",
            Operation::Divide => "DIVIDE",
            Operation::Max => "MAX",
            Operation::Min => "MIN",
            Operation::And => "AND",
            Operation::Or => "OR",
            Operation::Not => "NOT",
            Operation::If => "IF",
            Operation::In => "IN",
            Operation::List => "LIST",
            Operation::Age => "AGE",
            Operation::DateAdd => "DATE_ADD",
            Operation::Date => "DATE",
            Operation::DayOfWeek => "DAY_OF_WEEK",
            Operation::NotEquals => "NOT_EQUALS",
            Operation::IsNull => "IS_NULL",
            Operation::NotNull => "NOT_NULL",
            Operation::NotIn => "NOT_IN",
        }
    }
}

/// How the engine handles articles with `untranslatables` annotations (RFC-012).
///
/// Controls runtime behavior when an article declares legal constructs that
/// cannot be faithfully expressed with the current engine operation set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum UntranslatableMode {
    /// Hard error on any unaccepted untranslatable. Accepted ones execute partial logic.
    #[default]
    Error,
    /// Execute partial logic. Outputs from articles with untranslatables carry an
    /// `UNTRANSLATABLE` taint that propagates through downstream operations (like NaN).
    Propagate,
    /// Execute partial logic, log warning in trace. No taint propagation.
    Warn,
    /// Execute partial logic silently. Only valid for entries with `accepted: true` —
    /// unaccepted untranslatables still error.
    Ignore,
}

impl std::str::FromStr for UntranslatableMode {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "error" => Ok(UntranslatableMode::Error),
            "propagate" => Ok(UntranslatableMode::Propagate),
            "warn" => Ok(UntranslatableMode::Warn),
            "ignore" => Ok(UntranslatableMode::Ignore),
            _ => Err(format!(
                "unknown untranslatable mode '{s}', expected: error, propagate, warn, ignore"
            )),
        }
    }
}

/// Engine connectivity mode — whether this engine resolves cross-law references.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Connectivity {
    /// Engine runs standalone, no cross-law resolution.
    Solo,
}

/// Legal status of execution results.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LegalStatus {
    /// Results are for simulation/testing purposes only.
    Simulation,
}

/// Re-export the canonical regulatory layer types from the shared crate.
pub use regelrecht_shared::RegulatoryLayer;

/// Parameter type specification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ParameterType {
    #[default]
    String,
    Number,
    Boolean,
    Amount,
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
    /// Cross-law reference resolution (source.regulation lookup)
    CrossLawReference,
    /// Article-level execution
    Article,
    /// Cached cross-law result (memoized)
    Cached,
    /// Open term resolution via IoC (implements lookup)
    OpenTermResolution,
    /// Hook resolution (lifecycle hook firing, RFC-007)
    HookResolution,
    /// Override resolution (lex specialis replacement, RFC-007)
    OverrideResolution,
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
    /// Value resolved via lifecycle hook (RFC-007)
    Hook,
    /// Value resolved via lex specialis override (RFC-007)
    Override,
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
        // Untranslatable is falsy
        assert!(!Value::Untranslatable {
            article: "1".into(),
            construct: "test".into(),
        }
        .to_bool());
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
        assert!(Operation::NotEquals.is_comparison());
        assert!(Operation::Add.is_arithmetic());
        assert!(Operation::Max.is_aggregate());
        assert!(Operation::And.is_logical());
        assert!(Operation::Not.is_logical());
        assert!(Operation::If.is_conditional());
        assert!(Operation::In.is_collection());
        assert!(Operation::List.is_collection());
        assert!(Operation::IsNull.is_null_check());
        assert!(Operation::NotNull.is_null_check());
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
    fn test_untranslatable_equality() {
        let a = Value::Untranslatable {
            article: "1".into(),
            construct: "rounding".into(),
        };
        let b = Value::Untranslatable {
            article: "2".into(),
            construct: "aggregation".into(),
        };
        // Two Untranslatable values are always equal (like NaN)
        assert_eq!(a, b);
        // Untranslatable != other types
        assert_ne!(a, Value::Null);
        assert_ne!(a, Value::Int(0));
    }

    #[test]
    fn test_untranslatable_serde_roundtrip() {
        let value = Value::Untranslatable {
            article: "2".into(),
            construct: "afronden op hele euro's".into(),
        };
        let json = serde_json::to_string(&value).unwrap();
        assert!(json.contains("__untranslatable"));
        let parsed: Value = serde_json::from_str(&json).unwrap();
        assert!(parsed.is_untranslatable());
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

    #[test]
    fn operation_lists_are_exhaustive() {
        // ALL_VARIANTS must contain every variant. We verify this by
        // checking that ALL_VARIANTS and SCHEMA_OPERATIONS + COMPAT_ALIASES
        // contain the exact same set of operations (by name).
        let all_names: std::collections::HashSet<&str> =
            Operation::ALL_VARIANTS.iter().map(|op| op.name()).collect();
        assert_eq!(
            all_names.len(),
            Operation::ALL_VARIANTS.len(),
            "ALL_VARIANTS contains duplicates"
        );

        let classified_names: std::collections::HashSet<&str> = Operation::SCHEMA_OPERATIONS
            .iter()
            .chain(Operation::COMPAT_ALIASES.iter())
            .map(|op| op.name())
            .collect();
        assert_eq!(
            classified_names.len(),
            Operation::SCHEMA_OPERATIONS.len() + Operation::COMPAT_ALIASES.len(),
            "SCHEMA_OPERATIONS and COMPAT_ALIASES overlap"
        );

        assert_eq!(
            all_names,
            classified_names,
            "ALL_VARIANTS and SCHEMA_OPERATIONS + COMPAT_ALIASES differ.\n\
             In ALL_VARIANTS but not classified: {:?}\n\
             Classified but not in ALL_VARIANTS: {:?}",
            all_names.difference(&classified_names).collect::<Vec<_>>(),
            classified_names.difference(&all_names).collect::<Vec<_>>()
        );
    }
}
