//! Registers as the source of the policy that queries them (note "bron en
//! gram-id", 29-09-2026).
//!
//! A piece of data from outside follows four layers: a law establishes the
//! register and says what goes into it (`establishes`), a law gives the
//! consumer the legal basis to request it, and the policy of the keeper gives
//! the register a name and describes the query, as an article with an input
//! without a source (`source: {}`). The consumer fetches the data with an
//! ordinary `source` to that policy. Which system supplies the register (the
//! chronicle of a cell, a legacy API) is stated only by the deployment, in a
//! binding file (`CELL_REGISTERS`):
//!
//! ```yaml
//! registers:
//!   <policy>#<name of the register>: {cell: <cell-id>, chronicle: <chronicle>}
//! ```
//!
//! The runtime registers a [`RegisterSource`] per line: a data source with
//! the policy as `law_scope`, which fills that policy's source-less input
//! with the grams of the chronicle, as they are recorded now. Startup check:
//! every policy with such an input has a source, every source a policy with
//! such an input, a cell and a chronicle of that cell; if the policy names
//! the register (output `naam_register`), that is the name in the binding.
//!
//! A trial (an action that is not yet recorded) counts as in a trial
//! reduction: [`with_trial`] adds the gram of the draft, for the duration of
//! an engine run on this thread.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{Arc, OnceLock};

use regelrecht_engine::{DataSource, LawExecutionService, Value as EngineValue};
use serde::Deserialize;

use crate::chronicle::Chronicle;
use crate::gram::Gram;
use crate::lexostatus_engine;
use crate::load;

/// The output with which a policy names its register.
pub const NAME_REGISTER: &str = "naam_register";

thread_local! {
    /// The grams of a trial, for the duration of [`with_trial`].
    static TRIAL: RefCell<Vec<Gram>> = const { RefCell::new(Vec::new()) };
}

/// Run `f` with `grams` added to every register source: the draft of an
/// action on trial counts as if it were recorded. Only on this thread, and
/// only during `f` (an engine run is synchronous).
/// The overlay is removed even after a panic in `f`: the thread serves
/// another request afterwards.
pub fn with_trial<T>(grams: Vec<Gram>, f: impl FnOnce() -> T) -> T {
    struct Back(Option<Vec<Gram>>);
    impl Drop for Back {
        fn drop(&mut self) {
            let old = self.0.take().unwrap_or_default();
            TRIAL.with(|p| *p.borrow_mut() = old);
        }
    }
    let _back = Back(Some(
        TRIAL.with(|p| std::mem::replace(&mut *p.borrow_mut(), grams)),
    ));
    f()
}

/// A line of the binding file.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Binding {
    pub cell: String,
    pub chronicle: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    registers: BTreeMap<String, Binding>,
}

/// A register as a data source: the chronicle of a cell, for the source-less
/// input of a policy.
pub struct RegisterSource {
    name: String,
    policy: String,
    field: String,
    chronicle: String,
    source: Arc<OnceLock<Arc<Chronicle>>>,
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
    fn get(&self, field: &str, _criteria: &BTreeMap<String, EngineValue>) -> Option<EngineValue> {
        if field != self.field {
            return None;
        }
        let chronicle = self.source.get()?;
        let fixed = chronicle.read(&self.chronicle).ok()?;
        let trial: Vec<Gram> = TRIAL.with(|p| p.borrow().clone());
        let grams = fixed
            .iter()
            .map(|v| &v.gram)
            .chain(trial.iter().filter(|g| g.chronicle == self.chronicle));
        let list = lexostatus_engine::as_chronicle(grams, &self.chronicle).ok()?;
        Some(EngineValue::from(&list))
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

/// The place where a register source receives its chronicle.
type Lock = Arc<OnceLock<Arc<Chronicle>>>;

/// The registers of a deployment, once they are in the corpus as sources;
/// the chronicles are added when the cells open their chronicle
/// ([`Registers::open`]).
#[derive(Default)]
pub struct Registers {
    bindings: Vec<(String, Binding, Lock)>,
}

/// The source-less input (`source: {}`) of a regulation: the names.
fn register_input(service: &LawExecutionService, regulation: &str) -> Vec<String> {
    let Some(law) = service.resolver().get_law(regulation) else {
        return Vec::new();
    };
    let mut out: Vec<String> = law
        .articles
        .iter()
        .filter_map(|a| a.get_execution_spec())
        .flat_map(|e| e.input.iter().flatten())
        .filter(|i| {
            i.source
                .as_ref()
                .is_some_and(|s| s.regulation.is_none() && s.output.is_none())
        })
        .map(|i| i.name.clone())
        .collect();
    out.sort();
    out.dedup();
    out
}

/// The regulations of the corpus with a source-less input: a register that
/// the deployment must bind.
fn policy_with_register(service: &LawExecutionService) -> Vec<String> {
    service
        .resolver()
        .list_laws()
        .into_iter()
        .filter(|id| !register_input(service, id).is_empty())
        .map(str::to_string)
        .collect()
}

/// Read the binding file (`None`: none), add a source to the corpus per
/// register, and check it against the corpus. `date` is the day on which the
/// name of the register is read from the policy.
pub fn load(
    path: Option<&Path>,
    service: &mut LawExecutionService,
    date: &str,
) -> Result<Registers, Vec<String>> {
    let file = match path {
        Some(p) => load::load(p, load::yaml::<File>)?,
        None => File {
            registers: BTreeMap::new(),
        },
    };
    let source = path.map_or("CELL_REGISTERS".to_string(), |p| p.display().to_string());
    let mut errors = Vec::new();
    let mut registers = Registers::default();
    let mut bound: Vec<String> = Vec::new();
    for (key, k) in file.registers {
        let Some((policy, name)) = key.split_once('#') else {
            errors.push(format!("{source}: register '{key}' is not <policy>#<name>"));
            continue;
        };
        if service.resolver().get_law(policy).is_none() {
            errors.push(format!(
                "{source}: register '{key}': no regulation '{policy}' in the corpus"
            ));
            continue;
        }
        let input = register_input(service, policy);
        let field = match &input[..] {
            [v] => v.clone(),
            [] => {
                errors.push(format!(
                    "{source}: register '{key}': '{policy}' has no input without a source (source: {{}}); it queries no register"
                ));
                continue;
            }
            more => {
                errors.push(format!(
                    "{source}: register '{key}': '{policy}' has more than one input without a source ({}); a policy queries one register here",
                    more.join(", ")
                ));
                continue;
            }
        };
        // If the policy names the register, that is the name here.
        if service
            .get_law_info(policy)
            .is_some_and(|i| i.outputs.iter().any(|o| o == NAME_REGISTER))
        {
            let e = crate::assessment::evaluate(
                service,
                policy,
                &[NAME_REGISTER],
                &BTreeMap::new(),
                date,
            );
            match e.values.get(NAME_REGISTER).and_then(serde_json::Value::as_str) {
                Some(n) if n == name => {}
                Some(n) => errors.push(format!(
                    "{source}: register '{key}': the policy names the register '{n}', not '{name}'"
                )),
                None => errors.push(format!(
                    "{source}: register '{key}': the name of the register cannot be read from '{policy}' ({})",
                    e.reason("no name")
                )),
            }
        }
        if bound.contains(&policy.to_string()) {
            errors.push(format!("{source}: '{policy}' has more than one register"));
            continue;
        }
        bound.push(policy.to_string());
        let lock = Arc::new(OnceLock::new());
        service.add_data_source(Box::new(RegisterSource {
            name: format!("register:{key}"),
            policy: policy.to_string(),
            field,
            chronicle: k.chronicle.clone(),
            source: lock.clone(),
        }));
        registers.bindings.push((key, k, lock));
    }
    for policy in policy_with_register(service) {
        if !bound.contains(&policy) {
            errors.push(format!(
                "{source}: '{policy}' queries a register (an input without a source), but no register binds it to a cell or adapter"
            ));
        }
    }
    if errors.is_empty() {
        Ok(registers)
    } else {
        Err(errors)
    }
}

impl Registers {
    /// Check the cells and chronicles of the bindings, before the chronicles
    /// are open.
    pub fn check(&self, cells: &[(&str, Vec<&str>)]) -> Vec<String> {
        let mut errors = Vec::new();
        for (key, k, _) in &self.bindings {
            match cells.iter().find(|(id, _)| *id == k.cell) {
                None => errors.push(format!(
                    "register '{key}': cell '{}' does not run in this runtime",
                    k.cell
                )),
                Some((_, chronicles)) if !chronicles.contains(&k.chronicle.as_str()) => errors
                    .push(format!(
                        "register '{key}': cell '{}' has no chronicle '{}'",
                        k.cell, k.chronicle
                    )),
                Some(_) => {}
            }
        }
        errors
    }

    /// Give every source the chronicle of its cell.
    pub fn open(&self, cell: &str, chronicle: &Arc<Chronicle>) {
        for (_, k, lock) in &self.bindings {
            if k.cell == cell {
                let _ = lock.set(chronicle.clone());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn trial() -> usize {
        TRIAL.with(|p| p.borrow().len())
    }

    /// The overlay is removed, even if the engine run panics: the next
    /// request on this thread does not see the draft as recorded.
    #[test]
    fn a_trial_is_removed_even_after_a_panic() {
        let g = crate::gram::test_gram("00000000-0000-4000-8000-000000000001");
        assert_eq!(with_trial(vec![g.clone()], trial), 1);
        assert_eq!(trial(), 0);
        let out = std::panic::catch_unwind(|| with_trial(vec![g], || panic!("engine")));
        assert!(out.is_err());
        assert_eq!(trial(), 0);
    }
}
