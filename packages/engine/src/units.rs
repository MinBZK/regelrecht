//! Unit-of-measurement model and algebra (RFC-023).
//!
//! Single source of truth for unit rules, used by both the static validator
//! (`check_law`) and the runtime engine (`infer_unit` in `evaluate_action`).
//!
//! Cornerstone: `Unit::Unknown` never triggers a check. Laws without unit
//! annotations behave exactly as before — annotation is per-law opt-in.
//!
//! A unit is a *label*, never a computational constraint: inferring units never
//! changes a value, it only rejects nonsensical combinations (e.g. eurocent +
//! days). Whether a `percentage` is divided by 100 is a value concern expressed
//! by an explicit `DIVIDE … 100`, never implied by the label.

use crate::article::{ActionOperation, ActionValue, Article};
use crate::error::EngineError;
use crate::types::Value;
use std::collections::BTreeMap;

/// A unit of measurement. `Unknown` means "no declared unit" and never errors.
///
/// The set mirrors the schema `type_spec.unit` enum (RFC-023): money (`euro`,
/// `eurocent`), dimensionless scalars (`ratio`, `percentage`) and durations
/// (`years`, `months`, `weeks`, `days`).
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
            _ => Unit::Unknown,
        }
    }

    fn dimension(self) -> Option<Dimension> {
        match self {
            Unit::Euro | Unit::Eurocent => Some(Dimension::Money),
            Unit::Years | Unit::Months | Unit::Weeks | Unit::Days => Some(Dimension::Time),
            Unit::Ratio => Some(Dimension::Ratio),
            Unit::Percentage => Some(Dimension::Percentage),
            Unit::Unknown => None,
        }
    }

    /// A dimensionless scalar multiplier preserves the other operand's unit.
    ///
    /// `ratio` and `percentage` are both dimensionless: multiplying an amount by
    /// either yields the amount's unit. Whether a `percentage` value is later
    /// divided by 100 is a value concern, not a unit concern, so
    /// `eurocent × percentage` is unit-valid (it does NOT mean "forgot /100").
    ///
    /// Two scalars multiplied together stay a scalar (e.g. `percentage ×
    /// percentage → percentage`): we do not track compound units.
    fn is_scalar(self) -> bool {
        matches!(self, Unit::Ratio | Unit::Percentage)
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
/// Note this differs from [`fold_operands`], which treats `Unknown` as the fold
/// *identity* (skipping it) so a known unit keeps flowing across a unit-less
/// literal. For n-ary `+`/`-`/`×`/`÷` (and `IF` branches), prefer
/// [`fold_operands`]; call `combine` directly only for genuinely binary
/// operations (e.g. the `subject`/`value` pair of a comparison).
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

    /// Whether any symbol in this article declares a (known) unit. Used to skip
    /// the unit check entirely for un-annotated articles.
    pub fn has_any_unit(&self) -> bool {
        self.units.values().any(|u| *u != Unit::Unknown)
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
    pub fn from_article(article: &Article) -> Self {
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
        ActionValue::Operation(op) => infer_operation_unit(op, symbols),
    }
}

fn literal_unit(v: &Value, symbols: &SymbolUnits) -> Unit {
    match v {
        Value::String(s) if s.starts_with('$') => symbols.lookup(s),
        _ => Unit::Unknown,
    }
}

/// Fold `combine` left-to-right over a sequence of operands.
///
/// `Unknown` is the fold identity: a unit-less literal (e.g. `100`) is neutral
/// and must not force the result to Unknown, so a known unit keeps flowing and
/// a later incompatible operand is still caught.
fn fold_operands<'a>(
    op: AlgebraOp,
    op_name: &str,
    operands: impl IntoIterator<Item = &'a ActionValue>,
    symbols: &SymbolUnits,
) -> Result<Unit, EngineError> {
    let mut acc: Option<Unit> = None;
    for operand in operands {
        let u = infer_unit(operand, symbols)?;
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

/// Infer the unit of an [`ActionOperation`] directly. The runtime check in
/// `evaluate_action` already holds an `&ActionOperation`, so this entry point
/// lets it avoid cloning the operation AST.
pub fn infer_operation_unit(
    op: &ActionOperation,
    symbols: &SymbolUnits,
) -> Result<Unit, EngineError> {
    use ActionOperation::*;
    let name = op.operation_name();
    match op {
        // Additive
        Add { values } | Subtract { values } | Max { values } | Min { values } => {
            fold_operands(AlgebraOp::Additive, name, values, symbols)
        }
        Multiply { values } => fold_operands(AlgebraOp::Multiply, name, values, symbols),
        Divide { values } => fold_operands(AlgebraOp::Divide, name, values, symbols),

        // Rounding preserves the operand's unit (rounding a eurocent stays eurocent).
        // `precision` is a plain integer (decimal places), it carries no unit.
        Round { value, .. } | Ceil { value, .. } | Floor { value, .. } => {
            infer_unit(value, symbols)
        }

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
            for case in cases {
                infer_unit(&case.when, symbols)?; // check condition subtree
            }
            let branches = cases.iter().map(|c| &c.then).chain(default.as_ref());
            fold_operands(AlgebraOp::Additive, name, branches, symbols)
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

        // Null / collection / date → no numeric unit; recurse for nested arithmetic.
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
            // IN is "equals any of", so the subject must share the elements' unit
            // (like the comparison operators) — not just be internally consistent.
            let subject_unit = infer_unit(subject, symbols)?;
            if let Some(v) = value {
                combine(
                    AlgebraOp::Comparison,
                    name,
                    subject_unit,
                    infer_unit(v, symbols)?,
                )?;
            }
            if let Some(vs) = values {
                for elem in vs {
                    combine(
                        AlgebraOp::Comparison,
                        name,
                        subject_unit,
                        infer_unit(elem, symbols)?,
                    )?;
                }
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
        DateDiff { from, to, unit } => {
            check_children(&[from, to, unit], symbols)?;
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
/// Limitation (RFC-023): only actions expressed via a `value` expression are
/// walked here. Actions that use an inline action-level `operation` are covered
/// by the runtime check in `evaluate_action` instead. Most corpus actions use
/// the inline form, so this static pass examines only a fraction of annotated
/// actions — a "0 FAIL" result is not a guarantee that every annotated action
/// was statically unit-checked. Extending the static pass to the inline form is
/// follow-up.
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

        // Warnings: amount outputs without a unit — but only for articles that
        // have *already opted into* unit annotations. A fully un-annotated
        // article stays silent (the opt-in cornerstone), so `just validate`
        // isn't flooded with warnings about laws nobody has started annotating.
        if symbols.has_any_unit() {
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
    }

    findings
}

#[cfg(test)]
mod tests {
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
        // euro vs eurocent — the original factor-of-100 concern
        assert!(combine(AlgebraOp::Additive, "ADD", Unit::Euro, Unit::Eurocent).is_err());
    }

    #[test]
    fn multiply_by_scalar_preserves_unit() {
        assert_eq!(
            combine(AlgebraOp::Multiply, "MULTIPLY", Unit::Eurocent, Unit::Ratio).unwrap(),
            Unit::Eurocent
        );
        // amount × percentage preserves the amount unit (the `amount × pct / 100`
        // idiom in real law). Whether /100 is applied is a value concern.
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
    fn divide_same_dimension_is_ratio() {
        assert_eq!(
            combine(AlgebraOp::Divide, "DIVIDE", Unit::Eurocent, Unit::Eurocent).unwrap(),
            Unit::Ratio
        );
        assert_eq!(
            combine(AlgebraOp::Divide, "DIVIDE", Unit::Eurocent, Unit::Ratio).unwrap(),
            Unit::Eurocent
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
            Unit::Euro
        )
        .is_err());
    }

    fn lit(v: impl Into<Value>) -> ActionValue {
        ActionValue::Literal(v.into())
    }
    fn var(name: &str) -> ActionValue {
        ActionValue::Literal(Value::String(format!("${name}")))
    }

    fn symbols() -> SymbolUnits {
        let mut su = SymbolUnits::new();
        su.insert("inkomen", Unit::Eurocent);
        su.insert("premie", Unit::Eurocent);
        su.insert("dagen", Unit::Days);
        su.insert("tarief", Unit::Percentage);
        su
    }

    #[test]
    fn infer_add_eurocent_with_unitless_literal_ok() {
        // eurocent + 100 (unit-less) stays eurocent
        let op = ActionOperation::Add {
            values: vec![var("inkomen"), lit(100i64)],
        };
        assert_eq!(
            infer_operation_unit(&op, &symbols()).unwrap(),
            Unit::Eurocent
        );
    }

    #[test]
    fn infer_add_mismatch_errors() {
        let op = ActionOperation::Add {
            values: vec![var("inkomen"), var("dagen")],
        };
        assert!(matches!(
            infer_operation_unit(&op, &symbols()),
            Err(EngineError::UnitMismatch { .. })
        ));
    }

    #[test]
    fn infer_eurocent_times_percentage_is_eurocent() {
        let op = ActionOperation::Multiply {
            values: vec![var("inkomen"), var("tarief")],
        };
        assert_eq!(
            infer_operation_unit(&op, &symbols()).unwrap(),
            Unit::Eurocent
        );
    }

    #[test]
    fn infer_round_preserves_unit() {
        // ROUND(premie) stays eurocent
        let op = ActionOperation::Round {
            value: var("premie"),
            precision: 0,
        };
        assert_eq!(
            infer_operation_unit(&op, &symbols()).unwrap(),
            Unit::Eurocent
        );
    }

    #[test]
    fn infer_nested_mismatch_inside_round_is_caught() {
        // ROUND(inkomen + dagen) → mismatch surfaces from the nested ADD
        let inner = ActionValue::Operation(Box::new(ActionOperation::Add {
            values: vec![var("inkomen"), var("dagen")],
        }));
        let op = ActionOperation::Round {
            value: inner,
            precision: 0,
        };
        assert!(matches!(
            infer_operation_unit(&op, &symbols()),
            Err(EngineError::UnitMismatch { .. })
        ));
    }

    #[test]
    fn infer_in_subject_element_unit_mismatch_errors() {
        // $inkomen (eurocent) IN [$dagen (days)] — same requirement as EQUALS
        let op = ActionOperation::In {
            subject: var("inkomen"),
            value: None,
            values: Some(vec![var("dagen")]),
        };
        assert!(matches!(
            infer_operation_unit(&op, &symbols()),
            Err(EngineError::UnitMismatch { .. })
        ));
    }
}
