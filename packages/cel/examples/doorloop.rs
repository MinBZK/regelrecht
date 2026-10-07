//! De doorloop: een aanvraag en een besluit als chronolex-feit, stap voor
//! stap, met per stap wat er gebeurt en waarom. Voor wie het wil zien in
//! plaats van lezen (regelrecht-corpus-NAPP, chronicles/README.md hoofdstuk 5
//! en 6).
//!
//! ```text
//! CEL_WPP_CORPUS=<corpus> cargo run -p regelrecht-cel --example doorloop
//! DOORLOOP_TRACE=1  ...       # ook de trace van de engine per uitvoering
//! ```
//!
//! De invoer van de casus staat in `<corpus>/doorloop/casus.yaml`. Het script
//! doet wat een proces straks doet: indienen, teruglezen, aanvullen, de wet
//! laten rekenen, vastleggen. Het bouwt niets na: de cel, de engine en het
//! beleid zijn de echte.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use chrono::DateTime;
use http_body_util::BodyExt;
use regelrecht_cel::assessment;
use regelrecht_cel::config::{Config, DEFAULT_PORT};
use regelrecht_cel::regulations;
use regelrecht_cel::runtime::Runtime;
use regelrecht_cel::transport::RUNTIME_TOKEN_HEADER;
use serde_json::{json, Map, Value};
use tower::ServiceExt;

const CELL_ID: &str = "autoriteit_politieke_partijen";
const CELL: &str = "/cells/autoriteit_politieke_partijen";
const AUTHORITY: &str = "nederlandse_autoriteit_politieke_partijen";
const WPP: &str = "wet_op_de_politieke_partijen";
const POLICY: &str = "kroniek_autoriteit";
const LEXOSTATUS: &str = "kroniek_autoriteit#aanvragen";
const OUTPUTS_107: [&str; 7] = [
    "besluitdeadline_art107",
    "directe_vaststelling_art107",
    "besluit_tot_subsidievaststelling_genomen_art107",
    "besluit_tijdig_art107",
    "vastgesteld_subsidiebedrag_art107",
    "beschikking_zorgvuldig_art107",
    "aanvrager_bestaat_op_besluitdatum_art107",
];

fn kop(n: u32, titel: &str) {
    println!("\n{}", "═".repeat(78));
    println!("  STAP {n} · {titel}");
    println!("{}", "═".repeat(78));
}

fn uitleg(tekst: &str) {
    for r in tekst.lines() {
        println!("  {r}");
    }
    println!();
}

fn toon(label: &str, v: &Value) {
    println!("  ▸ {label}");
    for r in serde_json::to_string_pretty(v).unwrap().lines() {
        println!("    {r}");
    }
    println!();
}

fn trace(e: &assessment::Evaluation) {
    if std::env::var_os("DOORLOOP_TRACE").is_none() {
        println!("  (zet DOORLOOP_TRACE=1 voor de trace van de engine)\n");
        return;
    }
    if let Some(t) = &e.trace_text {
        println!("  ▸ trace van de engine");
        for r in t.lines() {
            println!("    {r}");
        }
        println!();
    }
}

fn as_runtime(rt: &Runtime, method: &str, uri: &str, body: Option<Value>) -> (StatusCode, Value) {
    let req = Request::builder()
        .method(method)
        .uri(uri)
        .header(RUNTIME_TOKEN_HEADER, rt.runtime_token.as_str());
    let req = match body {
        Some(b) => req
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(b.to_string()))
            .unwrap(),
        None => req.body(Body::empty()).unwrap(),
    };
    let rt_handle = tokio::runtime::Handle::current();
    let router = rt.router.clone();
    let (status, bytes) = rt_handle.block_on(async move {
        let resp = router.oneshot(req).await.unwrap();
        let status = resp.status();
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        (status, bytes)
    });
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

/// Een kopie van de regelgeving zonder `uitvoeringsbeleid/`: het besluit
/// wordt op de wet genomen (README 9.3).
fn law_only(regulation: &Path) -> tempfile::TempDir {
    fn copy(from: &Path, to: &Path) {
        std::fs::create_dir_all(to).unwrap();
        for entry in std::fs::read_dir(from).unwrap() {
            let entry = entry.unwrap();
            let target = to.join(entry.file_name());
            if entry.file_type().unwrap().is_dir() {
                copy(&entry.path(), &target);
            } else {
                std::fs::copy(entry.path(), target).unwrap();
            }
        }
    }
    let dir = tempfile::Builder::new().prefix("wet").tempdir().unwrap();
    for entry in std::fs::read_dir(regulation.join("nl")).unwrap() {
        let entry = entry.unwrap();
        if entry.file_name() != "uitvoeringsbeleid" {
            copy(
                &entry.path(),
                &dir.path().join("nl").join(entry.file_name()),
            );
        }
    }
    dir
}

fn object(v: &Value, pad: &str) -> Map<String, Value> {
    v.get(pad)
        .and_then(Value::as_object)
        .unwrap_or_else(|| panic!("casus.yaml: '{pad}' ontbreekt of is geen map"))
        .clone()
}

fn main() {
    let root = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("CEL_WPP_CORPUS").map(PathBuf::from))
        .expect("geef de corpuswortel als argument of in CEL_WPP_CORPUS");
    let casus: Value = serde_yaml_ng::from_reader(
        std::fs::File::open(root.join("doorloop/casus.yaml")).expect("doorloop/casus.yaml"),
    )
    .expect("casus.yaml is geen geldige YAML");
    let tokio_rt = tokio::runtime::Runtime::new().unwrap();
    let _guard = tokio_rt.enter();

    // ------------------------------------------------------------------ 1
    kop(
        1,
        "LADEN · de runtime leest de wet, de cel, de stromen en de binding",
    );
    uitleg(
        "De runtime laadt alles onder regulation/ (wet en beleid), de cel onder cells/\n\
         en de registerbinding (deployment/registers.yaml). Voor het event\n\
         `aanvraag_ontvangen` voert zij Wpp 102 uit (execute_stage) met een lege\n\
         aanvraag; de engine wacht en geeft het model: welke artikelen meedoen\n\
         (Wpp 102, Awb 4:2 en 4:13 via hun haken) en welke parameters zij vragen.\n\
         Dat model zijn de velden van het gram. Niets hiervan staat in configuratie.",
    );
    let aanvraag = object(&casus, "aanvraag");
    let now = Arc::new(Mutex::new(
        aanvraag["ontvangen_op"]
            .as_str()
            .expect("aanvraag.ontvangen_op")
            .to_string(),
    ));
    let data = tempfile::tempdir().unwrap();
    let config = Config {
        cells_path: root.join("cells"),
        regulation_path: root.join("regulation"),
        data_dir: data.path().to_path_buf(),
        port: DEFAULT_PORT,
        read_token: None,
        read_token_sources: Vec::new(),
        reduction: Default::default(),
        registers: Some(root.join("deployment/registers.yaml")),
        channels: None,
        synthesis: None,
        examples: None,
    };
    let clock_now = now.clone();
    let clock = Arc::new(move || DateTime::parse_from_rfc3339(&clock_now.lock().unwrap()).unwrap());
    let rt = Runtime::load(&config, clock).unwrap_or_else(|e| panic!("runtime start niet: {e:#?}"));
    println!(
        "  kroniek: {}/{CELL_ID}/napp.jsonl\n",
        data.path().display()
    );

    for stream in &rt.cells[0].cell.streams {
        for e in &stream.events {
            println!(
                "  ▸ event `{}` (stroom {}): de velden van het gram, uit de wet",
                e.name, stream.id
            );
            for f in &e.field_defs {
                println!(
                    "    - {:<28} {:<8} uit {:<36} grondslag {}",
                    f.name,
                    f.type_.as_deref().unwrap_or("-"),
                    f.declared_by,
                    if f.legal_basis.is_empty() {
                        "(het artikel zelf)".to_string()
                    } else {
                        f.legal_basis.join(", ")
                    }
                );
            }
            let excluded = &e.explanation.excluded;
            if !excluded.is_empty() {
                println!("    en wat de wet ook vraagt, maar geen veld van dit gram is:");
                for x in excluded.iter().take(6) {
                    println!("    - {:<40} {}", x.parameter, x.reason);
                }
                if excluded.len() > 6 {
                    println!(
                        "    - ... en {} andere (GET {CELL}/api/stream toont alles)",
                        excluded.len() - 6
                    );
                }
            }
            println!();
        }
    }

    // ------------------------------------------------------------------ 2
    kop(
        2,
        "INDIENEN · de partij dient in, de cel legt het gram vast",
    );
    uitleg(
        "Het proces stuurt de inhoud van de aanvraag (doorloop/casus.yaml: aanvraag.external)\n\
         naar POST /api/grams. De cel controleert de actor, neemt alleen velden aan die de\n\
         wet vraagt, vult `gevraagde_beschikking` zelf uit Wpp 107 (decides_on), zet\n\
         effective_at op de ontvangst (Awb 4:13 lid 1) en schrijft één regel in de kroniek.",
    );
    let indiening = json!({
        "actor": AUTHORITY, "stream": "napp_aanvragen", "event": "aanvraag_ontvangen",
        "intake": aanvraag["intake"], "external": aanvraag["external"],
    });
    toon("wat het proces stuurt", &indiening);
    let (status, body) = as_runtime(&rt, "POST", &format!("{CELL}/api/grams"), Some(indiening));
    if status != StatusCode::CREATED {
        toon(&format!("de cel weigert ({status})"), &body);
        std::process::exit(1);
    }
    let gram = body["gram"].clone();
    let id = gram["id"].as_str().unwrap().to_string();
    toon("het gram zoals het in de kroniek staat", &gram);
    let mut proef = object(&casus, "aanvraag");
    proef["external"]["schoenmaat"] = json!(44);
    let (status, body) = as_runtime(
        &rt,
        "POST",
        &format!("{CELL}/api/grams"),
        Some(
            json!({"actor": AUTHORITY, "stream": "napp_aanvragen", "event": "aanvraag_ontvangen",
                    "intake": proef["intake"], "external": proef["external"]}),
        ),
    );
    println!("  ▸ tegenproef: dezelfde aanvraag met een veld `schoenmaat` erbij");
    println!("    {status}: {}\n", body["error"].as_str().unwrap_or("?"));

    // ------------------------------------------------------------------ 3
    kop(
        3,
        "TERUGLEZEN · de Autoriteit leest de aanvraag terug via haar beleid",
    );
    uitleg(
        "Wpp 102 vraagt om gegevens (de parameters met hun `origin`). Hoe de Autoriteit die\n\
         uit haar kroniek haalt, staat in haar beleid: kroniek_autoriteit art. 1, in het\n\
         wetsformaat. De deployment bindt dat artikel aan de kroniek (register `aanvragen`).\n\
         De engine voert het uit met het id van het gram als `root`.",
    );
    let besluit = object(&casus, "besluit");
    *now.lock().unwrap() = besluit["genomen_op"]
        .as_str()
        .expect("besluit.genomen_op")
        .to_string();
    let service = &rt.cells[0].cell.service;
    let asked: Vec<&str> = vec![
        "laatste",
        "subsidiejaar",
        "aanvraagdatum",
        "statutaire_naam",
        "geregistreerde_aanduiding",
        "samengevoegde_aanduiding",
        "zeteltabel",
    ];
    let datum_besluit = besluit["besluitdatum"]
        .as_str()
        .expect("besluit.besluitdatum")
        .to_string();
    let read = assessment::evaluate_with_trace(
        service,
        POLICY,
        &asked,
        &BTreeMap::from([("root".to_string(), json!(id))]),
        &datum_besluit,
    );
    if let Some(e) = &read.error {
        println!("  het beleid rekent niet: {e}");
        std::process::exit(1);
    }
    let mut from_chronicle: Map<String, Value> = read.values.clone().into_iter().collect();
    let laatste = from_chronicle.remove("laatste");
    println!(
        "  ▸ welk gram gold: plaats {} in de kroniek (art. 1 lid 2: de laatste)\n",
        laatste.unwrap_or(Value::Null)
    );
    toon(
        "de parameters van Wpp 102, zoals het beleid ze uit het gram afleidt",
        &Value::Object(from_chronicle.clone()),
    );
    trace(&read);

    // ------------------------------------------------------------------ 4
    kop(
        4,
        "AANVULLEN · wat anderen weten en wat de Autoriteit zelf oordeelt",
    );
    uitleg(
        "Art. 107 leest meer dan de aanvraag: de registratie (Kiesraad), het inwonertal (CBS),\n\
         en het oordeel en dossier van de Autoriteit. Elke invoer krijgt haar herkomst\n\
         (provenance), zodat het besluit later te herhalen is (RFC-013). De bekendmaking\n\
         is nog niet gebeurd: die gaat niet naar de engine, wel als null in het decretogram.",
    );
    let mut parameters: BTreeMap<String, Value> = BTreeMap::new();
    let mut inputs = Map::new();
    let mut put = |bron: &Map<String, Value>, provenance: Value| {
        for (k, v) in bron {
            parameters.insert(k.clone(), v.clone());
            inputs.insert(k.clone(), json!({"value": v, "provenance": provenance}));
        }
    };
    let mut own = from_chronicle.clone();
    let mut tabel = own.remove("zeteltabel").unwrap_or(json!([]));
    put(&own, json!({"source": "own", "lexostatus": LEXOSTATUS}));
    let per_rij = object(&casus, "per_rij");
    for rij in tabel.as_array_mut().unwrap() {
        for (k, v) in &per_rij {
            rij[k] = v.clone();
        }
    }
    put(
        &Map::from_iter([("zeteltabel".to_string(), tabel.clone())]),
        json!({"source": "per_row", "lexostatus": LEXOSTATUS, "field": "zeteltabel"}),
    );
    put(
        &object(&casus, "kiesraad"),
        json!({"source": "cell", "cell": "kiesraad", "lexostatus": "registratie", "transport": "vaste waarden in doorloop/casus.yaml"}),
    );
    let mut handler = object(&casus, "autoriteit");
    handler.insert("besluitdatum".into(), besluit["besluitdatum"].clone());
    put(&handler, json!({"source": "handler"}));
    let nog_niet = &casus["nog_niet_gebeurd"];
    for p in nog_niet["parameters"].as_array().unwrap_or(&Vec::new()) {
        inputs.insert(
            p.as_str().unwrap().to_string(),
            json!({"value": null, "provenance": {"source": "state_at_decision", "stage": nog_niet["stage"]}}),
        );
    }
    let mut per_bron: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (k, v) in &inputs {
        per_bron
            .entry(
                v["provenance"]["source"]
                    .as_str()
                    .unwrap_or("?")
                    .to_string(),
            )
            .or_default()
            .push(k.clone());
    }
    for (bron, namen) in &per_bron {
        println!("  ▸ herkomst `{bron}`: {} invoer(en)", namen.len());
        println!("    {}\n", namen.join(", "));
    }
    toon(
        "de zeteltabel, per rij aangevuld door Kiesraad en CBS",
        &tabel,
    );

    // ------------------------------------------------------------------ 5
    kop(5, "UITVOEREN · de engine voert Wpp 107 uit, op de wet");
    uitleg(
        "Zonder uitvoeringsbeleid/ (README 9.3): de wet rekent, het beleid levert alleen de\n\
         feiten uit de kroniek. Art. 107 roept zelf 102, 105, 106 en 110 aan, en de Awb.\n\
         Elke uitkomst wordt een veld van het decretogram; wat de engine niet weet, blijft\n\
         onbekend en wacht op een later feit.",
    );
    let wet = law_only(&root.join("regulation"));
    let corpus = regulations::load(wet.path()).unwrap_or_else(|e| panic!("{e:#?}"));
    let evaluation = assessment::evaluate_with_trace(
        &corpus.service,
        WPP,
        &OUTPUTS_107,
        &parameters,
        &datum_besluit,
    );
    if let Some(e) = &evaluation.error {
        println!("  de engine rekent niet: {e}");
        std::process::exit(1);
    }
    let mut uitkomsten = Map::new();
    for o in OUTPUTS_107 {
        uitkomsten.insert(
            o.into(),
            evaluation.values.get(o).cloned().unwrap_or(Value::Null),
        );
    }
    toon(
        "de uitkomsten van art. 107 (null = onbekend)",
        &Value::Object(uitkomsten.clone()),
    );
    for (o, mist) in &evaluation.missing_per {
        println!("  ▸ `{o}` blijft onbekend; wacht op: {}\n", mist.join(", "));
    }
    if let Some(b) = evaluation
        .values
        .get("vastgesteld_subsidiebedrag_art107")
        .and_then(Value::as_i64)
    {
        println!(
            "  ▸ het vastgestelde bedrag: EUR {},{:02}\n",
            b / 100,
            b % 100
        );
    }
    trace(&evaluation);

    // ------------------------------------------------------------------ 6
    kop(
        6,
        "VASTLEGGEN · het decretogram, verwijzend naar de aanvraag",
    );
    uitleg(
        "Wpp 107 vestigt het besluit (establishes: decretogram, stage BESLUIT, refers_to\n\
         on_application). De cel controleert dat de aanvraag bestaat, dat het besluit niet\n\
         vóór de aanvraag ligt en dat er per aanvraag één besluit is. De velden zijn de\n\
         uitkomsten van art. 107; de invoer gaat mee met haar herkomst.",
    );
    let wpp = corpus.regulations.iter().find(|r| r.id == WPP).unwrap();
    let mut external = uitkomsten.clone();
    external.insert("besluitdatum".into(), besluit["besluitdatum"].clone());
    let decision = json!({
        "actor": AUTHORITY, "stream": "napp_besluiten", "event": "subsidie_vastgesteld",
        "intake": {"channel": "autoriteit"},
        "external": external,
        "refers_to": {"on_application": id},
        "decision": {
            "legal_character": "BESCHIKKING", "decision_type": "TOEKENNING",
            "regulation": WPP, "regulation_valid_from": wpp.valid_from,
            "competent_authority": "Nederlandse autoriteit politieke partijen",
            "inputs": inputs,
        },
    });
    let (status, body) = as_runtime(
        &rt,
        "POST",
        &format!("{CELL}/api/grams"),
        Some(decision.clone()),
    );
    if status != StatusCode::CREATED {
        toon(&format!("de cel weigert ({status})"), &body);
        std::process::exit(1);
    }
    let mut kort = body["gram"].clone();
    if let Some(o) = kort.as_object_mut() {
        o.remove("inputs");
    }
    toon(
        "het decretogram (zonder `inputs`, die staan hierboven)",
        &kort,
    );
    let (status, body) = as_runtime(&rt, "POST", &format!("{CELL}/api/grams"), Some(decision));
    println!("  ▸ tegenproef: een tweede besluit op dezelfde aanvraag");
    println!("    {status}: {}\n", body["error"].as_str().unwrap_or("?"));

    let (_, zaak) = as_runtime(&rt, "GET", &format!("{CELL}/api/cases/{id}"), None);
    println!("  ▸ de zaak zoals de cel haar ziet (api/cases/<root>): de grammen die naar elkaar verwijzen");
    for g in zaak.as_array().unwrap_or(&Vec::new()) {
        let g = &g["gram"];
        println!(
            "    - {:<22} {:<12} stage {:<10} op {}  id {}",
            g["name"].as_str().unwrap_or("?"),
            g["type"].as_str().unwrap_or("?"),
            g["stage"].as_str().unwrap_or("?"),
            g["effective_at"].as_str().unwrap_or("?"),
            g["id"].as_str().unwrap_or("?")
        );
    }
    println!();
    let kroniek = data.path().join(CELL_ID).join("napp.jsonl");
    let regels = std::fs::read_to_string(&kroniek).unwrap();
    println!(
        "  ▸ de kroniek: {} regel(s) in {}",
        regels.lines().count(),
        kroniek.display()
    );
    println!("    (tijdelijke map; verdwijnt als dit script stopt)\n");
    println!(
        "{}\n  KLAAR · aanvraag en besluit staan als twee grammen in de kroniek.\n{}",
        "═".repeat(78),
        "═".repeat(78)
    );
}
