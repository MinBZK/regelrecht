//! Value conversion helpers for Gherkin data tables
//!
//! Converts string values from feature files to engine Value types.

use regelrecht_engine::Value;
use rust_decimal::Decimal;

/// Convert a Gherkin table cell value to an engine Value.
///
/// Supports:
/// - `true` / `false` -> Bool
/// - `null` -> Null
/// - Integer literals -> Int
/// - Decimal literals -> Decimal
/// - Everything else -> String
pub fn convert_gherkin_value(val: &str) -> Value {
    let trimmed = val.trim();

    // Boolean
    if trimmed == "true" {
        return Value::Bool(true);
    }
    if trimmed == "false" {
        return Value::Bool(false);
    }

    // Null
    if trimmed == "null" || trimmed.is_empty() {
        return Value::Null;
    }

    // Try integer first
    if let Ok(i) = trimmed.parse::<i64>() {
        return Value::Int(i);
    }

    // Try decimal
    if let Ok(d) = trimmed.parse::<Decimal>() {
        return Value::Decimal(d);
    }

    // Default to string
    Value::String(trimmed.to_string())
}

/// Compare two Values with a small numeric tolerance.
///
/// For numeric values (Int/Decimal), uses an exact-decimal tolerance of 1e-9 to
/// absorb any literal-parsing noise; other types use exact equality. The
/// comparison stays in `Decimal` (no f64 round-trip) so it doesn't reintroduce
/// the float imprecision the engine deliberately avoids.
#[allow(dead_code)]
pub fn values_equal_with_tolerance(a: &Value, b: &Value) -> bool {
    match (a.as_decimal(), b.as_decimal()) {
        // Decimal::new(1, 9) == 1e-9
        (Some(da), Some(db)) => (da - db).abs() < Decimal::new(1, 9),
        _ => a == b,
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use super::{convert_gherkin_value, values_equal_with_tolerance};
    use regelrecht_engine::Value;
    use rust_decimal_macros::dec;

    #[test]
    fn test_convert_bool() {
        assert_eq!(convert_gherkin_value("true"), Value::Bool(true));
        assert_eq!(convert_gherkin_value("false"), Value::Bool(false));
        assert_eq!(convert_gherkin_value(" true "), Value::Bool(true));
    }

    #[test]
    fn test_convert_null() {
        assert_eq!(convert_gherkin_value("null"), Value::Null);
        assert_eq!(convert_gherkin_value(""), Value::Null);
    }

    #[test]
    fn test_convert_int() {
        assert_eq!(convert_gherkin_value("42"), Value::Int(42));
        assert_eq!(convert_gherkin_value("-10"), Value::Int(-10));
        assert_eq!(convert_gherkin_value("0"), Value::Int(0));
    }

    #[test]
    fn test_convert_float() {
        assert_eq!(convert_gherkin_value("3.14"), Value::Decimal(dec!(3.14)));
        assert_eq!(convert_gherkin_value("-1.5"), Value::Decimal(dec!(-1.5)));
        assert_eq!(convert_gherkin_value("0.5"), Value::Decimal(dec!(0.5)));
    }

    #[test]
    fn test_convert_string() {
        assert_eq!(
            convert_gherkin_value("GM0384"),
            Value::String("GM0384".to_string())
        );
        assert_eq!(
            convert_gherkin_value("hello world"),
            Value::String("hello world".to_string())
        );
    }

    #[test]
    fn test_values_equal_with_tolerance() {
        // Exact int match
        assert!(values_equal_with_tolerance(
            &Value::Int(100),
            &Value::Int(100)
        ));

        // Decimal tolerance
        assert!(values_equal_with_tolerance(
            &Value::Decimal(dec!(1.0)),
            &Value::Decimal(dec!(1.0000000001))
        ));

        // Int vs Decimal
        assert!(values_equal_with_tolerance(
            &Value::Int(100),
            &Value::Decimal(dec!(100.0))
        ));

        // Different values
        assert!(!values_equal_with_tolerance(
            &Value::Int(100),
            &Value::Int(101)
        ));
    }
}
