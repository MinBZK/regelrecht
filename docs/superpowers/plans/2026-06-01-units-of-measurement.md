# Units of Measurement on Values — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make units of measurement (euro/eurocent, ratio/percentage, time, count) a first-class property of values that is checked both statically (during `just validate`) and at runtime (engine hard-errors on incompatible arithmetic), without breaking existing unit-less laws.

**Architecture:** A single new module `packages/engine/src/units.rs` owns the unit model (`Unit`), the algebra (`combine`), the value-independent tree walker (`infer_unit`), the per-article symbol table (`SymbolUnits`), and a static entry point (`check_law`). The `validate` binary calls `check_law`; the engine's `evaluate_action` calls `infer_unit`. One source of truth, used by both paths. Backward-compatibility cornerstone: any value whose unit is `Unknown` never triggers a check.

**Tech Stack:** Rust (engine crate), serde, `thiserror`, JSON Schema (jsonschema crate), cucumber-rs (BDD), `just` task runner.

**Scope note (read first):** This plan implements checking against **locally declared** units only. Verifying a cross-law `source` input against the *source law's* output unit requires a multi-law registry view and is **explicit follow-up work** (noted in RFC-019 §4). Everything below works on a single law file / single article in isolation.

**Reference doc:** `docs/src/content/rfcs/rfc-019.md`

---

## File Structure

| File | Responsibility | Change |
|------|----------------|--------|
| `schema/v0.5.3/schema.json` | New schema version: structured `definitions`, extended unit enum, shared `typeSpec` | Create (copy of v0.5.2 + edits) |
| `schema/v0.5.3/annotation-schema.json` | Unchanged annotation schema for the new version | Create (copy) |
| `schema/latest` | Symlink → current version | Repoint to `v0.5.3` |
| `packages/engine/src/article.rs` | `Definition` gains `type_spec` + `unit()` accessor | Modify |
| `packages/engine/src/error.rs` | `EngineError::UnitMismatch` variant | Modify |
| `packages/engine/src/units.rs` | Unit model, `combine`, `infer_unit`, `SymbolUnits`, `check_law` | Create |
| `packages/engine/src/lib.rs` | Register `units` module | Modify |
| `packages/engine/src/engine.rs` | Runtime unit check in `evaluate_action` | Modify |
| `packages/engine/src/bin/validate.rs` | Register v0.5.3; add step 3 (unit check) | Modify |
| `features/units.feature` | BDD: runtime `UnitMismatch` on `eurocent + days` | Create |
| `features/...` step defs | Wire the BDD scenario if needed | Modify (only if no generic runner exists) |
| `corpus/regulation/nl/wet/wet_op_de_zorgtoeslag/2025-01-01.yaml` | Annotate definitions/inputs/outputs with units (reference) | Modify |

---

## Task 1: Create schema v0.5.3 (structured definitions + extended unit enum)

**Files:**
- Create: `schema/v0.5.3/schema.json` (copy of `schema/v0.5.2/schema.json`, then edit)
- Create: `schema/v0.5.3/annotation-schema.json` (copy of `schema/v0.5.2/annotation-schema.json`)
- Modify: `schema/latest` (symlink)

- [ ] **Step 1: Copy the v0.5.2 schema directory to v0.5.3**

```bash
cp -r schema/v0.5.2 schema/v0.5.3
```

- [ ] **Step 2: Bump the `$id` in the new schema**

In `schema/v0.5.3/schema.json`, line 3, change `v0.5.2` to `v0.5.3`:

```json
  "$id": "https://raw.githubusercontent.com/MinBZK/regelrecht/refs/heads/main/schema/v0.5.3/schema.json",
```

- [ ] **Step 3: Extract a shared `typeSpec` definition and extend the unit enum**

In `schema/v0.5.3/schema.json`, inside the top-level `"definitions": {` object (starts at line 405), add a new `typeSpec` definition as the first entry (right after the opening `"definitions": {` brace, before `"baseField"`):

```json
    "typeSpec": {
      "type": "object",
      "description": "Additional type specifications for numeric and temporal values",
      "properties": {
        "unit": {
          "type": "string",
          "description": "Unit of measurement for the value",
          "enum": [
            "eurocent",
            "euro",
            "ratio",
            "percentage",
            "count",
            "years",
            "weeks",
            "months",
            "days"
          ]
        },
        "precision": {
          "type": "number",
          "description": "Number of decimal places for numeric values",
          "minimum": 0
        },
        "min": {
          "type": "number",
          "description": "Minimum allowed value"
        },
        "max": {
          "type": "number",
          "description": "Maximum allowed value"
        }
      }
    },
```

- [ ] **Step 4: Point `baseField.type_spec` at the shared definition**

In `schema/v0.5.3/schema.json`, replace the inline `type_spec` object inside `baseField.properties` (the block at lines 434-463 in the original) with a `$ref`:

```json
        "type_spec": {
          "$ref": "#/definitions/typeSpec"
        },
```

- [ ] **Step 5: Make `definitions` (the constants block) structured**

In `schema/v0.5.3/schema.json`, find the law-level `definitions` property (the block `"definitions": { "type": "object", "description": "Definitions and constants", "additionalProperties": true }`, originally around line 1374). Replace it with:

```json
        "definitions": {
          "type": "object",
          "description": "Definitions and constants",
          "additionalProperties": {
            "oneOf": [
              {
                "type": ["number", "string", "boolean", "array"],
                "description": "Simple constant value (no unit)"
              },
              {
                "type": "object",
                "required": ["value"],
                "additionalProperties": false,
                "properties": {
                  "value": {
                    "description": "The constant value"
                  },
                  "type": {
                    "type": "string",
                    "enum": ["string", "number", "boolean", "amount", "object", "array", "date"]
                  },
                  "type_spec": { "$ref": "#/definitions/typeSpec" },
                  "description": { "type": "string" }
                }
              }
            ]
          }
        },
```

- [ ] **Step 6: Repoint the `latest` symlink**

```bash
rm schema/latest && ln -s v0.5.3 schema/latest
```

- [ ] **Step 7: Verify the new schema is valid JSON**

Run: `python3 -m json.tool schema/v0.5.3/schema.json > /dev/null && echo OK`
Expected: `OK`

- [ ] **Step 8: Commit**

```bash
git add schema/v0.5.3 schema/latest
git commit -m "feat(schema): v0.5.3 — structured definitions and extended unit enum"
```

---

## Task 2: Register v0.5.3 in the validate binary

**Files:**
- Modify: `packages/engine/src/bin/validate.rs` (`load_schemas` ~lines 13-46, `detect_version` ~lines 49-78)

- [ ] **Step 1: Embed the v0.5.3 schema**

In `packages/engine/src/bin/validate.rs`, inside `load_schemas()`, after the `v052` block, add:

```rust
    let v053: serde_json::Value =
        serde_json::from_str(include_str!("../../../../schema/v0.5.3/schema.json"))
            .map_err(|e| format!("invalid v0.5.3 schema JSON: {e}"))?;
```

Then after `schemas.insert("v0.5.2", v052);` add:

```rust
    schemas.insert("v0.5.3", v053);
```

- [ ] **Step 2: Detect the v0.5.3 version**

In `detect_version`, add a new first branch before the `v0.5.2` check:

```rust
    if schema_url.contains("v0.5.3") {
        Some("v0.5.3")
    } else if schema_url.contains("v0.5.2") {
```

(keep the rest of the `else if` chain unchanged)

- [ ] **Step 3: Verify it compiles**

Run: `cargo build --manifest-path packages/engine/Cargo.toml --features validate --bin validate`
Expected: builds successfully.

- [ ] **Step 4: Verify the existing corpus still validates**

Run: `just validate`
Expected: all files `OK` (no regressions — nothing references v0.5.3 yet).

- [ ] **Step 5: Commit**

```bash
git add packages/engine/src/bin/validate.rs
git commit -m "feat(validate): recognize schema v0.5.3"
```

---

## Task 3: `Definition` carries a unit

**Files:**
- Modify: `packages/engine/src/article.rs` (`Definition` enum ~lines 346-363)
- Test: inline `#[cfg(test)]` in `article.rs`

- [ ] **Step 1: Write the failing test**

Add to the existing `#[cfg(test)] mod tests` in `packages/engine/src/article.rs`:

```rust
    #[test]
    fn test_definition_with_unit() {
        let yaml = r#"
value: 3971900
type: amount
type_spec:
  unit: eurocent
"#;
        let def: Definition = serde_yaml_ng::from_str(yaml).expect("should parse");
        assert_eq!(def.value(), &Value::Int(3971900));
        assert_eq!(def.unit(), Some("eurocent"));
    }

    #[test]
    fn test_simple_definition_has_no_unit() {
        let def: Definition = serde_yaml_ng::from_str("123").expect("should parse");
        assert_eq!(def.unit(), None);
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test --manifest-path packages/engine/Cargo.toml definition_with_unit`
Expected: FAIL — no method `unit` on `Definition`, and `Structured` has no `type_spec`.

- [ ] **Step 3: Add `type_spec` to the `Structured` variant and a `unit()` accessor**

In `packages/engine/src/article.rs`, replace the `Definition` enum and impl (lines 346-363) with:

```rust
/// Definition value in definitions section
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Definition {
    /// Definition with explicit value field, optionally carrying type metadata
    Structured {
        value: Value,
        #[serde(default, rename = "type")]
        def_type: Option<ParameterType>,
        #[serde(default)]
        type_spec: Option<TypeSpec>,
    },
    /// Simple value (for backward compatibility)
    Simple(Value),
}

impl Definition {
    /// Get the value from this definition
    pub fn value(&self) -> &Value {
        match self {
            Definition::Structured { value, .. } => value,
            Definition::Simple(v) => v,
        }
    }

    /// Get the declared unit string, if any.
    pub fn unit(&self) -> Option<&str> {
        match self {
            Definition::Structured {
                type_spec: Some(ts),
                ..
            } => ts.unit.as_deref(),
            _ => None,
        }
    }
}
```

Note: `ParameterType` is already defined in `crate::types` and imported in `article.rs` (used by `Parameter`). If not in scope, add `use crate::types::ParameterType;` — verify the existing imports first.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test --manifest-path packages/engine/Cargo.toml definition`
Expected: PASS (both new tests, and existing `test_article_get_definitions` still green).

- [ ] **Step 5: Commit**

```bash
git add packages/engine/src/article.rs
git commit -m "feat(engine): Definition carries optional type_spec/unit"
```

---

## Task 4: `EngineError::UnitMismatch`

**Files:**
- Modify: `packages/engine/src/error.rs` (`EngineError` enum, add before `TracedError` ~line 122)

- [ ] **Step 1: Add the variant**

In `packages/engine/src/error.rs`, add this variant to `EngineError` (just before the `TracedError` variant):

```rust
    /// Two values with incompatible units were combined in an operation.
    #[error("Unit mismatch in {operation}: {left} vs {right}")]
    UnitMismatch {
        operation: String,
        left: String,
        right: String,
    },
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo build --manifest-path packages/engine/Cargo.toml`
Expected: builds successfully (no exhaustive matches on `EngineError` outside `error.rs` that would now be non-exhaustive; if any appear, the compiler will point to them — add a `UnitMismatch { .. }` arm forwarding like neighboring variants).

- [ ] **Step 3: Commit**

```bash
git add packages/engine/src/error.rs
git commit -m "feat(engine): add EngineError::UnitMismatch"
```

---

## Task 5: `units.rs` — the `Unit` model and `combine` algebra

**Files:**
- Create: `packages/engine/src/units.rs`
- Modify: `packages/engine/src/lib.rs` (add `pub mod units;`)

- [ ] **Step 1: Register the module**

In `packages/engine/src/lib.rs`, add alongside the other `pub mod` declarations:

```rust
pub mod units;
```

- [ ] **Step 2: Write the module with the `Unit` model, `AlgebraOp`, and `combine` — plus failing tests**

Create `packages/engine/src/units.rs`:

```rust
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
pub fn combine(
    op: AlgebraOp,
    op_name: &str,
    lhs: Unit,
    rhs: Unit,
) -> Result<Unit, EngineError> {
    if lhs == Unit::Unknown || rhs == Unit::Unknown {
        // Comparisons and the like still yield a boolean; represent as Unknown.
        return Ok(match op {
            AlgebraOp::Comparison => Unit::Unknown,
            _ => Unit::Unknown,
        });
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
        assert!(combine(AlgebraOp::Multiply, "MULTIPLY", Unit::Eurocent, Unit::Percentage).is_err());
        // amount × amount is meaningless → error
        assert!(combine(AlgebraOp::Multiply, "MULTIPLY", Unit::Eurocent, Unit::Eurocent).is_err());
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
            combine(AlgebraOp::Comparison, "GREATER_THAN", Unit::Eurocent, Unit::Eurocent).unwrap(),
            Unit::Unknown
        );
        assert!(combine(AlgebraOp::Comparison, "GREATER_THAN", Unit::Eurocent, Unit::Days).is_err());
    }
}
```

- [ ] **Step 3: Run the tests to verify they pass**

Run: `cargo test --manifest-path packages/engine/Cargo.toml combine_tests`
Expected: PASS (5 tests).

- [ ] **Step 4: Commit**

```bash
git add packages/engine/src/units.rs packages/engine/src/lib.rs
git commit -m "feat(engine): unit model and combine algebra (RFC-019)"
```

---

## Task 6: `units.rs` — `SymbolUnits` and `infer_unit`

**Files:**
- Modify: `packages/engine/src/units.rs` (append below `combine`)

- [ ] **Step 1: Write the failing tests**

Append to `packages/engine/src/units.rs`:

```rust
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
        In { subject, value, values } | NotIn { subject, value, values } => {
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
        Age { date_of_birth, reference_date } => {
            infer_unit(date_of_birth, symbols)?;
            infer_unit(reference_date, symbols)?;
            Ok(Unit::Years)
        }
        Date { year, month, day } => {
            check_children(&[year, month, day], symbols)?;
            Ok(Unit::Unknown)
        }
        DateAdd { date, years, months, weeks, days } => {
            infer_unit(date, symbols)?;
            for opt in [years, months, weeks, days] {
                if let Some(v) = opt {
                    infer_unit(v, symbols)?;
                }
            }
            Ok(Unit::Unknown)
        }
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
```

- [ ] **Step 2: Run the tests to verify they fail then pass**

Run: `cargo test --manifest-path packages/engine/Cargo.toml infer_tests`
Expected: compiles and PASS (5 tests). If `Article::get_definitions`/`get_execution_spec` signatures differ, adjust `from_article` to match (they are defined in `article.rs`).

- [ ] **Step 3: Commit**

```bash
git add packages/engine/src/units.rs
git commit -m "feat(engine): SymbolUnits and infer_unit tree walker (RFC-019)"
```

---

## Task 7: Runtime enforcement in `evaluate_action`

**Files:**
- Modify: `packages/engine/src/engine.rs` (`evaluate_action` ~lines 465-480; imports at top)

- [ ] **Step 1: Write the failing test**

Add to the `#[cfg(test)] mod tests` in `packages/engine/src/engine.rs` (use the existing test harness style; `make_simple_law`/engine construction patterns are in that module — mirror them):

```rust
    #[test]
    fn runtime_rejects_incompatible_add() {
        let yaml = r#"
$id: test_units
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    machine_readable:
      execution:
        input:
          - name: bedrag
            type: amount
            type_spec:
              unit: eurocent
          - name: dagen
            type: number
            type_spec:
              unit: days
        output:
          - name: resultaat
            type: amount
            type_spec:
              unit: eurocent
        actions:
          - output: resultaat
            operation: ADD
            values: ['$bedrag', '$dagen']
"#;
        let law: ArticleBasedLaw = serde_yaml_ng::from_str(yaml).expect("parse");
        // Build the engine the same way the other tests in this module do,
        // provide parameters bedrag/dagen, and evaluate output `resultaat`.
        // EXACT construction mirrors the neighbouring tests in this file.
        let result = evaluate_test_output(&law, "resultaat", &[("bedrag", 1000), ("dagen", 5)]);
        match result {
            Err(EngineError::UnitMismatch { .. }) => {}
            other => panic!("expected UnitMismatch, got {:?}", other),
        }
    }
```

> Implementer note: replace `evaluate_test_output(...)` with the actual evaluation entry used by the existing tests in `engine.rs` (look at how `make_simple_law` tests call into the engine and copy that exact call shape, passing inputs as parameters). The assertion on `EngineError::UnitMismatch` is the part that matters.

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test --manifest-path packages/engine/Cargo.toml runtime_rejects_incompatible_add`
Expected: FAIL — currently the add succeeds (units ignored), so no `UnitMismatch`.

- [ ] **Step 3: Add the runtime check**

In `packages/engine/src/engine.rs`, add the import near the top (with the other `use crate::...` lines):

```rust
use crate::units::{infer_unit, SymbolUnits};
```

Then in `evaluate_action` (currently lines 465-480), insert the unit check so it runs before executing the operation/value. Replace the body with:

```rust
    fn evaluate_action(&self, action: &Action, context: &RuleContext) -> Result<Value> {
        let symbols = SymbolUnits::from_article(self.article);

        // Check for operation at action level FIRST
        if let Some(operation) = &action.operation {
            let action_op = self.action_to_operation(action, operation)?;
            // Unit check (value-independent). Unknown units never error.
            infer_unit(
                &ActionValue::Operation(Box::new(action_op.clone())),
                &symbols,
            )?;
            return execute_operation(&action_op, context, 0);
        }

        // Check for direct value (only when no operation is specified)
        if let Some(value) = &action.value {
            infer_unit(value, &symbols)?;
            return evaluate_value(value, context, 0);
        }

        Ok(Value::Null)
    }
```

Note: `ActionValue` and `ActionOperation` are already imported in `engine.rs` (used by `action_to_operation`). `self.article` is the current `Article` (used by `get_actions`).

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test --manifest-path packages/engine/Cargo.toml runtime_rejects_incompatible_add`
Expected: PASS.

- [ ] **Step 5: Run the full engine test suite to check for regressions**

Run: `cargo test --manifest-path packages/engine/Cargo.toml`
Expected: PASS (existing laws have no units → all `Unknown` → no new errors).

- [ ] **Step 6: Commit**

```bash
git add packages/engine/src/engine.rs
git commit -m "feat(engine): runtime UnitMismatch check in evaluate_action"
```

---

## Task 8: Static enforcement in the `validate` binary

**Files:**
- Modify: `packages/engine/src/units.rs` (add `check_law` + `UnitFinding`)
- Modify: `packages/engine/src/bin/validate.rs` (add step 3)

- [ ] **Step 1: Write the failing test for `check_law`**

Append to `packages/engine/src/units.rs` (inside a new test module or extend `infer_tests`), but first the implementation it needs — add this public API above the tests:

```rust
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
pub fn check_law(law: &crate::article::ArticleBasedLaw) -> Vec<UnitFinding> {
    use crate::types::ParameterType;
    let mut findings = Vec::new();

    for article in &law.articles {
        let symbols = SymbolUnits::from_article(article);
        let article_no = article.number.clone();

        let Some(exec) = article.get_execution_spec() else {
            continue;
        };

        // Hard errors: walk each action's expression.
        if let Some(actions) = &exec.actions {
            for action in actions {
                let output = action.output.clone().unwrap_or_default();
                let expr = if let Some(op) = &action.operation {
                    // Reconstruct an ActionValue from the inline operation via the
                    // action's own value/values/subject/conditions is engine-internal;
                    // for static checking we inspect action.value when present,
                    // otherwise skip inline-operation-only actions (covered at runtime).
                    action.value.clone()
                } else {
                    action.value.clone()
                };
                if let Some(expr) = expr {
                    if let Err(e) = infer_unit(&expr, &symbols) {
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
                let has_unit = o.type_spec.as_ref().and_then(|t| t.unit.as_deref()).is_some();
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
```

> Implementer note on inline operations: actions can express an operation either as a nested `ActionValue::Operation` under `value` (covered above) **or** inline via `action.operation` + `action.values`/`subject`. The inline form has no single `ActionValue` to walk without reconstructing it. Reuse the engine's existing reconstruction by making `ArticleEngine::action_to_operation` logic available, OR (simpler for this task) statically check only the `value` form and rely on the runtime check (Task 7) for inline-operation actions. Pick the simpler path: document that inline-operation actions are runtime-checked only, and keep `check_law` focused on `action.value` expressions. Adjust the code above to drop the dead `if let Some(op)` branch.

Now the test:

```rust
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
}
```

- [ ] **Step 2: Run the tests to verify they pass**

Run: `cargo test --manifest-path packages/engine/Cargo.toml check_law_tests`
Expected: PASS (2 tests). Resolve any field-name mismatches against `article.rs` (`article.number`, `Output.output_type`).

- [ ] **Step 3: Wire `check_law` into the validate binary as step 3**

In `packages/engine/src/bin/validate.rs`, after the JSON-schema validation block prints `OK` for a file, add a step 3. First, the binary already deserializes via `ArticleBasedLaw::from_yaml_file(path)` at step 1 but discards it — capture it instead. Change step 1:

```rust
        // Step 1: serde deserialization check (catches type/structure errors)
        let law = match ArticleBasedLaw::from_yaml_file(path) {
            Ok(law) => law,
            Err(e) => {
                eprintln!("FAIL: {}: serde: {e}", path.display());
                failed = true;
                continue;
            }
        };
```

Then, after the schema `OK`/`FAIL` reporting (at the end of the per-file loop body, after the `match version { ... }` block), add:

```rust
        // Step 3: unit checking (RFC-019). Errors fail; missing units warn.
        for finding in regelrecht_engine::units::check_law(&law) {
            if finding.is_error {
                eprintln!(
                    "FAIL: {}: unit: art. {} output '{}': {}",
                    path.display(),
                    finding.article,
                    finding.output,
                    finding.message
                );
                failed = true;
            } else {
                eprintln!(
                    "WARN: {}: unit: art. {} output '{}': {}",
                    path.display(),
                    finding.article,
                    finding.output,
                    finding.message
                );
            }
        }
```

- [ ] **Step 4: Build and run validate on the corpus**

Run: `cargo build --manifest-path packages/engine/Cargo.toml --features validate --bin validate`
Then: `just validate`
Expected: builds; corpus still passes (no `FAIL: ... unit`). `WARN: ... unit` lines may appear for amount outputs without units — these do not fail CI. Confirm exit code is 0.

- [ ] **Step 5: Commit**

```bash
git add packages/engine/src/units.rs packages/engine/src/bin/validate.rs
git commit -m "feat(validate): static unit checking via check_law (RFC-019)"
```

---

## Task 9: BDD scenario for runtime mismatch

**Files:**
- Create: `features/units.feature`
- Inspect: existing `features/*.feature` and the cucumber step definitions to reuse the established Given/When/Then steps.

- [ ] **Step 1: Inspect existing BDD steps**

Run: `ls features/ && grep -rl "World\|#\[given\|#\[when\|#\[then" packages/engine/tests packages/engine/src 2>/dev/null | head`
Read one existing `.feature` file and its step definitions to learn the exact step phrasing for "given a law", "when I evaluate output X", "then evaluation fails with ...".

- [ ] **Step 2: Write the scenario using existing step phrasing**

Create `features/units.feature` mirroring the Gherkin style already used in the repo. The scenario:

```gherkin
Feature: Unit-of-measurement enforcement (RFC-019)

  Scenario: Adding eurocent to days fails at runtime
    Given the law "test_units" with an action adding a eurocent input and a days input
    When I evaluate the output "resultaat"
    Then evaluation fails with a unit mismatch error
```

> Implementer note: adapt the Given/When/Then wording to the existing step library. If the repo's BDD harness loads laws from `corpus/` or from inline tables, follow that convention — add a minimal fixture law if needed, or express the law inline using the existing "Given a law:" doc-string step. The required assertion is that evaluation returns `EngineError::UnitMismatch`. If adding a new step definition is necessary, place it alongside the existing step defs and assert on the error variant.

- [ ] **Step 3: Run BDD**

Run: `just bdd`
Expected: the new scenario passes; all existing scenarios still pass.

- [ ] **Step 4: Commit**

```bash
git add features/units.feature packages/engine/tests
git commit -m "test(bdd): runtime unit mismatch scenario (RFC-019)"
```

---

## Task 10: Annotate `wet_op_de_zorgtoeslag` as the reference (smoke test)

**Files:**
- Modify: `corpus/regulation/nl/wet/wet_op_de_zorgtoeslag/2025-01-01.yaml`

- [ ] **Step 1: Bump the law to schema v0.5.3**

In the law's `$schema` field, change the version suffix to `v0.5.3`.

- [ ] **Step 2: Annotate the definitions with units**

For each constant in `definitions`, convert to the structured form with a unit. Amounts → `eurocent`; the `percentage_*` decimals → `ratio` (they are stored as 0–1). Example:

```yaml
definitions:
  drempelinkomen_alleenstaande:
    value: 3971900
    type: amount
    type_spec:
      unit: eurocent
  percentage_drempelinkomen_alleenstaande:
    value: 0.01896
    type: number
    type_spec:
      unit: ratio
```

Apply consistently to every entry in this file's `definitions` block.

- [ ] **Step 3: Confirm inputs/outputs already carry units; add any missing `amount` units**

Inspect the `input` and `output` lists. Any `type: amount` field without `type_spec.unit: eurocent` should get one (to clear the WARN from Task 8). Leave non-amount fields as-is.

- [ ] **Step 4: Validate the annotated law**

Run: `just validate corpus/regulation/nl/wet/wet_op_de_zorgtoeslag/2025-01-01.yaml`
Expected: `OK` with no `FAIL: ... unit` and ideally no `WARN: ... unit`. If a `FAIL: ... unit` appears, it indicates a real mismatch in the annotations or the law logic — fix the annotation (or the logic) rather than removing the unit.

- [ ] **Step 5: Run the law's tests / BDD if it has scenarios**

Run: `just bdd`
Expected: zorgtoeslag scenarios still pass — annotation must not change computed results.

- [ ] **Step 6: Commit**

```bash
git add corpus/regulation/nl/wet/wet_op_de_zorgtoeslag/2025-01-01.yaml
git commit -m "feat(corpus): annotate wet_op_de_zorgtoeslag with units (RFC-019 reference)"
```

---

## Final verification

- [ ] **Run the full quality gate**

Run: `just check`
Expected: format + lint + build-check + validate + tests all green. The only non-fatal output should be `WARN: ... unit` lines for not-yet-annotated laws.

- [ ] **Self-check against RFC-019**

Confirm each RFC §1–§8 maps to a task: §1/§2 algebra → Task 5; §3 shared inference → Tasks 6+7+8; §4 symbol table → Task 6 (local-only; cross-law deferred); §5 schema → Task 1; §6 types → Tasks 3+4; §7 reporting → Tasks 7 (runtime) + 8 (static); §8 rollout → Tasks 1 + 10.

---

## Out of scope (explicit follow-up)

- **Cross-law source-unit verification** (RFC-019 §4): checking that a law's `source`-backed input unit matches the source law's *output* unit. Needs a multi-law registry view. Track as a separate RFC-019 follow-up plan.
- **Migrating integer-percent laws** (e.g. `30` → `0.30` with `unit: ratio`) and annotating the rest of the corpus: per-law follow-up PRs after this lands.
- **`euro` literals / euro↔eurocent conversion**: the algebra flags euro+eurocent mixing; automatic conversion is intentionally not implemented.
