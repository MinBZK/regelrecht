//! Het beeld van de wereld, vastgepind als contract.
//!
//! `tests/fixtures/snapshot.json` is wat een frontend van deze crate te zien
//! krijgt. Het staat in de repo en niet alleen in een assertie, om twee redenen:
//! wie het contract wijzigt, ziet dat in de diff van zijn eigen pull request, en
//! wie een frontend bouwt heeft een bestand om tegen te renderen zonder de crate
//! te draaien.
//!
//! De fixture wordt hier **opnieuw opgebouwd** en dan vergeleken. Een fixture die
//! alleen gelezen wordt, loopt achter zonder dat iemand het merkt; een fixture die
//! alleen geschreven wordt, keurt elke wijziging goed. Bijwerken doe je met
//! `UPDATE_SNAPSHOT=1`, en dan hoort de diff in de review te staan.

use regelrecht_simulator::{regulation_root, GramKind, Scenario, ScenarioRun, Snapshot};
use std::path::{Path, PathBuf};

/// De wereld waaruit het contract komt: het volledige verhaal van de publieke
/// wereld, met een aanvraag, een besluit, betalingen en een tweede besluit.
///
/// Dat scenario en niet een kleiner: een contract dat alleen een lege wereld dekt,
/// zegt niets over de vorm van een decretogram, een verplichting of een contact
/// over een celgrens.
fn scenario_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("scenarios")
        .join("toeslagen_volledig_verhaal.yaml")
}

fn fixture_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("snapshot.json")
}

/// Speel het scenario af.
fn run() -> ScenarioRun {
    let path = scenario_path();
    let scenario = Scenario::load(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let run = scenario
        .run(&regulation_root())
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    assert!(run.passed(), "{}:\n{}", path.display(), run.report());
    run
}

/// Het beeld dat uit die run volgde.
fn snapshot() -> Snapshot {
    run().snapshot
}

/// Het beeld als JSON, zoals het in de fixture staat.
fn rendered() -> String {
    let mut json = serde_json::to_string_pretty(&snapshot())
        .unwrap_or_else(|e| panic!("het beeld moet naar JSON te schrijven zijn: {e}"));
    json.push('\n');
    json
}

#[test]
fn de_fixture_is_het_beeld_van_de_publieke_wereld() {
    let json = rendered();
    let path = fixture_path();

    if std::env::var_os("UPDATE_SNAPSHOT").is_some() {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .unwrap_or_else(|e| panic!("kan {} niet maken: {e}", parent.display()));
        }
        std::fs::write(&path, &json)
            .unwrap_or_else(|e| panic!("kan {} niet schrijven: {e}", path.display()));
    }

    let stored = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("kan {} niet lezen: {e}", path.display()));
    assert_eq!(
        stored,
        json,
        "het beeld van de wereld wijkt af van {}. Is dat de bedoeling, draai dan \
         `UPDATE_SNAPSHOT=1 cargo test -p regelrecht-simulator --test snapshot` en \
         zet de diff in de review: dit is het contract naar de frontend.",
        path.display()
    );
}

/// Twee runs leveren hetzelfde beeld.
///
/// Een contract dat per run verschilt is geen contract. Dit is de reden dat het
/// receipt van een decretogram er níet in staat: dat draagt wandkloktijd.
#[test]
fn twee_runs_leveren_hetzelfde_beeld() {
    assert_eq!(
        rendered(),
        rendered(),
        "het beeld hoort niet van de wandklok of van een willekeurige volgorde af \
         te hangen"
    );
}

/// Wat het beeld hoort te dragen, los van hoe het eruitziet.
///
/// Deze asserties staan naast de fixture en niet erin: een bijgewerkte fixture
/// legt elke wijziging vast, ook een die iets weghaalt wat er hoort te zijn.
#[test]
fn het_beeld_draagt_de_cellen_de_grammen_en_de_herkomst() {
    let snapshot = snapshot();

    assert_eq!(
        snapshot
            .cells
            .iter()
            .map(|cell| cell.id.as_str())
            .collect::<Vec<_>>(),
        ["belastingdienst", "burger", "toeslagen"],
        "elke cel van de wereld hoort in het beeld te staan"
    );
    assert!(
        snapshot
            .cells
            .iter()
            .any(|cell| cell.laws.is_empty() && !cell.chronicles.is_empty()),
        "een bron-cel is aan haar lege wettenlijst te herkennen"
    );

    let grams: Vec<&regelrecht_simulator::GramSnapshot> = snapshot
        .cells
        .iter()
        .flat_map(|cell| &cell.chronicles)
        .flat_map(|chronicle| &chronicle.grams)
        .collect();
    let beschikkingen = snapshot
        .cells
        .iter()
        .flat_map(|cell| &cell.chronicles)
        .filter(|chronicle| chronicle.stream == "beschikkingen")
        .flat_map(|chronicle| &chronicle.grams);
    assert_eq!(
        beschikkingen
            .filter(|gram| gram.kind == GramKind::Decretogram)
            .count(),
        2,
        "de twee besluiten van dit verhaal horen twee decretogrammen te zijn"
    );
    assert!(
        grams.iter().any(|gram| gram.kind == GramKind::Executogram),
        "en de leveringen en betalingen executogrammen"
    );
    assert!(
        grams.iter().all(|gram| !gram.fields.is_empty()),
        "elk gram hoort zijn velden te dragen"
    );

    // Het observatielog-materiaal: twee besluiten die elk één waarde accepteren,
    // is precies twee contacten over een celgrens.
    assert_eq!(
        snapshot.crossings.len(),
        2,
        "twee geaccepteerde waarden horen twee contacten te zijn"
    );
    assert!(
        snapshot
            .crossings
            .iter()
            .all(|crossing| crossing.signature.contains("GESIMULEERDE")),
        "en dat de ondertekening nep is, hoort in het beeld te staan"
    );

    // Elke actie staat erin, met haar formulier. Aan het eind van het verhaal kan
    // alles, want elk feit waarop een actie wacht, ligt er dan.
    assert_eq!(snapshot.actions.len(), 3);
    assert!(
        snapshot.actions.iter().all(|action| action.available),
        "aan het eind van het verhaal kan elke actie"
    );
    assert!(
        snapshot
            .actions
            .iter()
            .all(|action| !action.form.is_empty() && !action.label.is_empty()),
        "een actie zonder formulier of label is niet te tonen"
    );

    assert!(
        snapshot.warnings.is_empty(),
        "in dit verhaal is geen termijn gemist: {:?}",
        snapshot.warnings
    );
    assert!(
        snapshot.locked_settings.contains_key("betalingsritme"),
        "het ritme is door een besluit gebruikt en staat daarmee vast"
    );
}

/// De contacten in het beeld zijn dezelfde als die de invarianten-gate leest.
///
/// Twee wegen naar dezelfde rij bewijsstukken: de wereld bewaart wat elk besluit
/// over de grens haalde, en de runner leidt zijn verkeer af uit de uitkomsten van
/// zijn stappen. Zouden die uiteenlopen, dan zou het beeld een contact kunnen
/// tonen dat de gate niet ziet — of, erger, de gate er een missen die wél gebeurde.
/// Dat de acties in dit verhaal de besluiten uitlokken, maakt het juist scherp:
/// een besluit via `act` moet in beide rijen staan.
#[test]
fn het_beeld_en_de_gate_zien_dezelfde_contacten() {
    let run = run();
    let gate: Vec<String> = run
        .crossings()
        .iter()
        .map(|signed| {
            format!(
                "{} -> {}.{} op {}",
                signed.asked_by, signed.answer.cell, signed.answer.name, signed.answer.op_moment
            )
        })
        .collect();
    let beeld: Vec<String> = run
        .snapshot
        .crossings
        .iter()
        .map(|crossing| {
            format!(
                "{} -> {}.{} op {}",
                crossing.asked_by,
                crossing.answer.cell,
                crossing.answer.name,
                crossing.answer.op_moment
            )
        })
        .collect();

    assert!(
        !gate.is_empty(),
        "dit verhaal gaat twee keer over een celgrens"
    );
    assert_eq!(
        beeld, gate,
        "het beeld van de wereld en de invarianten-gate horen dezelfde contacten \
         te zien, in dezelfde volgorde"
    );
}

/// Het receipt gaat niet mee, en dat is te zien aan de JSON zelf.
///
/// Via de tekst en niet via een veld, want het receipt is een geneste structuur:
/// wie hem ooit alsnog meestuurt, zet er `"receipt"` in, waar dan ook.
#[test]
fn het_beeld_draagt_geen_receipt() {
    assert!(
        !rendered().contains("\"receipt\""),
        "het receipt draagt wandkloktijd en hoort niet in het contract"
    );
}
