//! Het **ritme** van een verplichting als uitkomst van het besluit.
//!
//! `ritme: $naam` zoekt eerst een uitkomst — van het artikel dat de verplichting
//! declareert, of van `outputs` van de besluit-definitie — en pas daarna een
//! instelling van het wereldbestand. Wat dat oplevert, staat in
//! `scenarios/ritme_uit_uitkomst.yaml`: onder een drempel ineens, erboven per
//! kwartaal. Wat hier staat, is wat een scenariobestand niet kan dragen: waar het
//! gram zegt dat het ritme vandaan kwam, wat het schema erover zegt, een naam die
//! op beide plekken bestaat, en een uitkomst die geen ritme is.

use regelrecht_simulator::{
    regulation_root, DecretogramField, Herkomst, Scenario, ScenarioRun, Schedule, ScheduleOrigin,
    SimulatorError, Value, World,
};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

fn scenario_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("scenarios")
        .join("ritme_uit_uitkomst.yaml")
}

fn scenario() -> Scenario {
    let path = scenario_path();
    Scenario::load(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn run() -> ScenarioRun {
    let run = scenario()
        .run(&regulation_root())
        .unwrap_or_else(|e| panic!("{}: {e}", scenario_path().display()));
    assert!(run.passed(), "{}", run.report());
    run
}

fn date(text: &str) -> chrono::NaiveDate {
    chrono::NaiveDate::parse_from_str(text, "%Y-%m-%d")
        .unwrap_or_else(|e| panic!("'{text}' hoort een datum te zijn: {e}"))
}

/// De herkomst die een ritme uit een uitkomst van artikel 1 hoort te dragen.
fn uit_artikel_1() -> ScheduleOrigin {
    ScheduleOrigin::Lexogram {
        herkomst: regelrecht_simulator::cell::ObligationOrigin {
            regulation: "test_ritme_uit_uitkomst".to_string(),
            valid_from: Some("2024-01-01".to_string()),
            article: "1".to_string(),
        },
        uitkomst: Some("betaalritme".to_string()),
    }
}

/// **Het gram zegt waar het ritme vandaan kwam, en dat is de wet.**
///
/// Elke termijn draagt het ritme met zijn herkomst: de uitkomst en het artikel
/// dat haar uitrekende. Bij de vergoeding is dat artikel 1 en niet artikel 2 —
/// het artikel dat de verplichting declareert is niet het artikel dat het ritme
/// zegt, en wie het gram terugleest hoort het tweede te vinden.
#[test]
fn het_gram_noemt_de_uitkomst_en_het_artikel_waar_het_ritme_uit_kwam() {
    let run = run();
    let obligations = |index: usize| &run.decisions[index].decretogram.obligations;

    let onder = obligations(0);
    assert_eq!(onder.len(), 1, "onder de drempel: één termijn");
    assert_eq!(onder[0].schedule, Schedule::Ineens);

    let boven = obligations(1);
    assert_eq!(boven.len(), 4, "boven de drempel: vier termijnen");
    for termijn in onder.iter().chain(boven) {
        assert_eq!(termijn.ritme_herkomst, uit_artikel_1());
        assert_eq!(termijn.herkomst.article, "1");
    }

    let vergoeding = obligations(2);
    assert_eq!(vergoeding.len(), 4);
    for termijn in vergoeding {
        assert_eq!(termijn.schedule, Schedule::Kwartaal);
        assert_eq!(
            termijn.herkomst.article, "2",
            "de verplichting zelf staat in artikel 2"
        );
        assert_eq!(
            termijn.ritme_herkomst,
            uit_artikel_1(),
            "maar het ritme komt uit artikel 1"
        );
    }

    // En zo staat het ook in het vastgelegde gram, als waarde.
    let snapshot = &run.snapshot;
    let gram = snapshot
        .cells
        .iter()
        .find(|cell| cell.id == "verstrekker")
        .and_then(|cell| cell.chronicles.iter().find(|c| c.stream == "beschikkingen"))
        .and_then(|stream| stream.grams.first())
        .unwrap_or_else(|| panic!("het eerste besluit hoort als gram te liggen"));
    let Some(Value::Array(termijnen)) = gram.fields.get("obligations").map(|f| &f.value) else {
        panic!("het gram hoort zijn termijnen te dragen");
    };
    let Value::Object(termijn) = &termijnen[0] else {
        panic!("een termijn is een object");
    };
    let Some(Value::Object(herkomst)) = termijn.get("ritme_herkomst") else {
        panic!("een termijn hoort de herkomst van haar ritme te dragen: {termijn:?}");
    };
    for (veld, waarde) in [
        ("herkomst", "lexogram"),
        ("regulation", "test_ritme_uit_uitkomst"),
        ("regulation_valid_from", "2024-01-01"),
        ("artikel", "1"),
        ("uitkomst", "betaalritme"),
    ] {
        assert_eq!(
            herkomst.get(veld).and_then(Value::as_str),
            Some(waarde),
            "veld '{veld}' van de ritmeherkomst"
        );
    }
}

/// **Ook een verplichting van nul draagt het ritme uit de uitkomst.**
///
/// Nul is niets te betalen: geen termijn, maar wel een regel in het gram. Het
/// ritme wordt ook dan uit de uitkomst gelezen — nul ligt onder de drempel, dus
/// `ineens` — en de herkomst staat erbij, net als bij een termijn.
#[test]
fn een_verplichting_van_nul_noemt_het_ritme_uit_de_uitkomst() {
    let mut world = scenario()
        .world(&regulation_root())
        .unwrap_or_else(|e| panic!("de wereld moet op te tuigen zijn: {e}"));
    let params = BTreeMap::from([
        ("bsn".to_string(), Value::String("999993653".to_string())),
        ("subsidiebedrag".to_string(), Value::Int(0)),
    ]);
    let besluit = world
        .decide("verstrekker", "verlening", &params, date("2024-01-01"))
        .unwrap_or_else(|e| panic!("een verlening van nul hoort door te gaan: {e}"));

    assert!(
        besluit.decretogram.obligations.is_empty(),
        "nul levert geen termijn"
    );
    let [niets] = besluit.decretogram.niets_te_betalen.as_slice() else {
        panic!(
            "precies één verplichting hoort op nul uit te vallen, kreeg {:?}",
            besluit.decretogram.niets_te_betalen
        );
    };
    assert_eq!(niets.schedule, Schedule::Ineens);
    assert_eq!(niets.ritme_herkomst, uit_artikel_1());
    assert!(
        world.snapshot().locked_settings.is_empty(),
        "ook zonder termijn zet een ritme uit een uitkomst niets vast"
    );
}

/// **Een ritme uit een uitkomst zet niets vast.**
///
/// Een instelling die een besluit gebruikte, komt vast te staan: het gram leunt
/// erop. Een ritme uit een uitkomst leunt op niets in het wereldbestand, dus er
/// valt niets vast te zetten.
#[test]
fn een_ritme_uit_een_uitkomst_zet_geen_instelling_vast() {
    let run = run();
    assert!(
        run.snapshot.locked_settings.is_empty(),
        "er is geen instelling gebruikt, kreeg {:?}",
        run.snapshot.locked_settings
    );
}

/// Eén veld uit het schema van één besluit.
fn schema_veld(world: &World, besluit: &str, name: &str) -> DecretogramField {
    world
        .snapshot()
        .cells
        .iter()
        .find(|cell| cell.id == "verstrekker")
        .and_then(|cell| cell.besluiten.iter().find(|b| b.name == besluit))
        .and_then(|b| b.schema.iter().find(|field| field.name == name))
        .cloned()
        .unwrap_or_else(|| panic!("veld '{name}' van '{besluit}' hoort in het schema te staan"))
}

/// **Het schema labelt het ritme zoals het gram het draagt.**
///
/// Een ritme uit een uitkomst is geen gat: de wet zegt het, in het artikel dat
/// de uitkomst voortbrengt.
#[test]
fn het_schema_wijst_het_ritme_toe_aan_het_artikel_van_de_uitkomst() {
    let world = scenario()
        .world(&regulation_root())
        .unwrap_or_else(|e| panic!("de wereld moet op te tuigen zijn: {e}"));
    for besluit in ["verlening", "vergoeding"] {
        let veld = schema_veld(&world, besluit, "obligations[0].ritme");
        assert_eq!(veld.herkomst, Herkomst::Lexogram, "{besluit}");
        assert!(!veld.gat, "{besluit}: een ritme uit de wet is geen gat");
        assert_eq!(
            veld.lexogram
                .as_ref()
                .and_then(|lexogram| lexogram.article.as_deref()),
            Some("1"),
            "{besluit}: het artikel dat het ritme uitrekent"
        );
    }
}

/// **Een naam die zowel uitkomst als instelling is, weigert het optuigen.**
///
/// De uitkomst zou voorgaan, en dan staat er in het wereldbestand een knop die
/// stil niets doet. De melding noemt beide plekken, zodat wie haar leest weet
/// wat er hernoemd moet worden.
#[test]
fn een_naam_die_uitkomst_en_instelling_is_wordt_bij_het_optuigen_geweigerd() {
    let mut definition = scenario().definition();
    definition.settings.insert(
        "betaalritme".to_string(),
        Value::String("maand".to_string()),
    );
    let error = World::from_definition(&definition, &regulation_root())
        .err()
        .unwrap_or_else(|| panic!("een dubbele naam hoort geweigerd te worden"));

    assert!(
        matches!(&error, SimulatorError::AmbiguousScheduleReference { name, .. } if name == "betaalritme"),
        "verwachtte AmbiguousScheduleReference over 'betaalritme', kreeg {error}"
    );
    let melding = error.to_string();
    for deel in [
        "$betaalritme",
        "test_ritme_uit_uitkomst artikel 1",
        "uitkomst",
        "`settings` van het wereldbestand",
    ] {
        assert!(
            melding.contains(deel),
            "de melding hoort '{deel}' te noemen, kreeg: {melding}"
        );
    }
}

/// **Ook een uitkomst uit `outputs` van het besluit botst met een instelling.**
///
/// De uitkomst staat dan niet in het artikel dat de verplichting declareert, maar
/// in wat het besluit vastlegt. De melding noemt die plek: het besluit en zijn
/// `outputs`, naast de instelling van het wereldbestand.
#[test]
fn een_uitkomst_uit_outputs_die_ook_instelling_is_wordt_geweigerd() {
    let mut definition = scenario().definition();
    // Alleen de vergoeding: haar ritme is een uitkomst van een ánder artikel, die
    // het besluit met `outputs` vastlegt. De andere besluiten zouden eerder op
    // dezelfde naam weigeren, uit hun eigen artikel.
    for cell in &mut definition.cells {
        cell.besluit_definitions
            .retain(|besluit| besluit.name == "vergoeding");
    }
    definition.settings.insert(
        "betaalritme".to_string(),
        Value::String("maand".to_string()),
    );
    let error = World::from_definition(&definition, &regulation_root())
        .err()
        .unwrap_or_else(|| panic!("een dubbele naam hoort geweigerd te worden"));

    assert!(
        matches!(
            &error,
            SimulatorError::AmbiguousScheduleReference { besluit, name, .. }
                if besluit == "vergoeding" && name == "betaalritme"
        ),
        "verwachtte AmbiguousScheduleReference over 'betaalritme' bij de vergoeding, kreeg {error}"
    );
    let melding = error.to_string();
    for deel in [
        "$betaalritme",
        "besluit 'vergoeding' (`outputs`)",
        "`settings` van het wereldbestand",
    ] {
        assert!(
            melding.contains(deel),
            "de melding hoort '{deel}' te noemen, kreeg: {melding}"
        );
    }
}

/// **Een uitkomst die geen ritme is, laat het besluit omvallen, en er ligt
/// niets.**
///
/// Bij het optuigen valt dat niet te zien: de waarde bestaat pas bij het besluit.
/// De melding noemt de uitkomst, haar waarde en de ritmes die wél bestaan.
#[test]
fn een_uitkomst_die_geen_ritme_is_laat_het_besluit_omvallen() {
    let mut world = scenario()
        .world(&regulation_root())
        .unwrap_or_else(|e| panic!("de wereld moet op te tuigen zijn: {e}"));
    let params = BTreeMap::from([
        ("bsn".to_string(), Value::String("999993653".to_string())),
        ("subsidiebedrag".to_string(), Value::Int(2_000_000)),
    ]);
    let error = world
        .decide("verstrekker", "tegemoetkoming", &params, date("2024-01-01"))
        .err()
        .unwrap_or_else(|| {
            panic!("een ritme dat niet bestaat hoort het besluit te laten omvallen")
        });

    assert!(
        matches!(&error, SimulatorError::ScheduleOutputValue { output, .. } if output == "halfjaarritme"),
        "verwachtte ScheduleOutputValue over 'halfjaarritme', kreeg {error}"
    );
    let melding = error.to_string();
    for deel in ["halfjaarritme", "halfjaar", "ineens, kwartaal, maand"] {
        assert!(
            melding.contains(deel),
            "de melding hoort '{deel}' te noemen, kreeg: {melding}"
        );
    }

    let beschikkingen = world
        .snapshot()
        .cells
        .iter()
        .find(|cell| cell.id == "verstrekker")
        .and_then(|cell| cell.chronicles.iter().find(|c| c.stream == "beschikkingen"))
        .map_or(0, |stream| stream.grams.len());
    assert_eq!(beschikkingen, 0, "een omgevallen besluit legt niets vast");
}
