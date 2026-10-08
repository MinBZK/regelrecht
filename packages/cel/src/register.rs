//! A chronicle of the cell as a register: the data source of a policy of the
//! holder that reads it back (RFC-045 §1).
//!
//! The reduction of a chronicle to what the law asks belongs to the holder
//! and is written in the same language as the law: an article in the policy
//! of the holder, with one input without a source (`source: {}`), executed by
//! the engine. Which chronicle that input is, says only the cell
//! configuration (`registers:` in `cell.yaml`); the policy names no system.
//!
//! The cell registers a [`RegisterSource`] per binding with the engine
//! ([`bind`]), scoped to the policy. The source answers with the rows the
//! cell gives it for the duration of one reading ([`with_rows`]): the grams of
//! the chronicle that hold at the moment of reading, one row per gram, in the
//! order they hold ([`rows`]). A gram that holds only later is not in it.
//!
//! A row is the gram's fields, flattened, next to what the policy needs to
//! select and order on: `id`, `event`, `type`, `subtype`, `stage`,
//! `establishes`, `root` (the gram of the case), `sequence` (its place in the
//! order: the latest gram has the highest), `effective_at`, `effective_date`,
//! `recorded_at`, `recorded_date` and `period` (the value of the period the
//! gram concerns). The law format has no LAST: a policy takes the latest
//! gram as the MAX of `sequence` over its selection.

use std::cell::RefCell;
use std::collections::BTreeMap;

use chrono::{DateTime, FixedOffset};
use regelrecht_engine::{DataSource, LawExecutionService, Value};
use serde_json::{json, Map};

use crate::chronicle::Chronicle;
use crate::config::{CellConfig, Register};
use crate::error::{setup, Result};
use crate::lexostatus;

/// The attributes of a gram in a row; a field with one of these names would
/// be ambiguous.
pub const GRAM_KEYS: &[&str] = &[
    "id",
    "event",
    "type",
    "subtype",
    "stage",
    "establishes",
    "root",
    "sequence",
    "effective_at",
    "effective_date",
    "recorded_at",
    "recorded_date",
    "period",
];

thread_local! {
    /// The rows of each register source during a reading, by source name.
    static ROWS: RefCell<BTreeMap<String, Value>> = const { RefCell::new(BTreeMap::new()) };
}

/// The name of the engine data source of a register of `cell`.
pub fn source_name(cell: &str, register: &Register) -> String {
    format!("kroniek {cell}/{} ({})", register.chronicle, register.key())
}

/// A register as an engine data source: the one source-less input of a
/// policy, answered with the rows the cell set for the reading.
pub struct RegisterSource {
    name: String,
    policy: String,
    field: String,
}

impl DataSource for RegisterSource {
    fn name(&self) -> &str {
        &self.name
    }
    fn priority(&self) -> i32 {
        10
    }
    fn source_type(&self) -> &str {
        "register"
    }
    fn has_field(&self, field: &str) -> bool {
        field == self.field
    }
    fn get(&self, field: &str, _criteria: &BTreeMap<String, Value>) -> Option<Value> {
        if field != self.field {
            return None;
        }
        ROWS.with(|r| r.borrow().get(&self.name).cloned())
    }
    fn fields(&self) -> Vec<&str> {
        vec![self.field.as_str()]
    }
    fn law_scope(&self) -> Option<&str> {
        Some(&self.policy)
    }
    fn key_fields(&self) -> Option<&[String]> {
        Some(&[])
    }
}

/// The input without a source (`source: {}`) of the policy: what it reads
/// of its register. Exactly one, or the binding is refused.
pub fn register_input(service: &LawExecutionService, policy: &str) -> Result<String> {
    let law = service.resolver().get_law(policy).ok_or_else(|| {
        setup(format!(
            "register of policy '{policy}': no regulation '{policy}'"
        ))
    })?;
    let mut names: Vec<String> = law
        .articles
        .iter()
        .flat_map(|a| a.get_execution_spec().into_iter())
        .flat_map(|e| e.input.iter().flatten())
        .filter(|i| {
            i.source
                .as_ref()
                .is_some_and(|s| s.regulation.is_none() && s.output.is_none())
        })
        .map(|i| i.name.clone())
        .collect();
    names.sort();
    names.dedup();
    match names.as_slice() {
        [one] => Ok(one.clone()),
        [] => Err(setup(format!(
            "policy '{policy}' has no input without a source (`source: {{}}`); it reads no register"
        ))),
        more => Err(setup(format!(
            "policy '{policy}' has more than one input without a source ({}); it reads one register",
            more.join(", ")
        ))),
    }
}

/// Register a data source per register of the cell `config` with the
/// engine, replacing one of the same name: again after the caller cleared
/// the data sources.
pub fn bind(service: &mut LawExecutionService, config: &CellConfig) -> Result<()> {
    for register in &config.registers {
        let field = register_input(service, &register.policy)?;
        let name = source_name(&config.id, register);
        service.remove_data_source(&name);
        service.add_data_source(Box::new(RegisterSource {
            name,
            policy: register.policy.clone(),
            field,
        }));
    }
    Ok(())
}

/// Whether the register is bound with the engine ([`bind`]).
pub fn is_bound(service: &LawExecutionService, cell: &str, register: &Register) -> bool {
    let name = source_name(cell, register);
    service
        .data_registry()
        .list_sources()
        .iter()
        .any(|s| *s == name)
}

/// The grams of `chronicle` that hold at `as_of`, as rows (see the module).
/// A field the law gives the grams of an event (`event_fields`) and a gram
/// does not have is null in its row: asked and left empty (RFC-036), so a
/// policy can test for it rather than fail on a missing property.
pub fn rows(
    chronicle: &Chronicle,
    as_of: DateTime<FixedOffset>,
    event_fields: &BTreeMap<String, Vec<String>>,
) -> Result<Value> {
    let mut out = Vec::new();
    for (sequence, gram) in lexostatus::in_force(chronicle, as_of)?
        .into_iter()
        .enumerate()
    {
        let mut row = Map::new();
        for (name, value) in &gram.fields {
            if GRAM_KEYS.contains(&name.as_str()) {
                return Err(setup(format!(
                    "gram '{}' has a field '{name}', which a register row has as an attribute of the gram",
                    gram.id
                )));
            }
            row.insert(name.clone(), value.clone());
        }
        for name in event_fields.get(&gram.name).into_iter().flatten() {
            if !GRAM_KEYS.contains(&name.as_str()) {
                row.entry(name.clone()).or_insert(serde_json::Value::Null);
            }
        }
        let date = |moment: &str| moment.chars().take(10).collect::<String>();
        row.insert("id".into(), json!(gram.id));
        row.insert("event".into(), json!(gram.name));
        row.insert("type".into(), json!(gram.type_));
        row.insert("subtype".into(), json!(gram.subtype));
        row.insert("stage".into(), json!(gram.stage));
        row.insert("establishes".into(), json!(gram.establishes));
        row.insert("root".into(), json!(chronicle.root_of(gram)));
        row.insert("sequence".into(), json!(sequence));
        row.insert("effective_at".into(), json!(gram.effective_at));
        row.insert("effective_date".into(), json!(date(&gram.effective_at)));
        row.insert("recorded_at".into(), json!(gram.recorded_at));
        row.insert("recorded_date".into(), json!(date(&gram.recorded_at)));
        row.insert("period".into(), json!(gram.period.map(|p| p.value)));
        out.push(serde_json::Value::Object(row));
    }
    Ok(Value::from(&serde_json::Value::Array(out)))
}

/// Run `f` with `rows` as the answer of the register source `name`, on this
/// thread and only during `f` (an engine run is synchronous). The rows are
/// removed afterwards, also after a panic in `f`.
pub fn with_rows<T>(name: &str, rows: Value, f: impl FnOnce() -> T) -> T {
    struct Back(String, Option<Value>);
    impl Drop for Back {
        fn drop(&mut self) {
            let old = self.1.take();
            ROWS.with(|r| {
                let mut r = r.borrow_mut();
                match old {
                    Some(v) => r.insert(self.0.clone(), v),
                    None => r.remove(&self.0),
                };
            });
        }
    }
    let old = ROWS.with(|r| r.borrow_mut().insert(name.to_string(), rows));
    let _back = Back(name.to_string(), old);
    f()
}
