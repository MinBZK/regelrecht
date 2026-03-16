//! Given step definitions
//!
//! Steps that set up the initial state for scenarios.

use cucumber::{gherkin::Step, given};

use crate::helpers::value_conversion::{convert_gherkin_value, parse_table_to_params};
use crate::world::RegelrechtWorld;

// =============================================================================
// Background steps
// =============================================================================

#[given(expr = "the calculation date is {string}")]
fn set_calculation_date(world: &mut RegelrechtWorld, date: String) {
    world.calculation_date = date;
}

// =============================================================================
// Bijstand steps
// =============================================================================

#[given("a citizen with the following data:")]
fn set_citizen_data(world: &mut RegelrechtWorld, step: &Step) {
    if let Some(table) = &step.table {
        world.parameters = parse_table_to_params(table);

        // Ensure BSN is present (generate test BSN if not provided)
        if !world.parameters.contains_key("bsn") {
            world.parameters.insert(
                "bsn".to_string(),
                regelrecht_engine::Value::String("123456789".to_string()),
            );
        }
    }
}

// =============================================================================
// Erfgrensbeplanting steps
// =============================================================================

#[given("a query with the following data:")]
fn set_query_data(world: &mut RegelrechtWorld, step: &Step) {
    if let Some(table) = &step.table {
        world.parameters = parse_table_to_params(table);
    }
}

// =============================================================================
// Zorgtoeslag external data steps
// =============================================================================

#[given(regex = r#"the following RVIG "personal_data" data:"#)]
fn set_rvig_personal_data(world: &mut RegelrechtWorld, step: &Step) {
    if let Some(table) = &step.table {
        parse_external_data_table(table, &mut world.external_data.rvig_personal);
        // Store BSN in parameters (required article parameter)
        for data in world.external_data.rvig_personal.values() {
            if let Some(v) = data.get("bsn") {
                world.parameters.insert("bsn".to_string(), v.clone());
            }
        }
    }
}

#[given(regex = r#"the following RVIG "relationship_data" data:"#)]
fn set_rvig_relationship_data(world: &mut RegelrechtWorld, step: &Step) {
    if let Some(table) = &step.table {
        parse_external_data_table(table, &mut world.external_data.rvig_relationship);
    }
}

#[given(regex = r#"the following RVZ "insurance" data:"#)]
fn set_rvz_insurance_data(world: &mut RegelrechtWorld, step: &Step) {
    if let Some(table) = &step.table {
        parse_external_data_table(table, &mut world.external_data.rvz_insurance);
    }
}

#[given(regex = r#"the following BELASTINGDIENST "box1" data:"#)]
fn set_bd_box1_data(world: &mut RegelrechtWorld, step: &Step) {
    if let Some(table) = &step.table {
        parse_external_data_table(table, &mut world.external_data.bd_box1);
    }
}

#[given(regex = r#"the following BELASTINGDIENST "box2" data:"#)]
fn set_bd_box2_data(world: &mut RegelrechtWorld, step: &Step) {
    if let Some(table) = &step.table {
        parse_external_data_table(table, &mut world.external_data.bd_box2);
    }
}

#[given(regex = r#"the following BELASTINGDIENST "box3" data:"#)]
fn set_bd_box3_data(world: &mut RegelrechtWorld, step: &Step) {
    if let Some(table) = &step.table {
        parse_external_data_table(table, &mut world.external_data.bd_box3);
    }
}

#[given(regex = r#"the following DJI "detenties" data:"#)]
fn set_dji_detenties_data(world: &mut RegelrechtWorld, step: &Step) {
    if let Some(table) = &step.table {
        parse_external_data_table(table, &mut world.external_data.dji_detenties);
    }
}

#[given(regex = r#"the following DUO "inschrijvingen" data:"#)]
fn set_duo_inschrijvingen_data(world: &mut RegelrechtWorld, step: &Step) {
    if let Some(table) = &step.table {
        parse_external_data_table(table, &mut world.external_data.duo_inschrijvingen);
    }
}

#[given(regex = r#"the following DUO "studiefinanciering" data:"#)]
fn set_duo_studiefinanciering_data(world: &mut RegelrechtWorld, step: &Step) {
    if let Some(table) = &step.table {
        parse_external_data_table(table, &mut world.external_data.duo_studiefinanciering);
    }
}

/// Parse an external data table with headers.
///
/// Table format:
/// ```text
/// | bsn       | field1 | field2 |
/// | 999993653 | value1 | value2 |
/// ```
fn parse_external_data_table(
    table: &cucumber::gherkin::Table,
    storage: &mut std::collections::HashMap<
        String,
        std::collections::HashMap<String, regelrecht_engine::Value>,
    >,
) {
    if table.rows.len() < 2 {
        return;
    }

    // First row is headers
    let headers: Vec<String> = table.rows[0].iter().map(|s| s.trim().to_string()).collect();

    // Remaining rows are data
    for row in table.rows.iter().skip(1) {
        let mut record = std::collections::HashMap::new();
        let mut bsn = String::new();

        for (i, cell) in row.iter().enumerate() {
            if i < headers.len() {
                let header = &headers[i];
                let value = convert_gherkin_value(cell);

                if header == "bsn" {
                    if let regelrecht_engine::Value::String(s) = &value {
                        bsn = s.clone();
                    } else if let regelrecht_engine::Value::Int(n) = &value {
                        bsn = n.to_string();
                    }
                }

                record.insert(header.clone(), value);
            }
        }

        if !bsn.is_empty() {
            storage.insert(bsn, record);
        }
    }
}
