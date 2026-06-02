//! Unit-of-measurement model and algebra (RFC-019).
//!
//! Single source of truth for unit rules, used by both the static validator
//! (`check_law`) and the runtime engine (`infer_unit` in `evaluate_action`).
//!
//! Cornerstone: `Unit::Unknown` never triggers a check. Laws without unit
//! annotations behave exactly as before.

use crate::error::EngineError;

/// A unit of measurement. `Unknown` means "no declared unit" and never errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unit {
    Euro,
    Eurocent,
    Years,
    Months,
    Weeks,
    Days,
    Ratio,
    Percentage,
    Count,
    Unknown,
}

/// The dimension a unit belongs to. Two units are dimension-compatible for
/// additive operations only if they are the *same* unit (not just the same
/// dimension) — euro and eurocent are both Money but must not be added.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Dimension {
    Money,
    Time,
    Ratio,
    Percentage,
    Count,
}

impl Unit {
    /// Parse a unit string. Unrecognized or absent → `Unknown`.
    pub fn parse(s: Option<&str>) -> Unit {
        match s {
            Some("euro") => Unit::Euro,
            Some("eurocent") => Unit::Eurocent,
            Some("years") => Unit::Years,
            Some("months") => Unit::Months,
            Some("weeks") => Unit::Weeks,
            Some("days") => Unit::Days,
            Some("ratio") => Unit::Ratio,
            Some("percentage") => Unit::Percentage,
            Some("count") => Unit::Count,
            _ => Unit::Unknown,
        }
    }

    fn dimension(self) -> Option<Dimension> {
        match self {
            Unit::Euro | Unit::Eurocent => Some(Dimension::Money),
            Unit::Years | Unit::Months | Unit::Weeks | Unit::Days => Some(Dimension::Time),
            Unit::Ratio => Some(Dimension::Ratio),
            Unit::Percentage => Some(Dimension::Percentage),
            Unit::Count => Some(Dimension::Count),
            Unit::Unknown => None,
        }
    }

    /// A scalar multiplier (ratio or count) preserves the other operand's unit.
    fn is_scalar(self) -> bool {
        matches!(self, Unit::Ratio | Unit::Count)
    }

    fn label(self) -> &'static str {
        match self {
            Unit::Euro => "euro",
            Unit::Eurocent => "eurocent",
            Unit::Years => "years",
            Unit::Months => "months",
            Unit::Weeks => "weeks",
            Unit::Days => "days",
            Unit::Ratio => "ratio",
            Unit::Percentage => "percentage",
            Unit::Count => "count",
            Unit::Unknown => "unknown",
        }
    }
}

/// The class of operation, for unit-algebra purposes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlgebraOp {
    /// `+`, `-`, `MIN`, `MAX`, and the `then`/`default` branches of `IF`.
    Additive,
    /// Numeric/equality comparisons. Operands must share a unit; result is boolean (`Unknown`).
    Comparison,
    Multiply,
    Divide,
}

/// Combine two units under an operation. Returns the resulting unit, or an
/// `EngineError::UnitMismatch` if two *known* units are incompatible.
///
/// If either operand is `Unknown`, the result is `Unknown` and no error occurs.
pub fn combine(op: AlgebraOp, op_name: &str, lhs: Unit, rhs: Unit) -> Result<Unit, EngineError> {
    if lhs == Unit::Unknown || rhs == Unit::Unknown {
        return Ok(Unit::Unknown);
    }

    let mismatch = || EngineError::UnitMismatch {
        operation: op_name.to_string(),
        left: lhs.label().to_string(),
        right: rhs.label().to_string(),
    };

    match op {
        AlgebraOp::Additive => {
            if lhs == rhs {
                Ok(lhs)
            } else {
                Err(mismatch())
            }
        }
        AlgebraOp::Comparison => {
            if lhs == rhs {
                Ok(Unit::Unknown) // boolean result
            } else {
                Err(mismatch())
            }
        }
        AlgebraOp::Multiply => {
            if rhs.is_scalar() {
                Ok(lhs)
            } else if lhs.is_scalar() {
                Ok(rhs)
            } else {
                Err(mismatch())
            }
        }
        AlgebraOp::Divide => {
            if rhs.is_scalar() {
                Ok(lhs)
            } else if lhs.dimension() == rhs.dimension() {
                // e.g. money / money = ratio
                Ok(Unit::Ratio)
            } else {
                Err(mismatch())
            }
        }
    }
}

#[cfg(test)]
mod combine_tests {
    use super::*;

    #[test]
    fn unknown_never_errors() {
        assert_eq!(
            combine(AlgebraOp::Additive, "ADD", Unit::Eurocent, Unit::Unknown).unwrap(),
            Unit::Unknown
        );
        assert_eq!(
            combine(AlgebraOp::Additive, "ADD", Unit::Unknown, Unit::Days).unwrap(),
            Unit::Unknown
        );
    }

    #[test]
    fn additive_same_unit_ok_different_errors() {
        assert_eq!(
            combine(AlgebraOp::Additive, "ADD", Unit::Eurocent, Unit::Eurocent).unwrap(),
            Unit::Eurocent
        );
        assert!(combine(AlgebraOp::Additive, "ADD", Unit::Eurocent, Unit::Days).is_err());
        // euro vs eurocent — the original concern
        assert!(combine(AlgebraOp::Additive, "ADD", Unit::Euro, Unit::Eurocent).is_err());
    }

    #[test]
    fn multiply_by_scalar_preserves_unit() {
        assert_eq!(
            combine(AlgebraOp::Multiply, "MULTIPLY", Unit::Eurocent, Unit::Ratio).unwrap(),
            Unit::Eurocent
        );
        assert_eq!(
            combine(AlgebraOp::Multiply, "MULTIPLY", Unit::Count, Unit::Eurocent).unwrap(),
            Unit::Eurocent
        );
        // amount × percentage is suspicious (forgot /100) → error
        assert!(combine(
            AlgebraOp::Multiply,
            "MULTIPLY",
            Unit::Eurocent,
            Unit::Percentage
        )
        .is_err());
        // amount × amount is meaningless → error
        assert!(combine(
            AlgebraOp::Multiply,
            "MULTIPLY",
            Unit::Eurocent,
            Unit::Eurocent
        )
        .is_err());
    }

    #[test]
    fn divide_rules() {
        assert_eq!(
            combine(AlgebraOp::Divide, "DIVIDE", Unit::Eurocent, Unit::Count).unwrap(),
            Unit::Eurocent
        );
        assert_eq!(
            combine(AlgebraOp::Divide, "DIVIDE", Unit::Eurocent, Unit::Eurocent).unwrap(),
            Unit::Ratio
        );
        assert!(combine(AlgebraOp::Divide, "DIVIDE", Unit::Eurocent, Unit::Days).is_err());
    }

    #[test]
    fn comparison_requires_same_unit() {
        assert_eq!(
            combine(
                AlgebraOp::Comparison,
                "GREATER_THAN",
                Unit::Eurocent,
                Unit::Eurocent
            )
            .unwrap(),
            Unit::Unknown
        );
        assert!(combine(
            AlgebraOp::Comparison,
            "GREATER_THAN",
            Unit::Eurocent,
            Unit::Days
        )
        .is_err());
    }
}
