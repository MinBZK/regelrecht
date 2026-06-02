//! Unit-of-measurement model and algebra (RFC-019).
//!
//! Single source of truth for unit rules, used by both the static validator
//! (`check_law`) and the runtime engine (`infer_unit` in `evaluate_action`).
//!
//! Cornerstone: `Unit::Unknown` never triggers a check. Laws without unit
//! annotations behave exactly as before.

use crate::article::{ActionOperation, ActionValue};
use crate::error::EngineError;
use crate::types::Value;
use std::collections::BTreeMap;

/// A unit of measurement. `Unknown` means "no declared unit" and never errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unit {
    Euro,
    Eurocent,
    Meter,
    Centimeter,
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
    Length,
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
            Some("meter") => Unit::Meter,
            Some("centimeter") => Unit::Centimeter,
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
            Unit::Meter | Unit::Centimeter => Some(Dimension::Length),
            Unit::Years | Unit::Months | Unit::Weeks | Unit::Days => Some(Dimension::Time),
            Unit::Ratio => Some(Dimension::Ratio),
            Unit::Percentage => Some(Dimension::Percentage),
            Unit::Count => Some(Dimension::Count),
            Unit::Unknown => None,
        }
    }

    /// A dimensionless scalar multiplier preserves the other operand's unit.
    ///
    /// `ratio`, `percentage`, and `count` are all dimensionless: multiplying an
    /// amount by any of them yields the amount's unit. Whether a `percentage`
    /// value is later divided by 100 is a value concern, not a unit concern, so
    /// `eurocent × percentage` is unit-valid (it does NOT mean "forgot /100").
    fn is_scalar(self) -> bool {
        matches!(self, Unit::Ratio | Unit::Percentage | Unit::Count)
    }

    fn label(self) -> &'static str {
        match self {
            Unit::Euro => "euro",
            Unit::Eurocent => "eurocent",
            Unit::Meter => "meter",
            Unit::Centimeter => "centimeter",
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

/// Per-article table mapping symbol name → declared unit.
#[derive(Debug, Clone, Default)]
pub struct SymbolUnits {
    units: BTreeMap<String, Unit>,
}

impl SymbolUnits {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, name: &str, unit: Unit) {
        self.units.insert(name.to_string(), unit);
    }

    /// Look up the unit for a `$var` reference. Strips the leading `$` and any
    /// path suffix (`$foo.bar` → `foo`). Unknown symbol → `Unit::Unknown`.
    pub fn lookup(&self, var_ref: &str) -> Unit {
        let name = var_ref.trim_start_matches('$');
        let head = name.split('.').next().unwrap_or(name);
        self.units.get(head).copied().unwrap_or(Unit::Unknown)
    }

    /// Build the symbol table for one article from its declared units:
    /// definitions, inputs, and outputs.
    pub fn from_article(article: &crate::article::Article) -> Self {
        let mut su = SymbolUnits::new();
        if let Some(defs) = article.get_definitions() {
            for (name, def) in defs {
                su.insert(name, Unit::parse(def.unit()));
            }
        }
        if let Some(exec) = article.get_execution_spec() {
            if let Some(inputs) = &exec.input {
                for input in inputs {
                    let unit = input.type_spec.as_ref().and_then(|t| t.unit.as_deref());
                    su.insert(&input.name, Unit::parse(unit));
                }
            }
            if let Some(outputs) = &exec.output {
                for output in outputs {
                    let unit = output.type_spec.as_ref().and_then(|t| t.unit.as_deref());
                    su.insert(&output.name, Unit::parse(unit));
                }
            }
        }
        su
    }
}

/// Infer the unit of an expression, recursively, applying `combine` at every
/// arithmetic/comparison node. Value-independent: visits *all* branches of an
/// `IF`. Returns the result unit, or the first `UnitMismatch` encountered.
///
/// Operations whose result has no numeric unit (logical, null-checks, dates,
/// lists) still recurse into their children so nested arithmetic is checked;
/// they return `Ok(Unit::Unknown)`.
pub fn infer_unit(expr: &ActionValue, symbols: &SymbolUnits) -> Result<Unit, EngineError> {
    match expr {
        ActionValue::Literal(v) => Ok(literal_unit(v, symbols)),
        ActionValue::Operation(op) => infer_operation(op, symbols),
    }
}

fn literal_unit(v: &Value, symbols: &SymbolUnits) -> Unit {
    match v {
        Value::String(s) if s.starts_with('$') => symbols.lookup(s),
        _ => Unit::Unknown,
    }
}

/// Fold `combine` left-to-right over a list of operands.
fn fold_operands(
    op: AlgebraOp,
    op_name: &str,
    operands: &[ActionValue],
    symbols: &SymbolUnits,
) -> Result<Unit, EngineError> {
    let mut acc: Option<Unit> = None;
    for operand in operands {
        let u = infer_unit(operand, symbols)?;
        // Unknown is the fold identity: a unit-less literal (e.g. `100`) is
        // neutral and must not force the result to Unknown, so that a known
        // unit keeps flowing and a later incompatible operand is still caught.
        if u == Unit::Unknown {
            continue;
        }
        acc = Some(match acc {
            None => u,
            Some(prev) => combine(op, op_name, prev, u)?,
        });
    }
    Ok(acc.unwrap_or(Unit::Unknown))
}

/// Recurse into children for side-effect checking; ignore their result unit.
fn check_children(children: &[&ActionValue], symbols: &SymbolUnits) -> Result<(), EngineError> {
    for c in children {
        infer_unit(c, symbols)?;
    }
    Ok(())
}

fn infer_operation(op: &ActionOperation, symbols: &SymbolUnits) -> Result<Unit, EngineError> {
    use ActionOperation::*;
    let name = op.operation_name();
    match op {
        // Additive
        Add { values } | Subtract { values } | Max { values } | Min { values } => {
            fold_operands(AlgebraOp::Additive, name, values, symbols)
        }
        Multiply { values } => fold_operands(AlgebraOp::Multiply, name, values, symbols),
        Divide { values } => fold_operands(AlgebraOp::Divide, name, values, symbols),

        // Comparisons (subject + value) → boolean
        Equals { subject, value }
        | NotEquals { subject, value }
        | GreaterThan { subject, value }
        | LessThan { subject, value }
        | GreaterThanOrEqual { subject, value }
        | LessThanOrEqual { subject, value } => {
            let l = infer_unit(subject, symbols)?;
            let r = infer_unit(value, symbols)?;
            combine(AlgebraOp::Comparison, name, l, r)
        }

        // IF: all then/default branches must agree (Additive rule); conditions checked too.
        If { cases, default } => {
            let mut branches: Vec<ActionValue> = Vec::new();
            for case in cases {
                infer_unit(&case.when, symbols)?; // check condition subtree
                branches.push(case.then.clone());
            }
            if let Some(d) = default {
                branches.push(d.clone());
            }
            fold_operands(AlgebraOp::Additive, name, &branches, symbols)
        }

        // Logical → boolean; recurse into operands.
        And { conditions } | Or { conditions } => {
            let refs: Vec<&ActionValue> = conditions.iter().collect();
            check_children(&refs, symbols)?;
            Ok(Unit::Unknown)
        }
        Not { value } => {
            infer_unit(value, symbols)?;
            Ok(Unit::Unknown)
        }

        // Null / collection / date / list → no numeric unit; recurse for nested arithmetic.
        IsNull { subject } | NotNull { subject } | DayOfWeek { date: subject } => {
            infer_unit(subject, symbols)?;
            Ok(Unit::Unknown)
        }
        In {
            subject,
            value,
            values,
        }
        | NotIn {
            subject,
            value,
            values,
        } => {
            infer_unit(subject, symbols)?;
            if let Some(v) = value {
                infer_unit(v, symbols)?;
            }
            if let Some(vs) = values {
                let refs: Vec<&ActionValue> = vs.iter().collect();
                check_children(&refs, symbols)?;
            }
            Ok(Unit::Unknown)
        }
        List { items } => {
            let refs: Vec<&ActionValue> = items.iter().collect();
            check_children(&refs, symbols)?;
            Ok(Unit::Unknown)
        }
        Age {
            date_of_birth,
            reference_date,
        } => {
            infer_unit(date_of_birth, symbols)?;
            infer_unit(reference_date, symbols)?;
            Ok(Unit::Years)
        }
        Date { year, month, day } => {
            check_children(&[year, month, day], symbols)?;
            Ok(Unit::Unknown)
        }
        DateAdd {
            date,
            years,
            months,
            weeks,
            days,
        } => {
            infer_unit(date, symbols)?;
            for v in [years, months, weeks, days].into_iter().flatten() {
                infer_unit(v, symbols)?;
            }
            Ok(Unit::Unknown)
        }
    }
}

/// A finding from static unit-checking.
#[derive(Debug, Clone)]
pub struct UnitFinding {
    pub article: String,
    pub output: String,
    /// true = hard error (known incompatible units); false = warning (missing unit).
    pub is_error: bool,
    pub message: String,
}

/// Statically check every article's actions for unit mismatches, and warn on
/// `amount`-typed outputs that declare no unit. Never executes the law.
///
/// Limitation (RFC-019): only actions expressed via a `value` expression are
/// walked here. Actions that use an inline action-level `operation` are covered
/// by the runtime check in `evaluate_action` instead.
pub fn check_law(law: &crate::article::ArticleBasedLaw) -> Vec<UnitFinding> {
    use crate::types::ParameterType;
    let mut findings = Vec::new();

    for article in &law.articles {
        let symbols = SymbolUnits::from_article(article);
        let article_no = article.number.clone();

        let Some(exec) = article.get_execution_spec() else {
            continue;
        };

        // Hard errors: walk each action's `value` expression.
        if let Some(actions) = &exec.actions {
            for action in actions {
                let output = action.output.clone().unwrap_or_default();
                if let Some(expr) = &action.value {
                    if let Err(e) = infer_unit(expr, &symbols) {
                        findings.push(UnitFinding {
                            article: article_no.clone(),
                            output,
                            is_error: true,
                            message: e.to_string(),
                        });
                    }
                }
            }
        }

        // Warnings: amount outputs without a unit.
        if let Some(outputs) = &exec.output {
            for o in outputs {
                let has_unit = o
                    .type_spec
                    .as_ref()
                    .and_then(|t| t.unit.as_deref())
                    .is_some();
                if matches!(o.output_type, ParameterType::Amount) && !has_unit {
                    findings.push(UnitFinding {
                        article: article_no.clone(),
                        output: o.name.clone(),
                        is_error: false,
                        message: format!("amount output '{}' has no unit", o.name),
                    });
                }
            }
        }
    }

    findings
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
        // amount × percentage preserves the amount unit: percentage is a
        // dimensionless scalar (the `amount × pct / 100` idiom in real law,
        // e.g. participatiewet art. 18). Whether /100 is applied is a value
        // concern, not a unit concern.
        assert_eq!(
            combine(
                AlgebraOp::Multiply,
                "MULTIPLY",
                Unit::Eurocent,
                Unit::Percentage
            )
            .unwrap(),
            Unit::Eurocent
        );
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

    #[test]
    fn length_units_are_a_dimension() {
        // meter + meter = meter
        assert_eq!(
            combine(AlgebraOp::Additive, "ADD", Unit::Meter, Unit::Meter).unwrap(),
            Unit::Meter
        );
        // meter + centimeter = error (the m-vs-cm mix-up, like euro vs eurocent)
        assert!(combine(AlgebraOp::Additive, "ADD", Unit::Meter, Unit::Centimeter).is_err());
        // length ÷ length = ratio (same dimension)
        assert_eq!(
            combine(
                AlgebraOp::Divide,
                "DIVIDE",
                Unit::Centimeter,
                Unit::Centimeter
            )
            .unwrap(),
            Unit::Ratio
        );
        // length × scalar = length
        assert_eq!(
            combine(
                AlgebraOp::Multiply,
                "MULTIPLY",
                Unit::Centimeter,
                Unit::Count
            )
            .unwrap(),
            Unit::Centimeter
        );
        // length combined with another dimension = error
        assert!(combine(AlgebraOp::Additive, "ADD", Unit::Meter, Unit::Eurocent).is_err());
    }
}

#[cfg(test)]
mod infer_tests {
    use super::*;

    fn symbols() -> SymbolUnits {
        let mut su = SymbolUnits::new();
        su.insert("inkomen", Unit::Eurocent);
        su.insert("premie", Unit::Eurocent);
        su.insert("looptijd", Unit::Days);
        su.insert("percentage", Unit::Ratio);
        su
    }

    fn parse_op(yaml: &str) -> ActionValue {
        serde_yaml_ng::from_str(yaml).expect("valid ActionValue")
    }

    #[test]
    fn add_same_unit_ok() {
        let expr = parse_op("operation: ADD\nvalues: ['$inkomen', '$premie']");
        assert_eq!(infer_unit(&expr, &symbols()).unwrap(), Unit::Eurocent);
    }

    #[test]
    fn add_mixed_units_errors() {
        let expr = parse_op("operation: ADD\nvalues: ['$inkomen', '$looptijd']");
        assert!(infer_unit(&expr, &symbols()).is_err());
    }

    #[test]
    fn amount_times_ratio_ok() {
        let expr = parse_op("operation: MULTIPLY\nvalues: ['$inkomen', '$percentage']");
        assert_eq!(infer_unit(&expr, &symbols()).unwrap(), Unit::Eurocent);
    }

    #[test]
    fn nested_mismatch_inside_if_is_caught() {
        // IF whose `then` branch hides an illegal eurocent + days add
        let expr = parse_op(
            "operation: IF\ncases:\n  - when:\n      operation: GREATER_THAN\n      subject: '$inkomen'\n      value: '$premie'\n    then:\n      operation: ADD\n      values: ['$inkomen', '$looptijd']\ndefault: '$premie'",
        );
        assert!(infer_unit(&expr, &symbols()).is_err());
    }

    #[test]
    fn literal_numbers_are_unknown_and_never_error() {
        let expr = parse_op("operation: ADD\nvalues: [100, '$inkomen']");
        // 100 is Unknown → no error; result follows the known operand
        assert_eq!(infer_unit(&expr, &symbols()).unwrap(), Unit::Eurocent);
    }
}

#[cfg(test)]
mod check_law_tests {
    use super::*;
    use crate::article::ArticleBasedLaw;

    #[test]
    fn flags_mixed_unit_add_in_value_expression() {
        let yaml = r#"
$id: t
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: "Mixed-unit add."
    machine_readable:
      execution:
        input:
          - name: bedrag
            type: amount
            type_spec: {unit: eurocent}
          - name: dagen
            type: number
            type_spec: {unit: days}
        output:
          - name: r
            type: amount
            type_spec: {unit: eurocent}
        actions:
          - output: r
            value:
              operation: ADD
              values: ['$bedrag', '$dagen']
"#;
        let law: ArticleBasedLaw = serde_yaml_ng::from_str(yaml).expect("parse");
        let findings = check_law(&law);
        assert!(findings.iter().any(|f| f.is_error));
    }

    #[test]
    fn warns_on_amount_output_without_unit() {
        let yaml = r#"
$id: t
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: "Amount without unit."
    machine_readable:
      execution:
        output:
          - name: r
            type: amount
        actions:
          - output: r
            value: 100
"#;
        let law: ArticleBasedLaw = serde_yaml_ng::from_str(yaml).expect("parse");
        let findings = check_law(&law);
        assert!(findings.iter().any(|f| !f.is_error && f.output == "r"));
    }

    #[test]
    fn clean_annotated_law_has_no_errors() {
        let yaml = r#"
$id: t
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: "Clean."
    machine_readable:
      definitions:
        rate:
          value: 0.5
          type: number
          type_spec: {unit: ratio}
      execution:
        input:
          - name: bedrag
            type: amount
            type_spec: {unit: eurocent}
        output:
          - name: r
            type: amount
            type_spec: {unit: eurocent}
        actions:
          - output: r
            value:
              operation: MULTIPLY
              values: ['$bedrag', '$rate']
"#;
        let law: ArticleBasedLaw = serde_yaml_ng::from_str(yaml).expect("parse");
        let findings = check_law(&law);
        assert!(
            !findings.iter().any(|f| f.is_error),
            "no errors expected: {:?}",
            findings
        );
    }
}
