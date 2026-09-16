//! Het gebeurtenisschema van een kroniekstroom: wat een stroom belooft, en wat
//! er gebeurt als een vastlegging die belofte breekt.
//!
//! Het typeschema van een executogram is generiek en compile-time — het geldt
//! voor élke vastlegging van een naam, niet voor één casus — en hoort dus data
//! te zijn (RFC-022 §1.3). Wat dat oplevert, staat hier: een typfout valt bij het
//! **optuigen** en niet halverwege een tijdlijn, en de grondslag hoeft niet bij
//! elk gram opnieuw opgeschreven te worden.
//!
//! Wat een cel van binnen weigert, staat als eenheidstest bij de kroniekstore
//! zelf. Hier staat wat alleen een hele wereld kan laten zien: een wereldbestand
//! dat afbreekt, en een beeld dat het schema draagt.

use regelrecht_simulator::{regulation_root, Scenario, SimulatorError};
use std::path::{Path, PathBuf};

fn scenario_path(dir: &str, name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("scenarios")
        .join(dir)
        .join(name)
}

/// **Een fixture met een verkeerd type haalt het optuigen niet.**
///
/// De datum van die fixture ligt zes jaar na de klok, en dat mag niet uitmaken:
/// een startstand wordt getoetst zoals ze bij het vastleggen getoetst wordt, ook
/// als haar moment nog ver weg is. Zou dat niet zo zijn, dan hing het van de
/// lengte van de tijdlijn af of een typfout ooit boven water kwam.
#[test]
fn een_fixture_met_een_verkeerd_type_strandt_bij_het_optuigen() {
    let path = scenario_path("geweigerd", "fixture_past_niet_bij_het_schema.yaml");
    let scenario = Scenario::load(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let error = scenario
        .run(&regulation_root())
        .expect_err("een fixture die het schema breekt hoort de wereld te laten stranden");
    match &error {
        SimulatorError::GebeurtenisVeldType {
            field, expected, ..
        } => {
            assert_eq!(field, "is_verzekerde");
            assert_eq!(*expected, "boolean");
        }
        other => panic!("verwachtte GebeurtenisVeldType, kreeg {other}"),
    }
    // De melding noemt de stroom, de gebeurtenis, het veld en het type: wie haar
    // leest, hoort te weten wat hij moet veranderen zonder het bestand te hoeven
    // uitpluizen.
    let melding = error.to_string();
    for deel in [
        "inkomensleveringen",
        "inkomenslevering",
        "is_verzekerde",
        "boolean",
    ] {
        assert!(
            melding.contains(deel),
            "de melding hoort '{deel}' te noemen, kreeg: {melding}"
        );
    }
}

/// **Het beeld draagt het schema van elke stroom die er een declareert.**
///
/// En het draagt het los van de grammen. Dat is het punt: een betalingsstroom
/// begint leeg, en wie de kolommen uit het eerste gram zou aflezen, ziet dan
/// niets — terwijl er wél iets beloofd is.
#[test]
fn het_beeld_draagt_het_schema_van_een_lege_stroom() {
    let path = scenario_path("", "toeslagen_verplichtingen.yaml");
    let scenario = Scenario::load(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let run = scenario
        .run(&regulation_root())
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()));

    let betalingen = run
        .snapshot
        .cells
        .iter()
        .flat_map(|cel| &cel.chronicles)
        .find(|kroniek| kroniek.stream == "betalingen")
        .unwrap_or_else(|| panic!("deze wereld hoort een betalingsstroom te dragen"));

    let namen: Vec<&str> = betalingen
        .gebeurtenissen
        .iter()
        .map(|gebeurtenis| gebeurtenis.name.as_str())
        .collect();
    assert!(
        namen.contains(&"betaling_gedaan") && namen.contains(&"betaling_gemeld"),
        "het beeld hoort beide kanten van een betaling te tonen, kreeg {namen:?}"
    );

    let gedaan = betalingen
        .gebeurtenissen
        .iter()
        .find(|gebeurtenis| gebeurtenis.name == "betaling_gedaan")
        .unwrap_or_else(|| panic!("betaling_gedaan hoort in het schema te staan"));
    let velden: Vec<&str> = gedaan
        .fields
        .iter()
        .map(|veld| veld.name.as_str())
        .collect();
    for naam in ["zaakkenmerk", "bedrag", "volgnummer", "besluit"] {
        assert!(
            velden.contains(&naam),
            "het schema hoort '{naam}' te dragen, kreeg {velden:?}"
        );
    }
}

/// **De grondslag van het schema komt in het gram te staan.**
///
/// De grondslag is een eigenschap van het *soort* vastlegging: dat een aanvraag
/// op Awir art. 15 berust, geldt voor elke aanvraag. Ze staat dus één keer in het
/// schema, en een gram dat er zelf geen draagt, krijgt die van het schema — als
/// veld in de kroniek en niet als weergave, want een grondslag die alleen bij het
/// tonen verschijnt, staat nergens vast.
#[test]
fn een_gram_zonder_eigen_grondslag_krijgt_die_van_het_schema() {
    let path = scenario_path("", "toeslagen_volledig_verhaal.yaml");
    let scenario = Scenario::load(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let run = scenario
        .run(&regulation_root())
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()));

    let ingediend = run
        .snapshot
        .cells
        .iter()
        .filter(|cel| cel.id == "burger")
        .flat_map(|cel| &cel.chronicles)
        .filter(|kroniek| kroniek.stream == "aanvragen")
        .flat_map(|kroniek| &kroniek.grams)
        .find(|gram| gram.name == "aanvraag_ingediend")
        .unwrap_or_else(|| panic!("de ingediende aanvraag hoort in deze kroniek te liggen"));

    // De actie noemt geen grondslag meer: wat hier staat, komt uit het schema
    // van de stroom waarin het gram landt.
    assert_eq!(
        ingediend.grondslag, "Algemene wet inkomensafhankelijke regelingen, art. 15",
        "een gram zonder eigen grondslag hoort die van zijn schema te dragen"
    );
}
