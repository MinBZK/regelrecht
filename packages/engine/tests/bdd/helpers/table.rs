//! Gherkin data-table shapes shared by the BDD step dispatch.
//!
//! Lives in its own module so a plain `cargo test` target can exercise it: the
//! `bdd` test target runs `harness = false` with `test = false`, so a
//! `#[cfg(test)]` block next to the dispatch code would never run.

use std::collections::BTreeMap;

use regelrecht_engine::Value;

/// How a single cell becomes a `Value`. Passed in rather than called directly:
/// the rule lives in `bdd/grammar.yaml` (`value_typing.table_cell`) and reaches
/// the dispatch through codegen, which this module cannot see — it is also
/// compiled standalone by `tests/bdd_table.rs`.
pub type CellFn = fn(&str) -> Value;

/// Table rows straight from cucumber's `gherkin::Step.table.rows`. Whether
/// `row[0]` is a header depends on the step: a data-source table has one, a
/// parameter table does not.
pub type Rows = Vec<Vec<String>>;

/// Parse a two-column key/value parameter table.
///
/// An empty value cell means the parameter is not passed at all, the same
/// rule as for a data-table cell (RFC-036): the engine then treats an optional
/// parameter as unknown for lack of it and a required one as the caller's
/// omission. The word `null` passes an absence. So the empty cell means one
/// thing in every table, and the JS runner (`set_parameters_table`) agrees.
pub fn rows_to_params(rows: &Rows, cell: CellFn) -> BTreeMap<String, Value> {
    let mut params = BTreeMap::new();
    for row in rows {
        if row.len() >= 2 && !row[1].trim().is_empty() {
            params.insert(row[0].trim().to_string(), cell(&row[1]));
        }
    }
    params
}

/// Parse a header-row table into a list of records (one per data row).
///
/// A row whose width differs from the header row is rejected instead of
/// silently truncated. Today no such row reaches here, because the Gherkin
/// parser refuses a table with a varying cell count. That guarantee sits in a
/// dependency we bump, though, and the editor's `tableToRecords` leans on a
/// different parser with a different fallback. The explicit check makes both
/// sides fail identically and loudly if either parser ever loosens.
///
/// An empty cell means the record has no value for that column, so the key is
/// left out of the record: the engine then resolves the input as *unknown*
/// (nobody has the fact). The literal `null` in a cell is kept and becomes
/// `Value::Null`: the register says there is none (RFC-036). The two are
/// different statements, and a data table has to be able to make both.
pub fn rows_to_records(rows: &Rows, cell: CellFn) -> Vec<BTreeMap<String, Value>> {
    if rows.len() < 2 {
        return Vec::new();
    }
    let headers: Vec<String> = rows[0].iter().map(|s| s.trim().to_string()).collect();
    let mut records = Vec::new();
    for (row_index, row) in rows.iter().enumerate().skip(1) {
        assert!(
            row.len() == headers.len(),
            "data table row {row_index} has {} cells, header row has {}",
            row.len(),
            headers.len()
        );
        records.push(
            headers
                .iter()
                .zip(row)
                .filter(|(_, raw)| !raw.trim().is_empty())
                .map(|(header, raw)| (header.clone(), cell(raw)))
                .collect(),
        );
    }
    records
}
