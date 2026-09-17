//! Een beschikking die een eerdere vervangt: wat er nog openstond, vervalt.
//!
//! Een verplichting is niet in te trekken — haar schema staat in een gram, en een
//! gram verandert niet. Een tweede besluit over dezelfde zaak laat de termijnen
//! van het eerste dus gewoon vervallen, **tenzij** het artikel dat het tweede
//! besluit voortbrengt zegt dat deze beschikking in de plaats komt van de vorige.
//! De Awir zegt dat voor de vaststelling: die vervangt het voorschot, want de
//! tegemoetkoming staat nu vast en de verleende voorschotten worden ermee
//! verrekend (art. 19 jo. art. 24, tweede lid).
//!
//! De twee tests hieronder zijn elkaars tegenproef. De ene meet dat de termijnen
//! vervallen waar de wet dat zegt; de andere dat ze blijven staan waar ze dat
//! niet zegt — anders zou "een tweede besluit wist het eerste uit" een regel van
//! het platform zijn in plaats van een regel uit het recht.
//!
//! Wat de publieke wereld zelf doet, staat in
//! `scenarios/toeslagen_volledig_verhaal.yaml`: daar valt de vaststelling ná de
//! laatste voorschottermijn, en dan is er niets meer om te laten vervallen. De
//! vaststelling hieronder valt er met opzet vóór — precies het geval waarin het
//! vervallen iets doet, en waarin te zien is dat de verrekening erop aansluit:
//! art. 19 verrekent wat er van het voorschot is **uitbetaald**, dus wat de
//! aanvrager netto overhoudt is de vastgestelde tegemoetkoming, ongeacht wanneer
//! de vaststelling valt.

use regelrecht_simulator::{regulation_root, JournalKind, Value, World, WorldDefinition};
use rust_decimal::Decimal;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// De BSN uit het wereldbestand: een testnummer uit de officiële reeks.
const BSN: &str = "999993653";

fn world_file() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("worlds")
        .join("publieke_wereld.yaml")
}

fn publieke_wereld() -> World {
    let path = world_file();
    let definition =
        WorldDefinition::load(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    World::from_definition(&definition, &regulation_root())
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn params() -> BTreeMap<String, Value> {
    BTreeMap::from([("bsn".to_string(), Value::String(BSN.to_string()))])
}

fn dag(text: &str) -> chrono::NaiveDate {
    text.parse()
        .unwrap_or_else(|e| panic!("testdatum '{text}' moet leesbaar zijn: {e}"))
}

/// Hoeveel grammen er in één kroniek van één cel liggen.
fn grams(world: &World, cell: &str, chronicle: &str) -> usize {
    world
        .snapshot()
        .cells
        .iter()
        .find(|snapshot| snapshot.id == cell)
        .unwrap_or_else(|| panic!("cel '{cell}' hoort in het beeld te staan"))
        .chronicles
        .iter()
        .find(|stream| stream.stream == chronicle)
        .map_or(0, |stream| stream.grams.len())
}

/// Wat één cel op de zaak betaalde, opgeteld over haar eigen betalingen.
///
/// Dezelfde som die `betaald_tot_nu_toe` maakt, maar dan uit het beeld van de
/// wereld: de test rekent niets voor, ze telt op wat er in de kroniek ligt.
fn betaald(world: &World, cell: &str) -> Decimal {
    world
        .snapshot()
        .cells
        .iter()
        .find(|snapshot| snapshot.id == cell)
        .unwrap_or_else(|| panic!("cel '{cell}' hoort in het beeld te staan"))
        .chronicles
        .iter()
        .find(|stream| stream.stream == "betalingen")
        .map_or(Decimal::ZERO, |stream| {
            stream
                .grams
                .iter()
                .filter_map(|gram| gram.fields.get("bedrag"))
                .filter_map(|field| field.value.as_decimal())
                .sum()
        })
}

/// De journaalregels over een termijn die verviel zonder nagekomen te worden.
fn vervallen(world: &World) -> Vec<&str> {
    world
        .journal()
        .iter()
        .filter(|entry| matches!(entry.kind, JournalKind::Termijn))
        .map(|entry| entry.description.as_str())
        .filter(|description| description.contains("vervallen door besluit"))
        .collect()
}

/// De toekenning, met vier kwartaaltermijnen vanaf april: de laatste valt in
/// januari van het jaar daarna en staat dus nog open in december.
fn wereld_met_toekenning() -> World {
    let mut world = publieke_wereld();
    world
        .advance(dag("2024-04-01"))
        .unwrap_or_else(|e| panic!("de klok moet vooruit kunnen: {e}"));
    world
        .decide(
            "toeslagen",
            "zorgtoeslag_toekenning",
            &params(),
            dag("2024-04-01"),
        )
        .unwrap_or_else(|e| panic!("de toekenning moet kunnen: {e}"));
    world
}

/// **De vaststelling vervangt het voorschot: wat nog openstond, vervalt.**
///
/// Drie van de vier kwartaaltermijnen zijn vervallen als de vaststelling valt;
/// de vierde (januari) stond nog open. Die wordt niet meer nagekomen — de
/// tegemoetkoming staat vast en het verleende voorschot is ermee verrekend — en
/// het journaal zegt waarom, met de grondslag uit het lexogram erbij.
///
/// En de som klopt: art. 19 verrekent wat er van het voorschot is *uitbetaald*,
/// dus het slotbedrag vult aan tot precies de vastgestelde tegemoetkoming. Zou de
/// vaststelling het hele *verleende* voorschot verrekenen, dan trok ze de vierde
/// termijn af die ze hierboven zelf laat vervallen, en hield de aanvrager minder
/// over dan wat er is vastgesteld. Zie de README.
#[test]
fn een_vaststelling_laat_de_openstaande_voorschottermijnen_vervallen() {
    let mut world = wereld_met_toekenning();
    world
        .advance(dag("2024-12-15"))
        .unwrap_or_else(|e| panic!("de klok moet vooruit kunnen: {e}"));
    assert_eq!(
        grams(&world, "belastingdienst", "betalingen"),
        3,
        "drie kwartaaltermijnen (april, juli, oktober) zijn dan nagekomen"
    );

    world
        .decide(
            "toeslagen",
            "zorgtoeslag_vaststelling",
            &params(),
            dag("2024-12-15"),
        )
        .unwrap_or_else(|e| panic!("de vaststelling moet kunnen: {e}"));

    let regels = vervallen(&world);
    assert_eq!(
        regels.len(),
        1,
        "precies één termijn stond nog open, kreeg: {regels:?}"
    );
    let regel = regels[0];
    for deel in [
        "termijn 4 van 4",
        "zorgtoeslag_toekenning",
        "zorgtoeslag/999993653",
        "2025-01-01",
        "vervallen door besluit 'zorgtoeslag_vaststelling'",
        "Awir art. 19 jo. art. 24, tweede lid",
    ] {
        assert!(
            regel.contains(deel),
            "de journaalregel hoort '{deel}' te noemen, kreeg: {regel}"
        );
    }

    // En de klok voorbij die vervaldatum laten lopen levert niets meer op: de
    // termijn is weg uit de wachtrij en niet alleen overgeslagen.
    world
        .advance(dag("2025-06-01"))
        .unwrap_or_else(|e| panic!("de klok moet vooruit kunnen: {e}"));
    assert_eq!(
        grams(&world, "belastingdienst", "betalingen"),
        4,
        "drie voorschottermijnen en het slotbedrag; de vierde termijn van het \
         voorschot wordt niet meer nagekomen"
    );
    assert_eq!(
        grams(&world, "burger", "betalingen"),
        0,
        "hier is niets terug te vorderen: het slotbedrag vult juist aan"
    );

    // De kern van deze opstelling: wat de aanvrager netto overhoudt, is precies
    // wat er is vastgesteld — ook nu de vaststelling vóór de laatste
    // voorschottermijn viel.
    let vastgesteld = vastgestelde_tegemoetkoming(&world);
    assert_eq!(
        betaald(&world, "belastingdienst") - betaald(&world, "burger"),
        vastgesteld,
        "uitbetaald voorschot plus slotbedrag is de vastgestelde tegemoetkoming"
    );
}

/// Wat de vaststelling in haar gram vastlegde als vastgestelde tegemoetkoming.
fn vastgestelde_tegemoetkoming(world: &World) -> Decimal {
    world
        .snapshot()
        .cells
        .iter()
        .find(|snapshot| snapshot.id == "toeslagen")
        .unwrap_or_else(|| panic!("de besluitende cel hoort in het beeld te staan"))
        .chronicles
        .iter()
        .find(|stream| stream.stream == "beschikkingen")
        .and_then(|stream| stream.grams.last())
        .and_then(|gram| gram.fields.get("vastgestelde_tegemoetkoming"))
        .and_then(|field| field.value.as_decimal())
        .unwrap_or_else(|| panic!("de vaststelling hoort een bedrag vast te leggen"))
}

/// **Zonder die declaratie blijft staan wat er staat.**
///
/// De tegenproef, met hetzelfde besluit twee keer: het artikel dat de toekenning
/// voortbrengt (Wet op de zorgtoeslag art. 2) zegt niets over eerdere termijnen,
/// dus het tweede besluit neemt de openstaande termijn van het eerste niets af.
/// Die vervalt gewoon door — vier betalingen in plaats van drie, en dat ene
/// verschil met de test hierboven is precies de declaratie in het lexogram.
///
/// Dat de vier termijnen van het tweede besluit hier geen betalingen bovenop
/// leggen, is iets anders: één zaak, één besluit en één volgnummer is één
/// termijn, dus een besluit dat overgedaan wordt betaalt niet nog een keer.
#[test]
fn een_tweede_besluit_zonder_declaratie_laat_de_termijnen_staan() {
    let mut world = wereld_met_toekenning();
    world
        .advance(dag("2024-12-15"))
        .unwrap_or_else(|e| panic!("de klok moet vooruit kunnen: {e}"));
    world
        .decide(
            "toeslagen",
            "zorgtoeslag_toekenning",
            &params(),
            dag("2024-12-15"),
        )
        .unwrap_or_else(|e| panic!("een tweede toekenning moet kunnen: {e}"));
    assert!(
        vervallen(&world).is_empty(),
        "er is niets vervallen, want dat artikel zegt dat niet: {:?}",
        vervallen(&world)
    );

    world
        .advance(dag("2025-01-05"))
        .unwrap_or_else(|e| panic!("de klok moet vooruit kunnen: {e}"));
    assert_eq!(
        grams(&world, "belastingdienst", "betalingen"),
        4,
        "de vierde termijn van de toekenning vervalt gewoon en wordt nagekomen"
    );
}
