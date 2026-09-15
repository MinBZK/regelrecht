//! Record the trace the landing page replays.
//!
//! The landing page shows a law being executed. It does not run the engine: the
//! docs site ships no engine, and a full evaluation takes about three
//! milliseconds anyway, which is far too fast to watch. So the engine runs here,
//! at build time, and the page replays what it recorded at a pace a person can
//! follow. Every step the visitor sees was really taken.
//!
//! The inputs are those of "Inkomen boven het drempelinkomen bouwt de
//! zorgtoeslag af", in the scenario file beside the law
//! (`corpus/regulation/nl/wet/wet_op_de_zorgtoeslag/scenarios/eligibility.feature`),
//! which CI runs on every push. So the number on the landing page is the number
//! a failing test would catch, and the Gherkin the page displays is the very
//! scenario recorded here rather than an illustration. The BSN is the fictional
//! test number that scenario uses.
//!
//! That scenario and not one of the others: it sits above the threshold, where
//! the allowance tapers off. Below the threshold every income yields the same
//! amount, which shows nothing of the rule the memorandum quoted on the page
//! works out.
//!
//! Usage:
//!   cargo run --example record_landing_trace -- <output.json>

use regelrecht_engine::{LawExecutionService, Value};
use std::collections::BTreeMap;
use std::path::Path;
use walkdir::WalkDir;

const BSN: &str = "999993653";
const LAW: &str = "wet_op_de_zorgtoeslag";
const OUTPUT: &str = "hoogte_zorgtoeslag";
const DATE: &str = "2025-01-01";

/// What the scenario asserts. Recording a trace that computes something else
/// means the corpus moved, and the landing page should not quietly show the new
/// number: a demo that silently re-baselines proves nothing.
const EXPECTED: i64 = 133084;

fn record(entries: Vec<(&str, Value)>) -> BTreeMap<String, Value> {
    entries
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect()
}

/// How deep the page shows the tree.
///
/// The whole of it. A trace that says "131 steps" and lists seven is not the
/// trace, and the claim the page makes is precisely that this is what really
/// happened. The document is about 100 KB, which a landing page can carry.
const DISPLAY_DEPTH: usize = usize::MAX;

fn count(node: &regelrecht_engine::trace::PathNode) -> usize {
    1 + node.children.iter().map(count).sum::<usize>()
}

fn sum_duration(node: &regelrecht_engine::trace::PathNode) -> u64 {
    node.duration_us.unwrap_or(0) + node.children.iter().map(sum_duration).sum::<u64>()
}

/// Keep the top of the tree and drop what hangs below it.
///
/// Only ever removes: every step the page shows is a step the engine really
/// took, with the result it really produced. A pruned branch keeps its own
/// result, so the visitor sees the answer without the arithmetic that reached
/// it, and the full count is reported alongside so nothing looks smaller than
/// it was.
fn prune(
    node: &regelrecht_engine::trace::PathNode,
    depth: usize,
    constants: &std::collections::HashMap<String, regelrecht_engine::trace::ValueTypeSpec>,
) -> regelrecht_engine::trace::PathNode {
    prune_with_unit(node, depth, None, constants)
}

fn prune_with_unit(
    node: &regelrecht_engine::trace::PathNode,
    depth: usize,
    inherited: Option<&regelrecht_engine::trace::ValueTypeSpec>,
    constants: &std::collections::HashMap<String, regelrecht_engine::trace::ValueTypeSpec>,
) -> regelrecht_engine::trace::PathNode {
    let mut copy = node.clone();

    // A step can carry a value whose unit is declared somewhere else in the
    // tree: a cross-law call takes it from the step inside the called law that
    // produced it, and a rounding or a resolve takes it from the action it
    // belongs to. Both are the same declaration, made once by a law, so the
    // unit travels to every step holding that same value. An amount is shown
    // as an amount because a law said so, never because a number looked like
    // money.
    if copy.type_spec.is_none() {
        if let Some(found) = declared_unit(node) {
            copy.type_spec = Some(found);
        } else if let Some(declared) = constants.get(&copy.name) {
            copy.type_spec = Some(declared.clone());
        } else if let Some(from_above) = inherited {
            copy.type_spec = Some(from_above.clone());
        }
    }

    // Passed down only while the value is unchanged; a child computing
    // something else declares its own unit or shows none.
    let pass_down = copy
        .type_spec
        .clone()
        .filter(|_| copy.result.is_some())
        .map(|spec| (copy.result.clone(), spec));

    if depth >= DISPLAY_DEPTH {
        copy.children = Vec::new();
    } else {
        copy.children = node
            .children
            .iter()
            .map(|c| {
                // Only a child holding the very same value inherits the unit.
                let handed = pass_down
                    .as_ref()
                    .filter(|(value, _)| *value == c.result)
                    .map(|(_, spec)| spec);
                prune_with_unit(c, depth + 1, handed, constants)
            })
            .collect();
    }
    copy
}

/// Units the law declares on its constants, by name.
///
/// The engine stamps a unit when it resolves an input or computes a declared
/// output, but a constant is resolved straight out of a value map that carries
/// no declaration, so the unit does not reach the step. Rather than widen the
/// engine for the sake of a demo, the recorder reads the same declarations out
/// of the law it already loads. Still the law's word, just fetched here.
fn definition_units(
    law: &regelrecht_engine::article::ArticleBasedLaw,
) -> std::collections::HashMap<String, regelrecht_engine::trace::ValueTypeSpec> {
    let mut units = std::collections::HashMap::new();
    for article in &law.articles {
        let Some(mr) = &article.machine_readable else {
            continue;
        };
        for (name, definition) in mr.definitions.iter().flatten() {
            if let regelrecht_engine::article::Definition::Structured {
                type_spec: Some(spec),
                ..
            } = definition
            {
                let reported = regelrecht_engine::trace::ValueTypeSpec::from_declaration(spec);
                if !reported.is_empty() {
                    units.insert(name.clone(), reported);
                }
            }
        }
    }
    units
}

/// The unit declared by the step that produced this node's value.
///
/// Only accepts a descendant whose result is the same value, so a unit is never
/// borrowed from an unrelated sibling: an amount is reported as an amount
/// because some law said so, not because a number nearby happened to be money.
fn declared_unit(
    node: &regelrecht_engine::trace::PathNode,
) -> Option<regelrecht_engine::trace::ValueTypeSpec> {
    let want = node.result.as_ref()?;
    for child in &node.children {
        if child.result.as_ref() == Some(want) {
            if let Some(ts) = &child.type_spec {
                return Some(ts.clone());
            }
            if let Some(found) = declared_unit(child) {
                return Some(found);
            }
        }
    }
    None
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(out_path) = args.first() else {
        eprintln!("Usage: record_landing_trace <output.json>");
        std::process::exit(1);
    };

    let mut service = LawExecutionService::new();

    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let regulation_dir = Path::new(manifest_dir)
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.join("corpus").join("regulation").join("nl"))
        .expect("Could not find regulation directory");

    let mut loaded = 0;
    // Collected while loading, where the YAML is already in hand: the units the
    // laws declare on their constants, which the engine does not carry onto a
    // trace step (see `definition_units`).
    let mut constants: std::collections::HashMap<String, regelrecht_engine::trace::ValueTypeSpec> =
        std::collections::HashMap::new();
    for entry in WalkDir::new(&regulation_dir)
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if path.is_file() && path.extension().is_some_and(|ext| ext == "yaml") {
            if let Ok(content) = std::fs::read_to_string(path) {
                if service.load_law(&content).is_ok() {
                    loaded += 1;
                    if let Ok(law) = serde_yaml_ng::from_str::<
                        regelrecht_engine::article::ArticleBasedLaw,
                    >(&content)
                    {
                        constants.extend(definition_units(&law));
                    }
                }
            }
        }
    }
    eprintln!("Loaded {loaded} regulations");

    for (name, row) in [
        (
            "personal_data",
            record(vec![
                ("bsn", Value::String(BSN.to_string())),
                ("geboortedatum", Value::String("1990-01-01".to_string())),
            ]),
        ),
        (
            "relationship_data",
            record(vec![
                ("bsn", Value::String(BSN.to_string())),
                ("partnerschap_type", Value::String("GEEN".to_string())),
            ]),
        ),
        (
            "insurance",
            record(vec![
                ("bsn", Value::String(BSN.to_string())),
                ("polis_status", Value::String("ACTIEF".to_string())),
                ("verdragsinschrijving", Value::Bool(false)),
            ]),
        ),
        (
            "box1",
            record(vec![
                ("bsn", Value::String(BSN.to_string())),
                ("loon_uit_dienstbetrekking", Value::Int(3000000)),
                ("uitkeringen_en_pensioenen", Value::Int(0)),
                ("winst_uit_onderneming", Value::Int(0)),
                ("resultaat_overige_werkzaamheden", Value::Int(0)),
                ("eigen_woning", Value::Int(0)),
                ("buitenlands_inkomen", Value::Int(0)),
            ]),
        ),
        (
            "box2",
            record(vec![
                ("bsn", Value::String(BSN.to_string())),
                ("reguliere_voordelen", Value::Int(0)),
                ("vervreemdingsvoordelen", Value::Int(0)),
            ]),
        ),
        (
            "box3",
            record(vec![
                ("bsn", Value::String(BSN.to_string())),
                ("spaargeld", Value::Int(0)),
                ("beleggingen", Value::Int(0)),
                ("onroerend_goed", Value::Int(0)),
                ("schulden", Value::Int(0)),
            ]),
        ),
        (
            "detenties",
            record(vec![
                ("bsn", Value::String(BSN.to_string())),
                ("detentiestatus", Value::Null),
                ("inrichting_type", Value::Null),
                ("zorgtype", Value::Null),
                ("juridische_grondslag", Value::Null),
            ]),
        ),
    ] {
        service
            .register_dict_source(name, "bsn", vec![row])
            .unwrap_or_else(|e| panic!("could not register {name}: {e}"));
    }

    let mut params = BTreeMap::new();
    params.insert("bsn".to_string(), Value::String(BSN.to_string()));

    let result = service
        .evaluate_law_output_with_trace(LAW, OUTPUT, params, DATE)
        .expect("the landing-page evaluation should succeed");

    match result.outputs.get(OUTPUT) {
        Some(Value::Int(n)) if *n == EXPECTED => {}
        other => {
            eprintln!(
                "refusing to record: expected {OUTPUT} = {EXPECTED}, got {other:?}.\n\
                 The scenario beside the law asserts {EXPECTED}. Fix the corpus or \n\
                 update both the scenario and this recorder together."
            );
            std::process::exit(1);
        }
    }

    let trace = result
        .trace
        .expect("a traced evaluation produces a trace document");

    let total_steps = count(&trace);
    let total_us: u64 = sum_duration(&trace);

    let pruned = prune(&trace, 0, &constants);
    let document = regelrecht_engine::trace::TraceDocument::new(pruned);

    // The page states how long the real evaluation took and how many steps it
    // really had, so pruning the tree for display never shrinks the claim.
    let mut json = serde_json::to_value(&document).expect("the trace serializes");
    json["recording"] = serde_json::json!({
        "law": LAW,
        "output": OUTPUT,
        "date": DATE,
        "total_steps": total_steps,
        "total_duration_us": total_us,
        "scenario": "corpus/regulation/nl/wet/wet_op_de_zorgtoeslag/scenarios/eligibility.feature",
    });

    // Trailing newline: the repository's end-of-file hook adds one otherwise,
    // and the recording would differ from what it just wrote every time.
    let text = serde_json::to_string_pretty(&json).expect("the document serializes") + "\n";
    std::fs::write(out_path, text).unwrap_or_else(|e| panic!("could not write {out_path}: {e}"));

    eprintln!("{total_steps} steps in {:.2} ms", total_us as f64 / 1000.0);

    eprintln!("Recorded {OUTPUT} = {EXPECTED} to {out_path}");
}
