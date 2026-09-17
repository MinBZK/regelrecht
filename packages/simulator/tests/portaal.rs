//! Het portaal: wat het optuigen weigert, en wat een gekozen persona in het beeld
//! verandert.
//!
//! Elke weigering hieronder is een fout die anders stil niets zou doen: een
//! persona-veld dat in geen formulier voorkomt vult niets in, een waarde van het
//! verkeerde type valt pas bij het versturen om, en een vraag met de verkeerde
//! parameters toont op de pagina alleen een fout. Daarom vallen ze bij het
//! optuigen, met een melding die zegt wat er wél kan.
//!
//! Gemeten op de publieke wereld met steeds één ander `portaal`-blok, en niet op
//! een testwereld ernaast: de formulieren waartegen getoetst wordt, zijn precies
//! die van het bestand waarmee de app draait.

use regelrecht_simulator::{
    regulation_root, ActionSnapshot, SimulatorError, Value, World, WorldDefinition,
};
use std::path::{Path, PathBuf};

fn publieke_wereld_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("worlds")
        .join("publieke_wereld.yaml")
}

/// De publieke wereld zoals ze op schijf staat.
fn publieke_wereld() -> World {
    let path = publieke_wereld_path();
    let definition =
        WorldDefinition::load(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    World::from_definition(&definition, &regulation_root())
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// De publieke wereld met een ander `portaal`-blok, of zonder.
fn met_portaal(portaal: Option<&str>) -> WorldDefinition {
    let path = publieke_wereld_path();
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("kan {} niet lezen: {e}", path.display()));
    let (wereld, _) = text
        .split_once("\nportaal:")
        .unwrap_or_else(|| panic!("{} hoort een portaal te dragen", path.display()));
    let yaml = match portaal {
        Some(block) => format!("{wereld}\n{block}"),
        None => wereld.to_string(),
    };
    WorldDefinition::from_yaml(&yaml).unwrap_or_else(|e| panic!("het wereldbestand: {e}"))
}

/// Tuig op en geef de weigering, of een luide fout als het optuigen slaagt.
fn weigering(portaal: &str) -> SimulatorError {
    match World::from_definition(&met_portaal(Some(portaal)), &regulation_root()) {
        Ok(_) => panic!("dit portaal hoort geweigerd te worden:\n{portaal}"),
        Err(e) => e,
    }
}

/// Een `portaal`-blok met deze actor, deze waarden voor de ene persona en deze
/// regels voor het inzicht. Met [`WAARDEN`] en [`INZICHT`] klopt het; elke toets
/// verandert er precies één ding aan.
fn portaal(actor: &str, values: &str, inzicht: &str) -> String {
    format!(
        "portaal:
  actor: {actor}
  label: Aanvraagportaal
  personas:
    - id: a
      label: Aanvrager A
      values:
{values}
  inzicht:
{inzicht}
"
    )
}

const WAARDEN: &str = "        bsn: '999993653'\n        jaar: 2024";

const INZICHT: &str = "    - label: Beschikking
      cell: toeslagen
      lexostatus: zorgtoeslagbeschikking
      params:
        zaakkenmerk: zorgtoeslag/{bsn}";

fn action(world: &World, id: &str) -> ActionSnapshot {
    world
        .snapshot()
        .actions
        .into_iter()
        .find(|action| action.id == id)
        .unwrap_or_else(|| panic!("de wereld hoort actie '{id}' te kennen"))
}

#[test]
fn het_portaal_van_de_publieke_wereld_tuigt_op() {
    let world = publieke_wereld();
    assert_eq!(world.snapshot().persona, None, "er is nog niemand gekozen");
    // En hetzelfde blok, los opgebouwd, ook: de toetsen hieronder gaan over
    // wat ze veranderen en niet over de vorm van het bestand.
    World::from_definition(
        &met_portaal(Some(&portaal("burger", WAARDEN, INZICHT))),
        &regulation_root(),
    )
    .unwrap_or_else(|e| panic!("dit portaal hoort op te tuigen: {e}"));
}

#[test]
fn een_onbekende_actor_wordt_geweigerd() {
    let e = weigering(&portaal("kiezer", WAARDEN, INZICHT));
    assert!(
        matches!(&e, SimulatorError::PortaalUnknownActor { actor, .. } if actor == "kiezer"),
        "{e}"
    );
    assert!(e.to_string().contains("burger"), "{e}");
}

#[test]
fn een_veld_dat_in_geen_formulier_van_de_actor_staat_wordt_geweigerd() {
    // `zaakkenmerk` bestaat wel in de wereld, maar in geen formulier van de
    // burger: dan vult het nooit iets in.
    let e = weigering(&portaal(
        "burger",
        "        bsn: '999993653'\n        zaakkenmerk: zorgtoeslag/1",
        INZICHT,
    ));
    assert!(
        matches!(&e, SimulatorError::PersonaFieldNotInForm { field, .. } if field == "zaakkenmerk"),
        "{e}"
    );
    assert!(e.to_string().contains("ondertekend_op"), "{e}");
}

#[test]
fn een_waarde_die_de_typetoets_van_haar_veld_niet_haalt_wordt_geweigerd() {
    let e = weigering(&portaal(
        "burger",
        "        bsn: 999993653\n        jaar: 2024",
        INZICHT,
    ));
    assert!(
        matches!(&e, SimulatorError::ParameterType { parameter, .. } if parameter == "bsn"),
        "{e}"
    );

    let e = weigering(&portaal(
        "burger",
        "        bsn: '999993653'\n        ondertekend_op: 01-02-2024",
        INZICHT,
    ));
    assert!(
        matches!(&e, SimulatorError::ParameterDate { parameter, .. } if parameter == "ondertekend_op"),
        "{e}"
    );
}

#[test]
fn twee_personas_met_hetzelfde_id_worden_geweigerd() {
    let block = portaal("burger", WAARDEN, INZICHT).replace(
        "  inzicht:",
        "    - id: a\n      label: Nog een A\n  inzicht:",
    );
    let e = weigering(&block);
    assert!(
        matches!(&e, SimulatorError::DuplicatePersona { persona } if persona == "a"),
        "{e}"
    );
}

#[test]
fn een_inzicht_naar_een_onbekende_cel_of_lexostatus_wordt_geweigerd() {
    let e = weigering(&portaal(
        "burger",
        WAARDEN,
        &INZICHT.replace("cell: toeslagen", "cell: kiesraad"),
    ));
    assert!(
        matches!(&e, SimulatorError::InzichtUnknownCell { cell, .. } if cell == "kiesraad"),
        "{e}"
    );

    let e = weigering(&portaal(
        "burger",
        WAARDEN,
        &INZICHT.replace("zorgtoeslagbeschikking", "zetelverdeling"),
    ));
    assert!(
        matches!(&e, SimulatorError::UnknownLexostatus { requested, .. } if requested == "zetelverdeling"),
        "{e}"
    );
    assert!(e.to_string().contains("openstaande_termijnen"), "{e}");
}

#[test]
fn parameters_die_niet_exact_de_inputs_zijn_worden_geweigerd() {
    // Een parameter te veel…
    let e = weigering(&portaal(
        "burger",
        WAARDEN,
        &format!("{INZICHT}\n        jaar: '{{jaar}}'"),
    ));
    assert!(matches!(&e, SimulatorError::InzichtParams { .. }), "{e}");

    // …en een te weinig.
    let e = weigering(&portaal(
        "burger",
        WAARDEN,
        &INZICHT.replace("      params:\n        zaakkenmerk: zorgtoeslag/{bsn}", ""),
    ));
    assert!(
        matches!(&e, SimulatorError::InzichtParams { expected, .. } if expected == "zaakkenmerk"),
        "{e}"
    );
}

#[test]
fn een_verwijzing_naar_een_veld_dat_de_persona_niet_noemt_wordt_geweigerd() {
    let e = weigering(&portaal("burger", "        jaar: 2024", INZICHT));
    assert!(
        matches!(
            &e,
            SimulatorError::InzichtUnknownReference { reference, persona, .. }
                if reference == "bsn" && persona == "a"
        ),
        "{e}"
    );

    let e = weigering(&portaal(
        "burger",
        WAARDEN,
        &INZICHT.replace("zorgtoeslag/{bsn}", "zorgtoeslag/{bsn"),
    ));
    assert!(
        matches!(&e, SimulatorError::InzichtMalformedTemplate { .. }),
        "{e}"
    );
}

#[test]
fn zonder_portaal_is_er_niets_te_kiezen() {
    let mut world = World::from_definition(&met_portaal(None), &regulation_root())
        .unwrap_or_else(|e| panic!("de wereld zonder portaal hoort op te tuigen: {e}"));
    assert_eq!(world.snapshot().persona, None);
    let e = world
        .choose_persona(Some("aanvrager-a"))
        .expect_err("zonder portaal is er geen persona");
    assert!(matches!(e, SimulatorError::NoPortaal), "{e}");
    world
        .choose_persona(None)
        .unwrap_or_else(|e| panic!("niemand kiezen kan altijd: {e}"));
}

/// De kern van de voorinvulling: de waarde van de persona wint van de opgave in
/// het wereldbestand, een veld dat zij niet noemt houdt de gewone voorinvulling,
/// en de formulieren van een andere actor merken er niets van.
#[test]
fn de_waarde_van_de_persona_wint_van_de_voorinvulling_en_laat_de_rest_staan() {
    // Een eigen persona met een ander jaar dan de letterlijke opgave (2024): de
    // aanvragers van de publieke wereld delen dat jaar met het formulier, en dan
    // is niet te zien wie er wint.
    let block = portaal("burger", WAARDEN, INZICHT).replace(
        "  inzicht:",
        "    - id: b\n      label: Aanvrager B\n      values:\n        bsn: '999990019'\n        jaar: 2025\n  inzicht:",
    );
    let mut world = World::from_definition(&met_portaal(Some(&block)), &regulation_root())
        .unwrap_or_else(|e| panic!("dit portaal hoort op te tuigen: {e}"));
    let voor = action(&world, "burger.aanvraag");
    let uitvoerder_voor = action(&world, "toeslagen.toekenning");
    assert_eq!(voor.prefill.get("jaar"), Some(&Value::Int(2024)));

    world
        .choose_persona(Some("b"))
        .unwrap_or_else(|e| panic!("persona b hoort te kiezen te zijn: {e}"));
    assert_eq!(world.snapshot().persona.as_deref(), Some("b"));

    let na = action(&world, "burger.aanvraag");
    assert_eq!(
        na.prefill.get("bsn"),
        Some(&Value::String("999990019".to_string())),
        "haar bsn wint van `$last:brp.relaties.bsn`"
    );
    assert_eq!(
        na.prefill.get("jaar"),
        Some(&Value::Int(2025)),
        "haar jaar wint van de letterlijke opgave"
    );
    assert_eq!(
        na.prefill.get("ondertekend_op"),
        voor.prefill.get("ondertekend_op"),
        "een veld dat zij niet noemt houdt de gewone voorinvulling: de klok"
    );

    let uitvoerder_na = action(&world, "toeslagen.toekenning");
    assert_eq!(
        uitvoerder_na.prefill, uitvoerder_voor.prefill,
        "de formulieren van een andere actor blijven zoals ze waren, ook als ze \
         een veld met dezelfde naam dragen"
    );

    // Wisselen, en weer terug naar niemand.
    world
        .choose_persona(Some("a"))
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(
        action(&world, "burger.aanvraag").prefill.get("jaar"),
        Some(&Value::Int(2024))
    );
    world.choose_persona(None).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(action(&world, "burger.aanvraag").prefill, voor.prefill);
}

#[test]
fn een_onbekende_persona_wordt_geweigerd_en_laat_de_keuze_staan() {
    let mut world = publieke_wereld();
    world
        .choose_persona(Some("aanvrager-a"))
        .unwrap_or_else(|e| panic!("{e}"));
    let e = world
        .choose_persona(Some("aanvrager-z"))
        .expect_err("een onbekende persona hoort geweigerd te worden");
    assert!(
        matches!(&e, SimulatorError::UnknownPersona { persona, .. } if persona == "aanvrager-z"),
        "{e}"
    );
    assert!(e.to_string().contains("aanvrager-b"), "{e}");
    assert_eq!(world.snapshot().persona.as_deref(), Some("aanvrager-a"));
}

#[test]
fn opnieuw_beginnen_laat_de_keuze_staan() {
    let mut world = publieke_wereld();
    world
        .choose_persona(Some("aanvrager-b"))
        .unwrap_or_else(|e| panic!("{e}"));
    world.reset().unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(world.snapshot().persona.as_deref(), Some("aanvrager-b"));
    assert_eq!(
        action(&world, "burger.aanvraag").prefill.get("bsn"),
        Some(&Value::String("999990019".to_string()))
    );
}

/// Beide aanvragers van de publieke wereld komen tot een beschikking, elk met
/// een eigen bedrag.
///
/// Een tweede persona over wie niet besloten kan worden, bewijst op het
/// portaal niets: haar inzicht blijft leeg. Daarom draagt de startstand voor
/// elk van beide wat de toekenning nodig heeft, en toetst dit dat die startstand
/// er is — met de voorinvulling van het portaal, zoals een bezoeker het doet.
#[test]
fn over_beide_aanvragers_van_de_publieke_wereld_kan_besloten_worden() {
    let mut world = publieke_wereld();
    world
        .advance(
            chrono::NaiveDate::from_ymd_opt(2024, 3, 1)
                .unwrap_or_else(|| panic!("2024-03-01 hoort een geldige datum te zijn")),
        )
        .unwrap_or_else(|e| panic!("{e}"));

    let mut bedragen = Vec::new();
    for (persona, bsn) in [("aanvrager-a", "999993653"), ("aanvrager-b", "999990019")] {
        world
            .choose_persona(Some(persona))
            .unwrap_or_else(|e| panic!("{e}"));
        let aanvraag = action(&world, "burger.aanvraag").prefill;
        world
            .act("burger.aanvraag", &aanvraag)
            .unwrap_or_else(|e| panic!("{persona}: de aanvraag hoort te kunnen: {e}"));

        let toekenning = action(&world, "toeslagen.toekenning");
        assert_eq!(
            toekenning.prefill.get("bsn"),
            Some(&Value::String(bsn.to_string())),
            "{persona}: de uitvoerder beslist op de aanvraag die net binnenkwam"
        );
        world
            .act("toeslagen.toekenning", &toekenning.prefill)
            .unwrap_or_else(|e| panic!("{persona}: de toekenning hoort te kunnen: {e}"));

        let params = [(
            "zaakkenmerk".to_string(),
            Value::String(format!("zorgtoeslag/{bsn}")),
        )]
        .into_iter()
        .collect();
        let beschikking = world
            .reduce("toeslagen", "zorgtoeslagbeschikking", &params, world.now())
            .unwrap_or_else(|e| panic!("{persona}: {e}"));
        let hoogte = beschikking
            .values()
            .and_then(|values| values.get("hoogte_zorgtoeslag").cloned())
            .unwrap_or_else(|| panic!("{persona}: hoort een beschikking met een hoogte te hebben"));
        bedragen.push(hoogte);
    }
    assert_ne!(
        bedragen[0], bedragen[1],
        "een ander inkomen hoort een ander bedrag op te leveren"
    );
}

/// De vragen van het inzicht staan per persona klaar, ingevuld met haar waarden.
#[test]
fn het_inzicht_is_per_persona_ingevuld() {
    let definition = met_portaal(Some(&portaal("burger", WAARDEN, INZICHT)));
    let portaal = definition
        .portaal
        .as_ref()
        .expect("het portaal hoort gelezen te zijn")
        .snapshot();
    let vraag = &portaal.personas[0].inzicht[0];
    assert_eq!(vraag.cell, "toeslagen");
    assert_eq!(
        vraag.params.get("zaakkenmerk").map(String::as_str),
        Some("zorgtoeslag/999993653")
    );
    assert_eq!(
        portaal.inzicht[0]
            .params
            .get("zaakkenmerk")
            .map(String::as_str),
        Some("zorgtoeslag/{bsn}"),
        "het sjabloon zelf gaat ook mee"
    );
}
