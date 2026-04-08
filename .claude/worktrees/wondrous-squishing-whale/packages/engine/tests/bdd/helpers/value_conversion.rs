//! Value conversion helpers for Gherkin data tables
//!
//! Converts string values from feature files to engine Value types.

use regelrecht_engine::Value;

/// Convert a Gherkin table cell value to an engine Value.
///
/// Supports:
/// - `true` / `false` -> Bool
/// - `null` -> Null
/// - Integer literals -> Int
/// - Float literals -> Float
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

    // Try float
    if let Ok(f) = trimmed.parse::<f64>() {
        return Value::Float(f);
    }

    // Default to string
    Value::String(trimmed.to_string())
}

/// Parse a Gherkin data table row into a HashMap.
///
/// The table format is:
/// ```text
/// | key1 | value1 |
/// | key2 | value2 |
/// ```
pub fn parse_table_to_params(
    table: &cucumber::gherkin::Table,
) -> std::collections::HashMap<String, Value> {
    let mut params = std::collections::HashMap::new();

    for row in &table.rows {
        if row.len() >= 2 {
            let key = row[0].trim().to_string();
            let value = convert_gherkin_value(&row[1]);
            params.insert(key, value);
        }
    }

    params
}

/// Compare two Values with floating-point tolerance.
///
/// For Float values, uses a tolerance of 1e-9.
/// For Int values, exact equality is required.
#[allow(dead_code)]
pub fn values_equal_with_tolerance(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Float(fa), Value::Float(fb)) => (fa - fb).abs() < 1e-9,
        (Value::Int(ia), Value::Float(fb)) => ((*ia as f64) - fb).abs() < 1e-9,
        (Value::Float(fa), Value::Int(ib)) => (fa - (*ib as f64)).abs() < 1e-9,
        _ => a == b,
    }
}

/// Convert eurocent string to numeric value for comparison.
///
/// Handles both integer (eurocent) and float (euro) formats.
#[allow(dead_code)]
pub fn parse_eurocent(val: &str) -> Option<i64> {
    val.trim().parse::<i64>().ok()
}

/// Convert euro string to eurocent for comparison.
///
/// "1358.93" euro -> 135893 eurocent
#[allow(dead_code)]
pub fn parse_euro_to_eurocent(val: &str) -> Option<i64> {
    let trimmed = val.trim();
    let f: f64 = trimmed.parse().ok()?;
    Some((f * 100.0).round() as i64)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use super::{
        convert_gherkin_value, parse_euro_to_eurocent, parse_eurocent, values_equal_with_tolerance,
    };
    use regelrecht_engine::Value;

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
        assert_eq!(convert_gherkin_value("3.14"), Value::Float(3.14));
        assert_eq!(convert_gherkin_value("-1.5"), Value::Float(-1.5));
        assert_eq!(convert_gherkin_value("0.5"), Value::Float(0.5));
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

        // Float tolerance
        assert!(values_equal_with_tolerance(
            &Value::Float(1.0),
            &Value::Float(1.0 + 1e-10)
        ));

        // Int vs Float
        assert!(values_equal_with_tolerance(
            &Value::Int(100),
            &Value::Float(100.0)
        ));

        // Different values
        assert!(!values_equal_with_tolerance(
            &Value::Int(100),
            &Value::Int(101)
        ));
    }

    #[test]
    fn test_parse_eurocent() {
        assert_eq!(parse_eurocent("109171"), Some(109171));
        assert_eq!(parse_eurocent("0"), Some(0));
    }

    #[test]
    fn test_parse_euro_to_eurocent() {
        assert_eq!(parse_euro_to_eurocent("1358.93"), Some(135893));
        assert_eq!(parse_euro_to_eurocent("0.50"), Some(50));
    }
}
