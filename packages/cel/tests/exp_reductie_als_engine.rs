//! Experiment A: the same lexostatus via the reduction DSL and via the engine,
//! on the same grams. The outputs must be equal.
//!
//! The first tests run on the fictional register cell. The test
//! `compare_from_env` does the same for cells and articles outside this repo
//! (for example another corpus), with paths from the environment; without
//! those variables it skips. The derivations and the extra fields of a
//! lexostatus are both compared.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use regelrecht_cel::config::CellDefinition;
use regelrecht_cel::gram::Gram;
use regelrecht_cel::lexostatus_engine::{outputs_of, CellRoute, Mode};
use regelrecht_cel::reduction::AsOf;
use regelrecht_cel::stream;
use regelrecht_cel::{initial_state, lexostatus_engine, reduction};
use regelrecht_engine::LawExecutionService;
use serde_json::{json, Map, Value};

const DATE: &str = "2025-03-12";
/// The load time of the initial state: after every gram in the initial states.
const LOAD_TIME: &str = "2026-09-28T12:00:00+02:00";

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

/// The grams of a cell: the initial state, plus `extra` (json lines in the
/// shape of the initial state), placed as the runtime does (with a fixed load
/// time as `recorded_at`).
fn grams_of(cell_dir: &Path, extra: &[Value]) -> (CellDefinition, Vec<Gram>) {
    let def = CellDefinition::load(cell_dir).unwrap();
    let mut streams = Vec::new();
    for s in &def.streams {
        streams.extend(stream::load(&cell_dir.join(s)).unwrap());
    }
    // A stream with `establishes` gets its shape from the law: with
    // `EXP_REGULATION` the corpus that contains that law.
    if let Ok(path) = std::env::var("EXP_REGULATION") {
        let corpus = regelrecht_cel::regulations::load(Path::new(&path)).unwrap();
        let errors = regelrecht_cel::law::establish(&mut streams, &corpus.service, None);
        assert!(errors.is_empty(), "{errors:?}");
    }
    stream::derive_roles(&mut streams);
    let mut text =
        std::fs::read_to_string(cell_dir.join(def.initial_state.as_ref().unwrap())).unwrap();
    for e in extra {
        text.push('\n');
        text.push_str(&e.to_string());
    }
    let grams = initial_state::parse(&text, "initial_state", &streams).unwrap();
    let load_time = chrono::DateTime::parse_from_rfc3339(LOAD_TIME).unwrap();
    (def, initial_state::placed(&grams, &load_time).unwrap())
}

fn service_with(article: &Path) -> (Arc<LawExecutionService>, String) {
    let mut s = LawExecutionService::new();
    let id = s
        .load_law(&std::fs::read_to_string(article).unwrap())
        .unwrap();
    (Arc::new(s), id)
}

/// Compare per input: the lexostatus via the reduction DSL against the one via
/// the engine, along the same route as the runtime with
/// `CELL_REDUCTION=compare` (derivations, extra fields, not derived and the
/// picked gram). Returns the differences.
fn compare(
    def: &reduction::LexostatusDefinition,
    service: &Arc<LawExecutionService>,
    regulation: &str,
    grams: &[Gram],
    inputs: &[Map<String, Value>],
) -> Vec<String> {
    let route = CellRoute {
        service: service.clone(),
        modes: BTreeMap::from([(
            def.name.clone(),
            Mode::Engine {
                regulation: regulation.to_string(),
                article: None,
            },
        )]),
        compare: true,
    };
    inputs
        .iter()
        .filter_map(|i| {
            lexostatus_engine::reduce_lexostatus(
                &route,
                def,
                i,
                grams,
                &AsOf::default(),
                DATE,
                false,
            )
            .err()
            .map(|f| format!("{i:?}: {f}"))
        })
        .collect()
}

fn register() -> (reduction::LexostatusDefinition, Vec<Gram>) {
    let map = fixtures().join("cells/register");
    // More than the initial state: a removal, a second result, and two
    // notices of which the latest is in another time zone (08:30Z is later
    // than 09:00+01:00). That way `pick: latest` counts by the moment, not by
    // the text or the order of adding.
    let extra = [
        json!({"stream": "test_registers", "name": "aanduiding_ingeschreven", "effective_at": "2024-01-11T09:00:00+01:00", "provenance": "initial_state", "fields": {"aanduiding": "ANDERS", "orgaan": "raad", "gebied": "Buurdorp"}}),
        json!({"stream": "test_registers", "name": "aanduiding_geschrapt", "effective_at": "2024-06-01T09:00:00+01:00", "provenance": "initial_state", "fields": {"aanduiding": "ANDERS", "orgaan": "raad"}}),
        json!({"stream": "test_registers", "name": "uitslag_vastgesteld", "effective_at": "2024-03-21T09:00:00+01:00", "provenance": "initial_state", "fields": {"orgaan": "raad", "gebied": "Buurdorp", "lijst": "ANDERS", "zetels": 7}}),
        json!({"stream": "test_registers", "name": "mededeling_gedaan", "effective_at": "2024-12-02T08:30:00+00:00", "provenance": "initial_state", "fields": {"aanduiding": "VOORBEELD", "datum": "2024-12-02", "geblokkeerd_voor": ["raad"]}}),
        json!({"stream": "test_registers", "name": "mededeling_gedaan", "effective_at": "2024-12-02T09:00:00+01:00", "provenance": "initial_state", "fields": {"aanduiding": "VOORBEELD", "datum": "2024-12-01", "geblokkeerd_voor": []}}),
    ];
    let (def, grams) = grams_of(&map, &extra);
    let lexo = reduction::load(&map.join(&def.lexostatuses)).unwrap();
    (lexo.lexostatus("registerstatus").unwrap().clone(), grams)
}

/// Note "bron en gram-id", step 4: querying a register is in the policy of
/// the register keeper; which source delivers the register is up to the
/// deployment. The same policy article, once with the cell's chronicle as the
/// source ([`lexostatus_engine::ChronicleSource`]) and once with a legacy API
/// ([`DictDataSource`], which returns the matching records per requested
/// aanduiding): the output is the same. The policy does not know what is
/// behind it.
#[test]
fn a_register_from_the_chronicle_and_from_a_legacy_adapter_gives_the_same() {
    use regelrecht_engine::DictDataSource;
    const POLICY: &str = "testbeleid_registerhouder";
    let (_, grams) = register();
    let text =
        std::fs::read_to_string(fixtures().join("beleid/testbeleid_registerhouder.yaml")).unwrap();
    let mut chronicle = LawExecutionService::new();
    chronicle.load_law(&text).unwrap();
    chronicle.add_data_source(Box::new(
        lexostatus_engine::ChronicleSource::new(POLICY, &grams, "test_register").unwrap(),
    ));
    // The legacy API: per aanduiding the records of that aanduiding, in the
    // shape the policy reads.
    let aanduidingen = ["VOORBEELD", "ANDERS", "ONBEKEND"];
    let records: Vec<BTreeMap<String, regelrecht_engine::Value>> = aanduidingen
        .iter()
        .map(|a| {
            let own: Vec<&Gram> = grams
                .iter()
                .filter(|g| g.fields.get("aanduiding").and_then(Value::as_str) == Some(*a))
                .collect();
            let list = lexostatus_engine::as_chronicle(own, "test_register").unwrap();
            BTreeMap::from([
                (
                    "aanduiding".to_string(),
                    regelrecht_engine::Value::from(&json!(a)),
                ),
                ("grams".to_string(), regelrecht_engine::Value::from(&list)),
            ])
        })
        .collect();
    let mut api = LawExecutionService::new();
    api.load_law(&text).unwrap();
    api.add_data_source(Box::new(
        DictDataSource::from_records("register_api", 10, "aanduiding", records)
            .unwrap()
            .with_law_scope(POLICY),
    ));
    let outputs = ["is_ingeschreven_in_register", "is_geschrapt"];
    let mut seen = 0;
    for a in aanduidingen {
        for orgaan in ["raad", "staten"] {
            let p: BTreeMap<String, Value> = BTreeMap::from([
                ("aanduiding".into(), json!(a)),
                ("orgaan".into(), json!(orgaan)),
            ]);
            let k = regelrecht_cel::assessment::evaluate(&chronicle, POLICY, &outputs, &p, DATE);
            let d = regelrecht_cel::assessment::evaluate(&api, POLICY, &outputs, &p, DATE);
            assert!(k.complete(&outputs), "{a} {orgaan}: {k:?}");
            assert_eq!(k.values, d.values, "{a} {orgaan}");
            seen += usize::from(k.values["is_ingeschreven_in_register"] == json!(true));
        }
    }
    assert_eq!(seen, 2, "VOORBEELD and ANDERS are registered with the raad");
}

fn inputs(aanduidingen: &[&str]) -> Vec<Map<String, Value>> {
    aanduidingen
        .iter()
        .map(|a| json!({"aanduiding": a}).as_object().unwrap().clone())
        .collect()
}

#[test]
fn registerstatus_via_engine_equals_reduction() {
    let (def, grams) = register();
    let (service, id) = service_with(&fixtures().join("experiment/lexostatus_registerstatus.yaml"));
    let v = compare(
        &def,
        &service,
        &id,
        &grams,
        &inputs(&["VOORBEELD", "ANDERS", "ONBEKEND"]),
    );
    assert!(v.is_empty(), "{v:#?}");
}

#[test]
fn latest_is_by_moment_not_by_adding() {
    let (def, grams) = register();
    let (service, id) = service_with(&fixtures().join("experiment/lexostatus_registerstatus.yaml"));
    let i = &inputs(&["VOORBEELD"])[0];
    let out = lexostatus_engine::reduce(
        &service,
        &id,
        &["datum_mededeling", "geblokkeerd", "jaar_van_mededeling"],
        i,
        &grams,
        &def.reduction.chronicle,
        DATE,
    )
    .unwrap();
    assert_eq!(out["datum_mededeling"], json!("2024-12-02"));
    assert_eq!(out["geblokkeerd"], json!(true));
    assert_eq!(out["jaar_van_mededeling"], json!(2024));
}

#[test]
fn no_gram_is_no_zero_or_absent() {
    let (def, grams) = register();
    let (service, id) = service_with(&fixtures().join("experiment/lexostatus_registerstatus.yaml"));
    let names = outputs_of(&def);
    let outputs: Vec<&str> = names.iter().map(String::as_str).collect();
    let out = lexostatus_engine::reduce(
        &service,
        &id,
        &outputs,
        &inputs(&["ONBEKEND"])[0],
        &grams,
        &def.reduction.chronicle,
        DATE,
    )
    .unwrap();
    assert_eq!(out["zetels_toegewezen"], json!(0));
    assert_eq!(out["geblokkeerd"], json!(false));
    assert!(!out.contains_key("datum_mededeling"));
    assert!(!out.contains_key("jaar_van_mededeling"));
}

/// Time per reduction, DSL against engine, with a growing chronicle. Run with
/// `cargo test --release -- --ignored --nocapture measure_time`. Above 1000
/// grams FOREACH refuses (MAX_ARRAY_SIZE).
#[test]
#[ignore = "measurement, not a test"]
fn measure_time() {
    let (def, basis) = register();
    let (service, id) = service_with(&fixtures().join("experiment/lexostatus_registerstatus.yaml"));
    for extra in [0usize, 100, 990] {
        let mut grams = basis.clone();
        for i in 0..extra {
            let mut g = basis[0].clone();
            g.fields
                .insert("aanduiding".into(), json!(format!("ANDER{i}")));
            grams.push(g);
        }
        measure(&def, &service, &id, &grams, &inputs(&["VOORBEELD"])[0]);
    }
}

fn measure(
    def: &reduction::LexostatusDefinition,
    service: &LawExecutionService,
    id: &str,
    grams: &[Gram],
    i: &Map<String, Value>,
) {
    let n = 200;
    let names = outputs_of(def);
    let outputs: Vec<&str> = names.iter().map(String::as_str).collect();
    let t = Instant::now();
    for _ in 0..n {
        reduction::reduce(def, i, grams).unwrap();
    }
    let dsl = t.elapsed();
    let t = Instant::now();
    for _ in 0..n {
        lexostatus_engine::reduce(
            service,
            id,
            &outputs,
            i,
            grams,
            &def.reduction.chronicle,
            DATE,
        )
        .unwrap();
    }
    let engine = t.elapsed();
    println!(
        "{}: {} grams, {n}x: dsl {:?}/run, engine {:?}/run",
        def.name,
        grams.len(),
        dsl / n,
        engine / n
    );
}

/// The same comparison for cells and articles from outside this repo.
///
/// `EXP_COMPARE`: a json list, one object per comparison, with
///
/// - `cell_dir`: the directory of the cell (with `cell.yaml`);
/// - `extra` (optional): a file with json lines in the shape of the initial
///   state, appended after it;
/// - `article`: the engine article with the lexostatus;
/// - `lexostatus`: the name of the lexostatus in the cell;
/// - `inputs`: a list of inputs, one object per case.
///
/// Without that variable the test skips.
#[test]
fn compare_from_env() {
    let Ok(input) = std::env::var("EXP_COMPARE") else {
        eprintln!("EXP_COMPARE not set: skipped");
        return;
    };
    let list: Vec<Value> = serde_json::from_str(&input).unwrap();
    let (mut cases, mut values) = (0, 0);
    let mut differences = Vec::new();
    for v in &list {
        let map = PathBuf::from(v["cell_dir"].as_str().unwrap());
        let extra: Vec<Value> = match v.get("extra").and_then(Value::as_str) {
            Some(path) => std::fs::read_to_string(path)
                .unwrap()
                .lines()
                .filter(|r| !r.trim().is_empty())
                .map(|r| serde_json::from_str(r).unwrap())
                .collect(),
            None => Vec::new(),
        };
        let (def, grams) = grams_of(&map, &extra);
        let lexo = reduction::load(&map.join(&def.lexostatuses)).unwrap();
        let name = v["lexostatus"].as_str().unwrap();
        let def = lexo.lexostatus(name).unwrap().clone();
        let (service, id) = service_with(Path::new(v["article"].as_str().unwrap()));
        let inputs: Vec<Map<String, Value>> = serde_json::from_value(v["inputs"].clone()).unwrap();
        let names = outputs_of(&def);
        let outputs: Vec<&str> = names.iter().map(String::as_str).collect();
        println!(
            "{name} ({} grams, {} extra): {} cases x {} outputs {outputs:?}",
            grams.len(),
            extra.len(),
            inputs.len(),
            outputs.len()
        );
        for i in &inputs {
            let out = lexostatus_engine::reduce(
                &service,
                &id,
                &outputs,
                i,
                &grams,
                &def.reduction.chronicle,
                DATE,
            )
            .unwrap();
            println!("  {i:?}: {out:?}");
        }
        differences.extend(
            compare(&def, &service, &id, &grams, &inputs)
                .into_iter()
                .map(|f| format!("{name}: {f}")),
        );
        cases += inputs.len();
        values += inputs.len() * outputs.len();
        measure(&def, &service, &id, &grams, &inputs[0]);
    }
    println!(
        "compared: {} lexostatus runs, {cases} cases, {values} values",
        list.len()
    );
    assert!(differences.is_empty(), "{differences:#?}");
}

/// Step 2: the synthesis per row as an engine run, for a corpus from outside
/// this repo. Every lexostatus is a regulation that gets its chronicle via a
/// [`lexostatus_engine::ChronicleSource`]; the synthesis regulation fetches
/// them with `source` and builds the table; then the consuming regulation
/// computes.
///
/// - `EXP_REGULATION`: the directory with the regulations of the corpus;
/// - `EXP_SYNTHESIS_DIR`: the directory with the lexostatus and synthesis
///   regulations (a `koppeling.yaml` in it, for the runtime, does not count);
/// - `EXP_BINDING`: json list `{regulation, cell_dir, chronicle}` (which
///   lexostatus regulation reads which chronicle: the deployment
///   configuration);
/// - `EXP_SYNTHESIS`: json `{regulation, output, year, inputs, expected}` of
///   the synthesis; `expected` (optional) gives per output the value the
///   synthesis of the cell runtime produces (the table, the translated
///   parameters);
/// - `EXP_CONSUMER`: json `{regulation, output, table, year, expected}`: the
///   consumer gets the synthesis output as the parameter named by `table`
///   and the year as the parameter named by `year`.
#[test]
fn synthesis_from_env() {
    let (Ok(corpus), Ok(map), Ok(binding), Ok(synthesis), Ok(consumer)) = (
        std::env::var("EXP_REGULATION"),
        std::env::var("EXP_SYNTHESIS_DIR"),
        std::env::var("EXP_BINDING"),
        std::env::var("EXP_SYNTHESIS"),
        std::env::var("EXP_CONSUMER"),
    ) else {
        eprintln!("EXP_* not set: skipped");
        return;
    };
    let t = Instant::now();
    let mut service = regelrecht_cel::regulations::load(Path::new(&corpus))
        .unwrap()
        .service;
    // The regulations in the directory; the runtime's binding file
    // (`koppeling.yaml`) is not a regulation.
    let files = regelrecht_cel::load::yaml_files(Path::new(&map)).unwrap();
    for b in files
        .iter()
        .filter(|b| b.file_name().is_some_and(|n| n != "koppeling.yaml"))
    {
        service
            .load_law(&std::fs::read_to_string(b).unwrap())
            .map_err(|e| format!("{}: {e}", b.display()))
            .unwrap();
    }
    let binding: Vec<Value> = serde_json::from_str(&binding).unwrap();
    for k in &binding {
        let (_, grams) = grams_of(Path::new(k["cell_dir"].as_str().unwrap()), &[]);
        service.add_data_source(Box::new(
            lexostatus_engine::ChronicleSource::new(
                k["regulation"].as_str().unwrap(),
                &grams,
                k["chronicle"].as_str().unwrap(),
            )
            .unwrap(),
        ));
    }
    println!("loading: {:?}", t.elapsed());
    let s: Value = serde_json::from_str(&synthesis).unwrap();
    let a: Value = serde_json::from_str(&consumer).unwrap();
    let input: BTreeMap<String, Value> = serde_json::from_value(s["inputs"].clone()).unwrap();
    let expected: Map<String, Value> = s
        .get("expected")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let mut outputs = vec![s["output"].as_str().unwrap(), s["year"].as_str().unwrap()];
    outputs.extend(expected.keys().map(String::as_str));
    outputs.sort_unstable();
    outputs.dedup();
    let t = Instant::now();
    let e = regelrecht_cel::assessment::evaluate_with_trace(
        &service,
        s["regulation"].as_str().unwrap(),
        &outputs,
        &input,
        DATE,
    );
    println!("synthesis: {:?}", t.elapsed());
    assert!(
        e.error.is_none() && e.missing.is_empty(),
        "{:?} {:?}\n{}",
        e.error,
        e.missing,
        e.trace_text.unwrap_or_default()
    );
    let differences: Vec<String> = expected
        .iter()
        .filter(|(u, w)| e.values.get(*u) != Some(*w))
        .map(|(u, w)| format!("{u}: runtime {w}, engine {:?}", e.values.get(u)))
        .collect();
    println!(
        "synthesis: {} of {} outputs equal to the runtime",
        expected.len() - differences.len(),
        expected.len()
    );
    assert!(differences.is_empty(), "{differences:#?}");
    let table = e.values[s["output"].as_str().unwrap()].clone();
    let year = e.values[s["year"].as_str().unwrap()].clone();
    println!("table: {table}\nyear: {year}");
    let mut p = BTreeMap::new();
    p.insert(a["table"].as_str().unwrap().to_string(), table);
    p.insert(a["year"].as_str().unwrap().to_string(), year);
    let r = regelrecht_cel::assessment::evaluate(
        &service,
        a["regulation"].as_str().unwrap(),
        &[a["output"].as_str().unwrap()],
        &p,
        DATE,
    );
    assert!(
        r.error.is_none() && r.missing.is_empty(),
        "{:?} {:?}",
        r.error,
        r.missing
    );
    let amount = &r.values[a["output"].as_str().unwrap()];
    println!("{}: {amount}", a["output"]);
    assert_eq!(amount, &a["expected"]);
}

/// A gram for the step 8 check, as a cell recorded it.
fn step8_gram(
    id: &str,
    name: &str,
    kind: &str,
    stage: Option<&str>,
    refers_to: Value,
    fields: Value,
    day: &str,
) -> Gram {
    let mut g = json!({
        "kind": "chronolexogram", "id": id, "type": if stage.is_some() { "decretogram" } else { "executogram" },
        "subtype": kind, "name": name, "chronicle": "test_toeslag", "recording_actor": "test_toeslagdienst",
        "legal_basis": ["testregeling_toeslag#1"], "effective_at": format!("{day}T10:00:00+01:00"),
        "recorded_at": format!("{day}T10:00:00+01:00"), "refers_to": refers_to,
        "stream": {"id": "test_toeslag_zaakverloop", "sha256": "0".repeat(64)}, "fields": fields,
    });
    if let Some(s) = stage {
        g["stage"] = json!(s);
    }
    serde_json::from_value(g).unwrap()
}

/// Note "bron en gram-id", step 8, after the actual text: Awb 4:52 lid 1
/// ("overeenkomstig de subsidievaststelling") reads per decision, and Awb 4:95
/// lid 4 ("Betaalde voorschotten worden verrekend met de te betalen
/// geldsom") offsets the advances. A fictional allowance: advance 500 (paid),
/// determination 300. Per decision nothing has been paid on the
/// determination; with the offset there is nothing left to pay and 200 is
/// undue, and that is what the recovery (which refers to the determination,
/// and so belongs to the same application) recovers. The policy of the
/// allowance service executes both articles with an ordinary source; the
/// payments come from its own records (a chronicle as the source).
#[test]
fn advance_and_determination_per_decision_with_offset() {
    const POLICY: &str = "testbeleid_toeslagdienst";
    let id = |n: u32| format!("00000000-0000-4000-8000-{n:012}");
    let (g101, g110, g111, g120, g130, g131) =
        (id(101), id(110), id(111), id(120), id(130), id(131));
    let grams = vec![
        step8_gram(
            &g101,
            "aanvraag_ontvangen",
            "aanvraag",
            Some("AANVRAAG"),
            json!({}),
            json!({}),
            "2025-03-01",
        ),
        step8_gram(
            &g110,
            "voorschot_verleend",
            "voorschot",
            Some("BESLUIT"),
            json!({"on_application": g101}),
            json!({"voorschot": 500}),
            "2025-03-02",
        ),
        step8_gram(
            &g111,
            "voorschot_betaald",
            "betaling",
            None,
            json!({"decision": g110}),
            json!({"bedrag": 500}),
            "2025-03-03",
        ),
        step8_gram(
            &g120,
            "toeslag_vastgesteld",
            "vaststelling",
            Some("BESLUIT"),
            json!({"on_application": g101}),
            json!({"vastgestelde_toeslag": 300}),
            "2025-03-10",
        ),
    ];
    // The chronicle: the determination belongs to the application through its reference.
    let dir = tempfile::tempdir().unwrap();
    let chronicle =
        regelrecht_cel::chronicle::Chronicle::open(dir.path(), &["test_toeslag"]).unwrap();
    for g in &grams {
        chronicle.add(g).unwrap();
    }
    assert_eq!(
        chronicle.read_root(&["test_toeslag"], &g101).unwrap().len(),
        4
    );

    let evaluate = |grams: &[Gram], regulation: &str, outputs: &[&str], p: Value| {
        let mut corpus = regelrecht_cel::regulations::load(&fixtures().join("regulation")).unwrap();
        corpus
            .service
            .load_law(
                &std::fs::read_to_string(fixtures().join("beleid/testbeleid_toeslagdienst.yaml"))
                    .unwrap(),
            )
            .unwrap();
        corpus.service.add_data_source(Box::new(
            lexostatus_engine::ChronicleSource::new(POLICY, grams, "test_toeslag").unwrap(),
        ));
        let p: BTreeMap<String, Value> = serde_json::from_value(p).unwrap();
        let e =
            regelrecht_cel::assessment::evaluate(&corpus.service, regulation, outputs, &p, DATE);
        assert!(e.complete(outputs), "{e:?}");
        e.values
    };
    // The records: per decision, and the advances on the same application.
    let a = evaluate(
        &grams,
        POLICY,
        &["betaald_bij_besluit", "betaalde_voorschotten"],
        json!({"besluit": g120}),
    );
    assert_eq!(
        a["betaald_bij_besluit"],
        json!(0),
        "nothing has been paid on the determination itself (4:52 lid 1)"
    );
    assert_eq!(
        a["betaalde_voorschotten"],
        json!(500),
        "the advance on the same application (4:95 lid 4)"
    );
    // The obligation: nothing left to pay, 200 undue.
    let determination =
        json!({"besluit": g120, "vastgesteld_bedrag": 300, "datum_bekendmaking": "2025-03-11"});
    let v = evaluate(
        &grams,
        POLICY,
        &[
            "nog_te_betalen_verstrekker",
            "onverschuldigd_betaald_verstrekker",
        ],
        determination.clone(),
    );
    assert_eq!(v["nog_te_betalen_verstrekker"], json!(0));
    assert_eq!(v["onverschuldigd_betaald_verstrekker"], json!(200));
    // Only 4:52 per decision, without offset, would give 300 to pay: the
    // service would then pay twice.
    let only = evaluate(
        &grams,
        "testregeling_awb",
        &["nog_te_betalen"],
        json!({"vastgesteld_bedrag": 300, "betaald_bedrag": 0, "datum_bekendmaking": "2025-03-11"}),
    );
    assert_eq!(only["nog_te_betalen"], json!(300));

    // The recovery: an ex officio decision without an application, which
    // refers to the determination with `concerns` and so belongs to the same
    // application; it recovers what is undue. The repayment refers to the
    // recovery and does not count as a payment on the determination.
    let mut further = grams.clone();
    further.push(step8_gram(
        &g130,
        "terugvordering_vastgesteld",
        "terugvordering",
        Some("BESLUIT"),
        json!({"concerns": g120}),
        json!({"terug_te_vorderen": v["onverschuldigd_betaald_verstrekker"].clone()}),
        "2025-03-12",
    ));
    further.push(step8_gram(
        &g131,
        "terugbetaling_ontvangen",
        "terugbetaling",
        None,
        json!({"besluit": g130}),
        json!({"bedrag": 200}),
        "2025-03-12",
    ));
    for g in &further[4..] {
        chronicle.add(g).unwrap();
    }
    let group = chronicle.read_root(&["test_toeslag"], &g101).unwrap();
    assert_eq!(
        group.len(),
        6,
        "the recovery belongs to the application through concerns"
    );
    assert_eq!(group[4].gram.root.as_deref(), Some(g101.as_str()));
    let after = evaluate(
        &further,
        POLICY,
        &["betaald_bij_besluit", "betaalde_voorschotten"],
        json!({"besluit": g120}),
    );
    assert_eq!(after["betaald_bij_besluit"], json!(0));
    assert_eq!(after["betaalde_voorschotten"], json!(500));
    // An ex officio decision without a predecessor is its own root.
    let g300 = id(300);
    chronicle
        .add(&step8_gram(
            &g300,
            "terugvordering_vastgesteld",
            "terugvordering",
            Some("BESLUIT"),
            json!({}),
            json!({"terug_te_vorderen": 50}),
            "2025-03-12",
        ))
        .unwrap();
    assert_eq!(
        chronicle.read_root(&["test_toeslag"], &g300).unwrap().len(),
        1
    );
}

/// Step 8 for a corpus outside this repo (for example the variant of a real
/// case), with paths from the environment; without `EXP_PAYMENT` it skips.
/// `EXP_PAYMENT`: json `{regulation, grams (jsonl), policy, chronicle,
/// cases: [{regulation, outputs, parameters, date, expected}]}`: the policy
/// gets the grams as the source of its register, and every case must give
/// its expected outputs.
#[test]
fn payment_from_env() {
    let Ok(input) = std::env::var("EXP_PAYMENT") else {
        eprintln!("EXP_PAYMENT not set: skipped");
        return;
    };
    let v: Value = serde_json::from_str(&input).unwrap();
    let mut corpus =
        regelrecht_cel::regulations::load(Path::new(v["regulation"].as_str().unwrap())).unwrap();
    let grams: Vec<Gram> = std::fs::read_to_string(v["grams"].as_str().unwrap())
        .unwrap()
        .lines()
        .filter(|r| !r.trim().is_empty())
        .map(|r| serde_json::from_str(r).unwrap())
        .collect();
    corpus.service.add_data_source(Box::new(
        lexostatus_engine::ChronicleSource::new(
            v["policy"].as_str().unwrap(),
            &grams,
            v["chronicle"].as_str().unwrap(),
        )
        .unwrap(),
    ));
    for g in v["cases"].as_array().unwrap() {
        let outputs: Vec<&str> = g["outputs"]
            .as_array()
            .unwrap()
            .iter()
            .map(|u| u.as_str().unwrap())
            .collect();
        let p: BTreeMap<String, Value> = serde_json::from_value(g["parameters"].clone()).unwrap();
        let e = regelrecht_cel::assessment::evaluate(
            &corpus.service,
            g["regulation"].as_str().unwrap(),
            &outputs,
            &p,
            g["date"].as_str().unwrap(),
        );
        println!("{} {:?}: {:?}", g["regulation"], outputs, e.values);
        assert!(e.complete(&outputs), "{e:?}");
        for (k, w) in g["expected"].as_object().unwrap() {
            assert_eq!(&e.values[k], w, "{k}");
        }
    }
}
