//! Het scenario: de cellen van een run, de vragen die gesteld worden en wat
//! die vragen moeten opleveren.
//!
//! Een scenario is data. De assertie hoort erbij: wat een run moet opleveren
//! staat in het scenariobestand, niet in Rust. Zo blijft een testgeval een
//! bestand dat iemand kan lezen en wijzigen zonder de crate te kennen.

use crate::cell::{Cell, CellConfig, Lexostatus, LexostatusOutcome};
use crate::error::{Result, SimulatorError};
use crate::security::{Identity, SecurityContext, SignedAnswer};
use crate::transport::{CellTransport, InProcessTransport};
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
    /// De vragen die een cel aan een andere cel stelt, over de celgrens.
    ///
    /// **Test-only stap, en dat is tijdelijk.** In de opstelling die we bouwen
    /// stelt een cel zo'n vraag uitsluitend vanuit haar besluit-pad: ze heeft een
    /// input nodig die een andere organisatie vaststelt. Dat pad bestaat nog niet
    /// — een cel kan nog niets vastleggen en dus niets accepteren — en tot die tijd
    /// is dit de enige manier om het verkeer te laten zien en erop te asserteren.
    /// Zodra het besluit-pad er is, verhuist de aanroep daarheen en is deze stap
    /// hoogstens nog een sonde.
    #[serde(default)]
    pub query_via_transport: Vec<TransportQuery>,
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

/// Eén vraag van een cel aan een andere cel, over de celgrens.
///
/// Dezelfde vraag als een [`Query`] — een gepubliceerde naam, gedocumenteerde
/// parameters, een moment, en wat ze moet opleveren — met één veld erbij: wie
/// vraagt. Dat veld is het hele verschil tussen een consument die een cel
/// bevraagt en een cel die een andere cel bevraagt.
///
/// `deny_unknown_fields` staat hier nog een keer, en dat is geen dubbelop: een
/// geflatten veld wordt met een losse veldenlijst gevoed, dus de strengheid van
/// [`Query`] reikt niet tot deze vorm. Zonder deze regel zou `expct:` in een
/// vraag over de celgrens stil worden weggegooid en de verwachting met zich
/// meenemen — de vraag zou dan `ok` melden terwijl ze niets meer controleert.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TransportQuery {
    /// De vragende cel. Haar veiligheidscontext zet de vraag over de grens.
    pub from: String,
    /// De vraag zelf, in de vorm die een consument ook gebruikt.
    #[serde(flatten)]
    pub query: Query,
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

/// Het resultaat van één vraag over een celgrens.
#[derive(Debug, Clone)]
pub struct TransportOutcome {
    /// Het bewijsstuk van de veiligheidscontext: wie vroeg, ondertekend, met
    /// welke parameters, en wat de peer antwoordde.
    ///
    /// Dit is wat het observatielog vastlegt, en straks wat een decretogram als
    /// geaccepteerde waarde draagt.
    pub signed: SignedAnswer,
    /// De verwachtingen die niet uitkwamen; leeg is goed.
    pub failures: Vec<ExpectationFailure>,
}

/// Het resultaat van een hele run.
#[derive(Debug, Clone)]
pub struct ScenarioRun {
    /// De naam van het scenario dat gedraaid heeft.
    pub name: String,
    /// De uitkomsten van de vragen van een consument, in scenariovolgorde.
    pub outcomes: Vec<QueryOutcome>,
    /// De uitkomsten van de vragen over een celgrens, in scenariovolgorde.
    pub transport_outcomes: Vec<TransportOutcome>,
}

impl ScenarioRun {
    /// Kwamen alle verwachtingen uit?
    pub fn passed(&self) -> bool {
        self.outcomes.iter().all(|o| o.failures.is_empty())
            && self
                .transport_outcomes
                .iter()
                .all(|o| o.failures.is_empty())
    }

    /// Heeft deze run iets vastgelegd? Een run zonder vragen bewijst niets.
    pub fn proved_something(&self) -> bool {
        !self.outcomes.is_empty() || !self.transport_outcomes.is_empty()
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
            write_failures(&mut out, &outcome.failures);
        }

        for outcome in &self.transport_outcomes {
            let mark = if outcome.failures.is_empty() {
                "ok"
            } else {
                "FOUT"
            };
            let answer = &outcome.signed.answer;
            // De vrager staat erbij, en dat hij ondertekend heeft ook: een
            // cross-cel-vraag is iets anders dan een consument die vraagt, en het
            // verslag hoort dat verschil te laten zien.
            let _ = writeln!(
                out,
                "  [{mark}] {} -> {}.{} op {} (over de celgrens, {})",
                outcome.signed.asked_by,
                answer.cell,
                answer.name,
                answer.op_moment,
                outcome.signed.signature
            );
            if let Some(reason) = answer.not_established() {
                let _ = writeln!(out, "        niets vastgesteld: {reason}");
            }
            write_failures(&mut out, &outcome.failures);
        }

        out
    }
}

/// Schrijf de gemiste verwachtingen van één vraag in het verslag.
///
/// Eén plek, want een vraag over de celgrens wordt op precies dezelfde manier
/// afgerekend als een vraag van een consument.
fn write_failures(out: &mut String, failures: &[ExpectationFailure]) {
    for failure in failures {
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
        // Dezelfde eis aan beide soorten vraag: wie over de celgrens vraagt zonder
        // te zeggen wat eruit moet komen, bewijst even weinig.
        for query in self.all_queries() {
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

    /// Elke vraag in het scenario, van welke soort ook.
    fn all_queries(&self) -> impl Iterator<Item = &Query> {
        self.queries
            .iter()
            .chain(self.query_via_transport.iter().map(|via| &via.query))
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
    ///
    /// Vragen van een consument gaan rechtstreeks naar de publieke ingang van de
    /// cel. Vragen uit `query_via_transport` gaan langs de veiligheidscontext van
    /// de vragende cel naar het transport — de enige weg over een celgrens.
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

        let transport = InProcessTransport::over(&cells);
        let transport_outcomes = self.run_via_transport(&cells, &transport)?;

        Ok(ScenarioRun {
            name: self.name.clone(),
            outcomes,
            transport_outcomes,
        })
    }

    /// Stel de cross-cel-vragen, elk vanuit de veiligheidscontext van de vrager.
    ///
    /// Het transport is een parameter en geen keuze van deze functie: dat is de
    /// naad waarlangs later een HTTP-transport aanschuift zonder dat hier of in
    /// een cel iets verandert.
    fn run_via_transport(
        &self,
        cells: &BTreeMap<String, Cell>,
        transport: &dyn CellTransport,
    ) -> Result<Vec<TransportOutcome>> {
        let mut outcomes = Vec::with_capacity(self.query_via_transport.len());
        for via in &self.query_via_transport {
            // De vrager moet een cel in deze wereld zijn. Zonder deze controle zou
            // een typfout in `from` een identiteit opleveren die nergens bij hoort,
            // en dan zegt het vraaggraf iets over een cel die niet bestaat.
            if !cells.contains_key(&via.from) {
                return Err(SimulatorError::UnknownCell {
                    cell: via.from.clone(),
                });
            }

            let context = SecurityContext::new(Identity::for_cell(&via.from), transport);
            let query = &via.query;
            let signed = context.query(
                &query.cell,
                &query.lexostatus,
                &query.params,
                query.op_moment,
            )?;
            let failures = check_expectations(query, &signed.answer.outcome);

            outcomes.push(TransportOutcome { signed, failures });
        }
        Ok(outcomes)
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

    #[test]
    fn vraag_over_de_celgrens_zonder_verwachting_wordt_geweigerd() {
        let yaml = r"
name: bewijst niets over de grens
cells: []
query_via_transport:
  - from: toeslagen
    cell: brp
    lexostatus: partnerschap
    op_moment: 2025-01-01
";
        let err = Scenario::from_yaml(yaml)
            .expect_err("ook een vraag over de celgrens moet iets vastleggen");
        assert!(
            matches!(err, SimulatorError::QueryWithoutExpectation { .. }),
            "verwachtte QueryWithoutExpectation, kreeg {err}"
        );
    }

    #[test]
    fn onbekend_veld_in_een_vraag_over_de_celgrens_wordt_geweigerd() {
        // `from` staat naast de gewone velden van een vraag, en die combinatie is
        // precies waar strengheid stil kan wegvallen: bij een geflatten veld
        // komt de weigering niet van `Query` maar van `TransportQuery` zelf.
        //
        // Daarom staat er een geldige `expect` in dit scenario. Zonder die regel
        // slaagt deze test ook als de typfout ongemerkt doorglipt — dan struikelt
        // het scenario op "vraag zonder verwachting" en lijkt de poort te werken
        // terwijl hij niets meer doet. Met de `expect` erbij is het onbekende
        // veld de enige reden waarom dit nog kan falen, en dat rekenen we ook af
        // op de melding.
        let yaml = r"
name: typfout over de grens
cells: []
query_via_transport:
  - from: toeslagen
    cell: brp
    lexostatus: partnerschap
    op_moment: 2025-01-01
    expect:
      partnerschap_type: HUWELIJK
    expct:
      partnerschap_type: HUWELIJK
";
        let err = Scenario::from_yaml(yaml)
            .expect_err("een onbekend veld hoort te falen, anders verdwijnt een typfout stil");
        assert!(
            matches!(&err, SimulatorError::ScenarioParse(_)),
            "verwachtte een leesfout op het onbekende veld, kreeg {err}"
        );
        assert!(
            err.to_string().contains("expct"),
            "de melding hoort het onbekende veld te noemen, kreeg {err}"
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
