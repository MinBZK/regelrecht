//! Experiment A: dezelfde lexostatus via de reductie-DSL en via de engine,
//! op dezelfde grammen. De uitkomsten moeten gelijk zijn.
//!
//! De eerste tests draaien op de fictieve registercel. De test
//! `vergelijk_uit_omgeving` doet hetzelfde voor een cel en een artikel
//! buiten deze repo (bijvoorbeeld een ander corpus), met paden uit de
//! omgeving; zonder die variabelen slaat hij over.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use regelrecht_cel::config::CelDefinitie;
use regelrecht_cel::stroom::{self, Gram};
use regelrecht_cel::{lexostatus_engine, reductie, startstand};
use regelrecht_engine::LawExecutionService;
use serde_json::{json, Map, Value};

const DATUM: &str = "2025-03-12";

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

/// De grammen van een cel: de startstand, plus `extra` (json-regels in de
/// vorm van de startstand).
fn grammen_van(cel_map: &Path, extra: &[Value]) -> (CelDefinitie, Vec<Gram>) {
    let def = CelDefinitie::laad(cel_map).unwrap();
    let mut strommen = Vec::new();
    for s in &def.stromen {
        strommen.extend(stroom::laad(&cel_map.join(s)).unwrap());
    }
    let mut tekst =
        std::fs::read_to_string(cel_map.join(def.startstand.as_ref().unwrap())).unwrap();
    for e in extra {
        tekst.push('\n');
        tekst.push_str(&e.to_string());
    }
    let grammen = startstand::parse(&tekst, "startstand", &strommen).unwrap();
    (def, grammen)
}

fn service_met(artikel: &Path) -> (LawExecutionService, String) {
    let mut s = LawExecutionService::new();
    let id = s
        .load_law(&std::fs::read_to_string(artikel).unwrap())
        .unwrap();
    (s, id)
}

/// Vergelijk per input: de parameters van de reductie tegen de uitkomsten
/// van de engine. Geeft de verschillen terug.
fn vergelijk(
    def: &reductie::LexostatusDefinitie,
    service: &LawExecutionService,
    regeling: &str,
    grammen: &[Gram],
    inputs: &[Map<String, Value>],
) -> Vec<String> {
    let uitkomsten: Vec<&str> = def
        .reduction
        .afleidingen
        .keys()
        .map(String::as_str)
        .collect();
    let mut verschillen = Vec::new();
    for i in inputs {
        let dsl = reductie::reduceer(def, i, grammen)
            .unwrap()
            .map(|l| l.parameters)
            .unwrap_or_default();
        let engine = lexostatus_engine::reduceer(
            service,
            regeling,
            &uitkomsten,
            i,
            grammen,
            &def.reduction.kroniek,
            DATUM,
        )
        .unwrap();
        for u in &uitkomsten {
            let (a, b) = (dsl.get(*u), engine.get(*u));
            if a != b {
                verschillen.push(format!("{i:?} {u}: dsl {a:?}, engine {b:?}"));
            }
        }
    }
    verschillen
}

fn register() -> (reductie::LexostatusDefinitie, Vec<Gram>) {
    let map = fixtures().join("cellen/register");
    // Meer dan de startstand: een schrapping, een tweede uitslag, en twee
    // mededelingen waarvan de laatste in een andere tijdzone staat (08:30Z is
    // later dan 09:00+01:00). Zo telt `kies: laatste` op het moment, niet op
    // de tekst of de volgorde van toevoegen.
    let extra = [
        json!({"stroom": "test_registers", "name": "aanduiding_ingeschreven", "op_moment": "2024-01-11T09:00:00+01:00", "herkomst": "startstand", "fields": {"aanduiding": "ANDERS", "orgaan": "raad", "gebied": "Buurdorp"}}),
        json!({"stroom": "test_registers", "name": "aanduiding_geschrapt", "op_moment": "2024-06-01T09:00:00+01:00", "herkomst": "startstand", "fields": {"aanduiding": "ANDERS", "orgaan": "raad"}}),
        json!({"stroom": "test_registers", "name": "uitslag_vastgesteld", "op_moment": "2024-03-21T09:00:00+01:00", "herkomst": "startstand", "fields": {"orgaan": "raad", "gebied": "Buurdorp", "lijst": "ANDERS", "zetels": 7}}),
        json!({"stroom": "test_registers", "name": "mededeling_gedaan", "op_moment": "2024-12-02T08:30:00+00:00", "herkomst": "startstand", "fields": {"aanduiding": "VOORBEELD", "datum": "2024-12-02", "geblokkeerd_voor": ["raad"]}}),
        json!({"stroom": "test_registers", "name": "mededeling_gedaan", "op_moment": "2024-12-02T09:00:00+01:00", "herkomst": "startstand", "fields": {"aanduiding": "VOORBEELD", "datum": "2024-12-01", "geblokkeerd_voor": []}}),
    ];
    let (def, grammen) = grammen_van(&map, &extra);
    let lexo = reductie::laad(&map.join(&def.lexostatussen)).unwrap();
    (lexo.lexostatus("registerstatus").unwrap().clone(), grammen)
}

fn inputs(aanduidingen: &[&str]) -> Vec<Map<String, Value>> {
    aanduidingen
        .iter()
        .map(|a| json!({"aanduiding": a}).as_object().unwrap().clone())
        .collect()
}

#[test]
fn registerstatus_via_engine_gelijk_aan_reductie() {
    let (def, grammen) = register();
    let (service, id) = service_met(&fixtures().join("experiment/lexostatus_registerstatus.yaml"));
    let v = vergelijk(
        &def,
        &service,
        &id,
        &grammen,
        &inputs(&["VOORBEELD", "ANDERS", "ONBEKEND"]),
    );
    assert!(v.is_empty(), "{v:#?}");
}

#[test]
fn laatste_is_op_moment_niet_op_toevoegen() {
    let (def, grammen) = register();
    let (service, id) = service_met(&fixtures().join("experiment/lexostatus_registerstatus.yaml"));
    let i = &inputs(&["VOORBEELD"])[0];
    let uit = lexostatus_engine::reduceer(
        &service,
        &id,
        &["datum_mededeling", "geblokkeerd_raad", "jaar"],
        i,
        &grammen,
        &def.reduction.kroniek,
        DATUM,
    )
    .unwrap();
    assert_eq!(uit["datum_mededeling"], json!("2024-12-02"));
    assert_eq!(uit["geblokkeerd_raad"], json!(true));
    assert_eq!(uit["jaar"], json!(2024));
}

#[test]
fn geen_gram_is_nee_nul_of_weg() {
    let (def, grammen) = register();
    let (service, id) = service_met(&fixtures().join("experiment/lexostatus_registerstatus.yaml"));
    let uitkomsten: Vec<&str> = def
        .reduction
        .afleidingen
        .keys()
        .map(String::as_str)
        .collect();
    let uit = lexostatus_engine::reduceer(
        &service,
        &id,
        &uitkomsten,
        &inputs(&["ONBEKEND"])[0],
        &grammen,
        &def.reduction.kroniek,
        DATUM,
    )
    .unwrap();
    assert_eq!(uit["is_ingeschreven_raad"], json!(false));
    assert_eq!(uit["zetels_op_lijst"], json!(0));
    assert_eq!(uit["geblokkeerd_raad"], json!(false));
    assert!(!uit.contains_key("datum_mededeling"));
    assert!(!uit.contains_key("jaar"));
}

/// Tijd per reductie, DSL tegen engine, bij een groeiende kroniek. Draai met
/// `cargo test --release -- --ignored --nocapture meet_tijd`. Boven 1000
/// grammen weigert FOREACH (MAX_ARRAY_SIZE).
#[test]
#[ignore = "meting, geen test"]
fn meet_tijd() {
    let (def, basis) = register();
    let (service, id) = service_met(&fixtures().join("experiment/lexostatus_registerstatus.yaml"));
    for extra in [0usize, 100, 990] {
        let mut grammen = basis.clone();
        for i in 0..extra {
            let mut g = basis[0].clone();
            g.fields
                .insert("aanduiding".into(), json!(format!("ANDER{i}")));
            grammen.push(g);
        }
        meet(&def, &service, &id, &grammen, &inputs(&["VOORBEELD"])[0]);
    }
}

fn meet(
    def: &reductie::LexostatusDefinitie,
    service: &LawExecutionService,
    id: &str,
    grammen: &[Gram],
    i: &Map<String, Value>,
) {
    let n = 200;
    let uitkomsten: Vec<&str> = def
        .reduction
        .afleidingen
        .keys()
        .map(String::as_str)
        .collect();
    let t = Instant::now();
    for _ in 0..n {
        reductie::reduceer(def, i, grammen).unwrap();
    }
    let dsl = t.elapsed();
    let t = Instant::now();
    for _ in 0..n {
        lexostatus_engine::reduceer(
            service,
            id,
            &uitkomsten,
            i,
            grammen,
            &def.reduction.kroniek,
            DATUM,
        )
        .unwrap();
    }
    let engine = t.elapsed();
    println!(
        "{} grammen, {n}x: dsl {:?}/run, engine {:?}/run",
        grammen.len(),
        dsl / n,
        engine / n
    );
}

/// Dezelfde vergelijking voor een cel en een artikel van buiten deze repo.
///
/// - `EXP_CEL_MAP`: de map van de cel (met `cel.yaml`);
/// - `EXP_ARTIKEL`: het engine-artikel met de lexostatus;
/// - `EXP_LEXOSTATUS`: de naam van de lexostatus in de cel;
/// - `EXP_INPUTS`: een json-lijst van inputs, een object per geval.
#[test]
fn vergelijk_uit_omgeving() {
    let (Ok(map), Ok(artikel), Ok(naam), Ok(invoer)) = (
        std::env::var("EXP_CEL_MAP"),
        std::env::var("EXP_ARTIKEL"),
        std::env::var("EXP_LEXOSTATUS"),
        std::env::var("EXP_INPUTS"),
    ) else {
        eprintln!("EXP_* niet gezet: overgeslagen");
        return;
    };
    let map = PathBuf::from(map);
    let (def, grammen) = grammen_van(&map, &[]);
    let lexo = reductie::laad(&map.join(&def.lexostatussen)).unwrap();
    let def = lexo.lexostatus(&naam).unwrap().clone();
    let (service, id) = service_met(Path::new(&artikel));
    let gevallen: Vec<Map<String, Value>> = serde_json::from_str(&invoer).unwrap();
    for i in &gevallen {
        let uit: BTreeMap<String, Value> = lexostatus_engine::reduceer(
            &service,
            &id,
            &def.reduction
                .afleidingen
                .keys()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            i,
            &grammen,
            &def.reduction.kroniek,
            DATUM,
        )
        .unwrap();
        println!("{i:?}: {uit:?}");
    }
    let v = vergelijk(&def, &service, &id, &grammen, &gevallen);
    assert!(v.is_empty(), "{v:#?}");
    meet(&def, &service, &id, &grammen, &gevallen[0]);
}

/// Stap 2: de synthese per regel als engine-run, voor een corpus van buiten
/// deze repo. Elke lexostatus is een regeling die haar kroniek via een
/// [`lexostatus_engine::KroniekBron`] krijgt; de synthese-regeling haalt ze
/// op met `source` en bouwt de tabel; daarna rekent de afnemende regeling.
///
/// - `EXP_REGULATION`: de map met de regelingen van het corpus;
/// - `EXP_SYNTHESE_MAP`: de map met de lexostatus- en synthese-regelingen;
/// - `EXP_KOPPELING`: json-lijst `{regeling, cel_map, kroniek}` (welke
///   lexostatus-regeling welke kroniek leest: de deploymentconfiguratie);
/// - `EXP_SYNTHESE`: json `{regeling, uitkomst, inputs}` van de synthese;
/// - `EXP_AFNEMER`: json `{regeling, uitkomst, tabel, jaar, verwacht}`: de
///   afnemer krijgt de tabel als parameter `tabel` en het jaar als `jaar`.
#[test]
fn synthese_uit_omgeving() {
    let (Ok(corpus), Ok(map), Ok(koppeling), Ok(synthese), Ok(afnemer)) = (
        std::env::var("EXP_REGULATION"),
        std::env::var("EXP_SYNTHESE_MAP"),
        std::env::var("EXP_KOPPELING"),
        std::env::var("EXP_SYNTHESE"),
        std::env::var("EXP_AFNEMER"),
    ) else {
        eprintln!("EXP_* niet gezet: overgeslagen");
        return;
    };
    let t = Instant::now();
    let mut service = regelrecht_cel::regelingen::laad(Path::new(&corpus))
        .unwrap()
        .service;
    let mut bestanden: Vec<PathBuf> = std::fs::read_dir(&map)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "yaml"))
        .collect();
    bestanden.sort();
    for b in &bestanden {
        service
            .load_law(&std::fs::read_to_string(b).unwrap())
            .map_err(|e| format!("{}: {e}", b.display()))
            .unwrap();
    }
    let koppeling: Vec<Value> = serde_json::from_str(&koppeling).unwrap();
    for k in &koppeling {
        let (_, grammen) = grammen_van(Path::new(k["cel_map"].as_str().unwrap()), &[]);
        service.add_data_source(Box::new(
            lexostatus_engine::KroniekBron::new(
                k["regeling"].as_str().unwrap(),
                &grammen,
                k["kroniek"].as_str().unwrap(),
            )
            .unwrap(),
        ));
    }
    println!("laden: {:?}", t.elapsed());
    let s: Value = serde_json::from_str(&synthese).unwrap();
    let a: Value = serde_json::from_str(&afnemer).unwrap();
    let invoer: BTreeMap<String, Value> = serde_json::from_value(s["inputs"].clone()).unwrap();
    let t = Instant::now();
    let e = regelrecht_cel::toets::evalueer_met_trace(
        &service,
        s["regeling"].as_str().unwrap(),
        &[s["uitkomst"].as_str().unwrap(), s["jaar"].as_str().unwrap()],
        &invoer,
        DATUM,
    );
    println!("synthese: {:?}", t.elapsed());
    assert!(
        e.fout.is_none() && e.mist.is_empty(),
        "{:?} {:?}\n{}",
        e.fout,
        e.mist,
        e.trace_text.unwrap_or_default()
    );
    let tabel = e.waarden[s["uitkomst"].as_str().unwrap()].clone();
    let jaar = e.waarden[s["jaar"].as_str().unwrap()].clone();
    println!("tabel: {tabel}\njaar: {jaar}");
    let mut p = BTreeMap::new();
    p.insert(a["tabel"].as_str().unwrap().to_string(), tabel);
    p.insert(a["jaar"].as_str().unwrap().to_string(), jaar);
    let r = regelrecht_cel::toets::evalueer(
        &service,
        a["regeling"].as_str().unwrap(),
        &[a["uitkomst"].as_str().unwrap()],
        &p,
        DATUM,
    );
    assert!(
        r.fout.is_none() && r.mist.is_empty(),
        "{:?} {:?}",
        r.fout,
        r.mist
    );
    let bedrag = &r.waarden[a["uitkomst"].as_str().unwrap()];
    println!("{}: {bedrag}", a["uitkomst"]);
    assert_eq!(bedrag, &a["verwacht"]);
}
