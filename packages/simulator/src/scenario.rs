//! Het wereldbestand: de klok, de cellen, de startstand, de besluiten die
//! genomen worden, de vragen die gesteld worden en wat dat alles moet opleveren.
//!
//! Een wereld is data. De assertie hoort erbij: wat een run moet opleveren
//! staat in het bestand, niet in Rust. Zo blijft een testgeval een bestand dat
//! iemand kan lezen en wijzigen zonder de crate te kennen.

use crate::cell::{CellConfig, Decretogram, Lexostatus, LexostatusOutcome};
use crate::error::{Result, SimulatorError};
use crate::invariant::{
    check_invariants, observed_graph, DecisionTraffic, DeclaredQuery, InvariantFailure, QueryEdge,
    Traffic,
};
use crate::security::{Identity, SecurityContext, SignedAnswer};
use crate::snapshot::Snapshot;
use crate::transport::InProcessTransport;
use crate::values::equivalent;
use crate::world::{
    ActionDefinition, Clock, Deadline, Events, Fixture, Warning, World, WorldDefinition,
};
use chrono::NaiveDate;
use regelrecht_engine::Value;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
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
    /// De instellingen van deze wereld: casusdata die geen wet is.
    ///
    /// Een `schedule: $betalingsritme` in een verplichting leest hieruit. Dat
    /// zo'n keuze hier staat en niet in de besluit-definitie, is het verschil
    /// tussen wat de wet voorschrijft en wat een organisatie als beleid kiest.
    #[serde(default)]
    pub settings: BTreeMap<String, Value>,
    /// De startstand: vastleggingen met een moment. Wat vóór het startmoment
    /// van de klok valt staat er bij het optuigen al; de rest landt zodra de
    /// klok die datum passeert.
    #[serde(default)]
    pub fixtures: Vec<Fixture>,
    /// Wat de actoren in deze wereld op de tijdlijn kunnen doen.
    #[serde(default)]
    pub actions: Vec<ActionDefinition>,
    /// De termijnen die waarschuwen als een feit ontbreekt.
    #[serde(default)]
    pub deadlines: Vec<Deadline>,
    /// De acties die in deze run gedaan worden, elk op een moment.
    ///
    /// Dit is het verhaal van de wereld zoals een actor het afspeelt: een aanvraag
    /// indienen, een besluit nemen, een verantwoording insturen. Ze gaan vóór de
    /// besluiten uit [`Self::decide`], want een actie is de aansturing die het
    /// wereldbestand zelf kent en `decide` is de rechtstreekse.
    #[serde(default)]
    pub act: Vec<Act>,
    /// De besluiten die in deze run rechtstreeks genomen worden, elk op een moment.
    ///
    /// De aansturing naast een actie: het scenario zegt wie wanneer waarover
    /// besluit, zonder dat er een actie in het wereldbestand voor hoeft te staan.
    /// Handig om een besluit-pad los te beproeven; het verhaal van een wereld
    /// hoort langs [`Self::act`] te lopen.
    ///
    /// Ze gaan vóór de vragen, en dat is de volgorde die past bij wat ze zijn — een
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
    /// **Een sonde, geen onderdeel van de opstelling.** In de opstelling stelt een
    /// cel zo'n vraag uitsluitend vanuit haar besluit-pad: ze heeft een input
    /// nodig die een andere organisatie vaststelt, en dan *accepteert* ze die
    /// (`accept_from`, of een `source.regulation` in haar wet — zie
    /// [`Self::decide`]). Wat deze stap overhoudt is de mogelijkheid om één vraag
    /// los te stellen en op haar antwoord te asserteren, zonder er een besluit
    /// omheen te bouwen: handig om de naad zelf te beproeven, en verder niets.
    #[serde(default)]
    pub query_via_transport: Vec<TransportQuery>,
    /// Het **toegestane vraaggraf**: welke cel welke andere cel mag bevragen, en
    /// op welke lexostatus.
    ///
    /// De runner legt hier het feitelijke graf naast — afgeleid uit wat er over
    /// de celgrenzen ging — en laat elk verschil het scenario laten falen: een
    /// vraag die hier niet staat net zo goed als een vraag die hier wél staat en
    /// niet gesteld werd (invariant I3).
    ///
    /// Weglaten mag en betekent iets: **geen enkele vraag over een celgrens**.
    /// Dat is met opzet de default, want een gate die je moet aanzetten is een
    /// gate die iemand vergeet — een scenario dat stilletjes over een grens
    /// reikt, hoort rood te worden en niet te zwijgen.
    ///
    /// Wat hier staat is een declaratie en geen vrijbrief: de gate berekent uit
    /// de `accept_from`-inputs en de `accepts_from`-afspraken van elke cel wat
    /// haar eigen recht vraagt, en een declaratie die daarbuiten valt faalt ook
    /// als de vraag netjes gesteld wordt.
    #[serde(default)]
    pub query_graph: Vec<DeclaredQuery>,
    /// De labels van de termijnen die deze run moet melden, in alfabetische
    /// volgorde vergeleken.
    ///
    /// Een lege lijst is een volwaardige verwachting: ze zegt dat er geen enkele
    /// termijn gemist is. Daarom wordt de lijst altijd afgerekend, ook als hij niet
    /// in het bestand staat — een waarschuwing die niemand verwacht, hoort een run
    /// te laten falen in plaats van stil in het verslag te belanden.
    #[serde(default)]
    pub expect_warnings: Vec<String>,
}

/// Eén actie die een actor in deze run doet.
///
/// De tegenhanger van een [`Decision`], en een stap hoger: niet "deze cel besluit
/// hierover" maar "deze actor doet dit". Wat er dan gebeurt, staat in het
/// wereldbestand en niet in deze stap — een actie legt een feit vast, levert het
/// eventueel aan een ander, of start een besluit-pad.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Act {
    /// Vrije omschrijving, verschijnt in het verslag.
    #[serde(default)]
    pub description: Option<String>,
    /// De actie uit het wereldbestand.
    pub action: String,
    /// Het ingevulde formulier van de actie.
    #[serde(default)]
    pub values: BTreeMap<String, Value>,
    /// Het moment waarop de actor dit doet. De klok gaat er eerst naartoe.
    pub op_moment: NaiveDate,
    /// De verwachte uitkomsten, als deze actie een besluit start.
    ///
    /// Mag leeg blijven, net als bij een [`Decision`]: een actie legt iets vast,
    /// dus ze bewijst ook zonder verwachting dat de vragen erna iets te vinden
    /// hebben.
    #[serde(default)]
    pub expect: BTreeMap<String, Value>,
    /// Waarden die het besluit van deze actie **geaccepteerd** moet hebben, op
    /// naam, met de cel die haar vaststelde (invariant I5).
    #[serde(default)]
    pub expect_accepted: BTreeMap<String, String>,
    /// Waarden die het besluit van deze actie **zelf** moet hebben vastgesteld.
    #[serde(default)]
    pub expect_computed: Vec<String>,
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
    /// Waarden die dit besluit van een andere cel moet hebben **geaccepteerd**,
    /// op naam, met de cel die haar vaststelde.
    ///
    /// Dit is invariant I5 als verwachting in het bestand: niet "de uitkomst
    /// klopt" maar "deze waarde komt van die organisatie en is hier niet
    /// nagerekend". Werkt voor beide wegen — een input met `accept_from` en een
    /// waarde die de wet via `source.regulation` bij een cel haalde.
    #[serde(default)]
    pub expect_accepted: BTreeMap<String, String>,
    /// Waarden die dit besluit **zelf** moet hebben vastgesteld: uit een eigen
    /// kroniek, uit een parameter of uit een eigen wet.
    ///
    /// Het tegenbewijs bij [`Self::expect_accepted`], en zonder dat tegenbewijs
    /// bewijst die niets: een opstelling waarin *alles* als geaccepteerd geldt,
    /// haalt dezelfde assertie even makkelijk.
    #[serde(default)]
    pub expect_computed: Vec<String>,
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
    /// Een waarde in een decretogram kwam ergens anders vandaan dan verwacht —
    /// of nergens herleidbaar vandaan (invariant I5).
    Provenance {
        /// De waarde waarover het gaat.
        value: String,
        /// Wat er over haar herkomst mis is.
        reason: String,
    },
    /// De run meldde andere gemiste termijnen dan het scenario verwachtte.
    Warnings {
        /// De labels die het scenario verwachtte.
        expected: Vec<String>,
        /// De labels die de run meldde.
        actual: Vec<String>,
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
    /// Dit is wat het observatielog vastlegt, en wat een decretogram als
    /// geaccepteerde waarde draagt zodra een besluit zo'n waarde accepteert.
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
    /// Elk contact over een celgrens dat dit besluit nodig had, in volgorde.
    ///
    /// Dit is wat het observatielog van een besluit te zien krijgt. Het log komt
    /// niets halen — het staat buiten de band en is passief — dus de runner geeft
    /// het door, net als bij een vraag over de celgrens.
    pub crossings: Vec<SignedAnswer>,
    /// De verwachtingen die niet uitkwamen; leeg is goed.
    pub failures: Vec<ExpectationFailure>,
}

/// Het resultaat van één actie.
#[derive(Debug, Clone)]
pub struct ActOutcome {
    /// De omschrijving uit het scenario, als die er stond.
    pub description: Option<String>,
    /// De actie die gedaan is.
    pub action: String,
    /// Wat er door deze actie gebeurde.
    pub events: Events,
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
    /// De acties die gedaan zijn, in scenariovolgorde.
    pub acts: Vec<ActOutcome>,
    /// De besluiten die rechtstreeks genomen zijn, in scenariovolgorde.
    pub decisions: Vec<DecisionOutcome>,
    /// De uitkomsten van de vragen van een consument, in scenariovolgorde.
    pub outcomes: Vec<QueryOutcome>,
    /// De uitkomsten van de vragen over een celgrens, in scenariovolgorde.
    pub transport_outcomes: Vec<TransportOutcome>,
    /// De invarianten die deze run niet haalde; leeg is goed.
    ///
    /// Apart van de verwachtingen per vraag, en dat is geen ordening maar een
    /// verschil in soort: een verwachting is wat dít scenario beweert, een
    /// invariant is wat elk scenario moet halen. Ze staan dus niet bij één vraag
    /// en niet bij één besluit — ze gaan over de run als geheel.
    pub invariant_failures: Vec<InvariantFailure>,
    /// De termijnen die verstreken zonder dat het feit er lag, in volgorde.
    pub warnings: Vec<Warning>,
    /// Verwachtingen over de run als geheel die niet uitkwamen; leeg is goed.
    pub failures: Vec<ExpectationFailure>,
    /// Het beeld van de wereld na de run.
    ///
    /// Hoort bij de run omdat het de stand is die eruit volgde, en niet iets wat
    /// er los naast staat: een test die het contract naar een frontend vastpint,
    /// hoort dat te kunnen doen op de wereld die een scenario heeft afgespeeld.
    pub snapshot: Snapshot,
}

impl ScenarioRun {
    /// Kwamen alle verwachtingen uit, en haalde de run haar invarianten?
    pub fn passed(&self) -> bool {
        self.failures.is_empty()
            && self.acts.iter().all(|o| o.failures.is_empty())
            && self.decisions.iter().all(|o| o.failures.is_empty())
            && self.outcomes.iter().all(|o| o.failures.is_empty())
            && self
                .transport_outcomes
                .iter()
                .all(|o| o.failures.is_empty())
            && self.invariant_failures.is_empty()
    }

    /// Wat er in deze run over de celgrenzen ging, met de plek waar het vandaan
    /// kwam.
    ///
    /// Dit is wat de invarianten-gate leest, en het is ook exact wat een
    /// meetinstrument aangereikt krijgt: dezelfde bewijsstukken, in dezelfde
    /// volgorde. Publiek, zodat een test de twee tegen elkaar kan houden in
    /// plaats van te moeten geloven dat ze hetzelfde zien.
    ///
    /// **Elk** besluit van de run zit erin, of het door een actie is uitgelokt of
    /// rechtstreeks genomen. Die twee horen hier niet uit elkaar te vallen: een
    /// besluit dat via een actie over de celgrens reikt, doet dat langs dezelfde
    /// weg als een besluit uit `decide`, en zou de gate anders ontglippen. In
    /// runvolgorde: eerst de acties, dan de rechtstreekse besluiten, dan de sondes.
    pub fn traffic(&self) -> Traffic<'_> {
        let from_acts = self
            .acts
            .iter()
            .flat_map(|act| &act.events.decisions)
            .map(|record| DecisionTraffic {
                decretogram: &record.decretogram,
                crossings: &record.crossings,
            });
        let direct = self.decisions.iter().map(|decision| DecisionTraffic {
            decretogram: &decision.decretogram,
            crossings: &decision.crossings,
        });
        Traffic {
            decisions: from_acts.chain(direct).collect(),
            probes: self
                .transport_outcomes
                .iter()
                .map(|outcome| &outcome.signed)
                .collect(),
        }
    }

    /// Elk contact over een celgrens in deze run, in de volgorde waarin het
    /// plaatsvond.
    pub fn crossings(&self) -> Vec<&SignedAnswer> {
        self.traffic().entries()
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
        for act in &self.acts {
            let mark = if act.failures.is_empty() {
                "ok"
            } else {
                "FOUT"
            };
            let _ = writeln!(out, "  [{mark}] actie '{}'", act.action);
            if let Some(description) = &act.description {
                let _ = writeln!(out, "        {description}");
            }
            out.push_str(&act.events.describe());
            write_failures(&mut out, &act.failures);
        }

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
            // Wat er voor dit besluit over een celgrens ging. Een input met
            // `accept_from` staat hierboven al met haar herkomst; deze regels
            // zijn de enige plek waar een waarde die de *wet* bij een andere cel
            // haalde (tier 3) in het verslag zichtbaar is.
            for crossing in &decision.crossings {
                let _ = writeln!(
                    out,
                    "        vroeg {}.{} op {} (over de celgrens, {})",
                    crossing.answer.cell,
                    crossing.answer.name,
                    crossing.answer.op_moment,
                    crossing.signature,
                );
            }
            // Het schema hoort bij het gram en dus in het verslag: wat beloofd is,
            // staat er vóórdat er iets betaald is.
            for due in &gram.obligations {
                let _ = writeln!(out, "        verplichting: {}", due.describe());
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

        // De invarianten-gate staat in het verslag ook als hij niets vond. Een
        // gate die alleen bij een fout iets zegt, is niet te onderscheiden van
        // een gate die niet gedraaid heeft — en dat is precies het soort stilte
        // waar deze opstelling tegen bedoeld is.
        let contacts = self.crossings();
        let edges = observed_graph(contacts.iter().copied()).len();
        let mark = if self.invariant_failures.is_empty() {
            "ok"
        } else {
            "FOUT"
        };
        // Het aantal takken staat erbij en niet alleen het aantal contacten: het
        // graf is waar de gate over gaat, en dezelfde vraag twee keer is één tak.
        let _ = writeln!(
            out,
            "  [{mark}] invarianten: {} contact(en) over een celgrens, {edges} tak(ken) \
             in het vraaggraf",
            contacts.len(),
        );
        for failure in &self.invariant_failures {
            let _ = writeln!(out, "        {}", failure.describe());
        }
        // De waarschuwingen staan bij elkaar en niet bij de stap waar ze vielen:
        // een gemiste termijn is geen gevolg van een actie maar van wat er níet
        // gebeurde, en dat is aan het eind pas te overzien.
        for warning in &self.warnings {
            let _ = writeln!(out, "  waarschuwing: {}", warning.describe());
        }
        write_failures(&mut out, &self.failures);

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
            ExpectationFailure::Provenance { value, reason } => {
                writeln!(out, "        herkomst van '{value}': {reason}")
            }
            ExpectationFailure::Warnings { expected, actual } => writeln!(
                out,
                "        verwachtte gemiste termijnen [{}], kreeg [{}]",
                expected.join(", "),
                actual.join(", ")
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
        self.validate_query_graph()
    }

    /// Controleer het gedeclareerde vraaggraf op zichzelf.
    ///
    /// Bij het lezen en niet pas na een run, want een schrijffout in het bestand
    /// hoort niet als uitslag van de gate naar buiten te komen. Een tak naar een
    /// cel die niet bestaat wordt nooit gesteld en zou als "gedeclareerde vraag
    /// die uitbleef" verschijnen — een melding die de lezer naar de run stuurt in
    /// plaats van naar de typfout; een tak van een cel naar zichzelf net zo, want
    /// de veiligheidscontext laat die vraag niet over een grens.
    ///
    /// De dubbele tak staat er om de omgekeerde reden: die zou de gate juist
    /// *niets* laten zeggen. Een graf is een verzameling, dus de tweede regel
    /// wordt stil opgeslokt, en dan staat er een regel in het bestand die niets
    /// doet.
    fn validate_query_graph(&self) -> Result<()> {
        let known = || {
            self.cells
                .iter()
                .map(|cell| cell.id.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        };
        // Op de tak zelf en niet op haar tekst: de `Display`-vorm plakt cel en
        // lexostatus met een punt aan elkaar, en dan zou een lexostatus met een
        // punt erin twee verschillende takken als dubbel kunnen aanmerken.
        let mut seen: BTreeSet<QueryEdge> = BTreeSet::new();

        for declared in &self.query_graph {
            let edge = declared.edge();
            for cell in [&declared.from, &declared.to] {
                if !self.cells.iter().any(|config| &config.id == cell) {
                    return Err(SimulatorError::QueryGraphUnknownCell {
                        scenario: self.name.clone(),
                        edge: edge.to_string(),
                        cell: cell.clone(),
                        known: known(),
                    });
                }
            }
            if declared.from == declared.to {
                return Err(SimulatorError::QueryGraphToSelf {
                    scenario: self.name.clone(),
                    edge: edge.to_string(),
                });
            }
            let melding = edge.to_string();
            if !seen.insert(edge) {
                return Err(SimulatorError::DuplicateQueryGraphEdge {
                    scenario: self.name.clone(),
                    edge: melding,
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

    /// De wereld waarin dit scenario zich afspeelt, los van zijn stappen.
    ///
    /// Het scenariobestand draagt de velden van een wereldbestand plus de stappen
    /// erop, en die twee zijn hier niet samengevoegd tot één serde-vorm: een
    /// geflatten veld verdraagt geen `deny_unknown_fields`, en dan zou een typfout
    /// in een scenario stil verdwijnen. De prijs is deze kopie; de winst is dat
    /// een wereldbestand en een scenario dezelfde vorm hebben en dat een typfout
    /// in geen van beide doorglipt.
    pub fn definition(&self) -> WorldDefinition {
        WorldDefinition {
            clock: self.clock,
            cells: self.cells.clone(),
            settings: self.settings.clone(),
            fixtures: self.fixtures.clone(),
            actions: self.actions.clone(),
            deadlines: self.deadlines.clone(),
        }
    }

    /// Tuig de wereld op: de cellen, de klok op haar startmoment en de
    /// startstand die op dat moment al gebeurd was.
    pub fn world(&self, regulation_root: &Path) -> Result<World> {
        World::from_definition(&self.definition(), regulation_root)
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

        let acts = self.do_acts(&mut world)?;
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
        let warnings = world.warnings().to_vec();

        let mut run = ScenarioRun {
            name: self.name.clone(),
            clock: world.now(),
            pending_triggers: world.pending_triggers(),
            acts,
            decisions,
            outcomes,
            transport_outcomes,
            invariant_failures: Vec::new(),
            failures: check_warnings(&self.expect_warnings, &warnings),
            warnings,
            snapshot: world.snapshot(),
        };
        // De gate draait als laatste en over de hele run: hij vergelijkt het
        // gedeclareerde vraaggraf met wat er werkelijk over de grenzen ging, en
        // dat laatste is pas compleet als alles gedraaid heeft. Hij draait bij
        // élk scenario, ook bij eentje dat geen vraaggraf declareert — dan is
        // het toegestane graf leeg, en dat is een even geldige bewering.
        let failures = check_invariants(&self.query_graph, &self.cells, &run.traffic());
        run.invariant_failures = failures;
        Ok(run)
    }

    /// Laat de actoren hun acties doen, elk op zijn eigen moment.
    ///
    /// De klok gaat eerst vooruit tot dat moment, en dan doet de actor het: een
    /// actie heeft geen eigen moment mee te geven, ze gebeurt op de stand van de
    /// wereld. Zo landt een levering of een vervallen termijn die ertussen valt
    /// eerst — en dat is wat een actie die op zo'n feit wacht, nodig heeft.
    fn do_acts(&self, world: &mut World) -> Result<Vec<ActOutcome>> {
        let mut acts = Vec::with_capacity(self.act.len());
        for step in &self.act {
            if step.op_moment > world.now() {
                world.advance(step.op_moment)?;
            }

            let events = world.act(&step.action, &step.values)?;

            // Een actie die een besluit start, wordt op datzelfde besluit
            // afgerekend — verwachtingen én de herkomstgate van I5. Een actie die
            // vastlegt, heeft geen besluit, en dan is er niets om op te rekenen.
            let mut failures = Vec::new();
            for record in &events.decisions {
                let gram = &record.decretogram;
                failures.extend(check_values(&step.expect, &gram.outputs));
                failures.extend(check_provenance(gram));
                failures.extend(check_origins(
                    &step.expect_accepted,
                    &step.expect_computed,
                    gram,
                ));
            }

            acts.push(ActOutcome {
                description: step.description.clone(),
                action: step.action.clone(),
                events,
                failures,
            });
        }
        Ok(acts)
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

            let record = world.decide(
                &decision.cell,
                &decision.besluit,
                &decision.params,
                decision.op_moment,
            )?;
            let decretogram = record.decretogram;

            let mut failures = check_values(&decision.expect, &decretogram.outputs);
            // De gate draait bij élk besluit, ook als het scenario er niets over
            // zegt: een invariant die je moet aanzetten, is een invariant die
            // iemand vergeet.
            failures.extend(check_provenance(&decretogram));
            failures.extend(check_origins(
                &decision.expect_accepted,
                &decision.expect_computed,
                &decretogram,
            ));

            decisions.push(DecisionOutcome {
                description: decision.description.clone(),
                decretogram,
                crossings: record.crossings,
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

/// **Invariant I5** — narekenen versus accepteren, als gate over elk besluit.
///
/// De invariant zegt: is een waarde door een andere organisatie vastgesteld, dan
/// hoort ze geaccepteerd te zijn en niet hier herberekend. Dat is alleen te
/// controleren als elke waarde in een decretogram zegt waar ze vandaan komt, en
/// dus is dít wat de gate eist:
///
/// - **elke input is de waarde waarop gerekend is.** De herkomst staat bij de
///   input, dus als het receipt met een andere waarde rekende, hoort de herkomst
///   bij iets wat het besluit niet gebruikt heeft — en dan zegt het gram iets
///   anders dan er gebeurd is.
/// - **elke uitkomst komt uit deze uitvoering.** Een uitkomst die niet in het
///   receipt staat, is er langs een andere weg in gezet en is dus niet berekend
///   op de inputs die het gram noemt.
/// - **een geaccepteerde waarde wijst naar een ánder.** Een gram dat zegt een
///   waarde van zichzelf geaccepteerd te hebben, verbergt een eigen berekening
///   achter het woord "accepteren".
/// - **een geaccepteerde waarde is niet óók een eigen uitkomst.** Staat dezelfde
///   naam bij de uitkomsten van het gram, dan is ze hier alsnog uitgerekend, en
///   dan is de acceptatie versiering — precies het geval dat I5 uitsluit.
///
/// Hij draait bij elk besluit in elk scenario. Een scenario kan er met
/// [`Decision::expect_accepted`] een concrete verwachting bovenop leggen; deze
/// gate is wat er zonder die verwachting nog steeds geldt.
pub fn check_provenance(gram: &Decretogram) -> Vec<ExpectationFailure> {
    let mut failures = Vec::new();

    for (name, input) in &gram.inputs {
        match gram.receipt.execution.parameters.get(name) {
            Some(value) if *value == input.value => {}
            Some(value) => failures.push(ExpectationFailure::Provenance {
                value: name.clone(),
                reason: format!(
                    "het gram legt {} vast met herkomst {}, maar de uitvoering rekende \
                     met {value}",
                    input.value,
                    input.origin.describe()
                ),
            }),
            None => failures.push(ExpectationFailure::Provenance {
                value: name.clone(),
                reason: format!(
                    "staat als input in het gram ({}), maar de uitvoering kreeg haar niet",
                    input.origin.describe()
                ),
            }),
        }
    }

    for name in gram.outputs.keys() {
        if !gram.receipt.results.outputs.contains_key(name) {
            failures.push(ExpectationFailure::Provenance {
                value: name.clone(),
                reason: format!(
                    "staat als uitkomst in het gram, maar het receipt van regeling \
                     '{}' kent haar niet",
                    gram.regulation
                ),
            });
        }
    }

    for (name, cell) in gram.accepted_values() {
        if cell == gram.cell {
            failures.push(ExpectationFailure::Provenance {
                value: name.to_string(),
                reason: format!(
                    "zegt geaccepteerd te zijn van cel '{cell}', maar dat is de cel die \
                     besloot; dan is er narekenen als accepteren opgeschreven"
                ),
            });
        }
        if gram.outputs.contains_key(name) {
            failures.push(ExpectationFailure::Provenance {
                value: name.to_string(),
                reason: format!(
                    "is geaccepteerd van cel '{cell}' én staat als eigen uitkomst in het \
                     gram; accepteren en narekenen tegelijk kan niet"
                ),
            });
        }
    }

    failures
}

/// Reken een besluit af op de herkomst die het scenario verwacht.
///
/// Twee kanten, en ze horen bij elkaar: `expect_accepted` zegt dat een waarde
/// van een bepaalde cel komt, `expect_computed` dat een waarde hier is
/// vastgesteld. Alleen de eerste zou bewijzen dat de opstelling waarden kán
/// accepteren, niet dat ze het onderscheid máákt.
fn check_origins(
    expect_accepted: &BTreeMap<String, String>,
    expect_computed: &[String],
    gram: &Decretogram,
) -> Vec<ExpectationFailure> {
    let accepted = gram.accepted_values();
    let mut failures = Vec::new();

    for (value, expected) in expect_accepted {
        match accepted.get(value.as_str()) {
            Some(cell) if cell == expected => {}
            Some(cell) => failures.push(ExpectationFailure::Provenance {
                value: value.clone(),
                reason: format!("verwachtte geaccepteerd van cel '{expected}', kreeg '{cell}'"),
            }),
            None => failures.push(ExpectationFailure::Provenance {
                value: value.clone(),
                reason: format!(
                    "verwachtte geaccepteerd van cel '{expected}', maar deze waarde is \
                     niet geaccepteerd{}",
                    describe_origin(gram, value)
                ),
            }),
        }
    }

    for value in expect_computed {
        if let Some(cell) = accepted.get(value.as_str()) {
            failures.push(ExpectationFailure::Provenance {
                value: value.clone(),
                reason: format!(
                    "verwachtte een eigen vaststelling, maar deze waarde is geaccepteerd \
                     van cel '{cell}'"
                ),
            });
            continue;
        }
        if !gram.inputs.contains_key(value) && !gram.outputs.contains_key(value) {
            failures.push(ExpectationFailure::Provenance {
                value: value.clone(),
                reason: "komt in dit gram niet voor, dus er valt niets over na te rekenen"
                    .to_string(),
            });
        }
    }

    failures
}

/// Wat het gram wél over de herkomst van deze waarde zegt, voor in een melding.
///
/// Leeg als de naam er helemaal niet in voorkomt: dan is "deze waarde is niet
/// geaccepteerd" al het hele verhaal, en een lege haak erachter suggereert dat
/// er iets weggelaten is.
fn describe_origin(gram: &Decretogram, value: &str) -> String {
    match gram.inputs.get(value) {
        Some(input) => format!(" ({})", input.origin.describe()),
        None if gram.outputs.contains_key(value) => {
            format!(" (eigen uitkomst van regeling '{}')", gram.regulation)
        }
        None => String::new(),
    }
}

/// Reken de run af op de termijnen die ze miste.
///
/// Altijd, ook als het scenario er niets over zegt: dan is de verwachting "geen
/// enkele". Een waarschuwing die niemand verwachtte hoort een run te laten falen —
/// ze zegt dat er iets níet gebeurd is wat er hoorde te gebeuren, en dat is
/// precies het soort stilte waar een scenario voor bestaat.
fn check_warnings(expected: &[String], actual: &[Warning]) -> Vec<ExpectationFailure> {
    let mut found: Vec<String> = actual
        .iter()
        .map(|warning| warning.label.clone())
        .collect::<Vec<_>>();
    found.sort();
    let mut wanted = expected.to_vec();
    wanted.sort();
    if wanted == found {
        return Vec::new();
    }
    vec![ExpectationFailure::Warnings {
        expected: wanted,
        actual: found,
    }]
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

    /// Een wereld met twee cellen, waar een vraaggraf bij te schrijven is.
    ///
    /// De cellen zijn bron-cellen zonder kroniek: deze tests gaan over wat er bij
    /// het *lezen* van het bestand geweigerd wordt, en komen nooit aan een run
    /// toe.
    fn met_vraaggraf(query_graph: &str) -> Result<Scenario> {
        Scenario::from_yaml(&format!(
            "
name: twee cellen en een vraaggraf
clock: {{ start: 2025-01-01 }}
cells:
  - id: toeslagen
    laws: []
  - id: brp
    laws: []
query_graph:
{query_graph}"
        ))
    }

    #[test]
    fn een_vraaggraf_naar_een_onbekende_cel_wordt_geweigerd() {
        let err = met_vraaggraf(
            "  - from: toeslagen
    to: kiesraad
    lexostatus: zetels
",
        )
        .expect_err(
            "een tak naar een cel die het scenario niet heeft, zou na de run als \
             'gedeclareerde vraag die uitbleef' verschijnen en de lezer naar de run sturen \
             in plaats van naar de typfout",
        );
        assert!(
            matches!(err, SimulatorError::QueryGraphUnknownCell { .. }),
            "verwachtte QueryGraphUnknownCell, kreeg {err}"
        );
        let melding = err.to_string();
        assert!(
            melding.contains("kiesraad") && melding.contains("toeslagen, brp"),
            "de melding hoort de onbekende cel te noemen en de cellen die er wél zijn, \
             kreeg: {melding}"
        );
    }

    #[test]
    fn een_vraaggraf_waarin_een_cel_zichzelf_bevraagt_wordt_geweigerd() {
        let err = met_vraaggraf(
            "  - from: toeslagen
    to: toeslagen
    lexostatus: partnerschap
",
        )
        .expect_err("een cel komt voor eigen feiten niet over een celgrens");
        assert!(
            matches!(err, SimulatorError::QueryGraphToSelf { .. }),
            "verwachtte QueryGraphToSelf, kreeg {err}"
        );
    }

    #[test]
    fn dezelfde_tak_twee_keer_in_het_vraaggraf_wordt_geweigerd() {
        let err = met_vraaggraf(
            "  - from: toeslagen
    to: brp
    lexostatus: partnerschap
  - doc: dezelfde tak, andere toelichting
    from: toeslagen
    to: brp
    lexostatus: partnerschap
",
        )
        .expect_err(
            "een graf is een verzameling, dus de tweede regel wordt stil opgeslokt — \
             en dan staat er een regel in het bestand die niets doet",
        );
        assert!(
            matches!(err, SimulatorError::DuplicateQueryGraphEdge { .. }),
            "verwachtte DuplicateQueryGraphEdge, kreeg {err}"
        );
    }

    #[test]
    fn twee_takken_naar_dezelfde_cel_op_verschillende_lexostatussen_mogen() {
        assert!(
            met_vraaggraf(
                "  - from: toeslagen
    to: brp
    lexostatus: partnerschap
  - from: toeslagen
    to: brp
    lexostatus: woonplaats
",
            )
            .is_ok(),
            "wat een cel publiceert zijn losse namen, dus twee lexostatussen bij één peer \
             zijn twee takken en niet een dubbele"
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
    /// Een leeg beeld, voor de tests die over het **verslag** gaan.
    ///
    /// Die bouwen met opzet geen wereld op: wat ze controleren is wat er in de
    /// tekst staat, en een wereld eronder zou daar niets aan toevoegen behalve
    /// tijd.
    fn empty_snapshot() -> Snapshot {
        Snapshot {
            clock: moment(),
            settings: BTreeMap::new(),
            locked_settings: BTreeMap::new(),
            cells: Vec::new(),
            actions: Vec::new(),
            crossings: Vec::new(),
            warnings: Vec::new(),
        }
    }

    /// Een resultaat zonder run eronder, voor de verslagtests.
    fn report_of(pending_triggers: usize, outcomes: Vec<QueryOutcome>) -> ScenarioRun {
        ScenarioRun {
            name: "tijd".to_string(),
            clock: moment(),
            pending_triggers,
            acts: Vec::new(),
            decisions: Vec::new(),
            outcomes,
            transport_outcomes: Vec::new(),
            invariant_failures: Vec::new(),
            warnings: Vec::new(),
            failures: Vec::new(),
            snapshot: empty_snapshot(),
        }
    }

    #[test]
    fn het_verslag_meldt_vastleggingen_die_niet_afgingen() {
        let run = |pending_triggers| report_of(pending_triggers, Vec::new());

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

    /// Het verslag meldt de gate ook als hij niets vond.
    ///
    /// Dat staat in de moduledocs als eigenschap en is precies het soort regel dat
    /// bij een opschoning weggehaald wordt zonder dat er iets rood wordt: een
    /// verslag zonder die regel is niet te onderscheiden van een gate die niet
    /// gedraaid heeft, en dat is de stilte waar deze opstelling tegen bedoeld is.
    #[test]
    fn het_verslag_meldt_de_invarianten_ook_als_er_niets_te_melden_was() {
        let verslag = report_of(0, Vec::new()).report();
        assert!(
            verslag.contains("[ok] invarianten: 0 contact(en) over een celgrens"),
            "een run zonder bevindingen hoort de gate alsnog te melden; kreeg:\n{verslag}"
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
        let run = report_of(
            0,
            vec![outcome("vóór de vastlegging"), outcome("erna, ongewijzigd")],
        );

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
