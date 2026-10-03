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

use regelrecht_engine::{Article, DataSource, LawExecutionService, Value as EngineValue};
use regelrecht_law_model::Input;
use serde::Deserialize;

use crate::chronicle::Chronicle;
use crate::gram::Gram;
use crate::lexostatus_engine;
use crate::load;

/// The output with which a policy names its register.
pub const NAME_REGISTER: &str = "naam_register";

thread_local! {
    /// The grams of a trial, with the cell that would record them, for the
    /// duration of [`with_trial`].
    static TRIAL: RefCell<Vec<(String, Gram)>> = const { RefCell::new(Vec::new()) };
}

/// Run `f` with `grams` of the cell `cell` added to every register source
/// of that cell: the draft of an action on trial counts as if it were
/// recorded. Only on this thread, and only during `f` (an engine run is
/// synchronous).
/// The overlay is removed even after a panic in `f`: the thread serves
/// another request afterwards.
pub fn with_trial<T>(cell: &str, grams: Vec<Gram>, f: impl FnOnce() -> T) -> T {
    let grams = grams.into_iter().map(|g| (cell.to_string(), g)).collect();
    struct Back(Option<Vec<(String, Gram)>>);
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
    cell: String,
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
        // The engine's data source answers with a value or nothing; a
        // register that cannot be read is not silent about it.
        let fail = |what: &str, e: String| {
            tracing::error!(register = %self.name, cell = %self.cell, chronicle = %self.chronicle, error = %e, "{what}");
        };
        let Some(chronicle) = self.source.get() else {
            fail(
                "register: the chronicle of the cell is not open",
                String::new(),
            );
            return None;
        };
        let fixed = chronicle
            .read(&self.chronicle)
            .map_err(|e| fail("register: the chronicle cannot be read", e))
            .ok()?;
        let trial: Vec<Gram> = TRIAL.with(|p| {
            p.borrow()
                .iter()
                .filter(|(cell, g)| *cell == self.cell && g.chronicle == self.chronicle)
                .map(|(_, g)| g.clone())
                .collect()
        });
        let grams = fixed.iter().map(|v| &v.gram).chain(trial.iter());
        let list = lexostatus_engine::as_chronicle(grams, &self.chronicle)
            .map_err(|e| fail("register: the chronicle cannot be read as a list", e))
            .ok()?;
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

/// A register as the map of a process shows it (`GET /api/map`): the policy
/// that queries it and the name it has there (the key `<policy>#<name>` of
/// the binding file), and the cell and chronicle that supply it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisterLink {
    pub policy: String,
    pub name: String,
    pub cell: String,
    pub chronicle: String,
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

/// The source-less inputs (`source: {}`) of an article: what it asks of a
/// register.
pub fn source_less_inputs(article: &Article) -> impl Iterator<Item = &Input> {
    article
        .get_execution_spec()
        .into_iter()
        .flat_map(|e| e.input.iter().flatten())
        .filter(|i| {
            i.source
                .as_ref()
                .is_some_and(|s| s.regulation.is_none() && s.output.is_none())
        })
}

/// What a register policy reads of the grams of its register: per FOREACH
/// over its source-less input, the fields it reads (`$<as>.fields.<path>`)
/// and the events its filter names (`$<as>.name` EQUALS a literal; none:
/// any event of the chronicle).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RegisterRead {
    pub events: std::collections::BTreeSet<String>,
    pub paths: std::collections::BTreeSet<String>,
}

/// What the articles of `policy` read of their register (see
/// [`RegisterRead`]).
pub fn reads(service: &LawExecutionService, policy: &str) -> Vec<RegisterRead> {
    let Some(law) = service.resolver().get_law(policy) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for article in &law.articles {
        let inputs: Vec<String> = source_less_inputs(article)
            .map(|i| format!("${}", i.name))
            .collect();
        if inputs.is_empty() {
            continue;
        }
        let actions = article
            .get_execution_spec()
            .and_then(|e| e.actions.as_ref())
            .and_then(|a| serde_json::to_value(a).ok());
        if let Some(v) = actions {
            foreach_reads(&v, &inputs, &mut out);
        }
    }
    out
}

/// Every FOREACH in `v` over one of `inputs`, with what it reads.
fn foreach_reads(v: &serde_json::Value, inputs: &[String], out: &mut Vec<RegisterRead>) {
    use serde_json::Value;
    match v {
        Value::Object(m) => {
            let over = m.get("collection").and_then(Value::as_str);
            if let (Some("FOREACH"), Some(c), Some(var)) = (
                m.get("operation").and_then(Value::as_str),
                over,
                m.get("as").and_then(Value::as_str),
            ) {
                if inputs.iter().any(|i| i == c) {
                    let mut r = RegisterRead::default();
                    gram_reads(v, &format!("${var}"), &mut r);
                    out.push(r);
                }
            }
            for x in m.values() {
                foreach_reads(x, inputs, out);
            }
        }
        Value::Array(a) => a.iter().for_each(|x| foreach_reads(x, inputs, out)),
        _ => {}
    }
}

/// The fields of the gram `var` read in `v`, and the event names an EQUALS
/// on `<var>.name` compares with.
fn gram_reads(v: &serde_json::Value, var: &str, r: &mut RegisterRead) {
    use serde_json::Value;
    match v {
        Value::String(s) => {
            if let Some(path) = s.strip_prefix(&format!("{var}.fields.")) {
                r.paths.insert(path.to_string());
            }
        }
        Value::Object(m) => {
            if m.get("operation").and_then(Value::as_str) == Some("EQUALS") {
                let name = format!("{var}.name");
                let (a, b) = (m.get("subject"), m.get("value"));
                for (x, y) in [(a, b), (b, a)] {
                    if let (Some(Value::String(x)), Some(Value::String(y))) = (x, y) {
                        if *x == name && !y.starts_with('$') {
                            r.events.insert(y.clone());
                        }
                    }
                }
            }
            m.values().for_each(|x| gram_reads(x, var, r));
        }
        Value::Array(a) => a.iter().for_each(|x| gram_reads(x, var, r)),
        _ => {}
    }
}

/// The register (`<policy>#<name>`) that reads `field` of an event in
/// `chronicle` of `cell`, if one does.
pub fn read_by(
    links: &[RegisterLink],
    service: &LawExecutionService,
    cell: &str,
    chronicle: &str,
    event: &str,
    field: &str,
) -> Option<String> {
    links
        .iter()
        .filter(|l| l.cell == cell && l.chronicle == chronicle)
        .find(|l| {
            reads(service, &l.policy).iter().any(|r| {
                (r.events.is_empty() || r.events.contains(event))
                    && r.paths.iter().any(|p| crate::check::covered(field, p))
            })
        })
        .map(|l| format!("{}#{}", l.policy, l.name))
}

/// The source-less input (`source: {}`) of a regulation: the names.
fn register_input(service: &LawExecutionService, regulation: &str) -> Vec<String> {
    let Some(law) = service.resolver().get_law(regulation) else {
        return Vec::new();
    };
    let mut out: Vec<String> = law
        .articles
        .iter()
        .flat_map(source_less_inputs)
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
            cell: k.cell.clone(),
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
    /// Which policy queries which register, and who supplies it.
    pub fn links(&self) -> Vec<RegisterLink> {
        self.bindings
            .iter()
            .filter_map(|(key, k, _)| {
                // `load` only binds a key of the form `<policy>#<name>`.
                let (policy, name) = key.split_once('#')?;
                Some(RegisterLink {
                    policy: policy.to_string(),
                    name: name.to_string(),
                    cell: k.cell.clone(),
                    chronicle: k.chronicle.clone(),
                })
            })
            .collect()
    }

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
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn trial() -> usize {
        TRIAL.with(|p| p.borrow().len())
    }

    /// The registers of a deployment that binds the fictitious register
    /// policy to the chronicle of the register cell.
    fn loaded_test_registers() -> Registers {
        loaded_with_service().0
    }

    fn loaded_with_service() -> (Registers, LawExecutionService) {
        let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
        let mut service = LawExecutionService::new();
        service
            .load_law(
                &std::fs::read_to_string(fixtures.join("beleid/testbeleid_registerhouder.yaml"))
                    .unwrap(),
            )
            .unwrap();
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("registers.yaml");
        std::fs::write(
            &file,
            "registers:\n  testbeleid_registerhouder#register: {cell: test_register, chronicle: test_register}\n",
        )
        .unwrap();
        let r = load(Some(&file), &mut service, "2025-03-12").unwrap();
        (r, service)
    }

    /// The fictitious register policy reads `orgaan` and `aanduiding` of two
    /// events of its register, each in a FOREACH whose filter names the event.
    #[test]
    fn a_register_policy_reads_fields_of_named_events() {
        let (_, service) = loaded_with_service();
        let set = |v: &[&str]| v.iter().map(|s| s.to_string()).collect();
        let fields = set(&["aanduiding", "orgaan"]);
        assert_eq!(
            reads(&service, "testbeleid_registerhouder"),
            [
                RegisterRead {
                    events: set(&["aanduiding_ingeschreven"]),
                    paths: fields.clone(),
                },
                RegisterRead {
                    events: set(&["aanduiding_geschrapt"]),
                    paths: fields,
                },
            ]
        );
    }

    /// A field a register reads is read, in the cell and chronicle the
    /// deployment binds it to, and for the events its filter names.
    #[test]
    fn a_field_is_read_by_the_register_bound_to_its_chronicle() {
        let (r, service) = loaded_with_service();
        let links = r.links();
        let by = |cell: &str, chronicle: &str, event: &str, field: &str| {
            read_by(&links, &service, cell, chronicle, event, field)
        };
        let key = Some("testbeleid_registerhouder#register".to_string());
        assert_eq!(
            by(
                "test_register",
                "test_register",
                "aanduiding_geschrapt",
                "aanduiding"
            ),
            key
        );
        assert_eq!(
            by(
                "test_register",
                "test_register",
                "aanduiding_ingeschreven",
                "orgaan"
            ),
            key
        );
        // Not this field, not this event, not this chronicle or cell.
        assert_eq!(
            by(
                "test_register",
                "test_register",
                "aanduiding_ingeschreven",
                "gebied"
            ),
            None
        );
        assert_eq!(
            by(
                "test_register",
                "test_register",
                "mededeling_gedaan",
                "aanduiding"
            ),
            None
        );
        assert_eq!(
            by(
                "test_register",
                "elders",
                "aanduiding_geschrapt",
                "aanduiding"
            ),
            None
        );
        assert_eq!(
            by(
                "test_afnemer",
                "test_register",
                "aanduiding_geschrapt",
                "aanduiding"
            ),
            None
        );
    }

    #[test]
    fn links_name_policy_cell_and_chronicle() {
        let links = loaded_test_registers().links();
        assert_eq!(
            links,
            [RegisterLink {
                policy: "testbeleid_registerhouder".into(),
                name: "register".into(),
                cell: "test_register".into(),
                chronicle: "test_register".into(),
            }]
        );
    }

    /// The overlay is removed, even if the engine run panics: the next
    /// request on this thread does not see the draft as recorded.
    #[test]
    fn a_trial_is_removed_even_after_a_panic() {
        let g = crate::gram::test_gram("00000000-0000-4000-8000-000000000001");
        assert_eq!(with_trial("c", vec![g.clone()], trial), 1);
        assert_eq!(trial(), 0);
        let out = std::panic::catch_unwind(|| with_trial("c", vec![g], || panic!("engine")));
        assert!(out.is_err());
        assert_eq!(trial(), 0);
    }
}
