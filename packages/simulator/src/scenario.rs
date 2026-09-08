//! Het scenario: de cellen van een run, de vragen die gesteld worden en wat
//! die vragen moeten opleveren.
//!
//! Een scenario is data. De assertie hoort erbij: wat een run moet opleveren
//! staat in het scenariobestand, niet in Rust. Zo blijft een testgeval een
//! bestand dat iemand kan lezen en wijzigen zonder de crate te kennen.

use crate::cell::{Cell, CellConfig, Lexostatus};
use crate::error::{Result, SimulatorError};
use chrono::NaiveDate;
use regelrecht_engine::Value;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

/// Een volledig scenario.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scenario {
    /// Korte naam van het scenario.
    pub name: String,
    /// Waarom dit scenario bestaat; vrije tekst.
    #[serde(default)]
    pub description: Option<String>,
    /// De cellen in deze run.
    pub cells: Vec<CellConfig>,
    /// De vragen die een consument stelt.
    #[serde(default)]
    pub queries: Vec<Query>,
}

/// Eén vraag van een consument aan één cel.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Query {
    /// Vrije omschrijving, verschijnt in het verslag.
    #[serde(default)]
    pub description: Option<String>,
    /// De cel waaraan gevraagd wordt.
    pub cell: String,
    /// De gepubliceerde lexostatus.
    pub lexostatus: String,
    /// De gedocumenteerde parameters.
    #[serde(default)]
    pub params: BTreeMap<String, Value>,
    /// Het moment waarop gevraagd wordt. Expliciet, nooit de wandklok.
    pub op_moment: NaiveDate,
    /// De verwachte waarden in het antwoord. Uitkomsten die hier niet staan,
    /// worden niet gecontroleerd — maar minstens één verwachting is verplicht,
    /// anders bewijst de vraag niets.
    #[serde(default)]
    pub expect: BTreeMap<String, Value>,
}

/// Eén verwachting die niet uitkwam.
#[derive(Debug, Clone)]
pub struct ExpectationFailure {
    /// De uitkomst waarover de verwachting ging.
    pub output: String,
    /// Wat het scenario verwachtte.
    pub expected: Value,
    /// Wat de reductie opleverde; `None` als de uitkomst ontbrak.
    pub actual: Option<Value>,
}

/// Het resultaat van één vraag.
#[derive(Debug, Clone)]
pub struct QueryOutcome {
    /// De cel waaraan gevraagd is.
    pub cell: String,
    /// De gevraagde lexostatus.
    pub lexostatus: String,
    /// Het antwoord van de cel.
    pub lexostatus_value: Lexostatus,
    /// De verwachtingen die niet uitkwamen; leeg is goed.
    pub failures: Vec<ExpectationFailure>,
}

/// Het resultaat van een hele run.
#[derive(Debug, Clone)]
pub struct ScenarioRun {
    /// De naam van het scenario dat gedraaid heeft.
    pub name: String,
    /// De uitkomsten, in de volgorde van het scenario.
    pub outcomes: Vec<QueryOutcome>,
}

impl ScenarioRun {
    /// Kwamen alle verwachtingen uit?
    pub fn passed(&self) -> bool {
        self.outcomes.iter().all(|o| o.failures.is_empty())
    }

    /// Leesbaar verslag van de run, geschikt voor een terminal of een testfout.
    pub fn report(&self) -> String {
        let mut out = format!("scenario '{}':\n", self.name);
        for outcome in &self.outcomes {
            let mark = if outcome.failures.is_empty() {
                "ok"
            } else {
                "FOUT"
            };
            let _ = writeln!(
                out,
                "  [{mark}] {}.{} op {}",
                outcome.cell, outcome.lexostatus, outcome.lexostatus_value.op_moment
            );
            for failure in &outcome.failures {
                let actual = match &failure.actual {
                    Some(value) => value.to_string(),
                    None => "(niet in het antwoord)".to_string(),
                };
                let _ = writeln!(
                    out,
                    "        {}: verwacht {}, kreeg {actual}",
                    failure.output, failure.expected
                );
            }
        }
        out
    }
}

impl Scenario {
    /// Lees een scenario uit YAML.
    ///
    /// Weigert meteen een scenario dat niets vastlegt: zie [`Scenario::validate`].
    pub fn from_yaml(yaml: &str) -> Result<Self> {
        let scenario: Self = serde_yaml_ng::from_str(yaml)?;
        scenario.validate()?;
        Ok(scenario)
    }

    /// Controleer dat elke vraag ook iets vastlegt.
    ///
    /// De assertie hoort bij het scenario, en dat werkt alleen als een vraag
    /// zonder assertie geen groen kan opleveren. Zonder deze controle draait een
    /// scenariobestand dat niets verwacht mee in de suite en meldt het `ok` —
    /// het duurste soort groen, want het lijkt op bewijs.
    fn validate(&self) -> Result<()> {
        for query in &self.queries {
            if query.expect.is_empty() {
                return Err(SimulatorError::QueryWithoutExpectation {
                    scenario: self.name.clone(),
                    cell: query.cell.clone(),
                    lexostatus: query.lexostatus.clone(),
                });
            }
        }
        Ok(())
    }

    /// Lees een scenario van schijf.
    pub fn load(path: &Path) -> Result<Self> {
        let text =
            std::fs::read_to_string(path).map_err(|source| SimulatorError::ScenarioRead {
                path: path.to_path_buf(),
                source,
            })?;
        Self::from_yaml(&text)
    }

    /// Tuig de cellen op en stel alle vragen.
    ///
    /// De runner combineert niets: hij geeft elk antwoord terug zoals de cel het
    /// gaf. Combineren over cellen heen is synthese en hoort bij een consument.
    pub fn run(&self, regulation_root: &Path) -> Result<ScenarioRun> {
        let mut cells: BTreeMap<String, Cell> = BTreeMap::new();
        for config in &self.cells {
            if cells.contains_key(&config.id) {
                return Err(SimulatorError::DuplicateCell {
                    cell: config.id.clone(),
                });
            }
            cells.insert(
                config.id.clone(),
                Cell::from_config(config, regulation_root)?,
            );
        }

        let mut outcomes = Vec::with_capacity(self.queries.len());
        for query in &self.queries {
            let cell = cells
                .get(&query.cell)
                .ok_or_else(|| SimulatorError::UnknownCell {
                    cell: query.cell.clone(),
                })?;

            let answer = cell.reduce(&query.lexostatus, &query.params, query.op_moment)?;
            let failures = check_expectations(&query.expect, &answer.values);

            outcomes.push(QueryOutcome {
                cell: query.cell.clone(),
                lexostatus: query.lexostatus.clone(),
                lexostatus_value: answer,
                failures,
            });
        }

        Ok(ScenarioRun {
            name: self.name.clone(),
            outcomes,
        })
    }
}

/// Vergelijk de verwachtingen met wat de reductie opleverde.
fn check_expectations(
    expect: &BTreeMap<String, Value>,
    actual: &BTreeMap<String, Value>,
) -> Vec<ExpectationFailure> {
    expect
        .iter()
        .filter_map(|(output, expected)| {
            let found = actual.get(output);
            match found {
                Some(value) if equivalent(expected, value) => None,
                _ => Some(ExpectationFailure {
                    output: output.clone(),
                    expected: expected.clone(),
                    actual: found.cloned(),
                }),
            }
        })
        .collect()
}

/// Gelijkheid met één versoepeling: getallen vergelijken op waarde, niet op
/// variant. YAML kent geen verschil tussen `1` en `1.0`, de engine wel.
fn equivalent(expected: &Value, actual: &Value) -> bool {
    match (expected.as_decimal(), actual.as_decimal()) {
        (Some(left), Some(right)) => left == right,
        _ => expected == actual,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn onbekend_veld_in_scenario_wordt_geweigerd() {
        let yaml = r"
name: typfout
cells: []
queeries: []
";
        assert!(
            Scenario::from_yaml(yaml).is_err(),
            "een onbekend veld hoort te falen, anders verdwijnt een typfout stil"
        );
    }

    #[test]
    fn vraag_zonder_verwachting_wordt_geweigerd() {
        let yaml = r"
name: bewijst niets
cells: []
queries:
  - cell: toeslagen
    lexostatus: toeslagpartnerschap
    op_moment: 2025-01-01
";
        let err = Scenario::from_yaml(yaml).expect_err("een vraag zonder `expect` hoort te falen");
        assert!(
            matches!(err, SimulatorError::QueryWithoutExpectation { .. }),
            "verwachtte QueryWithoutExpectation, kreeg {err}"
        );
    }

    #[test]
    fn getallen_vergelijken_op_waarde() {
        assert!(equivalent(&Value::Int(1), &Value::Int(1)));
        assert!(!equivalent(&Value::Int(1), &Value::Int(2)));
        assert!(!equivalent(&Value::Int(1), &Value::String("1".to_string())));
    }

    #[test]
    fn ontbrekende_uitkomst_is_een_gemiste_verwachting() {
        let expect = BTreeMap::from([("hoogte".to_string(), Value::Int(1))]);
        let failures = check_expectations(&expect, &BTreeMap::new());
        assert_eq!(failures.len(), 1);
        assert!(failures[0].actual.is_none());
    }
}
