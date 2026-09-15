//! Het uitvoeringsreceipt van een decretogram, opgevraagd bij de wereld.
//!
//! RFC-022 §1.2 zegt dat een decretogram *is* het RFC-013 Execution Receipt van
//! het besluit. Dat het in het gram ligt, staat al vast in
//! [`tests/snapshot.rs`](snapshot.rs) — daar juist als het omgekeerde: het beeld
//! van de wereld draagt het **niet**, want het bevat wandkloktijd en een contract
//! dat per run verschilt is geen contract.
//!
//! Wat hier getoetst wordt, is de weg ernaartoe op verzoek. Drie dingen:
//!
//! 1. Het receipt is er, met alle secties van RFC-013 erin.
//! 2. Een geaccepteerde waarde noemt het **bevoegd gezag van de bron-cel**, en
//!    niet alleen het adres waar ze opgehaald is. Dat is de scherpe: een cel-id
//!    is een adres, een gezag is een gezag, en voor `accepted_values` telt het
//!    tweede.
//! 3. Wie naar een gram wijst dat geen decretogram uit het besluit-pad is, krijgt
//!    "er is er geen" — niet een leeg receipt.
//!
//! De wereld is het echte wereldbestand en niet een fixture: wat hier langs komt,
//! is wat een deployment te zien krijgt.

use regelrecht_simulator::{
    regulation_root, SimulatorError, Value, World, WorldDefinition, BESCHIKKINGEN,
};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// De BSN van de persoon in het verhaal van de publieke wereld.
const BSN: &str = "999993653";

fn world_file() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("worlds")
        .join("publieke_wereld.yaml")
}

/// Een wereld waarin één toekenning genomen is.
///
/// Het korte verhaal en niet het volledige: één besluit met één geaccepteerde
/// waarde is precies wat een receipt hier moet kunnen laten zien.
fn met_een_toekenning() -> World {
    let path = world_file();
    let definition =
        WorldDefinition::load(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let mut world = World::from_definition(&definition, &regulation_root())
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()));

    world
        .act(
            "burger.aanvraag",
            &BTreeMap::from([
                ("bsn".to_string(), Value::String(BSN.to_string())),
                ("jaar".to_string(), Value::Int(2024)),
                (
                    "ondertekend_op".to_string(),
                    Value::String("2024-01-09".to_string()),
                ),
            ]),
        )
        .expect("de aanvraag hoort te lukken");
    world
        .advance("2024-03-01".parse().expect("een datum"))
        .expect("de klok hoort vooruit te kunnen");
    world
        .act(
            "toeslagen.toekenning",
            &BTreeMap::from([("bsn".to_string(), Value::String(BSN.to_string()))]),
        )
        .expect("het besluit hoort te lukken");
    world
}

/// Het receipt van de toekenning draagt elke sectie van RFC-013.
///
/// Op naam en niet op vorm: wat er in een sectie staat is van de engine, maar
/// dát ze er zijn is wat een decretogram tot receipt maakt. `scope` moet de
/// geladen regelingen met hun hash noemen, want zonder die is het besluit niet
/// te reproduceren.
#[test]
fn het_receipt_van_een_decretogram_draagt_alle_rfc013_secties() {
    let world = met_een_toekenning();
    let receipt = world
        .gram_receipt("toeslagen", BESCHIKKINGEN, 0)
        .expect("het decretogram hoort een receipt te dragen");

    for sectie in [
        "provenance",
        "engine_config",
        "scope",
        "execution",
        "results",
    ] {
        assert!(
            receipt.sections.contains_key(sectie),
            "sectie '{sectie}' hoort in het receipt te staan (wel: {:?})",
            receipt.sections.keys().collect::<Vec<_>>()
        );
    }

    let provenance = receipt.sections["provenance"]
        .as_object()
        .expect("provenance is een object");
    assert_eq!(
        provenance.get("regulation_id").and_then(Value::as_str),
        Some("wet_op_de_zorgtoeslag"),
        "het receipt hoort de uitgevoerde regeling te noemen"
    );
    assert!(
        provenance
            .get("regulation_valid_from")
            .and_then(Value::as_str)
            .is_some(),
        "het receipt hoort de wetsversie te noemen die toen gold"
    );

    let geladen = receipt.sections["scope"]
        .as_object()
        .and_then(|scope| scope.get("loaded_regulations"))
        .and_then(Value::as_array)
        .expect("scope.loaded_regulations is een lijst");
    assert!(
        !geladen.is_empty(),
        "een uitvoering laadt ten minste de regeling die ze uitvoert"
    );
    for regeling in geladen {
        let regeling = regeling.as_object().expect("een geladen regeling");
        assert!(
            regeling.get("id").and_then(Value::as_str).is_some(),
            "een geladen regeling hoort haar $id te noemen"
        );
        assert!(
            regeling.get("hash").and_then(Value::as_str).is_some(),
            "een geladen regeling hoort haar hash te dragen: zonder die is de \
             uitvoering niet te reproduceren"
        );
    }

    // De wandkloktijd staat erin mét het label dat zegt wat voor tijd het is.
    // Precies dat onderscheid is de reden dat het beeld van de wereld dit receipt
    // niet draagt.
    assert!(
        receipt.timestamp.wall_clock.is_some(),
        "het receipt van een uitvoering draagt haar wandkloktijd"
    );
    assert!(
        receipt.timestamp.note.contains("wandkloktijd"),
        "de tijdstempel hoort gelabeld te zijn: {}",
        receipt.timestamp.note
    );
    assert_eq!(receipt.gram.op_moment.to_string(), "2024-03-01");
}

/// De scherpe: een geaccepteerde waarde noemt het gezag dat haar vaststelde.
///
/// Het toetsingsinkomen komt van de cel `belastingdienst`, en die publiceert bij
/// haar lexostatus het bevoegd gezag `Belastingdienst`. Dat gezag hoort in het
/// receipt van Dienst Toeslagen te staan: wie het terugleest, hoort te zien
/// wiens vaststelling geaccepteerd is en niet alleen bij welk systeem ze
/// opgehaald is.
#[test]
fn een_geaccepteerde_waarde_noemt_het_bevoegd_gezag_van_de_bron() {
    let world = met_een_toekenning();
    let receipt = world
        .gram_receipt("toeslagen", BESCHIKKINGEN, 0)
        .expect("het decretogram hoort een receipt te dragen");

    let toetsingsinkomen = receipt
        .accepted_values
        .iter()
        .find(|accepted| accepted.output == "toetsingsinkomen")
        .unwrap_or_else(|| {
            panic!(
                "het toetsingsinkomen is geaccepteerd en hoort in accepted_values te staan (wel: \
                 {:?})",
                receipt
                    .accepted_values
                    .iter()
                    .map(|accepted| &accepted.output)
                    .collect::<Vec<_>>()
            )
        });

    assert_eq!(toetsingsinkomen.cell, "belastingdienst");
    assert_eq!(
        toetsingsinkomen.authority.as_deref(),
        Some("Belastingdienst"),
        "de bron-cel noemt haar bevoegd gezag, en dat hoort in het receipt"
    );
    assert_eq!(
        toetsingsinkomen.lexostatus.as_deref(),
        Some("toetsingsinkomen")
    );
    assert_eq!(
        toetsingsinkomen.op_moment.map(|moment| moment.to_string()),
        Some("2024-03-01".to_string())
    );
    assert_eq!(
        toetsingsinkomen.zaakkenmerk.as_deref(),
        Some("zorgtoeslag/999993653"),
        "de zaak waarvoor geaccepteerd is, hoort erbij te staan"
    );
    assert!(
        toetsingsinkomen.asked_by.is_some() && toetsingsinkomen.signature.is_some(),
        "een geaccepteerde waarde draagt wie er vroeg en hoe dat ondertekend was"
    );

    // Het gezag van de bron is niet het gezag van het besluit: dat laatste is
    // Dienst Toeslagen, en dat die twee verschillen is het hele punt van een
    // geaccepteerde waarde.
    assert_ne!(
        toetsingsinkomen.authority.as_deref(),
        Some("Dienst Toeslagen")
    );
}

/// Een gram dat geen decretogram uit het besluit-pad is, draagt geen receipt —
/// en dan is "er is er geen" het antwoord en niet een leeg receipt.
#[test]
fn een_executogram_draagt_geen_receipt() {
    let world = met_een_toekenning();
    let error = world
        .gram_receipt("toeslagen", "aanvragen", 0)
        .expect_err("een aanvraag is geen uitvoering");
    assert!(
        matches!(error, SimulatorError::GramWithoutReceipt { .. }),
        "{error}"
    );
}

/// Niet elk decretogram draagt een receipt, en het beeld zegt aan welk veld dat
/// te zien is.
///
/// Een bron-cel zonder engine legt haar eigen vaststelling ook als decretogram
/// vast — even goed een besluit van die cel, alleen heeft er nooit een uitvoering
/// gedraaid. Wie op de soort alleen afgaat, biedt een receipt aan dat er niet is.
/// Het onderscheid staat in het beeld: `Decretogram::event` schrijft de
/// **regeling** en het receipt in dezelfde vastlegging, dus een gram met een
/// regeling in beeld is precies een gram met een receipt. Daar leunt de uitklap
/// in het Grammen-tabblad op, en daarom staat het hier vast.
#[test]
fn het_beeld_zegt_aan_de_regeling_welk_gram_een_receipt_draagt() {
    let world = met_een_toekenning();
    let beeld = world.snapshot();

    let mut zonder_receipt = 0;
    for cel in &beeld.cells {
        for kroniek in &cel.chronicles {
            for (plek, gram) in kroniek.grams.iter().enumerate() {
                let draagt_regeling = gram.fields.contains_key("regulation");
                let receipt = world.gram_receipt(&cel.id, &kroniek.stream, plek);
                assert_eq!(
                    receipt.is_ok(),
                    draagt_regeling,
                    "cel '{}', stroom '{}', plek {plek} ('{}'): het beeld en het receipt \
                     horen het eens te zijn over of er een uitvoering achter zat",
                    cel.id,
                    kroniek.stream,
                    gram.name
                );
                if !draagt_regeling {
                    zonder_receipt += 1;
                }
            }
        }
    }
    assert!(
        zonder_receipt > 0,
        "deze wereld hoort grammen te hebben waar geen uitvoering achter zat"
    );
}

/// Een eigen vaststelling van een bron-cel is een decretogram zonder receipt.
///
/// De scherpe van hierboven, op één gram: `belastingdienst` heeft geen engine en
/// legt haar aanslag zelf vast. Dat is `intake: eigen_besluit` en dus een
/// decretogram, maar er is nooit iets uitgevoerd — en dan is "er is er geen" het
/// antwoord.
#[test]
fn een_eigen_vaststelling_zonder_engine_draagt_geen_receipt() {
    let world = met_een_toekenning();
    let error = world
        .gram_receipt("belastingdienst", "aanslagen", 0)
        .expect_err("een bron-cel zonder engine voert niets uit");
    assert!(
        matches!(error, SimulatorError::GramWithoutReceipt { .. }),
        "{error}"
    );
}

/// Wijzen naar iets dat er niet is, zegt wat er wél is.
#[test]
fn een_onbekende_kroniek_of_plek_noemt_wat_er_wel_ligt() {
    let world = met_een_toekenning();

    let stroom = world
        .gram_receipt("toeslagen", "bestaat_niet", 0)
        .expect_err("die stroom houdt deze cel niet");
    assert!(
        matches!(stroom, SimulatorError::UnknownChronicle { .. }),
        "{stroom}"
    );
    assert!(
        stroom.to_string().contains(BESCHIKKINGEN),
        "de melding hoort te noemen welke stromen er wél zijn: {stroom}"
    );

    let plek = world
        .gram_receipt("toeslagen", BESCHIKKINGEN, 7)
        .expect_err("op plek 7 ligt niets");
    assert!(
        matches!(plek, SimulatorError::UnknownGram { count: 1, .. }),
        "{plek}"
    );

    let cel = world
        .gram_receipt("bestaat_niet", BESCHIKKINGEN, 0)
        .expect_err("die cel kent deze wereld niet");
    assert!(matches!(cel, SimulatorError::UnknownCell { .. }), "{cel}");
}
