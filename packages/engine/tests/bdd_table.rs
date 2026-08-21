//! Unit tests for the BDD data-table parsing shared with the editor's JS
//! mirror (`frontend/src/gherkin/actions.js`). The `bdd` target itself runs
//! `harness = false` with `test = false`, so these live in their own target.

#![allow(dead_code, clippy::panic, clippy::expect_used, clippy::unwrap_used)]

#[path = "bdd/helpers/value_conversion.rs"]
mod value_conversion;

#[path = "bdd/helpers/table.rs"]
mod table;

use regelrecht_engine::Value;
use table::{rows_to_records, Rows};
use value_conversion::convert_gherkin_value;

fn rows(raw: &[&[&str]]) -> Rows {
    raw.iter()
        .map(|row| row.iter().map(|c| (*c).to_string()).collect())
        .collect()
}

#[test]
fn records_use_the_header_row() {
    let records = rows_to_records(
        &rows(&[
            &["naam", "leeftijd", "verzekerd"],
            &["Jansen", "30", "true"],
        ]),
        convert_gherkin_value,
    );

    assert_eq!(records.len(), 1);
    assert_eq!(
        records[0].get("naam"),
        Some(&Value::String("Jansen".to_string()))
    );
    assert_eq!(records[0].get("leeftijd"), Some(&Value::Int(30)));
    assert_eq!(records[0].get("verzekerd"), Some(&Value::Bool(true)));
}

#[test]
#[should_panic(expected = "data table row 1 has 2 cells, header row has 3")]
fn a_short_row_is_rejected_instead_of_dropping_a_column() {
    rows_to_records(
        &rows(&[&["naam", "leeftijd", "verzekerd"], &["Jansen", "30"]]),
        convert_gherkin_value,
    );
}

#[test]
#[should_panic(expected = "data table row 2 has 4 cells, header row has 3")]
fn a_long_row_is_rejected_too() {
    rows_to_records(
        &rows(&[
            &["naam", "leeftijd", "verzekerd"],
            &["Jansen", "30", "true"],
            &["De Vries", "40", "false", "extra"],
        ]),
        convert_gherkin_value,
    );
}
