//! Voorwaarden op een actie: uitgerekend door de actor zelf, getoond en niet
//! gehandhaafd.
//!
//! Drie dingen worden hier vastgepind, en ze horen bij elkaar:
//!
//! - de uitkomst volgt de wereld: een regelingsvoorwaarde uit de wet van de
//!   actor, een lexostatusvoorwaarde uit haar eigen kroniek, allebei met een
//!   reden die zegt waarom;
//! - een voorwaarde op `onwaar` of `onbekend` houdt de actie niet tegen, en
//!   `available` houdt zijn eigen, technische betekenis;
//! - de uitrekening komt niet over een celgrens (I1). Wat de actor niet weet, is
//!   onbekend — ook als de wet het bij een andere organisatie zou halen.
//!
//! En een voorwaarde die nooit uit te rekenen is, faalt bij het optuigen: een
//! typfout hoort niet als eeuwig "onbekend" op het scherm te staan.

use chrono::NaiveDate;
use regelrecht_simulator::observation::ObservationLog;
use regelrecht_simulator::{
    regulation_root, ActionSnapshot, ConditionOutcome, ConditionSnapshot, ConditionSource,
    JournalKind, SimulatorError, Value, World, WorldDefinition,
};
use std::collections::BTreeMap;
use std::path::Path;

fn date(text: &str) -> NaiveDate {
    NaiveDate::parse_from_str(text, "%Y-%m-%d")
        .unwrap_or_else(|e| panic!("testdatum '{text}' moet leesbaar zijn: {e}"))
}

fn text(value: &str) -> Value {
    Value::String(value.to_string())
}

/// De publieke wereld: wat een lezer op zijn scherm ziet, is wat hier gemeten
/// wordt.
fn publieke_wereld() -> World {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("worlds")
        .join("publieke_wereld.yaml");
    let definition =
        WorldDefinition::load(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    World::from_definition(&definition, &regulation_root())
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn wereld(yaml: &str) -> Result<World, SimulatorError> {
    World::from_definition(&WorldDefinition::from_yaml(yaml)?, &regulation_root())
}

fn action(world: &World, id: &str) -> ActionSnapshot {
    world
        .snapshot()
        .actions
        .into_iter()
        .find(|action| action.id == id)
        .unwrap_or_else(|| panic!("de wereld hoort actie '{id}' te kennen"))
}

fn condition<'a>(action: &'a ActionSnapshot, label: &str) -> &'a ConditionSnapshot {
    action
        .conditions
        .iter()
        .find(|condition| condition.label == label)
        .unwrap_or_else(|| panic!("actie '{}' hoort voorwaarde '{label}' te dragen", action.id))
}

const TERMIJN: &str = "aanvraag binnen de termijn";
const GEEN_AANVRAAG: &str = "nog geen aanvraag ingediend";

fn aanvraag(bsn: &str, ondertekend_op: &str) -> BTreeMap<String, Value> {
    BTreeMap::from([
        ("bsn".to_string(), text(bsn)),
        ("jaar".to_string(), Value::Int(2024)),
        ("ondertekend_op".to_string(), text(ondertekend_op)),
    ])
}

/// De twee voorwaarden van de aanvraag in de publieke wereld, door het verhaal
/// heen: eerst allebei waar, na de aanvraag de eigen stand onwaar, na 1 april
/// van het jaar erna ook de wet — en de actie blijft al die tijd uitvoerbaar.
#[test]
fn de_voorwaarden_van_de_aanvraag_volgen_de_wereld_en_blokkeren_niet() {
    let mut world = publieke_wereld();

    let voor = action(&world, "burger.aanvraag");
    assert!(voor.available, "een vastlegging kan altijd");
    let termijn = condition(&voor, TERMIJN);
    assert_eq!(
        termijn.outcome,
        ConditionOutcome::Waar,
        "{}",
        termijn.reason
    );
    assert_eq!(termijn.cell, "burger", "de actor rekent zelf");
    let ConditionSource::Regulation {
        regulation,
        output,
        article,
        ..
    } = &termijn.source
    else {
        panic!("'{TERMIJN}' hoort uit een regeling te komen");
    };
    assert_eq!(regulation, "algemene_wet_inkomensafhankelijke_regelingen");
    assert_eq!(output, "aanvraag_binnen_termijn");
    assert_eq!(
        article.as_deref(),
        Some("15"),
        "de voorwaarde noemt het artikel waar de uitkomst vandaan komt"
    );
    assert!(
        termijn.reason.contains("art. 15"),
        "de reden noemt het artikel: {}",
        termijn.reason
    );
    let geen = condition(&voor, GEEN_AANVRAAG);
    assert_eq!(geen.outcome, ConditionOutcome::Waar, "{}", geen.reason);
    assert!(
        matches!(geen.source, ConditionSource::Lexostatus { .. }),
        "'{GEEN_AANVRAAG}' komt uit de eigen stand en niet uit een wet"
    );

    world
        .act("burger.aanvraag", &aanvraag("999993653", "2024-01-01"))
        .unwrap_or_else(|e| panic!("de aanvraag moet kunnen: {e}"));
    let na = action(&world, "burger.aanvraag");
    let geen = condition(&na, GEEN_AANVRAAG);
    assert_eq!(
        geen.outcome,
        ConditionOutcome::Onwaar,
        "na de aanvraag ligt er een in de eigen kroniek: {}",
        geen.reason
    );
    assert!(
        geen.reason.contains("ingediende_aanvraag") && geen.reason.contains("2024"),
        "de reden noemt de lexostatus en wat er gevonden is: {}",
        geen.reason
    );
    assert_eq!(condition(&na, TERMIJN).outcome, ConditionOutcome::Waar);
    assert!(
        na.available && na.unavailable_reason.is_none(),
        "een voorwaarde op onwaar laat de actie uitvoerbaar"
    );

    world
        .advance(date("2025-04-01"))
        .unwrap_or_else(|e| panic!("de klok moet door kunnen: {e}"));
    let laat = action(&world, "burger.aanvraag");
    let termijn = condition(&laat, TERMIJN);
    assert_eq!(
        termijn.outcome,
        ConditionOutcome::Onwaar,
        "op 1 april van het jaar erna is de termijn voorbij: {}",
        termijn.reason
    );
    assert!(laat.available, "en de actie kan nog steeds");
    world
        .act("burger.aanvraag", &aanvraag("999993653", "2025-04-01"))
        .unwrap_or_else(|e| panic!("een late aanvraag wordt niet tegengehouden: {e}"));
}

/// Een veld dat het formulier (nog) niet draagt, maakt de voorwaarde onbekend en
/// niet onwaar — met het veld in de reden.
#[test]
fn een_leeg_veld_maakt_de_voorwaarde_onbekend() {
    let world = publieke_wereld();
    let values = BTreeMap::from([("bsn".to_string(), text("999993653"))]);
    let conditions = world
        .conditions("burger.aanvraag", &values)
        .unwrap_or_else(|e| panic!("{e}"));
    let termijn = conditions
        .iter()
        .find(|condition| condition.label == TERMIJN)
        .unwrap_or_else(|| panic!("de aanvraag hoort '{TERMIJN}' te dragen"));
    assert_eq!(termijn.outcome, ConditionOutcome::Onbekend);
    assert!(
        termijn.reason.contains("jaar") && termijn.reason.contains("ondertekend_op"),
        "de reden noemt de velden die ontbreken: {}",
        termijn.reason
    );
    assert!(
        world.conditions("bestaat.niet", &values).is_err(),
        "een onbekende actie is een fout"
    );
}

/// Een wereld waarin de wet van de actor een waarde bij een **andere
/// organisatie** haalt (tier 3). Het besluit kan dat, langs de
/// veiligheidscontext en het transport; een voorwaarde niet.
const TIER_DRIE: &str = r"
clock:
  start: 2024-01-01

cells:
  - id: brp
    laws: []
    chronicles:
      - stream: relaties
        key: bsn
        events:
          - name: relatie_gewijzigd
            intake: levering
            recording_actor: brp
            grondslag: eigen registratie
            op_moment: 2023-03-01
            fields:
              bsn: '999993653'
              partnerschap_type: HUWELIJK
    lexostatus_definitions:
      - name: partnerschap
        inputs:
          - name: bsn
            type: string
        outputs:
          - partnerschap_type
        reduction:
          chronicle: relaties
          key: bsn
          latest: true

  - id: toeslagen
    identity: Dienst Toeslagen
    laws:
      - test_partnerschapstoets
    accepts_from:
      - cell: brp
        output: partnerschap
        lexostatus: partnerschap
        field: partnerschap_type
    besluit_definitions:
      - name: partnerschapstoets
        regulation: test_partnerschapstoets
        output: is_gehuwd
        zaakkenmerk: partnerschapstoets/{bsn}
        params:
          - name: bsn
            type: string
        inputs:
          bsn:
            param: bsn

actions:
  - id: toeslagen.toets
    actor: toeslagen
    label: Toets het partnerschap
    decides:
      cell: toeslagen
      besluit: partnerschapstoets
    conditions:
      - label: gehuwd
        regulation: test_partnerschapstoets
        output: is_gehuwd
        params:
          bsn: $bsn
";

/// **Een voorwaarde gaat niet over een celgrens (I1).**
///
/// De wet van de actor haalt het partnerschap bij `brp`. Het besluit mag dat; de
/// voorwaarde rekent op de engine zonder cel-tier en komt dus niet verder dan
/// "dat weet deze cel niet". Gemeten aan beide kanten van de naad: het beeld en
/// het journaal van de wereld, en het meetinstrument dat elk contact ziet — ook
/// na een paar keer het beeld opvragen, zoals een frontend doet.
#[test]
fn een_voorwaarde_komt_niet_over_een_celgrens() {
    let world = wereld(TIER_DRIE).unwrap_or_else(|e| panic!("{e}"));
    let values = BTreeMap::from([("bsn".to_string(), text("999993653"))]);

    let beelden: Vec<_> = (0..3).map(|_| world.snapshot()).collect();
    let conditions = world
        .conditions("toeslagen.toets", &values)
        .unwrap_or_else(|e| panic!("{e}"));
    let gehuwd = &conditions[0];
    assert_eq!(
        gehuwd.outcome,
        ConditionOutcome::Onbekend,
        "wat de actor niet zelf weet, is onbekend: {}",
        gehuwd.reason
    );
    assert!(
        gehuwd.reason.contains("brp") && gehuwd.reason.contains("andere organisatie"),
        "de reden zegt dat het antwoord bij een ander ligt: {}",
        gehuwd.reason
    );

    for beeld in &beelden {
        assert!(
            beeld.crossings.is_empty(),
            "het beeld ging niet over een grens"
        );
        assert!(
            !beeld
                .journal
                .iter()
                .any(|entry| entry.kind == JournalKind::Vraag),
            "en het journaal schreef geen vraag op"
        );
    }
    assert!(world.crossings().is_empty());
    let mut log = ObservationLog::new();
    for crossing in world.crossings() {
        log.record(crossing);
    }
    assert!(
        log.is_empty(),
        "het meetinstrument zag niets, want er gebeurde niets"
    );
}

/// Hetzelfde met een voorwaarde die de actor wél zelf kan uitrekenen: ook dan
/// blijft het meetinstrument leeg. De publieke wereld draagt er twee.
#[test]
fn het_beeld_met_voorwaarden_levert_geen_verkeer_op() {
    let world = publieke_wereld();
    let beeld = world.snapshot();
    assert!(
        beeld
            .actions
            .iter()
            .flat_map(|action| &action.conditions)
            .any(|condition| condition.outcome == ConditionOutcome::Waar),
        "deze meting is alleen iets waard als er een voorwaarde is uitgerekend"
    );
    assert!(beeld.crossings.is_empty() && world.crossings().is_empty());
}

/// De publieke wereld met één voorwaarde vervangen: om de toetsen bij het
/// optuigen te raken zonder er een tweede wereld naast te schrijven.
fn met_voorwaarde(voorwaarde: &str) -> Result<World, SimulatorError> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("worlds")
        .join("publieke_wereld.yaml");
    let yaml = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{e}"));
    let start = yaml
        .find("    conditions:\n")
        .unwrap_or_else(|| panic!("de publieke wereld hoort voorwaarden te dragen"));
    let end = start
        + yaml[start..]
            .find("\n  - id: toeslagen.toekenning")
            .unwrap_or_else(|| panic!("na de aanvraag hoort de toekenning te staan"));
    let vervangen = format!(
        "{}    conditions:\n{voorwaarde}{}",
        &yaml[..start],
        &yaml[end..]
    );
    wereld(&vervangen)
}

fn melding(voorwaarde: &str) -> String {
    match met_voorwaarde(voorwaarde) {
        Ok(_) => panic!("deze voorwaarde hoort het optuigen te laten falen:\n{voorwaarde}"),
        Err(error) => error.to_string(),
    }
}

#[test]
fn een_regeling_die_de_actor_niet_laadt_faalt_bij_het_optuigen() {
    let melding = melding(
        "      - label: recht
        regulation: wet_op_de_zorgtoeslag
        output: heeft_recht_op_zorgtoeslag
        params:
          bsn: $bsn
",
    );
    assert!(
        melding.contains("laadt regeling 'wet_op_de_zorgtoeslag' niet")
            && melding.contains("laws:"),
        "de melding zegt wat er mist en waar het hoort: {melding}"
    );
}

#[test]
fn een_uitkomst_die_niet_bestaat_faalt_bij_het_optuigen() {
    let melding = melding(
        "      - label: termijn
        regulation: algemene_wet_inkomensafhankelijke_regelingen
        output: aanvraag_op_tijd
        params:
          berekeningsjaar: $jaar
",
    );
    assert!(
        melding.contains("aanvraag_op_tijd") && melding.contains("aanvraag_binnen_termijn"),
        "de melding noemt wat er wel is: {melding}"
    );
}

#[test]
fn een_uitkomst_die_geen_ja_of_nee_is_faalt_bij_het_optuigen() {
    let melding = melding(
        "      - label: inkomen
        regulation: algemene_wet_inkomensafhankelijke_regelingen
        output: toetsingsinkomen
        params:
          bsn: $bsn
",
    );
    assert!(melding.contains("geen ja-of-nee"), "{melding}");
}

#[test]
fn een_parameter_die_de_regeling_niet_kent_faalt_bij_het_optuigen() {
    let melding = melding(
        "      - label: termijn
        regulation: algemene_wet_inkomensafhankelijke_regelingen
        output: aanvraag_binnen_termijn
        params:
          berekeningsjaarr: $jaar
",
    );
    assert!(melding.contains("berekeningsjaarr"), "{melding}");
}

#[test]
fn een_veld_dat_het_formulier_niet_kent_faalt_bij_het_optuigen() {
    let melding = melding(
        "      - label: termijn
        regulation: algemene_wet_inkomensafhankelijke_regelingen
        output: aanvraag_binnen_termijn
        params:
          berekeningsjaar: $berekeningsjaar
",
    );
    assert!(
        melding.contains("$berekeningsjaar") && melding.contains("ondertekend_op"),
        "de melding noemt de velden van het formulier: {melding}"
    );
}

#[test]
fn een_lexostatus_die_de_actor_niet_publiceert_faalt_bij_het_optuigen() {
    let melding = melding(
        "      - label: beschikking
        lexostatus: zorgtoeslagbeschikking
        output: heeft_recht_op_zorgtoeslag
        expect: true
        params:
          zaakkenmerk: $bsn
",
    );
    assert!(
        melding.contains("zorgtoeslagbeschikking") && melding.contains("ingediende_aanvraag"),
        "alleen de eigen stand van de actor: {melding}"
    );
}

#[test]
fn een_lexostatusvoorwaarde_met_de_verkeerde_parameters_faalt_bij_het_optuigen() {
    let melding = melding(
        "      - label: geen aanvraag
        lexostatus: ingediende_aanvraag
        output: jaar
        expect: null
        params:
          burgerservicenummer: $bsn
",
    );
    assert!(melding.contains("burgerservicenummer"), "{melding}");
}

#[test]
fn een_lexostatusvoorwaarde_zonder_verwachting_wordt_geweigerd() {
    let melding = melding(
        "      - label: geen aanvraag
        lexostatus: ingediende_aanvraag
        output: jaar
        params:
          bsn: $bsn
",
    );
    assert!(melding.contains("mist `expect`"), "{melding}");
}

#[test]
fn twee_voorwaarden_met_hetzelfde_label_worden_geweigerd() {
    let melding = melding(
        "      - label: geen aanvraag
        lexostatus: ingediende_aanvraag
        output: jaar
        expect: null
        params:
          bsn: $bsn
      - label: geen aanvraag
        lexostatus: ingediende_aanvraag
        output: jaar
        expect: 2024
        params:
          bsn: $bsn
",
    );
    assert!(melding.contains("hetzelfde label"), "{melding}");
}

/// De verwachte waarde telt: `expect: 2024` is waar zodra er een aanvraag over
/// 2024 ligt, en daarvoor niet.
#[test]
fn een_lexostatusvoorwaarde_vergelijkt_met_de_verwachte_waarde() {
    let mut world = met_voorwaarde(
        "      - label: aanvraag over 2024
        lexostatus: ingediende_aanvraag
        output: jaar
        expect: 2024
        params:
          bsn: $bsn
",
    )
    .unwrap_or_else(|e| panic!("{e}"));
    let uitkomst =
        |world: &World| condition(&action(world, "burger.aanvraag"), "aanvraag over 2024").outcome;
    assert_eq!(uitkomst(&world), ConditionOutcome::Onwaar);
    world
        .act("burger.aanvraag", &aanvraag("999993653", "2024-01-01"))
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(uitkomst(&world), ConditionOutcome::Waar);
}
