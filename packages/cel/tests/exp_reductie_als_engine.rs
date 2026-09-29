//! Experiment A: dezelfde lexostatus via de reductie-DSL en via de engine,
//! op dezelfde grammen. De uitkomsten moeten gelijk zijn.
//!
//! De eerste tests draaien op de fictieve registercel. De test
//! `vergelijk_uit_omgeving` doet hetzelfde voor cellen en artikelen buiten
//! deze repo (bijvoorbeeld een ander corpus), met paden uit de omgeving;
//! zonder die variabelen slaat hij over. De afleidingen en de extra velden
//! van een lexostatus worden beide vergeleken.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use regelrecht_cel::config::CelDefinitie;
use regelrecht_cel::gram::Gram;
use regelrecht_cel::lexostatus_engine::{uitkomsten_van, CelRoute, Wijze};
use regelrecht_cel::reductie::Peil;
use regelrecht_cel::stroom;
use regelrecht_cel::{lexostatus_engine, reductie, startstand};
use regelrecht_engine::LawExecutionService;
use serde_json::{json, Map, Value};

const DATUM: &str = "2025-03-12";
/// De laadtijd van de startstand: na elk gram in de startstanden.
const LAADTIJD: &str = "2026-09-28T12:00:00+02:00";

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

/// De grammen van een cel: de startstand, plus `extra` (json-regels in de
/// vorm van de startstand), geplaatst zoals de runtime dat doet (met een
/// vaste laadtijd als `vastgelegd_op`).
fn grammen_van(cel_map: &Path, extra: &[Value]) -> (CelDefinitie, Vec<Gram>) {
    let def = CelDefinitie::laad(cel_map).unwrap();
    let mut strommen = Vec::new();
    for s in &def.stromen {
        strommen.extend(stroom::laad(&cel_map.join(s)).unwrap());
    }
    // Een stroom met `vestigt` krijgt zijn vorm uit de wet: met
    // `EXP_REGULATION` het corpus waarin die wet staat.
    if let Ok(pad) = std::env::var("EXP_REGULATION") {
        let corpus = regelrecht_cel::regelingen::laad(Path::new(&pad)).unwrap();
        let fouten = regelrecht_cel::wet::vestig(&mut strommen, &corpus.service);
        assert!(fouten.is_empty(), "{fouten:?}");
    }
    stroom::leid_rollen_af(&mut strommen);
    let mut tekst =
        std::fs::read_to_string(cel_map.join(def.startstand.as_ref().unwrap())).unwrap();
    for e in extra {
        tekst.push('\n');
        tekst.push_str(&e.to_string());
    }
    let grammen = startstand::parse(&tekst, "startstand", &strommen).unwrap();
    let laadtijd = chrono::DateTime::parse_from_rfc3339(LAADTIJD).unwrap();
    (def, startstand::geplaatst(&grammen, &laadtijd).unwrap())
}

fn service_met(artikel: &Path) -> (Arc<LawExecutionService>, String) {
    let mut s = LawExecutionService::new();
    let id = s
        .load_law(&std::fs::read_to_string(artikel).unwrap())
        .unwrap();
    (Arc::new(s), id)
}

/// Vergelijk per input: de lexostatus via de reductie-DSL tegen die via de
/// engine, langs dezelfde route als de runtime met `CEL_REDUCTIE=vergelijk`
/// (afleidingen, extra velden, niet afgeleid en het gekozen gram). Geeft
/// de verschillen terug.
fn vergelijk(
    def: &reductie::LexostatusDefinitie,
    service: &Arc<LawExecutionService>,
    regeling: &str,
    grammen: &[Gram],
    inputs: &[Map<String, Value>],
) -> Vec<String> {
    let route = CelRoute {
        service: service.clone(),
        wijzen: BTreeMap::from([(
            def.name.clone(),
            Wijze::Engine {
                regeling: regeling.to_string(),
                artikel: None,
            },
        )]),
        vergelijk: true,
    };
    inputs
        .iter()
        .filter_map(|i| {
            lexostatus_engine::reduceer_lexostatus(
                &route,
                def,
                i,
                grammen,
                &Peil::default(),
                DATUM,
                false,
            )
            .err()
            .map(|f| format!("{i:?}: {f}"))
        })
        .collect()
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

/// Notitie "bron en gram-id", stap 4: de bevraging van een register staat in
/// het beleid van de beheerder; welke bron het register levert, zegt de
/// deployment. Hetzelfde beleidsartikel, een keer met de kroniek van de cel
/// als bron ([`lexostatus_engine::KroniekBron`]) en een keer met een oude API
/// ([`DictDataSource`], die per gevraagde aanduiding de passende records
/// teruggeeft): de uitkomst is dezelfde. Het beleid weet niet wat erachter
/// zit.
#[test]
fn een_register_uit_de_kroniek_en_uit_een_legacy_adapter_geeft_hetzelfde() {
    use regelrecht_engine::DictDataSource;
    const BELEID: &str = "testbeleid_registerhouder";
    let (_, grammen) = register();
    let tekst =
        std::fs::read_to_string(fixtures().join("beleid/testbeleid_registerhouder.yaml")).unwrap();
    let mut kroniek = LawExecutionService::new();
    kroniek.load_law(&tekst).unwrap();
    kroniek.add_data_source(Box::new(
        lexostatus_engine::KroniekBron::new(BELEID, &grammen, "test_register").unwrap(),
    ));
    // De oude API: per aanduiding de records van die aanduiding, in de vorm
    // die het beleid leest.
    let aanduidingen = ["VOORBEELD", "ANDERS", "ONBEKEND"];
    let records: Vec<BTreeMap<String, regelrecht_engine::Value>> = aanduidingen
        .iter()
        .map(|a| {
            let eigen: Vec<&Gram> = grammen
                .iter()
                .filter(|g| g.fields.get("aanduiding").and_then(Value::as_str) == Some(*a))
                .collect();
            let lijst = lexostatus_engine::als_kroniek(eigen, "test_register").unwrap();
            BTreeMap::from([
                (
                    "aanduiding".to_string(),
                    regelrecht_engine::Value::from(&json!(a)),
                ),
                (
                    "grammen".to_string(),
                    regelrecht_engine::Value::from(&lijst),
                ),
            ])
        })
        .collect();
    let mut api = LawExecutionService::new();
    api.load_law(&tekst).unwrap();
    api.add_data_source(Box::new(
        DictDataSource::from_records("register_api", 10, "aanduiding", records)
            .unwrap()
            .with_law_scope(BELEID),
    ));
    let uitkomsten = ["is_ingeschreven_in_register", "is_geschrapt"];
    let mut gezien = 0;
    for a in aanduidingen {
        for orgaan in ["raad", "staten"] {
            let p: BTreeMap<String, Value> = BTreeMap::from([
                ("aanduiding".into(), json!(a)),
                ("orgaan".into(), json!(orgaan)),
            ]);
            let k = regelrecht_cel::toets::evalueer(&kroniek, BELEID, &uitkomsten, &p, DATUM);
            let d = regelrecht_cel::toets::evalueer(&api, BELEID, &uitkomsten, &p, DATUM);
            assert!(k.volledig(&uitkomsten), "{a} {orgaan}: {k:?}");
            assert_eq!(k.waarden, d.waarden, "{a} {orgaan}");
            gezien += usize::from(k.waarden["is_ingeschreven_in_register"] == json!(true));
        }
    }
    assert_eq!(
        gezien, 2,
        "VOORBEELD en ANDERS staan bij de raad ingeschreven"
    );
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
        &["datum_mededeling", "geblokkeerd", "jaar_van_mededeling"],
        i,
        &grammen,
        &def.reduction.kroniek,
        DATUM,
    )
    .unwrap();
    assert_eq!(uit["datum_mededeling"], json!("2024-12-02"));
    assert_eq!(uit["geblokkeerd"], json!(true));
    assert_eq!(uit["jaar_van_mededeling"], json!(2024));
}

#[test]
fn geen_gram_is_nee_nul_of_weg() {
    let (def, grammen) = register();
    let (service, id) = service_met(&fixtures().join("experiment/lexostatus_registerstatus.yaml"));
    let namen = uitkomsten_van(&def);
    let uitkomsten: Vec<&str> = namen.iter().map(String::as_str).collect();
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
    assert_eq!(uit["zetels_toegewezen"], json!(0));
    assert_eq!(uit["geblokkeerd"], json!(false));
    assert!(!uit.contains_key("datum_mededeling"));
    assert!(!uit.contains_key("jaar_van_mededeling"));
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
    let namen = uitkomsten_van(def);
    let uitkomsten: Vec<&str> = namen.iter().map(String::as_str).collect();
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
        "{}: {} grammen, {n}x: dsl {:?}/run, engine {:?}/run",
        def.name,
        grammen.len(),
        dsl / n,
        engine / n
    );
}

/// Dezelfde vergelijking voor cellen en artikelen van buiten deze repo.
///
/// `EXP_VERGELIJK`: een json-lijst, een object per vergelijking, met
///
/// - `cel_map`: de map van de cel (met `cel.yaml`);
/// - `extra` (optioneel): een bestand met json-regels in de vorm van de
///   startstand, die erachter komen;
/// - `artikel`: het engine-artikel met de lexostatus;
/// - `lexostatus`: de naam van de lexostatus in de cel;
/// - `inputs`: een lijst inputs, een object per geval.
///
/// Zonder die variabele slaat de test over.
#[test]
fn vergelijk_uit_omgeving() {
    let Ok(invoer) = std::env::var("EXP_VERGELIJK") else {
        eprintln!("EXP_VERGELIJK niet gezet: overgeslagen");
        return;
    };
    let lijst: Vec<Value> = serde_json::from_str(&invoer).unwrap();
    let (mut gevallen, mut waarden) = (0, 0);
    let mut verschillen = Vec::new();
    for v in &lijst {
        let map = PathBuf::from(v["cel_map"].as_str().unwrap());
        let extra: Vec<Value> = match v.get("extra").and_then(Value::as_str) {
            Some(pad) => std::fs::read_to_string(pad)
                .unwrap()
                .lines()
                .filter(|r| !r.trim().is_empty())
                .map(|r| serde_json::from_str(r).unwrap())
                .collect(),
            None => Vec::new(),
        };
        let (def, grammen) = grammen_van(&map, &extra);
        let lexo = reductie::laad(&map.join(&def.lexostatussen)).unwrap();
        let naam = v["lexostatus"].as_str().unwrap();
        let def = lexo.lexostatus(naam).unwrap().clone();
        let (service, id) = service_met(Path::new(v["artikel"].as_str().unwrap()));
        let inputs: Vec<Map<String, Value>> = serde_json::from_value(v["inputs"].clone()).unwrap();
        let namen = uitkomsten_van(&def);
        let uitkomsten: Vec<&str> = namen.iter().map(String::as_str).collect();
        println!(
            "{naam} ({} grammen, {} extra): {} gevallen x {} uitkomsten {uitkomsten:?}",
            grammen.len(),
            extra.len(),
            inputs.len(),
            uitkomsten.len()
        );
        for i in &inputs {
            let uit = lexostatus_engine::reduceer(
                &service,
                &id,
                &uitkomsten,
                i,
                &grammen,
                &def.reduction.kroniek,
                DATUM,
            )
            .unwrap();
            println!("  {i:?}: {uit:?}");
        }
        verschillen.extend(
            vergelijk(&def, &service, &id, &grammen, &inputs)
                .into_iter()
                .map(|f| format!("{naam}: {f}")),
        );
        gevallen += inputs.len();
        waarden += inputs.len() * uitkomsten.len();
        meet(&def, &service, &id, &grammen, &inputs[0]);
    }
    println!(
        "vergeleken: {} lexostatus-runs, {gevallen} gevallen, {waarden} waarden",
        lijst.len()
    );
    assert!(verschillen.is_empty(), "{verschillen:#?}");
}

/// Stap 2: de synthese per regel als engine-run, voor een corpus van buiten
/// deze repo. Elke lexostatus is een regeling die haar kroniek via een
/// [`lexostatus_engine::KroniekBron`] krijgt; de synthese-regeling haalt ze
/// op met `source` en bouwt de tabel; daarna rekent de afnemende regeling.
///
/// - `EXP_REGULATION`: de map met de regelingen van het corpus;
/// - `EXP_SYNTHESE_MAP`: de map met de lexostatus- en synthese-regelingen
///   (een `koppeling.yaml` erin, voor de runtime, telt niet mee);
/// - `EXP_KOPPELING`: json-lijst `{regeling, cel_map, kroniek}` (welke
///   lexostatus-regeling welke kroniek leest: de deploymentconfiguratie);
/// - `EXP_SYNTHESE`: json `{regeling, uitkomst, jaar, inputs, verwacht}` van
///   de synthese; `verwacht` (optioneel) geeft per uitkomst de waarde die de
///   synthese van de cel-runtime oplevert (de tabel, de vertaalde
///   parameters);
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
    // De regelingen in de map; het koppelbestand van de runtime
    // (`koppeling.yaml`) is geen regeling.
    let bestanden = regelrecht_cel::laden::yaml_bestanden(Path::new(&map)).unwrap();
    for b in bestanden
        .iter()
        .filter(|b| b.file_name().is_some_and(|n| n != "koppeling.yaml"))
    {
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
    let verwacht: Map<String, Value> = s
        .get("verwacht")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let mut uitkomsten = vec![s["uitkomst"].as_str().unwrap(), s["jaar"].as_str().unwrap()];
    uitkomsten.extend(verwacht.keys().map(String::as_str));
    uitkomsten.sort_unstable();
    uitkomsten.dedup();
    let t = Instant::now();
    let e = regelrecht_cel::toets::evalueer_met_trace(
        &service,
        s["regeling"].as_str().unwrap(),
        &uitkomsten,
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
    let verschillen: Vec<String> = verwacht
        .iter()
        .filter(|(u, w)| e.waarden.get(*u) != Some(*w))
        .map(|(u, w)| format!("{u}: runtime {w}, engine {:?}", e.waarden.get(u)))
        .collect();
    println!(
        "synthese: {} van {} uitkomsten gelijk aan de runtime",
        verwacht.len() - verschillen.len(),
        verwacht.len()
    );
    assert!(verschillen.is_empty(), "{verschillen:#?}");
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
