//! Given steps: scenario setup.

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use crate::world::NappWorld;
use cucumber::gherkin::Step;
use cucumber::given;
use regelrecht_engine::Value;

/// Parse a Gherkin table cell into a typed engine value.
fn parse_value(s: &str) -> Value {
    match s {
        "true" => Value::Bool(true),
        "false" => Value::Bool(false),
        "null" => Value::Null,
        _ => {
            if let Ok(n) = s.parse::<i64>() {
                Value::Int(n)
            } else if let Ok(d) = s.parse::<rust_decimal::Decimal>() {
                // Decimal en niet f64: de engine rekent met exacte decimalen,
                // en een scenario dat "0.10" schrijft bedoelt tien cent.
                Value::Decimal(d)
            } else {
                Value::String(s.to_string())
            }
        }
    }
}

#[given(regex = r#"^the calculation date is "([^"]+)"$"#)]
fn set_calculation_date(world: &mut NappWorld, date: String) {
    world.calculation_date = date;
}

#[given(regex = r"^an application with the following data:$")]
fn set_application_data(world: &mut NappWorld, step: &Step) {
    let table = step.table.as_ref().expect("step requires a data table");
    for row in &table.rows {
        let key = row[0].trim().to_string();
        let value = parse_value(row[1].trim());
        world.parameters.insert(key, value);
    }
}

/// Feiten uit de orchestratie (databronnen) die als parameters de wet in
/// gaan — zelfde mechaniek als de aanvraagdata, andere lading.
#[given(regex = r"^the following facts:$")]
fn set_facts(world: &mut NappWorld, step: &Step) {
    let table = step.table.as_ref().expect("step requires a data table");
    for row in &table.rows {
        let key = row[0].trim().to_string();
        let value = parse_value(row[1].trim());
        world.parameters.insert(key, value);
    }
}
