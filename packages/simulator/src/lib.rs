//! Testopstelling voor chronolexografie (RFC-022).
//!
//! Deze crate bouwt een gesimuleerde wereld van *cellen*. Een cel houdt haar
//! eigen kronieken, laadt haar eigen wetten en reduceert daarover tot een
//! *lexostatus*. Wat een cel niet is: ze houdt geen sleutels, ze heeft geen
//! bevoegd gezag en ze combineert niets over cellen heen (RFC-022 §2, §4.1).
//! Wetten zijn optioneel: een cel met `laws: []` is een **bron-cel** die
//! vastlegt en reduceert zonder engine, en dat is de toets dat het
//! lexostatus-contract engine-onafhankelijk is.
//!
//! De grens tussen cellen is een taalgrens en geen afspraak: een cel bezit haar
//! [`ChronicleStore`] privé, en [`Cell::reduce`] is de enige publieke ingang voor
//! een consument. Gaat een vraag over een celgrens, dan loopt hij langs
//! [`SecurityContext`] (identiteit en ondertekening) naar [`CellTransport`] (de
//! naad: in-process nu, HTTP later). Een cel houdt zelf geen van beide.
//!
//! Het [`observation`]-log is het meetinstrument daarnaast: test-only, passief en
//! met opzet buiten de band, want een observator die álles ziet kent wél het
//! totaalbeeld waarvan deze opstelling zegt dat het nergens bestaat.
//!
//! Tijd hoort bij de [`World`] en niet bij een vraag: één logische klok per
//! wereld, nooit de wandklok, en [`World::advance`] laat de tijd lopen zodat
//! triggers op hun eigen moment vastleggen.
//!
//! De lus is rond: een cel kan ook **besluiten**. [`World::decide`] laat haar een
//! eigen regeling uitvoeren en legt het resultaat vast als [`Decretogram`] — het
//! RFC-013 Execution Receipt plus moment en zaakkenmerk — in haar eigen kroniek.
//! Terugzien doe je met een gewone reductie over die kroniek: ligt er geen
//! decretogram, dan is het antwoord "niets vastgesteld" en nooit een
//! herberekening onder een inmiddels andere wetsversie.
//!
//! Een besluit mag **accepteren**: heeft het een waarde nodig die een andere
//! organisatie vaststelt, dan wordt die opgehaald in plaats van nagerekend, en ze
//! landt met haar herkomst in het decretogram — bron-cel, lexostatus, moment en
//! ondertekening (invariant I5). Twee wegen, één mechanisme: `accept_from` in de
//! besluit-definitie, of een `source.regulation` in de wet die een cel aanwijst
//! (tier 3 van RFC-022 §4.2). Beide lopen langs de veiligheidscontext en het
//! transport; de reduce-engine kan geen van beide, en dat is een ontbrekende
//! capability en geen afspraak.
//!
//! Een besluit laat iets achter dat later moet gebeuren. Een [`ObligationDue`]
//! is één termijn van een verplichting uit een decretogram; [`World::advance`]
//! laat haar op de vervaldatum nakomen, en dan legt de betalende cel een
//! betaling vast en de besluitende cel dat het haar gemeld is. Wat er betaald is,
//! is daarna een **reductie** over die vastleggingen (`sum: bedrag`) en geen
//! saldo dat ernaast wordt bijgehouden.
//!
//! ```no_run
//! use regelrecht_simulator::{regulation_root, Scenario};
//! use std::path::Path;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let scenario = Scenario::load(Path::new("scenarios/toeslagen_zorgtoeslag.yaml"))?;
//! let run = scenario.run(&regulation_root())?;
//! println!("{}", run.report());
//! # Ok(())
//! # }
//! ```

#![deny(missing_docs)]

mod accept;
pub mod cell;
mod corpus;
pub mod error;
// Meetinstrument, geen onderdeel van de opstelling: zie de moduledocs. Met opzet
// geen re-export hieronder — wie hem gebruikt, noemt hem bij zijn volle naam, en
// geen ander bestand in `src/` mag dat doen.
pub mod observation;
pub mod scenario;
pub mod security;
pub mod transport;
mod values;
pub mod world;

pub use cell::{
    AcceptanceRequest, AcceptedSource, Aggregate, BesluitDefinition, BesluitInput, Cell,
    CellConfig, ChronicleEvent, ChronicleStore, ChronicleStream, Decretogram, DecretogramInput,
    DocumentedParameter, InputOrigin, Intake, Lexostatus, LexostatusDefinition, LexostatusOutcome,
    ObligationDefinition, ObligationDue, ParameterType, Reduction, Schedule, BESCHIKKINGEN,
    BETALINGEN,
};
pub use corpus::regulation_root;
pub use error::{Result, SimulatorError, Subject};
pub use scenario::{
    check_provenance, Decision, DecisionOutcome, ExpectationFailure, Query, QueryOutcome, Scenario,
    ScenarioRun, TransportOutcome, TransportQuery,
};
pub use security::{Identity, SecurityContext, Signature, SignedAnswer};
pub use transport::{CellTransport, InProcessTransport};
pub use world::{Clock, DecisionRecord, Fixture, Recording, World};
