//! Wie er mag besluiten, en wat er gebeurt als dat niet klopt.
//!
//! Drie regels, en ze horen in deze volgorde gelezen te worden:
//!
//! 1. **De wet bepaalt wie het bevoegd gezag is** (`competent_authority`,
//!    RFC-002). Het platform leest dat uit het law-model en nergens anders — een
//!    celconfiguratie kan zichzelf geen gezag toebedelen.
//! 2. **De cel beweert wie zij is** (`identity` op de cel; standaard haar id).
//! 3. **De bewering geldt voor nu als waar.** Er wordt niets bewezen: er is geen
//!    sleutelmateriaal en de ondertekening is gesimuleerd. Wat er wél gebeurt is
//!    de toets tegen de wet, en die is hier vastgelegd.
//!
//! De positieve gevallen staan in de scenariobestanden, want daar horen ze:
//! een besluit dat slaagt, draagt zijn bevoegd gezag in zijn eigen verwachting.
//! Wat hier staat, is wat geen scenario kán dragen — een run die afbreekt, en
//! een vraag die juist níet gesteld is.

use regelrecht_simulator::{
    regulation_root, Decretogram, Scenario, ScenarioRun, SimulatorError, Value, Warning,
};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// De fixtures die met opzet geweigerd worden, met de fout die erbij hoort.
///
/// Dezelfde vorm als de tabel in [`tests/invarianten.rs`](invarianten.rs), en om
/// dezelfde reden: een bestand dat niemand draait, doet stil niets, en een
/// tabelregel zonder bestand is een test die op niets afgaat. Deze map staat
/// náást `scenarios/negatief/`, want die gaat over de invarianten-gate en deze
/// scenario's halen die gate niet eens — ze vallen eerder om.
type Expected = fn(&SimulatorError) -> bool;
const GEWEIGERD: [(&str, Expected); 1] = [("cel_is_niet_het_bevoegd_gezag.yaml", |error| {
    matches!(error, SimulatorError::NotCompetentAuthority { .. })
})];

fn geweigerd_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("scenarios")
        .join("geweigerd")
}

fn scenario_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("scenarios")
        .join(name)
}

fn scenario(path: &Path) -> Scenario {
    Scenario::load(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn run(path: &Path) -> ScenarioRun {
    let run = scenario(path)
        .run(&regulation_root())
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    assert!(run.passed(), "{}:\n{}", path.display(), run.report());
    run
}

fn bsn() -> BTreeMap<String, Value> {
    BTreeMap::from([("bsn".to_string(), Value::String("999993653".to_string()))])
}

fn date(text: &str) -> chrono::NaiveDate {
    chrono::NaiveDate::parse_from_str(text, "%Y-%m-%d")
        .unwrap_or_else(|e| panic!("testdatum '{text}' moet leesbaar zijn: {e}"))
}

/// Het eerste decretogram van een run.
fn eerste_gram(run: &ScenarioRun) -> &Decretogram {
    &run.decisions
        .first()
        .unwrap_or_else(|| panic!("deze run hoort een besluit te nemen"))
        .decretogram
}

/// **Gelijk: het gram draagt het gezag van de wet én wie besloot.**
///
/// Twee velden en niet één, en ze zeggen verschillende dingen: het eerste is wat
/// de regeling aanwijst, het tweede wie er feitelijk besloot. Ze zijn hier gelijk
/// — anders was er geen gram — en juist daarom hoort het allebei vast te liggen.
/// Wie er later een handtekening onder zet, heeft een veld om die aan te hangen
/// in plaats van een afleiding uit de afwezigheid van een weigering.
#[test]
fn een_besluit_van_het_bevoegd_gezag_draagt_beide_namen() {
    let run = run(&scenario_path("toeslagen_besluit.yaml"));
    let gram = eerste_gram(&run);

    assert_eq!(
        gram.competent_authority.as_deref(),
        Some("Dienst Toeslagen"),
        "het gram hoort het bevoegd gezag van de regeling te dragen"
    );
    assert_eq!(
        gram.besloten_door, "Dienst Toeslagen",
        "en de identiteit van de cel die besloot"
    );
    assert!(
        run.warnings.is_empty(),
        "hier valt wél iets te toetsen, dus er is niets te melden: {:?}",
        run.warnings
    );
}

/// **Wie de vraag ondertekent, is wie het besluit draagt.**
///
/// De naam waaronder een cel zich uitgeeft, woont in haar veiligheidscontext en
/// niet in de cel (RFC-022 §2): het is wat een ondertekening straks moet
/// bewijzen. Eén register, dus de identiteit die een vraag over de grens
/// ondertekende en de naam in `besloten_door` van het gram komen van dezelfde
/// plek. Zouden dat er twee zijn, dan tekent bij de eerste echte handtekening
/// een ander dan wie besloot.
#[test]
fn de_identiteit_die_ondertekent_is_de_identiteit_die_besluit() {
    let run = run(&scenario_path("toeslagen_accepteert_toetsingsinkomen.yaml"));
    let decision = run
        .decisions
        .iter()
        .find(|decision| !decision.crossings.is_empty())
        .unwrap_or_else(|| panic!("dit scenario heeft een besluit dat over de grens vraagt"));

    let signer = decision.crossings[0].signature.signer();
    assert_eq!(signer.cell(), "toeslagen", "het adres: het cel-id");
    assert_eq!(
        signer.name(),
        decision.decretogram.besloten_door,
        "de naam die tekende is de naam die besloot"
    );
    assert_eq!(
        decision.decretogram.besloten_door, "Dienst Toeslagen",
        "en dat is wat het wereldbestand aan de veiligheidscontext van de cel bond"
    );
}

/// **Een `#`-verwijzing wordt opgelost, niet doorgegeven.**
///
/// `wet_op_de_zorgtoeslag` schrijft `competent_authority: '#bevoegd_gezag'`: geen
/// naam maar een verwijzing naar een uitkomst van de regeling zelf. Zou het
/// platform die letterlijk overnemen, dan zou de toets op de tekst `#bevoegd_gezag`
/// gaan en zou geen enkele cel ooit bevoegd zijn — of, erger, zou een cel die
/// zichzelf `#bevoegd_gezag` noemt er langs komen.
#[test]
fn een_verwijzing_naar_een_uitkomst_wordt_tot_een_naam_opgelost() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("corpus")
        .join("regulation")
        .join("nl")
        .join("wet")
        .join("wet_op_de_zorgtoeslag")
        .join("2024-01-01.yaml");
    let wet = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("kan {} niet lezen: {e}", path.display()));
    assert!(
        wet.contains("competent_authority: '#bevoegd_gezag'"),
        "deze test gaat over de verwijzingsvorm; staat er een kale naam in de wet, \
         dan bewijst hij niets meer"
    );

    let run = run(&scenario_path("toeslagen_besluit.yaml"));
    let gezag = eerste_gram(&run)
        .competent_authority
        .clone()
        .unwrap_or_else(|| panic!("de wet wijst wel degelijk een gezag aan"));

    assert!(
        !gezag.starts_with('#'),
        "een verwijzing hoort opgelost te worden, kreeg '{gezag}'"
    );
    assert_eq!(gezag, "Dienst Toeslagen");
}

/// **Ongelijk: geen besluit, en niets vastgelegd.**
///
/// De melding moet te lezen zijn door wie dit bestand niet kent: welke cel, wat
/// zij beweert te zijn, welke regeling, en wie die aanwijst. En er mag niets
/// liggen — een geweigerd besluit dat toch een gram achterlaat, is erger dan
/// geen weigering, want dan staat er een beschikking van een onbevoegde tussen
/// de echte.
#[test]
fn een_cel_die_niet_het_bevoegd_gezag_is_besluit_niet_en_legt_niets_vast() {
    let path = geweigerd_dir().join("cel_is_niet_het_bevoegd_gezag.yaml");
    let mut world = scenario(&path)
        .world(&regulation_root())
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    world
        .advance(date("2024-06-01"))
        .unwrap_or_else(|e| panic!("de klok moet vooruit kunnen: {e}"));

    let error = world
        .decide(
            "toeslagen",
            "zorgtoeslag_vaststelling",
            &bsn(),
            date("2024-06-01"),
        )
        .expect_err("een cel die niet het bevoegd gezag is, hoort niet te besluiten");

    assert!(
        matches!(&error, SimulatorError::NotCompetentAuthority { cell, .. } if cell == "toeslagen"),
        "verwachtte NotCompetentAuthority over 'toeslagen', kreeg {error}"
    );
    let melding = error.to_string();
    for deel in [
        "toeslagen",
        "Belastingdienst",
        "wet_op_de_zorgtoeslag",
        "Dienst Toeslagen",
    ] {
        assert!(
            melding.contains(deel),
            "de melding hoort '{deel}' te noemen, kreeg: {melding}"
        );
    }

    let terugkijk = world
        .reduce(
            "toeslagen",
            "zorgtoeslagbeschikking",
            &BTreeMap::from([(
                "zaakkenmerk".to_string(),
                Value::String("zorgtoeslag/999993653".to_string()),
            )]),
            date("2024-06-01"),
        )
        .unwrap_or_else(|e| panic!("terugkijken moet kunnen: {e}"));
    assert!(
        matches!(
            terugkijk.outcome,
            regelrecht_simulator::LexostatusOutcome::NotEstablished { .. }
        ),
        "een geweigerd besluit hoort geen gram achter te laten, kreeg {:?}",
        terugkijk.outcome
    );
}

/// **De weigering valt vóór de eerste vraag over een celgrens.**
///
/// Dezelfde regel als bij het zaakkenmerk: een vraag over een celgrens is bij de
/// bevraagde organisatie een gebeurtenis — zij ziet wie er iets over wie kwam
/// opvragen — en die hoort niet te vallen voor een besluit dat toch niet genomen
/// kan worden. Het besluit in deze fixture accepteert met opzet een waarde van
/// een andere cel, zodat er iets te meten valt.
#[test]
fn een_geweigerd_besluit_vraagt_een_andere_cel_niets() {
    let path = geweigerd_dir().join("cel_is_niet_het_bevoegd_gezag.yaml");
    let mut world = scenario(&path)
        .world(&regulation_root())
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    world
        .advance(date("2024-06-01"))
        .unwrap_or_else(|e| panic!("de klok moet vooruit kunnen: {e}"));

    world
        .decide(
            "toeslagen",
            "zorgtoeslag_vaststelling",
            &bsn(),
            date("2024-06-01"),
        )
        .expect_err("dit besluit hoort geweigerd te worden");

    assert!(
        world.snapshot().crossings.is_empty(),
        "een cel die de regeling niet mag uitvoeren, hoort niemand iets gevraagd te \
         hebben: {:?}",
        world.snapshot().crossings
    );
}

/// **Een andere schrijfwijze is dezelfde organisatie.**
///
/// Trim en kasus, en verder niets. Dat is de grens: een spatie of een hoofdletter
/// is een schrijfwijze, een afkorting is een andere naam. Deze test zet de
/// weigering van de fixture hierboven om in een geslaagd besluit door alléén de
/// schrijfwijze goed te zetten — dus hij meet allebei de kanten van dezelfde
/// vergelijking.
#[test]
fn de_vergelijking_kijkt_niet_naar_kasus_of_witruimte() {
    let path = geweigerd_dir().join("cel_is_niet_het_bevoegd_gezag.yaml");
    let mut scenario = scenario(&path);
    let cel = scenario
        .cells
        .iter_mut()
        .find(|config| config.id == "toeslagen")
        .unwrap_or_else(|| panic!("{}: 'toeslagen' hoort erin te staan", path.display()));
    cel.identity = Some("  dienst toeslagen  ".to_string());

    let run = scenario
        .run(&regulation_root())
        .unwrap_or_else(|e| panic!("met de juiste naam hoort dit besluit te lukken: {e}"));
    assert!(run.passed(), "{}", run.report());
    assert_eq!(
        eerste_gram(&run).besloten_door,
        "  dienst toeslagen  ",
        "het gram legt vast wat de cel beweerde, niet wat de vergelijking ervan maakte"
    );
}

/// **Ontbrekend: het besluit gaat door, en de wereld waarschuwt.**
///
/// Het scenario draagt de verwachting zelf (`expect_warnings`), dus wat hier
/// staat is de vorm van de waarschuwing: dat ze in het beeld van de wereld
/// terechtkomt en dat ze leesbaar is voor wie het scenario niet kent. Zonder dat
/// tweede zou een gat in een regeling als een regel ruis in een verslag eindigen.
#[test]
fn een_regeling_zonder_bevoegd_gezag_levert_een_gram_zonder_gezag_en_een_waarschuwing() {
    let run = run(&scenario_path("besluit_zonder_bevoegd_gezag.yaml"));
    let gram = eerste_gram(&run);

    assert_eq!(
        gram.competent_authority, None,
        "de regeling wijst niemand aan, en dat hoort het gram te zeggen"
    );
    assert_eq!(
        gram.besloten_door, "uitvoerder",
        "wie besloot staat er wél, ook zonder gezag om aan te toetsen"
    );

    let waarschuwing = run
        .warnings
        .iter()
        .find(|warning| matches!(warning, Warning::GeenBevoegdGezag { .. }))
        .unwrap_or_else(|| panic!("verwachtte een waarschuwing, kreeg {:?}", run.warnings));
    let melding = waarschuwing.describe();
    for deel in [
        "declareert geen bevoegd gezag",
        "test_zonder_bevoegd_gezag",
        "uitvoerder",
    ] {
        assert!(
            melding.contains(deel),
            "de waarschuwing hoort '{deel}' te noemen, kreeg: {melding}"
        );
    }
    assert!(
        run.report().contains(&melding),
        "het verslag hoort de waarschuwing te dragen:\n{}",
        run.report()
    );
    assert!(
        run.snapshot
            .warnings
            .iter()
            .any(|warning| matches!(warning, Warning::GeenBevoegdGezag { .. })),
        "en het beeld van de wereld ook: daar leest de frontend hem"
    );
}

/// Elke geweigerde fixture wordt gedraaid, en elke tabelregel wijst iets aan.
#[test]
fn elke_geweigerde_fixture_staat_in_de_tabel() {
    let dir = geweigerd_dir();
    let bestanden: BTreeSet<String> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("kan {} niet lezen: {e}", dir.display()))
        .filter_map(std::result::Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().to_string())
        .filter(|name| name.ends_with(".yaml"))
        .collect();
    let tabel: BTreeSet<String> = GEWEIGERD
        .iter()
        .map(|(name, _)| (*name).to_string())
        .collect();

    assert_eq!(
        bestanden, tabel,
        "elke geweigerde fixture hoort in de tabel te staan en omgekeerd"
    );
}

/// Elke geweigerde fixture breekt af op de fout waar hij over gaat.
///
/// Dát hij faalt is niet genoeg: een bestand dat om een typfout rood staat,
/// bewijst niets over de weigering die het zegt te meten.
#[test]
fn elke_geweigerde_fixture_breekt_af_op_de_fout_die_hij_meet() {
    for (name, expected) in GEWEIGERD {
        let path = geweigerd_dir().join(name);
        let error = scenario(&path)
            .run(&regulation_root())
            .err()
            .unwrap_or_else(|| panic!("{name}: deze fixture hoort af te breken"));
        assert!(
            expected(&error),
            "{name}: dit is niet de fout waar de fixture over gaat: {error}"
        );
    }
}
