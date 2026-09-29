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
    let mut streams = Vec::new();
    for s in &def.streams {
        streams.extend(stroom::laad(&cel_map.join(s)).unwrap());
    }
    // Een stroom met `vestigt` krijgt zijn vorm uit de wet: met
    // `EXP_REGULATION` het corpus waarin die wet staat.
    if let Ok(path) = std::env::var("EXP_REGULATION") {
        let corpus = regelrecht_cel::regelingen::laad(Path::new(&path)).unwrap();
        let fouten = regelrecht_cel::wet::vestig(&mut streams, &corpus.service);
        assert!(fouten.is_empty(), "{fouten:?}");
    }
    stroom::leid_rollen_af(&mut streams);
    let mut tekst =
        std::fs::read_to_string(cel_map.join(def.initial_state.as_ref().unwrap())).unwrap();
    for e in extra {
        tekst.push('\n');
        tekst.push_str(&e.to_string());
    }
    let grams = startstand::parse(&tekst, "startstand", &streams).unwrap();
    let laadtijd = chrono::DateTime::parse_from_rfc3339(LAADTIJD).unwrap();
    (def, startstand::geplaatst(&grams, &laadtijd).unwrap())
}

fn service_met(article: &Path) -> (Arc<LawExecutionService>, String) {
    let mut s = LawExecutionService::new();
    let id = s
        .load_law(&std::fs::read_to_string(article).unwrap())
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
    regulation: &str,
    grams: &[Gram],
    inputs: &[Map<String, Value>],
) -> Vec<String> {
    let route = CelRoute {
        service: service.clone(),
        wijzen: BTreeMap::from([(
            def.name.clone(),
            Wijze::Engine {
                regulation: regulation.to_string(),
                article: None,
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
                grams,
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
    let map = fixtures().join("cells/register");
    // Meer dan de startstand: een schrapping, een tweede uitslag, en twee
    // mededelingen waarvan de laatste in een andere tijdzone staat (08:30Z is
    // later dan 09:00+01:00). Zo telt `kies: laatste` op het moment, niet op
    // de tekst of de volgorde van toevoegen.
    let extra = [
        json!({"stream": "test_registers", "name": "aanduiding_ingeschreven", "effective_at": "2024-01-11T09:00:00+01:00", "provenance": "initial_state", "fields": {"aanduiding": "ANDERS", "orgaan": "raad", "gebied": "Buurdorp"}}),
        json!({"stream": "test_registers", "name": "aanduiding_geschrapt", "effective_at": "2024-06-01T09:00:00+01:00", "provenance": "initial_state", "fields": {"aanduiding": "ANDERS", "orgaan": "raad"}}),
        json!({"stream": "test_registers", "name": "uitslag_vastgesteld", "effective_at": "2024-03-21T09:00:00+01:00", "provenance": "initial_state", "fields": {"orgaan": "raad", "gebied": "Buurdorp", "lijst": "ANDERS", "zetels": 7}}),
        json!({"stream": "test_registers", "name": "mededeling_gedaan", "effective_at": "2024-12-02T08:30:00+00:00", "provenance": "initial_state", "fields": {"aanduiding": "VOORBEELD", "datum": "2024-12-02", "geblokkeerd_voor": ["raad"]}}),
        json!({"stream": "test_registers", "name": "mededeling_gedaan", "effective_at": "2024-12-02T09:00:00+01:00", "provenance": "initial_state", "fields": {"aanduiding": "VOORBEELD", "datum": "2024-12-01", "geblokkeerd_voor": []}}),
    ];
    let (def, grams) = grammen_van(&map, &extra);
    let lexo = reductie::laad(&map.join(&def.lexostatuses)).unwrap();
    (lexo.lexostatus("registerstatus").unwrap().clone(), grams)
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
    let (_, grams) = register();
    let tekst =
        std::fs::read_to_string(fixtures().join("beleid/testbeleid_registerhouder.yaml")).unwrap();
    let mut chronicle = LawExecutionService::new();
    chronicle.load_law(&tekst).unwrap();
    chronicle.add_data_source(Box::new(
        lexostatus_engine::KroniekBron::new(BELEID, &grams, "test_register").unwrap(),
    ));
    // De oude API: per aanduiding de records van die aanduiding, in de vorm
    // die het beleid leest.
    let aanduidingen = ["VOORBEELD", "ANDERS", "ONBEKEND"];
    let records: Vec<BTreeMap<String, regelrecht_engine::Value>> = aanduidingen
        .iter()
        .map(|a| {
            let eigen: Vec<&Gram> = grams
                .iter()
                .filter(|g| g.fields.get("aanduiding").and_then(Value::as_str) == Some(*a))
                .collect();
            let list = lexostatus_engine::als_kroniek(eigen, "test_register").unwrap();
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
    api.load_law(&tekst).unwrap();
    api.add_data_source(Box::new(
        DictDataSource::from_records("register_api", 10, "aanduiding", records)
            .unwrap()
            .with_law_scope(BELEID),
    ));
    let outputs = ["is_ingeschreven_in_register", "is_geschrapt"];
    let mut gezien = 0;
    for a in aanduidingen {
        for orgaan in ["raad", "staten"] {
            let p: BTreeMap<String, Value> = BTreeMap::from([
                ("aanduiding".into(), json!(a)),
                ("orgaan".into(), json!(orgaan)),
            ]);
            let k = regelrecht_cel::toets::evalueer(&chronicle, BELEID, &outputs, &p, DATUM);
            let d = regelrecht_cel::toets::evalueer(&api, BELEID, &outputs, &p, DATUM);
            assert!(k.volledig(&outputs), "{a} {orgaan}: {k:?}");
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
    let (def, grams) = register();
    let (service, id) = service_met(&fixtures().join("experiment/lexostatus_registerstatus.yaml"));
    let v = vergelijk(
        &def,
        &service,
        &id,
        &grams,
        &inputs(&["VOORBEELD", "ANDERS", "ONBEKEND"]),
    );
    assert!(v.is_empty(), "{v:#?}");
}

#[test]
fn laatste_is_op_moment_niet_op_toevoegen() {
    let (def, grams) = register();
    let (service, id) = service_met(&fixtures().join("experiment/lexostatus_registerstatus.yaml"));
    let i = &inputs(&["VOORBEELD"])[0];
    let uit = lexostatus_engine::reduceer(
        &service,
        &id,
        &["datum_mededeling", "geblokkeerd", "jaar_van_mededeling"],
        i,
        &grams,
        &def.reduction.chronicle,
        DATUM,
    )
    .unwrap();
    assert_eq!(uit["datum_mededeling"], json!("2024-12-02"));
    assert_eq!(uit["geblokkeerd"], json!(true));
    assert_eq!(uit["jaar_van_mededeling"], json!(2024));
}

#[test]
fn geen_gram_is_nee_nul_of_weg() {
    let (def, grams) = register();
    let (service, id) = service_met(&fixtures().join("experiment/lexostatus_registerstatus.yaml"));
    let namen = uitkomsten_van(&def);
    let outputs: Vec<&str> = namen.iter().map(String::as_str).collect();
    let uit = lexostatus_engine::reduceer(
        &service,
        &id,
        &outputs,
        &inputs(&["ONBEKEND"])[0],
        &grams,
        &def.reduction.chronicle,
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
        let mut grams = basis.clone();
        for i in 0..extra {
            let mut g = basis[0].clone();
            g.fields
                .insert("aanduiding".into(), json!(format!("ANDER{i}")));
            grams.push(g);
        }
        meet(&def, &service, &id, &grams, &inputs(&["VOORBEELD"])[0]);
    }
}

fn meet(
    def: &reductie::LexostatusDefinitie,
    service: &LawExecutionService,
    id: &str,
    grams: &[Gram],
    i: &Map<String, Value>,
) {
    let n = 200;
    let namen = uitkomsten_van(def);
    let outputs: Vec<&str> = namen.iter().map(String::as_str).collect();
    let t = Instant::now();
    for _ in 0..n {
        reductie::reduceer(def, i, grams).unwrap();
    }
    let dsl = t.elapsed();
    let t = Instant::now();
    for _ in 0..n {
        lexostatus_engine::reduceer(
            service,
            id,
            &outputs,
            i,
            grams,
            &def.reduction.chronicle,
            DATUM,
        )
        .unwrap();
    }
    let engine = t.elapsed();
    println!(
        "{}: {} grammen, {n}x: dsl {:?}/run, engine {:?}/run",
        def.name,
        grams.len(),
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
fn compare_from_env() {
    let Ok(input) = std::env::var("EXP_COMPARE") else {
        eprintln!("EXP_VERGELIJK niet gezet: overgeslagen");
        return;
    };
    let list: Vec<Value> = serde_json::from_str(&input).unwrap();
    let (mut gevallen, mut waarden) = (0, 0);
    let mut verschillen = Vec::new();
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
        let (def, grams) = grammen_van(&map, &extra);
        let lexo = reductie::laad(&map.join(&def.lexostatuses)).unwrap();
        let name = v["lexostatus"].as_str().unwrap();
        let def = lexo.lexostatus(name).unwrap().clone();
        let (service, id) = service_met(Path::new(v["article"].as_str().unwrap()));
        let inputs: Vec<Map<String, Value>> = serde_json::from_value(v["inputs"].clone()).unwrap();
        let namen = uitkomsten_van(&def);
        let outputs: Vec<&str> = namen.iter().map(String::as_str).collect();
        println!(
            "{name} ({} grammen, {} extra): {} gevallen x {} uitkomsten {outputs:?}",
            grams.len(),
            extra.len(),
            inputs.len(),
            outputs.len()
        );
        for i in &inputs {
            let uit = lexostatus_engine::reduceer(
                &service,
                &id,
                &outputs,
                i,
                &grams,
                &def.reduction.chronicle,
                DATUM,
            )
            .unwrap();
            println!("  {i:?}: {uit:?}");
        }
        verschillen.extend(
            vergelijk(&def, &service, &id, &grams, &inputs)
                .into_iter()
                .map(|f| format!("{name}: {f}")),
        );
        gevallen += inputs.len();
        waarden += inputs.len() * outputs.len();
        meet(&def, &service, &id, &grams, &inputs[0]);
    }
    println!(
        "vergeleken: {} lexostatus-runs, {gevallen} gevallen, {waarden} waarden",
        list.len()
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
fn synthesis_from_env() {
    let (Ok(corpus), Ok(map), Ok(koppeling), Ok(synthesis), Ok(afnemer)) = (
        std::env::var("EXP_REGULATION"),
        std::env::var("EXP_SYNTHESIS_DIR"),
        std::env::var("EXP_BINDING"),
        std::env::var("EXP_SYNTHESIS"),
        std::env::var("EXP_CONSUMER"),
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
        let (_, grams) = grammen_van(Path::new(k["cell_dir"].as_str().unwrap()), &[]);
        service.add_data_source(Box::new(
            lexostatus_engine::KroniekBron::new(
                k["regulation"].as_str().unwrap(),
                &grams,
                k["chronicle"].as_str().unwrap(),
            )
            .unwrap(),
        ));
    }
    println!("laden: {:?}", t.elapsed());
    let s: Value = serde_json::from_str(&synthesis).unwrap();
    let a: Value = serde_json::from_str(&afnemer).unwrap();
    let input: BTreeMap<String, Value> = serde_json::from_value(s["inputs"].clone()).unwrap();
    let verwacht: Map<String, Value> = s
        .get("expected")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let mut outputs = vec![s["output"].as_str().unwrap(), s["year"].as_str().unwrap()];
    outputs.extend(verwacht.keys().map(String::as_str));
    outputs.sort_unstable();
    outputs.dedup();
    let t = Instant::now();
    let e = regelrecht_cel::toets::evalueer_met_trace(
        &service,
        s["regulation"].as_str().unwrap(),
        &outputs,
        &input,
        DATUM,
    );
    println!("synthesis: {:?}", t.elapsed());
    assert!(
        e.error.is_none() && e.missing.is_empty(),
        "{:?} {:?}\n{}",
        e.error,
        e.missing,
        e.trace_text.unwrap_or_default()
    );
    let verschillen: Vec<String> = verwacht
        .iter()
        .filter(|(u, w)| e.waarden.get(*u) != Some(*w))
        .map(|(u, w)| format!("{u}: runtime {w}, engine {:?}", e.waarden.get(u)))
        .collect();
    println!(
        "synthesis: {} van {} uitkomsten gelijk aan de runtime",
        verwacht.len() - verschillen.len(),
        verwacht.len()
    );
    assert!(verschillen.is_empty(), "{verschillen:#?}");
    let table = e.waarden[s["output"].as_str().unwrap()].clone();
    let jaar = e.waarden[s["year"].as_str().unwrap()].clone();
    println!("table: {table}\njaar: {jaar}");
    let mut p = BTreeMap::new();
    p.insert(a["table"].as_str().unwrap().to_string(), table);
    p.insert(a["year"].as_str().unwrap().to_string(), jaar);
    let r = regelrecht_cel::toets::evalueer(
        &service,
        a["regulation"].as_str().unwrap(),
        &[a["output"].as_str().unwrap()],
        &p,
        DATUM,
    );
    assert!(
        r.error.is_none() && r.missing.is_empty(),
        "{:?} {:?}",
        r.error,
        r.missing
    );
    let bedrag = &r.waarden[a["output"].as_str().unwrap()];
    println!("{}: {bedrag}", a["output"]);
    assert_eq!(bedrag, &a["expected"]);
}

/// Een gram voor de toets van stap 8, zoals een cel het vastlegde.
fn stap8_gram(
    id: &str,
    name: &str,
    soort: &str,
    stage: Option<&str>,
    refers_to: Value,
    fields: Value,
    dag: &str,
) -> Gram {
    let mut g = json!({
        "kind": "chronolexogram", "id": id, "type": if stage.is_some() { "decretogram" } else { "executogram" },
        "subtype": soort, "name": name, "chronicle": "test_toeslag", "recording_actor": "test_toeslagdienst",
        "legal_basis": ["testregeling_toeslag#1"], "effective_at": format!("{dag}T10:00:00+01:00"),
        "recorded_at": format!("{dag}T10:00:00+01:00"), "refers_to": refers_to,
        "stream": {"id": "test_toeslag_zaakverloop", "sha256": "0".repeat(64)}, "fields": fields,
    });
    if let Some(s) = stage {
        g["stage"] = json!(s);
    }
    serde_json::from_value(g).unwrap()
}

/// Notitie "bron en gram-id", stap 8, naar de echte tekst: Awb 4:52 lid 1
/// ("overeenkomstig de subsidievaststelling") leest per besluit, en Awb 4:95
/// lid 4 ("Betaalde voorschotten worden verrekend met de te betalen
/// geldsom") verrekent de voorschotten. Een fictieve toeslag: voorschot 500
/// (betaald), vaststelling 300. Per besluit is op de vaststelling niets
/// betaald; met de verrekening is er niets meer te betalen en is 200
/// onverschuldigd, en dat is wat de terugvordering (die naar de vaststelling
/// verwijst, en zo bij dezelfde aanvraag hoort) terugvordert. Het beleid van
/// de toeslagdienst voert beide artikelen uit met een gewone source; de
/// betalingen komen uit zijn eigen administratie (een kroniek als bron).
#[test]
fn voorschot_en_vaststelling_per_besluit_met_verrekening() {
    const BELEID: &str = "testbeleid_toeslagdienst";
    let id = |n: u32| format!("00000000-0000-4000-8000-{n:012}");
    let (g101, g110, g111, g120, g130, g131) =
        (id(101), id(110), id(111), id(120), id(130), id(131));
    let grams = vec![
        stap8_gram(
            &g101,
            "aanvraag_ontvangen",
            "aanvraag",
            Some("AANVRAAG"),
            json!({}),
            json!({}),
            "2025-03-01",
        ),
        stap8_gram(
            &g110,
            "voorschot_verleend",
            "voorschot",
            Some("BESLUIT"),
            json!({"on_application": g101}),
            json!({"voorschot": 500}),
            "2025-03-02",
        ),
        stap8_gram(
            &g111,
            "voorschot_betaald",
            "betaling",
            None,
            json!({"decision": g110}),
            json!({"bedrag": 500}),
            "2025-03-03",
        ),
        stap8_gram(
            &g120,
            "toeslag_vastgesteld",
            "vaststelling",
            Some("BESLUIT"),
            json!({"on_application": g101}),
            json!({"vastgestelde_toeslag": 300}),
            "2025-03-10",
        ),
    ];
    // De kroniek: de vaststelling hoort via haar verwijzing bij de aanvraag.
    let dir = tempfile::tempdir().unwrap();
    let chronicle = regelrecht_cel::kroniek::Kroniek::open(dir.path(), &["test_toeslag"]).unwrap();
    for g in &grams {
        chronicle.voeg_toe(g).unwrap();
    }
    assert_eq!(
        chronicle
            .lees_wortel(&["test_toeslag"], &g101)
            .unwrap()
            .len(),
        4
    );

    let evalueer = |grams: &[Gram], regulation: &str, outputs: &[&str], p: Value| {
        let mut corpus = regelrecht_cel::regelingen::laad(&fixtures().join("regulation")).unwrap();
        corpus
            .service
            .load_law(
                &std::fs::read_to_string(fixtures().join("beleid/testbeleid_toeslagdienst.yaml"))
                    .unwrap(),
            )
            .unwrap();
        corpus.service.add_data_source(Box::new(
            lexostatus_engine::KroniekBron::new(BELEID, grams, "test_toeslag").unwrap(),
        ));
        let p: BTreeMap<String, Value> = serde_json::from_value(p).unwrap();
        let e = regelrecht_cel::toets::evalueer(&corpus.service, regulation, outputs, &p, DATUM);
        assert!(e.volledig(outputs), "{e:?}");
        e.waarden
    };
    // De administratie: per besluit, en de voorschotten op dezelfde aanvraag.
    let a = evalueer(
        &grams,
        BELEID,
        &["betaald_bij_besluit", "betaalde_voorschotten"],
        json!({"besluit": g120}),
    );
    assert_eq!(
        a["betaald_bij_besluit"],
        json!(0),
        "op de vaststelling zelf is niets betaald (4:52 lid 1)"
    );
    assert_eq!(
        a["betaalde_voorschotten"],
        json!(500),
        "het voorschot op dezelfde aanvraag (4:95 lid 4)"
    );
    // De verplichting: niets meer te betalen, 200 onverschuldigd.
    let vaststelling =
        json!({"besluit": g120, "vastgesteld_bedrag": 300, "datum_bekendmaking": "2025-03-11"});
    let v = evalueer(
        &grams,
        BELEID,
        &[
            "nog_te_betalen_verstrekker",
            "onverschuldigd_betaald_verstrekker",
        ],
        vaststelling.clone(),
    );
    assert_eq!(v["nog_te_betalen_verstrekker"], json!(0));
    assert_eq!(v["onverschuldigd_betaald_verstrekker"], json!(200));
    // Alleen 4:52 per besluit, zonder verrekening, zou 300 te betalen geven:
    // de dienst zou dan dubbel betalen.
    let alleen = evalueer(
        &grams,
        "testregeling_awb",
        &["nog_te_betalen"],
        json!({"vastgesteld_bedrag": 300, "betaald_bedrag": 0, "datum_bekendmaking": "2025-03-11"}),
    );
    assert_eq!(alleen["nog_te_betalen"], json!(300));

    // De terugvordering: een ambtshalve besluit zonder aanvraag, dat met
    // betreft naar de vaststelling verwijst en zo bij dezelfde aanvraag
    // hoort; zij vordert terug wat onverschuldigd is. De terugbetaling
    // verwijst naar de terugvordering en telt niet als betaling op de
    // vaststelling.
    let mut verder = grams.clone();
    verder.push(stap8_gram(
        &g130,
        "terugvordering_vastgesteld",
        "terugvordering",
        Some("BESLUIT"),
        json!({"concerns": g120}),
        json!({"terug_te_vorderen": v["onverschuldigd_betaald_verstrekker"].clone()}),
        "2025-03-12",
    ));
    verder.push(stap8_gram(
        &g131,
        "terugbetaling_ontvangen",
        "terugbetaling",
        None,
        json!({"besluit": g130}),
        json!({"bedrag": 200}),
        "2025-03-12",
    ));
    for g in &verder[4..] {
        chronicle.voeg_toe(g).unwrap();
    }
    let group = chronicle.lees_wortel(&["test_toeslag"], &g101).unwrap();
    assert_eq!(
        group.len(),
        6,
        "de terugvordering hoort via betreft bij de aanvraag"
    );
    assert_eq!(group[4].gram.root.as_deref(), Some(g101.as_str()));
    let after = evalueer(
        &verder,
        BELEID,
        &["betaald_bij_besluit", "betaalde_voorschotten"],
        json!({"besluit": g120}),
    );
    assert_eq!(after["betaald_bij_besluit"], json!(0));
    assert_eq!(after["betaalde_voorschotten"], json!(500));
    // Een ambtshalve besluit zonder voorganger is zijn eigen wortel.
    let g300 = id(300);
    chronicle
        .voeg_toe(&stap8_gram(
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
        chronicle
            .lees_wortel(&["test_toeslag"], &g300)
            .unwrap()
            .len(),
        1
    );
}

/// Stap 8 voor een corpus buiten deze repo (bijvoorbeeld de variant van een
/// echte casus), met paden uit de omgeving; zonder `EXP_BETALING` slaat hij
/// over. `EXP_BETALING`: json `{regulation, grammen (jsonl), beleid,
/// kroniek, gevallen: [{regeling, uitkomsten, parameters, datum,
/// verwacht}]}`: het beleid krijgt de grammen als bron van zijn register, en
/// elk geval moet zijn verwachte uitkomsten geven.
#[test]
fn payment_from_env() {
    let Ok(input) = std::env::var("EXP_PAYMENT") else {
        eprintln!("EXP_BETALING niet gezet: overgeslagen");
        return;
    };
    let v: Value = serde_json::from_str(&input).unwrap();
    let mut corpus =
        regelrecht_cel::regelingen::laad(Path::new(v["regulation"].as_str().unwrap())).unwrap();
    let grams: Vec<Gram> = std::fs::read_to_string(v["grams"].as_str().unwrap())
        .unwrap()
        .lines()
        .filter(|r| !r.trim().is_empty())
        .map(|r| serde_json::from_str(r).unwrap())
        .collect();
    corpus.service.add_data_source(Box::new(
        lexostatus_engine::KroniekBron::new(
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
        let e = regelrecht_cel::toets::evalueer(
            &corpus.service,
            g["regulation"].as_str().unwrap(),
            &outputs,
            &p,
            g["date"].as_str().unwrap(),
        );
        println!("{} {:?}: {:?}", g["regulation"], outputs, e.waarden);
        assert!(e.volledig(&outputs), "{e:?}");
        for (k, w) in g["expected"].as_object().unwrap() {
            assert_eq!(&e.waarden[k], w, "{k}");
        }
    }
}
