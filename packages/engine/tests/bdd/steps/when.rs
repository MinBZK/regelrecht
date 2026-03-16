//! When step definitions
//!
//! Steps that execute actions (law evaluations).

use cucumber::when;
use regelrecht_engine::Value;

use crate::world::RegelrechtWorld;

// =============================================================================
// Bijstand steps
// =============================================================================

#[when(regex = r"^the bijstandsaanvraag is executed for participatiewet article (\d+)$")]
fn execute_bijstandsaanvraag(world: &mut RegelrechtWorld, _article: String) {
    // The bijstand calculation uses article 43 which produces uitkering_bedrag
    // We execute the law for the "uitkering_bedrag" output
    world.execute_law("participatiewet", "uitkering_bedrag");
}

// =============================================================================
// Erfgrensbeplanting steps
// =============================================================================

#[when(regex = r"^the erfgrensbeplanting is requested for (\S+) article (\d+)$")]
fn execute_erfgrensbeplanting(world: &mut RegelrechtWorld, law_id: String, _article: String) {
    // Execute for minimale_afstand_cm output
    world.execute_law(&law_id, "minimale_afstand_cm");
}

// =============================================================================
// Zorgtoeslag steps
// =============================================================================

#[when(regex = r"^I request the standard premium for year (\d+)$")]
fn request_standard_premium(world: &mut RegelrechtWorld, year: String) {
    // Set the calculation date to the requested year
    world.calculation_date = format!("{}-01-01", year);

    // The standard premium is defined in regeling_standaardpremie
    world.execute_law("regeling_standaardpremie", "standaardpremie");
}

#[when("the healthcare allowance law is executed")]
fn execute_healthcare_allowance(world: &mut RegelrechtWorld) {
    // Register raw external data as DataSources (no pre-computation).
    // The engine resolves through cross-law references automatically.
    register_if_present(
        &mut world.service,
        "personal_data",
        &world.external_data.rvig_personal,
    );
    register_if_present(
        &mut world.service,
        "relationship_data",
        &world.external_data.rvig_relationship,
    );
    register_if_present(
        &mut world.service,
        "insurance",
        &world.external_data.rvz_insurance,
    );
    register_if_present(&mut world.service, "box1", &world.external_data.bd_box1);
    register_if_present(&mut world.service, "box2", &world.external_data.bd_box2);
    register_if_present(&mut world.service, "box3", &world.external_data.bd_box3);
    register_if_present(
        &mut world.service,
        "detenties",
        &world.external_data.dji_detenties,
    );
    register_if_present(
        &mut world.service,
        "inschrijvingen",
        &world.external_data.duo_inschrijvingen,
    );
    register_if_present(
        &mut world.service,
        "studiefinanciering",
        &world.external_data.duo_studiefinanciering,
    );

    // Execute — engine resolves through cross-law references automatically.
    // BSN stays in parameters (set by set_rvig_personal_data in given.rs).
    world.execute_law("zorgtoeslagwet", "hoogte_zorgtoeslag");
}

fn register_if_present(
    service: &mut regelrecht_engine::LawExecutionService,
    name: &str,
    data: &std::collections::HashMap<String, std::collections::HashMap<String, Value>>,
) {
    if !data.is_empty() {
        let records: Vec<_> = data.values().cloned().collect();
        service
            .register_dict_source(name, "bsn", records)
            .expect("Failed to register data source");
    }
}
