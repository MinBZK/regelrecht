//! Het scenario: de cellen van een run, de vragen die gesteld worden en wat
//! die vragen moeten opleveren.
//!
//! Een scenario is data. De assertie hoort erbij: wat een run moet opleveren
//! staat in het scenariobestand, niet in Rust. Zo blijft een testgeval een
//! bestand dat iemand kan lezen en wijzigen zonder de crate te kennen.

use crate::cell::{Cell, CellConfig, Lexostatus, LexostatusOutcome};
use crate::error::{Result, SimulatorError};
use crate::values::equivalent;
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
    /// Verwacht dat de cel op dit moment niets vastgesteld had.
    ///
    /// Een volwaardige verwachting, en de enige manier om dat antwoord vast te
    /// leggen: "niets vastgesteld" is geen fout en geen leeg antwoord, dus een
    /// scenario moet erop kunnen asserteren. Sluit `expect` uit.
    #[serde(default)]
    pub expect_not_established: bool,
}

/// Eén verwachting die niet uitkwam.
#[derive(Debug, Clone)]
pub enum ExpectationFailure {
    /// Een uitkomst had een andere waarde dan verwacht, of ontbrak.
    Value {
        /// De uitkomst waarover de verwachting ging.
        output: String,
        /// Wat het scenario verwachtte.
        expected: Value,
        /// Wat de reductie opleverde; `None` als de uitkomst ontbrak.
        actual: Option<Value>,
    },
    /// Het scenario verwachtte waarden, maar de cel stelde niets vast.
    NotEstablished {
        /// Wat de cel als reden gaf.
        reason: String,
    },
    /// Het scenario verwachtte "niets vastgesteld", maar de cel gaf een feit.
    Established {
        /// De waarden die de cel toch opleverde.
        values: BTreeMap<String, Value>,
    },
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
            // "Niets vastgesteld" is een antwoord, dus het verslag zegt het ook
            // als het klopte: anders staat er `ok` bij een regel waarvan de
            // lezer niet kan zien wat de cel antwoordde.
            if let Some(reason) = outcome.lexostatus_value.not_established() {
                let _ = writeln!(out, "        niets vastgesteld: {reason}");
            }
            for failure in &outcome.failures {
                let _ = match failure {
                    ExpectationFailure::Value {
                        output,
                        expected,
                        actual,
                    } => {
                        let actual = match actual {
                            Some(value) => value.to_string(),
                            None => "(niet in het antwoord)".to_string(),
                        };
                        writeln!(out, "        {output}: verwacht {expected}, kreeg {actual}")
                    }
                    ExpectationFailure::NotEstablished { reason } => writeln!(
                        out,
                        "        verwachtte waarden, maar de cel stelde niets vast: {reason}"
                    ),
                    ExpectationFailure::Established { values } => writeln!(
                        out,
                        "        verwachtte 'niets vastgesteld', maar de cel gaf {values:?}"
                    ),
                };
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
    ///
    /// Een vraag die beide verwachtingen tegelijk stelt, kan nooit slagen; dat
    /// is een schrijffout en wordt hier ook geweigerd.
    fn validate(&self) -> Result<()> {
        for query in &self.queries {
            if query.expect.is_empty() && !query.expect_not_established {
                return Err(SimulatorError::QueryWithoutExpectation {
                    scenario: self.name.clone(),
                    cell: query.cell.clone(),
                    lexostatus: query.lexostatus.clone(),
                });
            }
            if !query.expect.is_empty() && query.expect_not_established {
                return Err(SimulatorError::ContradictoryExpectation {
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
            let failures = check_expectations(query, &answer.outcome);

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

/// Vergelijk de verwachtingen van een vraag met wat de reductie opleverde.
///
/// De twee soorten antwoord worden apart afgerekend: wie waarden verwacht en
/// "niets vastgesteld" krijgt, heeft geen ontbrekende uitkomst maar een ander
/// antwoord, en het verslag zegt dat ook zo.
fn check_expectations(query: &Query, outcome: &LexostatusOutcome) -> Vec<ExpectationFailure> {
    match (outcome, query.expect_not_established) {
        (LexostatusOutcome::NotEstablished { .. }, true) => Vec::new(),
        (LexostatusOutcome::NotEstablished { reason }, false) => {
            vec![ExpectationFailure::NotEstablished {
                reason: reason.clone(),
            }]
        }
        (LexostatusOutcome::Established(values), true) => {
            vec![ExpectationFailure::Established {
                values: values.clone(),
            }]
        }
        (LexostatusOutcome::Established(values), false) => query
            .expect
            .iter()
            .filter_map(|(output, expected)| {
                let found = values.get(output);
                match found {
                    Some(value) if equivalent(expected, value) => None,
                    _ => Some(ExpectationFailure::Value {
                        output: output.clone(),
                        expected: expected.clone(),
                        actual: found.cloned(),
                    }),
                }
            })
            .collect(),
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
    fn vraag_die_waarden_en_niets_vastgesteld_verwacht_wordt_geweigerd() {
        let yaml = r"
name: kan niet uitkomen
cells: []
queries:
  - cell: brp
    lexostatus: partnerschap
    op_moment: 2025-01-01
    expect_not_established: true
    expect:
      partnerschap_type: GEEN
";
        let err = Scenario::from_yaml(yaml)
            .expect_err("twee verwachtingen die elkaar uitsluiten horen te falen");
        assert!(
            matches!(err, SimulatorError::ContradictoryExpectation { .. }),
            "verwachtte ContradictoryExpectation, kreeg {err}"
        );
    }

    #[test]
    fn niets_vastgesteld_mag_de_enige_verwachting_zijn() {
        let yaml = r"
name: bewijst dat er niets was
cells: []
queries:
  - cell: brp
    lexostatus: partnerschap
    op_moment: 2025-01-01
    expect_not_established: true
";
        assert!(
            Scenario::from_yaml(yaml).is_ok(),
            "`expect_not_established` is een volwaardige verwachting"
        );
    }

    /// Een vraag met precies één verwachting, voor de assertietests hieronder.
    fn query(expect_not_established: bool, expect: BTreeMap<String, Value>) -> Query {
        Query {
            description: None,
            cell: "brp".to_string(),
            lexostatus: "partnerschap".to_string(),
            params: BTreeMap::new(),
            op_moment: NaiveDate::default(),
            expect,
            expect_not_established,
        }
    }

    #[test]
    fn ontbrekende_uitkomst_is_een_gemiste_verwachting() {
        let expect = BTreeMap::from([("hoogte".to_string(), Value::Int(1))]);
        let failures = check_expectations(
            &query(false, expect),
            &LexostatusOutcome::Established(BTreeMap::new()),
        );
        assert!(
            matches!(
                failures.as_slice(),
                [ExpectationFailure::Value { actual: None, .. }]
            ),
            "verwachtte één gemiste uitkomst, kreeg {failures:?}"
        );
    }

    #[test]
    fn niets_vastgesteld_is_geen_gemiste_uitkomst_maar_een_ander_antwoord() {
        let expect = BTreeMap::from([("partnerschap_type".to_string(), Value::Null)]);
        let outcome = LexostatusOutcome::NotEstablished {
            reason: "geen vastlegging".to_string(),
        };
        let failures = check_expectations(&query(false, expect), &outcome);
        assert!(
            matches!(
                failures.as_slice(),
                [ExpectationFailure::NotEstablished { .. }]
            ),
            "verwachtte NotEstablished, kreeg {failures:?}"
        );
    }

    #[test]
    fn een_feit_waar_niets_verwacht_werd_is_een_gemiste_verwachting() {
        let outcome = LexostatusOutcome::Established(BTreeMap::from([(
            "partnerschap_type".to_string(),
            Value::String("GEEN".to_string()),
        )]));
        let failures = check_expectations(&query(true, BTreeMap::new()), &outcome);
        assert!(
            matches!(
                failures.as_slice(),
                [ExpectationFailure::Established { .. }]
            ),
            "verwachtte Established, kreeg {failures:?}"
        );
    }
}
