//! Wanneer een actie kan, en waarom ze anders niet kan.
//!
//! Het antwoord staat **niet** in het wereldbestand. Een besluit zegt in zijn
//! eigen definitie al welke feiten het uit welke eigen kroniek leest, dus dat is
//! ook waar "kan dit nu?" uit volgt: levert elke input die de cel bij zichzelf
//! ophaalt op dit moment een waarde op, dan kan de actie. Een voorwaarde die er
//! met de hand naast geschreven wordt, is een tweede opsomming van hetzelfde — en
//! die loopt uit de pas zodra er een input bij komt.
//!
//! Twee dingen worden hier vastgepind, en ze horen bij elkaar. Dát de
//! beschikbaarheid meebeweegt met de kronieken, en dat de reden het **feit**
//! noemt dat ontbreekt: welke cel, welke stroom, welk onderwerp en op welk
//! moment. Een kaart die alleen "kan nu niet" zegt, laat een lezer raden waar het
//! verhaal vastzit.
//!
//! Wat hier met opzet *niet* meetelt is een input die van een andere organisatie
//! geaccepteerd wordt. Dat zou een vraag over een celgrens vergen, en het beeld
//! van de wereld wordt bij elke stap opgevraagd; zie
//! `tests/invarianten.rs` voor de meting die dat afdwingt.

use regelrecht_simulator::{
    regulation_root, ActionDefinition, ActionEffect, ActionSnapshot, DecidesAction, Value, World,
    WorldDefinition,
};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// De publieke wereld: het bestand waarmee de HTTP-laag lokaal draait.
///
/// Dat bestand en geen testwereld ernaast: wat een lezer op zijn scherm ziet, is
/// precies wat hier gemeten wordt.
fn publieke_wereld_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("worlds")
        .join("publieke_wereld.yaml")
}

fn publieke_wereld() -> World {
    let path = publieke_wereld_path();
    let definition =
        WorldDefinition::load(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    World::from_definition(&definition, &regulation_root())
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// Eén actie uit het beeld, of een luide fout: een test die stilvalt omdat de
/// actie niet meer bestaat, bewijst niets.
fn action(world: &World, id: &str) -> ActionSnapshot {
    world
        .snapshot()
        .actions
        .into_iter()
        .find(|action| action.id == id)
        .unwrap_or_else(|| panic!("de wereld hoort actie '{id}' te kennen"))
}

/// Waarom een actie nu niet kan, of een luide fout als ze wél kan.
fn reason(world: &World, id: &str) -> String {
    let action = action(world, id);
    assert!(
        !action.available,
        "'{id}' hoort nu niet te kunnen, maar het beeld biedt haar aan"
    );
    action
        .unavailable_reason
        .unwrap_or_else(|| panic!("een actie die niet kan, hoort te zeggen waarom niet"))
}

fn text(value: &str) -> Value {
    Value::String(value.to_string())
}

/// Het formulier van de aanvraag, met het burgerservicenummer dat de aanvrager
/// invult.
fn aanvraag(bsn: &str) -> BTreeMap<String, Value> {
    BTreeMap::from([
        ("bsn".to_string(), text(bsn)),
        ("jaar".to_string(), Value::Int(2024)),
        ("ondertekend_op".to_string(), text("2024-01-01")),
    ])
}

/// Vóór de aanvraag kan het besluit niet, erna wel.
///
/// Nergens in het wereldbestand staat dat. Het besluit leest `is_verzekerde` uit
/// de eigen kroniek `inkomensleveringen`, op de persoon uit de aanvraag die deze
/// cel geleverd kreeg — dus zolang die levering er niet ligt, is er geen persoon
/// en dus geen feit. Zodra ze er ligt, wijst de voorinvulling de persoon aan en
/// levert de kroniek haar feit.
#[test]
fn een_besluit_kan_pas_als_zijn_eigen_feit_er_ligt() {
    let mut world = publieke_wereld();

    let reden = reason(&world, "toeslagen.toekenning");
    assert!(
        reden.contains("inkomensleveringen") && reden.contains("bsn"),
        "de reden hoort te zeggen welk feit over wie ontbreekt, kreeg: {reden}"
    );

    world
        .act("burger.aanvraag", &aanvraag("999993653"))
        .unwrap_or_else(|e| panic!("de aanvraag moet kunnen: {e}"));

    let na = action(&world, "toeslagen.toekenning");
    assert!(
        na.available && na.unavailable_reason.is_none(),
        "met de aanvraag erbij ligt het feit er, dus het besluit kan: {:?}",
        na.unavailable_reason
    );
}

/// De reden noemt het feit: de cel, de stroom, het onderwerp en het moment.
///
/// Hier ligt er wél een aanvraag, maar over iemand van wie deze cel niets weet.
/// Dat is het interessante geval, want dan is de voorinvulling ingevuld en is het
/// de kroniek zelf die niets oplevert — en dan hoort er in de reden te staan
/// waarnaar er gezocht is, tot en met de dag waarop.
#[test]
fn de_reden_noemt_het_feit_dat_ontbreekt() {
    let mut world = publieke_wereld();
    world
        .act("burger.aanvraag", &aanvraag("111111111"))
        .unwrap_or_else(|e| panic!("de aanvraag moet kunnen: {e}"));

    let reden = reason(&world, "toeslagen.toekenning");
    for deel in ["toeslagen", "inkomensleveringen", "111111111", "2024-01-01"] {
        assert!(
            reden.contains(deel),
            "de reden hoort '{deel}' te noemen, kreeg: {reden}"
        );
    }
}

/// Een actie die een feit vastlegt, kan altijd.
///
/// Zij *is* het feit. Haar laten wachten tot er iets ligt zou betekenen dat een
/// actor niet kan vastleggen wat hem overkwam — en dan is er in een verse wereld
/// niets te beginnen.
#[test]
fn een_vastlegging_kan_altijd() {
    let mut world = publieke_wereld();
    let vastleggingen = |world: &World| -> Vec<ActionSnapshot> {
        world
            .snapshot()
            .actions
            .into_iter()
            .filter(|action| {
                matches!(
                    action.effect,
                    regelrecht_simulator::ActionEffectSnapshot::Records { .. }
                )
            })
            .collect()
    };

    for stand in ["vóór", "ná"] {
        let acties = vastleggingen(&world);
        assert!(
            !acties.is_empty(),
            "{stand} de aanvraag hoort deze wereld vastleggende acties te hebben, \
             anders meet deze test niets"
        );
        for actie in acties {
            assert!(
                actie.available && actie.unavailable_reason.is_none(),
                "{stand} de aanvraag hoort '{}' te kunnen: {:?}",
                actie.id,
                actie.unavailable_reason
            );
        }
        if stand == "vóór" {
            world
                .act("burger.aanvraag", &aanvraag("999993653"))
                .unwrap_or_else(|e| panic!("de aanvraag moet kunnen: {e}"));
        }
    }
}

/// Een wereld met twee besluiten over dezelfde zaak, waarvan het tweede het
/// eerste **terugleest**.
///
/// Klein en met opzet buiten de publieke wereld: geen corpuswet kent "wat er
/// eerder is toegekend" als input, dus de vaststelling hieronder voert een
/// testregeling van deze crate uit (`fixtures/regulation/test_nabetaling`). Wat
/// zij moet laten zien is één ding — dat `from_decretogram` op precies dezelfde
/// manier meetelt als een eigen kroniek — en daar is geen heel verhaal voor
/// nodig.
const TERUGLEZENDE_WERELD: &str = r"
clock:
  start: 2024-01-01

cells:
  - id: toeslagen
    identity: Dienst Toeslagen
    laws:
      - wet_op_de_zorgtoeslag
      - algemene_wet_inkomensafhankelijke_regelingen
      - regeling_standaardpremie
      - test_nabetaling

    chronicles:
      - stream: relaties
        key: bsn
        events:
          - name: relatie_gewijzigd
            intake: levering
            recording_actor: toeslagen
            grondslag: melding uit de basisregistratie personen
            op_moment: 2023-01-01
            fields:
              bsn: '999993653'
              partnerschap_type: GEEN

      - stream: inkomensleveringen
        key: bsn
        events:
          - name: inkomenslevering
            intake: levering
            recording_actor: toeslagen
            grondslag: jaarlijkse inkomenslevering
            op_moment: 2023-11-15
            fields:
              bsn: '999993653'
              is_verzekerde: true
              verzamelinkomen: 79547
              buitenlands_inkomen: 0
              vermogen: 0

    besluit_definitions:
      - name: zorgtoeslag_toekenning
        doc: het eerste besluit over de zaak
        regulation: wet_op_de_zorgtoeslag
        output: heeft_recht_op_zorgtoeslag
        outputs:
          - hoogte_zorgtoeslag
        zaakkenmerk: zorgtoeslag/{bsn}
        params:
          - name: bsn
            type: string
            prefill: $last:toeslagen.inkomensleveringen.bsn
        inputs:
          bsn:
            param: bsn
          is_verzekerde:
            from_chronicle: inkomensleveringen
            field: is_verzekerde

      - name: zorgtoeslag_nabetaling
        doc: de vaststelling, die het toegekende bedrag uit het eerste gram haalt
        regulation: test_nabetaling
        output: nog_te_betalen
        zaakkenmerk: zorgtoeslag/{bsn}
        params:
          - name: bsn
            type: string
            prefill: $last:toeslagen.inkomensleveringen.bsn
        inputs:
          bsn:
            param: bsn
          toegekend_bedrag:
            from_decretogram: zorgtoeslag_toekenning
            field: hoogte_zorgtoeslag

    lexostatus_definitions:
      - name: zorgtoeslagbeschikking
        doc: wat deze cel over deze zaak besloten heeft
        inputs:
          - name: zaakkenmerk
            type: string
        outputs:
          - heeft_recht_op_zorgtoeslag
          - hoogte_zorgtoeslag
        reduction:
          chronicle: beschikkingen
          key: zaakkenmerk
          latest: true
";

/// Eén `decides`-actie, in Rust en niet in het YAML hierboven.
///
/// De wereld hierboven is er om de **besluiten** te dragen; welke knoppen er
/// boven staan, is voor deze meting bijzaak en hoort de lezer niet nog eens door
/// te moeten lezen.
fn besluit_actie(id: &str, besluit: &str) -> ActionDefinition {
    ActionDefinition {
        id: id.to_string(),
        actor: "toeslagen".to_string(),
        label: format!("Neem besluit {besluit}"),
        doc: None,
        effect: ActionEffect::Decides(DecidesAction {
            cell: "toeslagen".to_string(),
            besluit: besluit.to_string(),
        }),
    }
}

/// Een besluit dat een eerder besluit terugleest, kan pas als dat er ligt.
///
/// Dezelfde regel als bij een eigen kroniek, en dat is het punt: de cel leest het
/// eerdere gram bij zichzelf, over dezelfde zaak, dus het is gewoon een van haar
/// eigen feiten. Ligt het er niet, dan noemt de reden het besluit en het
/// zaakkenmerk waarop gezocht is.
#[test]
fn een_besluit_dat_terugleest_kan_pas_als_dat_eerdere_besluit_er_ligt() {
    let mut definition = WorldDefinition::from_yaml(TERUGLEZENDE_WERELD)
        .unwrap_or_else(|e| panic!("de testwereld moet leesbaar zijn: {e}"));
    definition.actions = vec![
        besluit_actie("toeslagen.toekenning", "zorgtoeslag_toekenning"),
        besluit_actie("toeslagen.nabetaling", "zorgtoeslag_nabetaling"),
    ];
    let mut world = World::from_definition(&definition, &regulation_root())
        .unwrap_or_else(|e| panic!("de testwereld moet op te tuigen zijn: {e}"));

    let toekenning = action(&world, "toeslagen.toekenning");
    assert!(
        toekenning.available,
        "het eerste besluit rekent op een eigen kroniek die gevuld is: {:?}",
        toekenning.unavailable_reason
    );

    let reden = reason(&world, "toeslagen.nabetaling");
    assert!(
        reden.contains("zorgtoeslag_toekenning") && reden.contains("zorgtoeslag/999993653"),
        "de reden hoort het besluit en de zaak te noemen waarop gezocht is, kreeg: {reden}"
    );

    world
        .act(
            "toeslagen.toekenning",
            &BTreeMap::from([("bsn".to_string(), text("999993653"))]),
        )
        .unwrap_or_else(|e| panic!("het eerste besluit moet genomen kunnen worden: {e}"));

    let na = action(&world, "toeslagen.nabetaling");
    assert!(
        na.available && na.unavailable_reason.is_none(),
        "met het eerste gram in de kroniek kan de vaststelling: {:?}",
        na.unavailable_reason
    );
}
