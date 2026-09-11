//! De invarianten-gate: het gedeclareerde vraaggraf naast het feitelijke.
//!
//! Een scenario declareert welke cel welke andere cel mag bevragen en op welke
//! lexostatus. Een run levert wat er werkelijk over de celgrenzen ging. Elk
//! verschil tussen die twee is een falend scenario — een niet-gedeclareerde
//! vraag net zo goed als een gedeclareerde vraag die uitbleef. Dat tweede is
//! geen bijzaak: een scenario waarin de vraag stilletjes wegvalt, meet niets
//! meer en zou zonder die kant van de vergelijking groen blijven.
//!
//! Wat deze gate toetst, en aan wat:
//!
//! | invariant | de toets | waaraan |
//! |---|---|---|
//! | **I2** | elke waarde die een gram geaccepteerd noemt, heeft een vastgelegd contact | het gram tegen de contacten |
//! | **I3** | het feitelijke graf is precies het gedeclareerde graf | declaratie tegen contacten |
//! | **I3** | elke gestelde vraag past binnen de definities van de vragende cel | contacten tegen de celconfiguratie |
//! | **I3** | elke gedeclareerde vraag past binnen die definities | declaratie tegen de celconfiguratie |
//! | **I4** | een cel combineert niet buiten een besluit-pad | contacten per vragende cel |
//! | **I4** | wat een besluit over de grens haalde, staat in zijn decretogram | de contacten tegen het gram |
//!
//! I1 staat er niet bij en dat is geen vergeten regel: "een cel bezit haar
//! kroniekstore privé" is afgedwongen door het typesysteem — er is geen
//! `pub fn store()` — en een gate die dat naloopt zou een compileerfout
//! nameten. I5 staat er ook niet bij: die draait als
//! [`crate::check_provenance`] over elk decretogram.
//!
//! **I1 tot en met I5 en het meetinstrument waarop deze gate rust, komen niet
//! uit RFC-022.** Die RFC beweert autonomie maar zegt nergens hoe je die meet;
//! dit is toegevoegd meetgereedschap, en het hoort niet als RFC-inhoud gelezen
//! te worden.
//!
//! ## Waarom het toegestane graf berekend wordt en niet geloofd
//!
//! Een cel bevraagt alleen cellen die haar eigen wetten of besluit-definities
//! noemen (I3). Dat is uit de configuratie te *berekenen*: de `accept_from`-
//! inputs van haar besluiten, en de afspraken in `accepts_from` waarmee ze de
//! `source.regulation`-verwijzingen van haar wetten bij een peer uit laat komen.
//! [`defined_graph`] doet dat, en dus kan een declaratie ernaast liggen: een
//! scenario dat een vraag toestaat die geen definitie van de vragende cel vraagt,
//! declareert iets wat die cel niet hoort te doen, en dat is een fout in het
//! bestand en niet in de run.
//!
//! Daarom ook de regel die op het eerste gezicht dubbelop lijkt: een gestelde
//! vraag die buiten de definities valt, is een fout **ook als het scenario haar
//! declareert**. Zonder die regel zou elke declaratie een vrijbrief zijn en zou
//! I3 niets meer tegenhouden dan slordigheid.
//!
//! ## Waarop de gate rust, en wat dat kost
//!
//! De gate leest de bewijsstukken van de veiligheidscontext ([`SignedAnswer`]) —
//! exact de regels die het meetinstrument bewaart, niet een tweede boekhouding
//! ernaast. Dat het instrument zelf hier niet geïmporteerd wordt, is met opzet:
//! het staat buiten de band en geen productiepad mag ernaar verwijzen. Dat de
//! twee hetzelfde zien is daarom een assertie en geen aanname — zie
//! `tests/invarianten.rs`, dat het graf uit het instrument haalt en met het graf
//! van de gate vergelijkt.
//!
//! Wat de gate niet kan zien, hoort er expliciet bij te staan: een contact dat
//! nergens wordt aangereikt. Volledigheid is structureel — de
//! veiligheidscontext is de enige weg over een grens en `World::decide` geeft
//! elk contact mee — en niet door deze gate afgedwongen. Eén kant is wél
//! gedicht, en dat is de kant die in de praktijk misgaat: een gram dat zegt te
//! hebben geaccepteerd zonder dat er een contact bij staat, valt op (I2).

use crate::cell::{BesluitInput, CellConfig, Decretogram};
use crate::security::SignedAnswer;
use chrono::NaiveDate;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// Eén tak van een vraaggraf: welke cel bevraagt welke andere cel, en waarover.
///
/// De lexostatus hoort erbij en is geen detail. "Cel A mag cel B bevragen" is
/// een blanco machtiging; wat een cel publiceert zijn losse, gedocumenteerde
/// namen (RFC-022 §4.1), en het graf hoort op diezelfde korrel te staan.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct QueryEdge {
    /// De vragende cel.
    pub from: String,
    /// De bevraagde cel.
    pub to: String,
    /// De gepubliceerde lexostatus die gevraagd wordt.
    pub lexostatus: String,
}

impl fmt::Display for QueryEdge {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} -> {}.{}", self.from, self.to, self.lexostatus)
    }
}

impl QueryEdge {
    /// De tak die dit bewijsstuk beschrijft.
    ///
    /// Dit is de hele afleiding van het feitelijke graf: wie vroeg staat in het
    /// bewijsstuk, en aan wie en waarover staat in het antwoord. Er wordt niets
    /// bij bedacht.
    pub fn of(answer: &SignedAnswer) -> Self {
        Self {
            from: answer.asked_by.cell().to_string(),
            to: answer.answer.cell.clone(),
            lexostatus: answer.answer.name.clone(),
        }
    }
}

/// Eén tak zoals een wereldbestand hem declareert.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeclaredQuery {
    /// Vrije toelichting: waarom deze vraag mag. Verschijnt nergens in een
    /// antwoord.
    #[serde(default)]
    pub doc: Option<String>,
    /// De cel die mag vragen.
    pub from: String,
    /// De cel aan wie ze mag vragen.
    pub to: String,
    /// De lexostatus die ze daar mag vragen.
    pub lexostatus: String,
}

impl DeclaredQuery {
    /// Deze declaratie als tak.
    pub fn edge(&self) -> QueryEdge {
        QueryEdge {
            from: self.from.clone(),
            to: self.to.clone(),
            lexostatus: self.lexostatus.clone(),
        }
    }
}

/// Wat er in één run over de celgrenzen ging, met de plek waar het vandaan kwam.
///
/// De herkomst hoort erbij, want I4 hangt eraan: een contact vanuit een besluit
/// mag combineren — het gram laat het zien — en een contact daarbuiten niet.
/// Zonder dat onderscheid zou de gate een besluit dat twee organisaties bevraagt
/// niet van een cel kunnen onderscheiden die stilletjes hetzelfde doet.
#[derive(Debug, Default)]
pub struct Traffic<'a> {
    /// Per besluit: het gram, en de contacten die dat besluit nodig had.
    pub decisions: Vec<DecisionTraffic<'a>>,
    /// Contacten die niet bij een besluit horen: de sonde
    /// `query_via_transport`, waarmee een scenario één vraag los over de naad
    /// zet. In de opstelling zelf komt een cel alleen vanuit een besluit over
    /// haar grens, dus dit is de enige weg waarlangs een scenario een
    /// I3- of I4-schending kan uitdrukken — en daarom wordt de sonde aan
    /// dezelfde definities gehouden als een besluit.
    pub probes: Vec<&'a SignedAnswer>,
}

/// De contacten van één besluit, met het gram dat eruit kwam.
#[derive(Debug)]
pub struct DecisionTraffic<'a> {
    /// Het vastgelegde besluit.
    pub decretogram: &'a Decretogram,
    /// Wat dit besluit over een celgrens haalde, in volgorde.
    pub crossings: &'a [SignedAnswer],
}

impl<'a> Traffic<'a> {
    /// Elk contact van deze run, in de volgorde waarin het plaatsvond.
    ///
    /// Dezelfde volgorde als een run ze aflegt: eerst de besluiten, dan de
    /// sondes. Dat is ook de volgorde waarin het meetinstrument ze krijgt, dus
    /// de gate en het instrument lezen dezelfde rij.
    pub fn entries(&self) -> Vec<&'a SignedAnswer> {
        self.decisions
            .iter()
            .flat_map(|decision| decision.crossings.iter())
            .chain(self.probes.iter().copied())
            .collect()
    }
}

/// Eén invariant die niet gehaald werd.
///
/// Elke variant draagt wat een lezer nodig heeft die het scenario niet kent:
/// welke invariant, welke actoren, en welk moment. Dat laatste is geen luxe —
/// twee cellen kunnen dezelfde vraag op twee momenten stellen, en dan is
/// "welke call het betrof" zonder moment geen antwoord.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InvariantFailure {
    /// Er ging een vraag over een celgrens die het scenario niet declareert.
    UndeclaredCall {
        /// De vraag die gesteld werd.
        edge: QueryEdge,
        /// Het moment waarop ze gesteld werd.
        op_moment: NaiveDate,
        /// Wat het scenario wél declareert.
        declared: String,
    },
    /// Het scenario declareert een vraag die in deze run niet gesteld is.
    CallDidNotHappen {
        /// De gedeclareerde vraag.
        edge: QueryEdge,
        /// Wat er wél gevraagd is.
        observed: String,
    },
    /// Een cel bevroeg een cel die haar eigen definities niet noemen (I3).
    CallOutsideDefinitions {
        /// De vraag die gesteld werd.
        edge: QueryEdge,
        /// Het moment waarop ze gesteld werd.
        op_moment: NaiveDate,
        /// Wat de definities van de vragende cel wél toestaan.
        defined: String,
    },
    /// Het scenario declareert een vraag waar de definities niet om vragen.
    DeclarationOutsideDefinitions {
        /// De gedeclareerde vraag.
        edge: QueryEdge,
        /// Wat de definities van de vragende cel wél toestaan.
        defined: String,
    },
    /// Een cel combineerde antwoorden van meer dan één cel buiten een
    /// besluit-pad (I4).
    SynthesisOutsideBesluit {
        /// De cel die combineerde.
        cell: String,
        /// De cellen die ze bevroeg.
        peers: String,
        /// Het moment waarop de tweede bron erbij kwam.
        op_moment: NaiveDate,
    },
    /// Een besluit haalde iets over de grens dat zijn decretogram niet laat
    /// zien (I4).
    CrossingNotInDecretogram {
        /// De vraag die het besluit stelde.
        edge: QueryEdge,
        /// Het moment waarop ze gesteld werd.
        op_moment: NaiveDate,
        /// De zaak waarover besloten werd.
        zaakkenmerk: String,
        /// Wat het gram wél als geaccepteerd opschrijft.
        accepted: String,
    },
    /// Een gram noemt een waarde geaccepteerd zonder dat er een contact met die
    /// cel is vastgelegd (I2).
    AcceptedWithoutCrossing {
        /// De waarde die geaccepteerd zou zijn.
        value: String,
        /// De cel die haar vastgesteld zou hebben.
        peer: String,
        /// De zaak waarover besloten werd.
        zaakkenmerk: String,
        /// De contacten die er wél zijn.
        observed: String,
    },
}

impl InvariantFailure {
    /// Welke invariant hier niet gehaald werd.
    ///
    /// Als tekst en niet als eigen enum: het nummer is wat een lezer zoekt, en
    /// een tweede opsomming naast de README zou ernaast gaan lopen.
    pub fn invariant(&self) -> &'static str {
        match self {
            Self::AcceptedWithoutCrossing { .. } => "I2",
            Self::UndeclaredCall { .. }
            | Self::CallDidNotHappen { .. }
            | Self::CallOutsideDefinitions { .. }
            | Self::DeclarationOutsideDefinitions { .. } => "I3",
            Self::SynthesisOutsideBesluit { .. } | Self::CrossingNotInDecretogram { .. } => "I4",
        }
    }

    /// De melding, leesbaar voor wie dit scenario niet kent.
    pub fn describe(&self) -> String {
        let invariant = self.invariant();
        match self {
            Self::UndeclaredCall {
                edge,
                op_moment,
                declared,
            } => format!(
                "{invariant} (vraaggraf): cel '{}' vroeg '{}.{}' op {op_moment}, maar dit \
                 scenario declareert die vraag niet ({declared})",
                edge.from, edge.to, edge.lexostatus
            ),
            Self::CallDidNotHappen { edge, observed } => format!(
                "{invariant} (vraaggraf): dit scenario declareert dat cel '{}' '{}.{}' mag \
                 vragen, maar die vraag is in deze run niet gesteld ({observed})",
                edge.from, edge.to, edge.lexostatus
            ),
            Self::CallOutsideDefinitions {
                edge,
                op_moment,
                defined,
            } => format!(
                "{invariant} (definities): cel '{}' vroeg '{}.{}' op {op_moment}, maar haar \
                 eigen wetten en besluit-definities vragen daar niet om ({defined}); \
                 een cel bevraagt alleen de cellen die haar definities noemen, ook als \
                 het scenario de vraag toestaat",
                edge.from, edge.to, edge.lexostatus
            ),
            Self::DeclarationOutsideDefinitions { edge, defined } => format!(
                "{invariant} (declaratie): dit scenario declareert dat cel '{}' '{}.{}' mag \
                 vragen, maar geen `accept_from` of `accepts_from` van die cel vraagt daarom \
                 ({defined}); declaratie en definitie lopen uit de pas",
                edge.from, edge.to, edge.lexostatus
            ),
            Self::SynthesisOutsideBesluit {
                cell,
                peers,
                op_moment,
            } => format!(
                "{invariant}: cel '{cell}' bevroeg buiten een besluit-pad meer dan één cel \
                 ({peers}); de tweede kwam erbij op {op_moment}. Combineren over cellen heen \
                 hoort in een besluit, zichtbaar in het decretogram, of bij een consument — \
                 nooit onzichtbaar in een cel"
            ),
            Self::CrossingNotInDecretogram {
                edge,
                op_moment,
                zaakkenmerk,
                accepted,
            } => format!(
                "{invariant}: cel '{}' vroeg voor zaak '{zaakkenmerk}' op {op_moment} \
                 '{}.{}', maar het decretogram laat geen waarde van cel '{}' zien \
                 ({accepted}); dan combineert de cel onzichtbaar",
                edge.from, edge.to, edge.lexostatus, edge.to
            ),
            Self::AcceptedWithoutCrossing {
                value,
                peer,
                zaakkenmerk,
                observed,
            } => format!(
                "{invariant}: het decretogram van zaak '{zaakkenmerk}' noemt waarde \
                 '{value}' geaccepteerd van cel '{peer}', maar er is geen contact met die \
                 cel vastgelegd ({observed})"
            ),
        }
    }
}

/// Het **feitelijke** vraaggraf: de takken die de bewijsstukken beschrijven.
///
/// Neemt wat het meetinstrument bewaart — een rij [`SignedAnswer`]s — en leidt
/// daar het graf uit af. Dezelfde vraag twee keer is één tak: een graf zegt wie
/// wie mag bevragen en niet hoe vaak.
pub fn observed_graph<'a>(
    entries: impl IntoIterator<Item = &'a SignedAnswer>,
) -> BTreeSet<QueryEdge> {
    entries.into_iter().map(QueryEdge::of).collect()
}

/// Het **toegestane** vraaggraf, berekend uit de celconfiguraties.
///
/// Twee soorten afspraak wijzen een peer aan, en ze staan hier op één hoop
/// omdat I3 ze niet onderscheidt:
///
/// - een `accept_from`-input van een besluit-definitie: de cel zegt zelf dat
///   deze waarde van een andere organisatie komt;
/// - een `accepts_from`-afspraak van de cel: daarmee komt een
///   `source.regulation` uit een van haar eigen wetten bij een lexostatus van
///   een peer uit (tier 3 van RFC-022 §4.2).
///
/// Dat de tweede lijst er staat in plaats van de wet zelf, is geen omweg: de wet
/// noemt een cel-id en een uitkomstnaam, en welke gepubliceerde lexostatus daar
/// bij hoort is een afspraak tussen twee organisaties en geen recht. De
/// afspraken zijn bovendien al aan de wetten gebonden — een `accepts_from` die
/// geen van de eigen wetten opvraagt, wordt bij het optuigen van de cel
/// geweigerd — dus wat hier uit komt is precies wat het recht van die cel vraagt.
pub fn defined_graph(cells: &[CellConfig]) -> BTreeSet<QueryEdge> {
    let mut edges = BTreeSet::new();
    for config in cells {
        let from_besluiten = config
            .besluit_definitions
            .iter()
            .flat_map(|definition| definition.inputs.values())
            .filter_map(BesluitInput::accepted_query);
        let from_laws = config
            .accepts_from
            .iter()
            .map(|source| (source.cell.as_str(), source.lexostatus.as_str()));

        for (to, lexostatus) in from_besluiten.chain(from_laws) {
            edges.insert(QueryEdge {
                from: config.id.clone(),
                to: to.to_string(),
                lexostatus: lexostatus.to_string(),
            });
        }
    }
    edges
}

/// Draai de gate over één run.
///
/// Hij draait bij élke run, ook als het scenario geen vraaggraf declareert:
/// dan is het toegestane graf leeg en is elk contact over een celgrens
/// niet-gedeclareerd. Een invariant die je moet aanzetten, is een invariant die
/// iemand vergeet.
pub fn check_invariants(
    declared: &[DeclaredQuery],
    cells: &[CellConfig],
    traffic: &Traffic<'_>,
) -> Vec<InvariantFailure> {
    let entries = traffic.entries();
    let observed = observed_graph(entries.iter().copied());
    let declared_edges: BTreeSet<QueryEdge> = declared.iter().map(DeclaredQuery::edge).collect();
    let defined = defined_graph(cells);

    let mut failures = Vec::new();
    failures.extend(compare_graphs(&declared_edges, &observed, &entries));
    failures.extend(check_definitions(
        &declared_edges,
        &observed,
        &defined,
        &entries,
    ));
    failures.extend(check_synthesis(traffic));
    failures.extend(check_visibility(traffic, &observed));
    failures
}

/// Het gedeclareerde graf naast het feitelijke, beide kanten op (I3).
fn compare_graphs(
    declared: &BTreeSet<QueryEdge>,
    observed: &BTreeSet<QueryEdge>,
    entries: &[&SignedAnswer],
) -> Vec<InvariantFailure> {
    let mut failures = Vec::new();
    for edge in observed.difference(declared) {
        failures.push(InvariantFailure::UndeclaredCall {
            edge: edge.clone(),
            op_moment: first_moment(entries, edge),
            declared: listing("gedeclareerd", declared),
        });
    }
    for edge in declared.difference(observed) {
        failures.push(InvariantFailure::CallDidNotHappen {
            edge: edge.clone(),
            observed: listing("wel gesteld", observed),
        });
    }
    failures
}

/// Elke gestelde en elke gedeclareerde vraag tegen de definities (I3).
///
/// Een gestelde vraag daarbuiten is een fout ook als het scenario haar
/// declareert; anders zou een declaratie een vrijbrief zijn.
fn check_definitions(
    declared: &BTreeSet<QueryEdge>,
    observed: &BTreeSet<QueryEdge>,
    defined: &BTreeSet<QueryEdge>,
    entries: &[&SignedAnswer],
) -> Vec<InvariantFailure> {
    let mut failures = Vec::new();
    for edge in observed.difference(defined) {
        failures.push(InvariantFailure::CallOutsideDefinitions {
            edge: edge.clone(),
            op_moment: first_moment(entries, edge),
            defined: defined_for(defined, &edge.from),
        });
    }
    for edge in declared.difference(defined) {
        failures.push(InvariantFailure::DeclarationOutsideDefinitions {
            edge: edge.clone(),
            defined: defined_for(defined, &edge.from),
        });
    }
    failures
}

/// Combineert een cel buiten een besluit-pad over cellen heen? (I4)
///
/// Binnen een besluit mag het: daar staat het in het decretogram, en dat is
/// precies het verschil tussen combineren en *onzichtbaar* combineren. Buiten
/// een besluit is er geen gram, dus twee bronnen bij één vrager is synthese in
/// een cel.
fn check_synthesis(traffic: &Traffic<'_>) -> Vec<InvariantFailure> {
    let mut seen: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    let mut failures = Vec::new();

    for signed in &traffic.probes {
        let asker = signed.asked_by.cell();
        let peers = seen.entry(asker).or_default();
        let is_new = peers.insert(signed.answer.cell.as_str());
        // Het moment waarop de tweede bron erbij kwam, en niet het laatste
        // contact: dat is het moment waarop de cel meer dan één cel kende, en
        // daarmee het moment dat in de melding thuishoort.
        if is_new && peers.len() == 2 {
            failures.push(InvariantFailure::SynthesisOutsideBesluit {
                cell: asker.to_string(),
                peers: peers.iter().copied().collect::<Vec<_>>().join(", "),
                op_moment: signed.answer.op_moment,
            });
        }
    }
    failures
}

/// Staat wat een besluit over de grens haalde in zijn gram, en omgekeerd?
///
/// Twee kanten van dezelfde naad:
///
/// - een contact zonder geaccepteerde waarde van die cel is combineren zonder
///   dat het gram het laat zien (I4);
/// - een geaccepteerde waarde zonder contact met die cel betekent dat het
///   contact nergens is vastgelegd, of dat de waarde ergens anders vandaan
///   kwam. Beide zijn een gat in wat de opstelling meet (I2).
fn check_visibility(
    traffic: &Traffic<'_>,
    observed: &BTreeSet<QueryEdge>,
) -> Vec<InvariantFailure> {
    let mut failures = Vec::new();
    for decision in &traffic.decisions {
        let gram = decision.decretogram;
        let accepted = gram.accepted_values();
        let peers: BTreeSet<&str> = accepted.values().copied().collect();
        let asked: BTreeSet<&str> = decision
            .crossings
            .iter()
            .map(|signed| signed.answer.cell.as_str())
            .collect();

        for signed in decision.crossings {
            if peers.contains(signed.answer.cell.as_str()) {
                continue;
            }
            failures.push(InvariantFailure::CrossingNotInDecretogram {
                edge: QueryEdge::of(signed),
                op_moment: signed.answer.op_moment,
                zaakkenmerk: gram.zaakkenmerk.clone(),
                accepted: if accepted.is_empty() {
                    "het gram noemt geen enkele geaccepteerde waarde".to_string()
                } else {
                    format!(
                        "geaccepteerd: {}",
                        accepted
                            .iter()
                            .map(|(value, cell)| format!("{value} van {cell}"))
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                },
            });
        }

        for (value, peer) in &accepted {
            if asked.contains(peer) {
                continue;
            }
            failures.push(InvariantFailure::AcceptedWithoutCrossing {
                value: (*value).to_string(),
                peer: (*peer).to_string(),
                zaakkenmerk: gram.zaakkenmerk.clone(),
                observed: listing("vastgelegd", observed),
            });
        }
    }
    failures
}

/// Het eerste moment waarop deze tak gevraagd werd.
///
/// Een tak kan meer dan één keer gesteld zijn; de melding noemt het eerste
/// voorval, want dat is het moment waarop de schending begon. Zonder contact
/// valt er niets te noemen — dat kan niet gebeuren voor een tak die uit de
/// contacten is afgeleid, en dan is de eerste dag van de jaartelling een
/// zichtbaar onzinnige uitkomst in plaats van een stille aanname.
fn first_moment(entries: &[&SignedAnswer], edge: &QueryEdge) -> NaiveDate {
    entries
        .iter()
        .find(|signed| QueryEdge::of(signed) == *edge)
        .map_or_else(NaiveDate::default, |signed| signed.answer.op_moment)
}

/// Wat de definities van één vragende cel toestaan, voor in een melding.
fn defined_for(defined: &BTreeSet<QueryEdge>, from: &str) -> String {
    let own: BTreeSet<QueryEdge> = defined
        .iter()
        .filter(|edge| edge.from == from)
        .cloned()
        .collect();
    if own.is_empty() {
        return "haar definities noemen geen enkele andere cel".to_string();
    }
    listing("haar definities vragen", &own)
}

/// Een opsomming van takken voor een melding, of de mededeling dat er geen zijn.
fn listing(label: &str, edges: &BTreeSet<QueryEdge>) -> String {
    if edges.is_empty() {
        return format!("{label}: geen enkele vraag over een celgrens");
    }
    format!(
        "{label}: {}",
        edges
            .iter()
            .map(QueryEdge::to_string)
            .collect::<Vec<_>>()
            .join("; ")
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cell::LexostatusOutcome;
    use crate::security::{Identity, SecurityContext};
    use crate::transport::CellTransport;
    use crate::Lexostatus;
    use crate::Result;
    use regelrecht_engine::Value;
    use std::collections::BTreeMap;

    /// Een transport dat elke vraag met een leeg feit beantwoordt.
    ///
    /// Genoeg voor deze tests: ze gaan over de takken van het graf, niet over
    /// wat een cel antwoordt. Een echte cel erbij halen zou een corpus vragen
    /// en de assertie niet scherper maken.
    struct AnyPeer;

    impl CellTransport for AnyPeer {
        fn query(
            &self,
            cell: &str,
            lexostatus: &str,
            _params: &BTreeMap<String, Value>,
            op_moment: NaiveDate,
        ) -> Result<Lexostatus> {
            Ok(Lexostatus {
                cell: cell.to_string(),
                name: lexostatus.to_string(),
                op_moment,
                outcome: LexostatusOutcome::Established(BTreeMap::new()),
            })
        }
    }

    fn moment(day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2024, 6, day)
            .unwrap_or_else(|| panic!("2024-06-{day} moet een geldige datum zijn"))
    }

    /// Eén bewijsstuk zoals de veiligheidscontext het aflevert.
    fn crossing(from: &str, to: &str, lexostatus: &str, day: u32) -> SignedAnswer {
        let transport = AnyPeer;
        SecurityContext::new(Identity::for_cell(from), &transport)
            .query(to, lexostatus, &BTreeMap::new(), moment(day))
            .unwrap_or_else(|e| panic!("de sonde hoort door te gaan: {e}"))
    }

    fn edge(from: &str, to: &str, lexostatus: &str) -> QueryEdge {
        QueryEdge {
            from: from.to_string(),
            to: to.to_string(),
            lexostatus: lexostatus.to_string(),
        }
    }

    fn declared(from: &str, to: &str, lexostatus: &str) -> DeclaredQuery {
        DeclaredQuery {
            doc: None,
            from: from.to_string(),
            to: to.to_string(),
            lexostatus: lexostatus.to_string(),
        }
    }

    /// Een cel die met een `accept_from` één peer mag bevragen.
    fn cell_with_accept_from() -> CellConfig {
        serde_yaml_ng::from_str(
            r"
id: toeslagen
laws: []
besluit_definitions:
  - name: toekenning
    regulation: wet_op_de_zorgtoeslag
    output: heeft_recht_op_zorgtoeslag
    zaakkenmerk: 'zorgtoeslag/{bsn}'
    params:
      - name: bsn
        type: string
    inputs:
      toetsingsinkomen:
        accept_from: belastingdienst
        lexostatus: toetsingsinkomen
        field: toetsingsinkomen
        params:
          bsn: $bsn
",
        )
        .unwrap_or_else(|e| panic!("testconfig moet parsen: {e}"))
    }

    /// Een cel die met een `accepts_from`-afspraak één peer mag bevragen.
    fn cell_with_accepts_from() -> CellConfig {
        serde_yaml_ng::from_str(
            r"
id: toeslagen
laws:
  - test_partnerschapstoets
accepts_from:
  - cell: brp
    output: partnerschap
    lexostatus: partnerschap
    field: partnerschap_type
",
        )
        .unwrap_or_else(|e| panic!("testconfig moet parsen: {e}"))
    }

    #[test]
    fn het_toegestane_graf_komt_uit_een_accept_from_input() {
        assert_eq!(
            defined_graph(&[cell_with_accept_from()]),
            BTreeSet::from([edge("toeslagen", "belastingdienst", "toetsingsinkomen")]),
            "een `accept_from`-input is een tak van het toegestane graf"
        );
    }

    #[test]
    fn het_toegestane_graf_komt_ook_uit_de_afspraken_bij_de_wetten() {
        assert_eq!(
            defined_graph(&[cell_with_accepts_from()]),
            BTreeSet::from([edge("toeslagen", "brp", "partnerschap")]),
            "een `accepts_from`-afspraak hoort er net zo goed in: daarmee komt een \
             `source.regulation` van een eigen wet bij een peer uit"
        );
    }

    #[test]
    fn een_niet_gedeclareerde_vraag_faalt_en_noemt_welke() {
        let signed = crossing("toeslagen", "brp", "partnerschap", 1);
        let traffic = Traffic {
            decisions: Vec::new(),
            probes: vec![&signed],
        };
        let failures = check_invariants(&[], &[cell_with_accepts_from()], &traffic);

        let melding = failures
            .iter()
            .find(|failure| matches!(failure, InvariantFailure::UndeclaredCall { .. }))
            .map(InvariantFailure::describe)
            .unwrap_or_else(|| {
                panic!("verwachtte een niet-gedeclareerde vraag, kreeg {failures:?}")
            });
        assert!(
            melding.contains("toeslagen") && melding.contains("brp.partnerschap"),
            "de melding hoort te zeggen welke vraag het betrof, kreeg: {melding}"
        );
        assert!(
            melding.contains("2024-06-01"),
            "en op welk moment, kreeg: {melding}"
        );
    }

    #[test]
    fn een_gedeclareerde_vraag_die_uitbleef_faalt_ook() {
        let traffic = Traffic::default();
        let failures = check_invariants(
            &[declared("toeslagen", "brp", "partnerschap")],
            &[cell_with_accepts_from()],
            &traffic,
        );
        assert!(
            failures
                .iter()
                .any(|failure| matches!(failure, InvariantFailure::CallDidNotHappen { .. })),
            "een declaratie die niets meet hoort te falen, kreeg {failures:?}"
        );
    }

    #[test]
    fn een_gedeclareerde_vraag_buiten_de_definities_faalt_ook_als_ze_gesteld_wordt() {
        // De vraag staat in het toegestane graf van het scenario, en dat is met
        // opzet: I3 gaat over wat de definities van de cel vragen, en een
        // declaratie is geen vrijbrief.
        let signed = crossing("toeslagen", "kiesraad", "zetels", 2);
        let traffic = Traffic {
            decisions: Vec::new(),
            probes: vec![&signed],
        };
        let failures = check_invariants(
            &[declared("toeslagen", "kiesraad", "zetels")],
            &[cell_with_accepts_from()],
            &traffic,
        );

        assert!(
            !failures
                .iter()
                .any(|failure| matches!(failure, InvariantFailure::UndeclaredCall { .. })),
            "de vraag ís gedeclareerd, dus dáárover hoort niets te komen: {failures:?}"
        );
        let melding = failures
            .iter()
            .find(|failure| matches!(failure, InvariantFailure::CallOutsideDefinitions { .. }))
            .map(InvariantFailure::describe)
            .unwrap_or_else(|| panic!("verwachtte een vraag buiten de definities: {failures:?}"));
        assert!(
            melding.contains("kiesraad") && melding.contains("2024-06-02"),
            "de melding hoort de actoren en het moment te noemen, kreeg: {melding}"
        );
    }

    #[test]
    fn twee_bronnen_bij_een_vrager_buiten_een_besluit_is_synthese() {
        let eerste = crossing("toeslagen", "brp", "partnerschap", 1);
        let tweede = crossing("toeslagen", "belastingdienst", "toetsingsinkomen", 3);
        let traffic = Traffic {
            decisions: Vec::new(),
            probes: vec![&eerste, &tweede],
        };
        let failures = check_synthesis(&traffic);

        let melding = failures
            .first()
            .map(InvariantFailure::describe)
            .unwrap_or_else(|| panic!("twee bronnen bij één vrager hoort te falen"));
        assert_eq!(
            failures.len(),
            1,
            "één melding per vrager, niet één per bron"
        );
        assert!(
            melding.starts_with("I4")
                && melding.contains("brp")
                && melding.contains("belastingdienst")
                && melding.contains("2024-06-03"),
            "de melding hoort I4, beide bronnen en het moment te noemen, kreeg: {melding}"
        );
    }

    #[test]
    fn een_vrager_met_een_bron_is_geen_synthese() {
        let signed = crossing("toeslagen", "brp", "partnerschap", 1);
        let traffic = Traffic {
            decisions: Vec::new(),
            probes: vec![&signed],
        };
        assert!(
            check_synthesis(&traffic).is_empty(),
            "één bron is niet combineren"
        );
    }
}
