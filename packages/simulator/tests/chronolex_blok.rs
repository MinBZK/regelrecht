//! **Het `chronolex`-blok is gesloten, en dat blijkt bij het optuigen.**
//!
//! `produces.extensions` is per namespace ondoorzichtig (RFC-022 §3.2): het
//! document draagt het blok mee en legt het niet uit, en het JSON-schema zet
//! geen `additionalProperties: false` op `produces`. Een typfout in een sleutel
//! van *onze* namespace valideert dus gewoon — de opstelling is de enige die hem
//! kan weigeren.
//!
//! Wat hier gemeten wordt is dat ze dat ook doet, en wanneer: bij het **optuigen
//! van de cel** en niet bij de eerste aanvrager. Zou het blok stil overgeslagen
//! worden, dan lag er een regeling die denkt te verplichten of te weigeren en
//! niets doet, en dat is aan een gram niet te zien. De twee testregelingen staan
//! in `fixtures/regulation/` en niet in het corpus: ze bestaan om een eigenschap
//! van de opstelling aan te tonen en niet om recht weer te geven.

use regelrecht_simulator::{regulation_root, SimulatorError, World, WorldDefinition};

/// Tuig de wereld van dit bestand op, en verwacht dat het misgaat.
fn optuigen(yaml: &str) -> SimulatorError {
    let definition = WorldDefinition::from_yaml(yaml)
        .unwrap_or_else(|e| panic!("het wereldbestand van deze test hoort te lezen: {e}"));
    World::from_definition(&definition, &regulation_root())
        .err()
        .unwrap_or_else(|| panic!("deze wereld hoort niet op te tuigen"))
}

/// De melding moet te lezen zijn door wie het wetsbestand niet voor zich heeft:
/// welke regeling, welke versie, welk artikel, en wat er dan wél mag staan.
fn melding_wijst_de_weg(error: &SimulatorError, regeling: &str) {
    assert!(
        matches!(error, SimulatorError::MalformedChronolexBlock { .. }),
        "verwachtte MalformedChronolexBlock, kreeg {error}"
    );
    let melding = error.to_string();
    for deel in [
        regeling,
        "artikel 1",
        "2024-01-01",
        "afwijzing_wanneer",
        "verplichtingen",
    ] {
        assert!(
            melding.contains(deel),
            "de melding hoort '{deel}' te noemen, kreeg: {melding}"
        );
    }
}

/// **Een typfout in een sleutel is geen artikel dat niets oplegt.**
///
/// `test_chronolex_typfout` schrijft `verplichtignen`. Zonder de gesloten
/// sleutellijst zou dat een beschikking opleveren die niets achterlaat — en een
/// beschikking zonder verplichting is een gewone beschikking, dus er valt
/// nergens meer aan te zien dat de wet iets anders bedoelde.
#[test]
fn een_typfout_in_een_sleutel_weigert_bij_het_optuigen() {
    let error = optuigen(
        r"
clock:
  start: 2024-01-01

cells:
  - id: uitvoerder
    identity: Dienst Toeslagen
    laws:
      - test_chronolex_typfout
    besluit_definitions:
      - name: tegemoetkoming
        doc: de tegemoetkoming die deze testregeling toekent
        regulation: test_chronolex_typfout
        output: tegemoetkoming_toegekend
        outputs:
          - hoogte_tegemoetkoming
        zaakkenmerk: tegemoetkoming/{bsn}
        params:
          - name: bsn
            type: string
        inputs:
          bsn:
            param: bsn
",
    );
    melding_wijst_de_weg(&error, "test_chronolex_typfout");
    assert!(
        error.to_string().contains("verplichtignen"),
        "de melding hoort de sleutel te noemen die er staat: {error}"
    );
}

/// **Een namespace die geen mapping is, wordt niet stil overgeslagen.**
///
/// En de cel hier neemt geen enkel besluit: ze laadt de regeling, meer niet. Dat
/// is met opzet — de strengheid hangt aan de **wet** en niet aan het besluit-pad,
/// dus een blok dat niet te lezen is hoort ook te vallen in een cel die er (nog)
/// niets mee doet.
#[test]
fn een_namespace_die_geen_mapping_is_weigert_bij_het_optuigen() {
    let error = optuigen(
        r"
clock:
  start: 2024-01-01

cells:
  - id: uitvoerder
    identity: Dienst Toeslagen
    laws:
      - test_chronolex_geen_mapping
",
    );
    melding_wijst_de_weg(&error, "test_chronolex_geen_mapping");
    assert!(
        error.to_string().contains("geen array"),
        "de melding hoort te zeggen wat er dan wél staat: {error}"
    );
}

/// **Een andere namespace blijft ongelezen en ongemoeid.**
///
/// `chronolex` is de namespace van deze opstelling; wat een wet onder een andere
/// namespace zet, is niet aan haar. De publieke wereld tuigt gewoon op, en dat
/// is de tegenproef bij de twee weigeringen hierboven: streng is de namespace,
/// niet `extensions` als geheel.
#[test]
fn de_publieke_wereld_tuigt_gewoon_op() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("worlds")
        .join("publieke_wereld.yaml");
    let definition =
        WorldDefinition::load(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    World::from_definition(&definition, &regulation_root())
        .unwrap_or_else(|e| panic!("de publieke wereld hoort op te tuigen: {e}"));
}
