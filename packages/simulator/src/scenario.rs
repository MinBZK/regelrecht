//! Het wereldbestand: de klok, de cellen, de startstand, de besluiten die
//! genomen worden, de vragen die gesteld worden en wat dat alles moet opleveren.
//!
//! Een wereld is data. De assertie hoort erbij: wat een run moet opleveren
//! staat in het bestand, niet in Rust. Zo blijft een testgeval een bestand dat
//! iemand kan lezen en wijzigen zonder de crate te kennen.

use crate::cell::{CellConfig, Decretogram, Lexostatus, LexostatusOutcome};
use crate::error::{Result, SimulatorError};
use crate::security::{Identity, SecurityContext, SignedAnswer};
use crate::transport::InProcessTransport;
use crate::values::equivalent;
use crate::world::{Clock, Fixture, World};
use chrono::NaiveDate;
use regelrecht_engine::Value;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

/// Een volledig wereldbestand.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scenario {
    /// Korte naam van het scenario.
    pub name: String,
    /// Waarom dit scenario bestaat; vrije tekst.
    #[serde(default)]
    pub description: Option<String>,
    /// De logische klok van deze wereld. Verplicht en expliciet: een run mag
    /// niet van de wandklok afhangen.
    pub clock: Clock,
    /// De cellen in deze run.
    pub cells: Vec<CellConfig>,
    /// De startstand: vastleggingen met een moment. Wat vóór het startmoment
    /// van de klok valt staat er bij het optuigen al; de rest landt zodra de
    /// klok die datum passeert.
    #[serde(default)]
    pub fixtures: Vec<Fixture>,
    /// De besluiten die in deze run genomen worden, elk op een moment.
    ///
    /// Dit is de aansturing van het besluit-pad zolang een cel nog geen actie
    /// van een actor kent: het scenario zegt wie wanneer waarover besluit. Ze
    /// gaan vóór de vragen, en dat is de volgorde die past bij wat ze zijn — een
    /// besluit is een gebeurtenis op de tijdlijn, een vraag kijkt erop terug. Wie
    /// een vraag over een moment *vóór* een besluit stelt, krijgt nog altijd het
    /// beeld van toen: de reductie filtert zelf op `op_moment`.
    #[serde(default)]
    pub decide: Vec<Decision>,
    /// De vragen die een consument stelt.
    #[serde(default)]
    pub queries: Vec<Query>,
    /// De vragen die een cel aan een andere cel stelt, over de celgrens.
    ///
    /// **Test-only stap, en dat is tijdelijk.** In de opstelling die we bouwen
    /// stelt een cel zo'n vraag uitsluitend vanuit haar besluit-pad: ze heeft een
    /// input nodig die een andere organisatie vaststelt. Dat pad bestaat inmiddels
    /// ([`Self::decide`]), maar het accepteren van een waarde van een andere cel
    /// nog niet: een besluit haalt zijn inputs uit de eigen kronieken en uit zijn
    /// parameters. Tot dat er is, is deze stap de enige manier om het verkeer te
    /// laten zien en erop te asserteren; daarna verhuist de aanroep naar het
    /// besluit-pad en is ze hoogstens nog een sonde.
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

/// Eén besluit dat een cel in deze run neemt.
///
/// De tegenhanger van een [`Query`]: geen vraag maar een gebeurtenis. Wat eruit
/// komt is een decretogram in de eigen kroniek van de cel, en dat is ook waar
/// het daarna vandaan gehaald wordt — met een gewone vraag over een lexostatus
/// die over die stroom reduceert.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Decision {
    /// Vrije omschrijving, verschijnt in het verslag.
    #[serde(default)]
    pub description: Option<String>,
    /// De cel die besluit.
    pub cell: String,
    /// De besluit-definitie die uitgevoerd wordt.
    pub besluit: String,
    /// De gedocumenteerde parameters van dat besluit.
    #[serde(default)]
    pub params: BTreeMap<String, Value>,
    /// Het moment waarop besloten wordt. Expliciet, nooit de wandklok: het
    /// bepaalt zowel welke feiten de cel kent als welke wetsversie geldt.
    pub op_moment: NaiveDate,
    /// De verwachte uitkomsten in het decretogram.
    ///
    /// Mag leeg blijven, anders dan bij een vraag: een besluit legt iets vast,
    /// dus het bewijst ook zonder verwachting iets — namelijk dat de vragen
    /// erna iets te vinden hebben.
    #[serde(default)]
    pub expect: BTreeMap<String, Value>,
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
    /// De omschrijving uit het scenario, als die er stond.
    pub description: Option<String>,
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

/// Het resultaat van één besluit.
#[derive(Debug, Clone)]
pub struct DecisionOutcome {
    /// De omschrijving uit het scenario, als die er stond.
    pub description: Option<String>,
    /// Het vastgelegde decretogram.
    pub decretogram: Decretogram,
    /// De verwachtingen die niet uitkwamen; leeg is goed.
    pub failures: Vec<ExpectationFailure>,
}

/// Het resultaat van een hele run.
#[derive(Debug, Clone)]
pub struct ScenarioRun {
    /// De naam van het scenario dat gedraaid heeft.
    pub name: String,
    /// Waar de logische klok na de run staat.
    pub clock: NaiveDate,
    /// Hoeveel vastleggingen niet afgegaan zijn omdat hun moment ná de klok
    /// ligt. De runner loopt de tijd af tot de laatste vraag, dus een fixture
    /// verder in de toekomst gebeurt in deze run niet.
    pub pending_triggers: usize,
    /// De besluiten die genomen zijn, in scenariovolgorde.
    pub decisions: Vec<DecisionOutcome>,
    /// De uitkomsten van de vragen van een consument, in scenariovolgorde.
    pub outcomes: Vec<QueryOutcome>,
    /// De uitkomsten van de vragen over een celgrens, in scenariovolgorde.
    pub transport_outcomes: Vec<TransportOutcome>,
}

impl ScenarioRun {
    /// Kwamen alle verwachtingen uit?
    pub fn passed(&self) -> bool {
        self.decisions.iter().all(|o| o.failures.is_empty())
            && self.outcomes.iter().all(|o| o.failures.is_empty())
            && self
                .transport_outcomes
                .iter()
                .all(|o| o.failures.is_empty())
    }

    /// Heeft deze run iets vastgelegd? Een run zonder vragen bewijst niets.
    ///
    /// Besluiten tellen hier met opzet niet mee. Een besluit legt iets vast, maar
    /// wat het waard is blijkt pas als iemand het terugvraagt: een scenario dat
    /// alleen besluit en niets vraagt, laat de hele tijdreductie onbeproefd.
    pub fn proved_something(&self) -> bool {
        !self.outcomes.is_empty() || !self.transport_outcomes.is_empty()
    }

    /// Leesbaar verslag van de run, geschikt voor een terminal of een testfout.
    pub fn report(&self) -> String {
        let mut out = format!("scenario '{}':\n", self.name);
        for decision in &self.decisions {
            let mark = if decision.failures.is_empty() {
                "ok"
            } else {
                "FOUT"
            };
            let gram = &decision.decretogram;
            // Het rechtskarakter en het bevoegd gezag staan erbij, want dat is
            // het verschil tussen een berekening en een besluit. Wat er níet
            // bij staat is het tijdstempel van het receipt: dat is wandkloktijd,
            // en een verslag dat per run verschilt is geen verslag.
            let _ = writeln!(
                out,
                "  [{mark}] {} besluit '{}' op {} -> zaakkenmerk '{}' ({}{}{})",
                gram.cell,
                gram.besluit,
                gram.op_moment,
                gram.zaakkenmerk,
                gram.regulation,
                gram.regulation_valid_from
                    .as_ref()
                    .map(|valid_from| format!(", versie {valid_from}"))
                    .unwrap_or_default(),
                describe_authority(gram),
            );
            if let Some(description) = &decision.description {
                let _ = writeln!(out, "        {description}");
            }
            for (name, value) in &gram.outputs {
                let _ = writeln!(out, "        {name} = {value}");
            }
            for (name, input) in &gram.inputs {
                let _ = writeln!(
                    out,
                    "        input {name} = {} ({})",
                    input.value,
                    input.origin.describe()
                );
            }
            write_failures(&mut out, &decision.failures);
        }

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
            // De omschrijving erbij, want twee vragen kunnen dezelfde cel, naam
            // en moment hebben — in de kernassertie over tijd is dat juist het
            // punt — en dan is de regel hierboven twee keer dezelfde.
            if let Some(description) = &outcome.description {
                let _ = writeln!(out, "        {description}");
            }
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

        let _ = writeln!(out, "  klok staat op {}", self.clock);
        // Een fixture waar de klok nooit aan toe komt, is een regel in het
        // bestand die niets doet. Dat mag, maar het hoort niet stil te zijn:
        // wie hem als startstand bedoelde, ziet hier dat hij niet meedeed.
        if self.pending_triggers > 0 {
            let _ = writeln!(
                out,
                "  {} vastlegging(en) gingen niet af: hun moment ligt ná de klok",
                self.pending_triggers
            );
        }
        out
    }
}

/// Het rechtskarakter en het bevoegd gezag van een decretogram, voor het verslag.
///
/// Leeg als de regeling er niets over zegt, en dan staat het er ook niet:
/// een besluit waarvan de wet het karakter niet noemt, hoort niet met een lege
/// haak te suggereren dat het er wel een heeft.
fn describe_authority(gram: &Decretogram) -> String {
    match (&gram.legal_character, &gram.competent_authority) {
        (Some(character), Some(authority)) => format!(", {character} door {authority}"),
        (Some(character), None) => format!(", {character}"),
        (None, Some(authority)) => format!(", door {authority}"),
        (None, None) => String::new(),
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

    /// Tuig de wereld op: de cellen, de klok op haar startmoment en de
    /// startstand die op dat moment al gebeurd was.
    pub fn world(&self, regulation_root: &Path) -> Result<World> {
        World::new(&self.cells, self.clock, &self.fixtures, regulation_root)
    }

    /// Tuig de wereld op, laat de tijd lopen en stel alle vragen.
    ///
    /// De vragen lopen de tijdlijn af in de volgorde van het bestand: staat de
    /// klok nog vóór het moment van een vraag, dan gaat de wereld eerst vooruit
    /// en gaan onderweg de triggers af. Een vraag over een eerder moment kan
    /// altijd; die levert het beeld van toen. Vooruitkijken kan niet.
    ///
    /// De runner combineert niets: hij geeft elk antwoord terug zoals de cel het
    /// gaf. Combineren over cellen heen is synthese en hoort bij een consument.
    ///
    /// Vragen van een consument gaan rechtstreeks naar de publieke ingang van de
    /// cel. Vragen uit `query_via_transport` gaan langs de veiligheidscontext van
    /// de vragende cel naar het transport — de enige weg over een celgrens.
    pub fn run(&self, regulation_root: &Path) -> Result<ScenarioRun> {
        let mut world = self.world(regulation_root)?;

        let decisions = self.take_decisions(&mut world)?;

        let mut outcomes = Vec::with_capacity(self.queries.len());
        for query in &self.queries {
            if query.op_moment > world.now() {
                world.advance(query.op_moment)?;
            }

            let answer = world.reduce(
                &query.cell,
                &query.lexostatus,
                &query.params,
                query.op_moment,
            )?;
            let failures = check_expectations(query, &answer.outcome);

            outcomes.push(QueryOutcome {
                description: query.description.clone(),
                cell: query.cell.clone(),
                lexostatus: query.lexostatus.clone(),
                lexostatus_value: answer,
                failures,
            });
        }

        let transport_outcomes = self.run_via_transport(&mut world)?;

        Ok(ScenarioRun {
            name: self.name.clone(),
            clock: world.now(),
            pending_triggers: world.pending_triggers(),
            decisions,
            outcomes,
            transport_outcomes,
        })
    }

    /// Laat de cellen hun besluiten nemen, elk op zijn eigen moment.
    ///
    /// De klok gaat eerst vooruit tot dat moment, zodat een levering die
    /// ertussen valt eerst landt: een besluit rekent op de feiten die de cel op
    /// dat moment heeft, en op de wetsversie die dan geldt.
    fn take_decisions(&self, world: &mut World) -> Result<Vec<DecisionOutcome>> {
        let mut decisions = Vec::with_capacity(self.decide.len());
        for decision in &self.decide {
            if decision.op_moment > world.now() {
                world.advance(decision.op_moment)?;
            }

            let decretogram = world.decide(
                &decision.cell,
                &decision.besluit,
                &decision.params,
                decision.op_moment,
            )?;
            let failures = check_values(&decision.expect, &decretogram.outputs);

            decisions.push(DecisionOutcome {
                description: decision.description.clone(),
                decretogram,
                failures,
            });
        }
        Ok(decisions)
    }

    /// Stel de cross-cel-vragen, elk vanuit de veiligheidscontext van de vrager.
    ///
    /// Net als bij een vraag van een consument gaat de klok eerst vooruit tot het
    /// gevraagde moment, zodat een fixture die ertussen valt onderweg vastlegt.
    /// Het transport zelf wordt per vraag opnieuw opgebouwd over `world.cells()`
    /// — dat is de naad waarlangs later een HTTP-transport aanschuift zonder dat
    /// hier of in een cel iets verandert — en dat kan niet één keer vooraf: de
    /// lening zou anders `world.advance` in de weg staan.
    fn run_via_transport(&self, world: &mut World) -> Result<Vec<TransportOutcome>> {
        let mut outcomes = Vec::with_capacity(self.query_via_transport.len());
        for via in &self.query_via_transport {
            // De vrager moet een cel in deze wereld zijn. Zonder deze controle zou
            // een typfout in `from` een identiteit opleveren die nergens bij hoort,
            // en dan zegt het vraaggraf iets over een cel die niet bestaat.
            if !world.cells().contains_key(&via.from) {
                return Err(SimulatorError::UnknownCell {
                    cell: via.from.clone(),
                });
            }

            let query = &via.query;
            if query.op_moment > world.now() {
                world.advance(query.op_moment)?;
            }

            let transport = InProcessTransport::over(world.cells());
            let context = SecurityContext::new(Identity::for_cell(&via.from), &transport);
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
        (LexostatusOutcome::Established(values), false) => check_values(&query.expect, values),
    }
}

/// Vergelijk verwachte waarden met wat er werkelijk uitkwam.
///
/// Eén plek, want een besluit wordt op zijn uitkomsten afgerekend zoals een
/// vraag op haar antwoord: uitkomsten waarover niets verwacht wordt, worden niet
/// gecontroleerd, en een uitkomst die ontbreekt is een gemiste verwachting en
/// geen stilte.
fn check_values(
    expect: &BTreeMap<String, Value>,
    values: &BTreeMap<String, Value>,
) -> Vec<ExpectationFailure> {
    expect
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
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn onbekend_veld_in_scenario_wordt_geweigerd() {
        let yaml = r"
name: typfout
clock: { start: 2025-01-01 }
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
clock: { start: 2025-01-01 }
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
clock: { start: 2025-01-01 }
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
    fn een_wereld_zonder_klok_wordt_geweigerd() {
        let yaml = r"
name: zonder klok
cells: []
";
        assert!(
            Scenario::from_yaml(yaml).is_err(),
            "zonder startmoment zou de wereld op de wandklok moeten terugvallen, \
             en dan is een run morgen een andere run"
        );
    }

    #[test]
    fn niets_vastgesteld_mag_de_enige_verwachting_zijn() {
        let yaml = r"
name: bewijst dat er niets was
clock: { start: 2025-01-01 }
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
clock: { start: 2025-01-01 }
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

    /// Twee vragen met dezelfde cel, naam en moment horen in het verslag uit
    /// elkaar te vallen. In de kernassertie over tijd staat dezelfde vraag twee
    /// keer, vóór en ná een vastlegging; zonder de omschrijving zijn dat twee
    /// identieke regels en zegt het verslag niet welke welke is.
    fn moment() -> NaiveDate {
        NaiveDate::from_ymd_opt(2024, 6, 1)
            .unwrap_or_else(|| panic!("2024-06-01 moet een geldige datum zijn"))
    }

    /// Een vastlegging waar de klok niet aan toe kwam, hoort in het verslag te
    /// staan. Zij is geldig bevonden bij het optuigen en doet daarna niets; dat
    /// stil laten betekent dat een regel in het wereldbestand niets bewijst
    /// zonder dat iemand het merkt.
    #[test]
    fn het_verslag_meldt_vastleggingen_die_niet_afgingen() {
        let run = |pending_triggers| ScenarioRun {
            name: "tijd".to_string(),
            clock: moment(),
            pending_triggers,
            decisions: Vec::new(),
            outcomes: Vec::new(),
            transport_outcomes: Vec::new(),
        };

        assert!(
            run(2).report().contains("2 vastlegging(en) gingen niet af"),
            "het verslag hoort te melden dat er nog iets wachtte; kreeg:\n{}",
            run(2).report()
        );
        assert!(
            !run(0).report().contains("gingen niet af"),
            "zonder wachtende vastleggingen hoort die regel weg te blijven; kreeg:\n{}",
            run(0).report()
        );
    }

    #[test]
    fn het_verslag_onderscheidt_twee_gelijke_vragen_aan_hun_omschrijving() {
        let moment = moment();
        let outcome = |description: &str| QueryOutcome {
            description: Some(description.to_string()),
            cell: "toeslagen".to_string(),
            lexostatus: "toeslagpartnerschap".to_string(),
            lexostatus_value: Lexostatus {
                cell: "toeslagen".to_string(),
                name: "toeslagpartnerschap".to_string(),
                op_moment: moment,
                outcome: LexostatusOutcome::Established(BTreeMap::new()),
            },
            failures: Vec::new(),
        };
        let run = ScenarioRun {
            name: "tijd".to_string(),
            clock: moment,
            pending_triggers: 0,
            decisions: Vec::new(),
            outcomes: vec![outcome("vóór de vastlegging"), outcome("erna, ongewijzigd")],
            transport_outcomes: Vec::new(),
        };

        let report = run.report();
        assert!(
            report.contains("vóór de vastlegging") && report.contains("erna, ongewijzigd"),
            "het verslag hoort elke omschrijving te noemen; kreeg:\n{report}"
        );
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
