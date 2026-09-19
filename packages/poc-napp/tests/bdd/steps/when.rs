//! When steps: law execution.

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use crate::world::NappWorld;
use cucumber::when;

#[when("the subsidiebesluit is executed")]
fn execute_subsidiebesluit(world: &mut NappWorld) {
    world.execute_besluit();
}

/// Wpp art. 14 rechtstreeks: de specificatie van de landelijke subsidie in
/// de vier delen (partij + neveninstellingen).
#[when("the subsidiebedragen of artikel 14 are calculated")]
fn execute_artikel_14(world: &mut NappWorld) {
    world.apply_besluit_defaults();
    world.execute(
        "wet_op_de_politieke_partijen",
        &[
            "subsidie_partij",
            "subsidie_wetenschappelijk_instituut",
            "subsidie_jongerenorganisatie",
            "subsidie_buitenland",
            "subsidie_landelijk",
        ],
    );
}

/// Wpp art. 17 (lex specialis t.o.v. AWB 4:13): aanvraag uiterlijk
/// 1 november voorafgaand aan het subsidiejaar, besluit voor 1 januari.
#[when("the verleningstermijnen are calculated")]
fn execute_verleningstermijnen(world: &mut NappWorld) {
    world.execute(
        "wet_op_de_politieke_partijen",
        &[
            "aanvraagtermijn_einddatum",
            "beslistermijn_einddatum",
            "voorschotpercentage",
        ],
    );
}

#[when("the termijnverlenging is calculated")]
fn execute_termijnverlenging(world: &mut NappWorld) {
    world.execute("algemene_termijnenwet", &["verlengde_einddatum"]);
}

/// Orkestratie zoals de backend: AWB 4:13 (beslistermijn) gevolgd door de
/// Algemene termijnenwet op die einddatum.
#[when("the beslistermijn is calculated including the termijnenwet")]
fn execute_beslistermijn_keten(world: &mut NappWorld) {
    world.execute("algemene_wet_bestuursrecht", &["beslistermijn_einddatum"]);
    let einddatum = world
        .get_output("beslistermijn_einddatum")
        .cloned()
        .expect("AWB 4:13 leverde geen einddatum");
    world.parameters.insert("einddatum".to_string(), einddatum);
    world.execute("algemene_termijnenwet", &["verlengde_einddatum"]);
}

/// Orkestratie zoals de backend: eerst AWB 6:8 (einddatum), daarna de
/// Algemene termijnenwet (weekend-verlenging) op die einddatum.
#[when("the bezwaartermijn is calculated including the termijnenwet")]
fn execute_bezwaartermijn_keten(world: &mut NappWorld) {
    world.execute(
        "algemene_wet_bestuursrecht",
        &["bezwaartermijn_startdatum", "bezwaartermijn_einddatum"],
    );
    let einddatum = world
        .get_output("bezwaartermijn_einddatum")
        .cloned()
        .expect("AWB 6:8 leverde geen einddatum");
    world.parameters.insert("einddatum".to_string(), einddatum);
    world.execute("algemene_termijnenwet", &["verlengde_einddatum"]);
}

/// Wpp art. 13: eenmalige verstrekking per subsidiejaar.
#[when("the beschikbaarheid of artikel 13 is evaluated")]
fn execute_artikel_13(world: &mut NappWorld) {
    world.execute("wet_op_de_politieke_partijen", &["onderdeel_beschikbaar"]);
}

/// Wpp art. 27: rekening op naam van de rechtspersoon, wijzigen alleen
/// door het tekenbevoegd bestuur, aanhouden zonder rekening.
#[when("the rekening-regels of artikel 27 are evaluated")]
fn execute_artikel_27(world: &mut NappWorld) {
    world.execute(
        "wet_op_de_politieke_partijen",
        &[
            "rekening_aanvaardbaar",
            "mag_rekening_wijzigen",
            "uitbetaling_aangehouden",
        ],
    );
}

/// Kieswet G 1: de registratie-eisen voor een aanduiding.
#[when("the registratie-eisen of Kieswet G 1 are evaluated")]
fn execute_kieswet_g1(world: &mut NappWorld) {
    world.execute(
        "kieswet",
        &[
            "voldoet_aan_registratie_eisen",
            "voldoet_eis_inschrijving",
            "voldoet_eis_rechtsvorm",
            "voldoet_eis_naam",
        ],
    );
}

/// Generieke AWB-evaluatie: de gevraagde outputs, kommagescheiden.
#[when(regex = r#"^the AWB outputs "([^"]+)" are evaluated$"#)]
fn execute_awb_outputs(world: &mut NappWorld, outputs: String) {
    let gevraagd: Vec<&str> = outputs.split(',').map(str::trim).collect();
    world.execute("algemene_wet_bestuursrecht", &gevraagd);
}
