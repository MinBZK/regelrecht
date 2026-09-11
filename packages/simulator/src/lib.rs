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
    Cell, CellConfig, ChronicleEvent, ChronicleStore, ChronicleStream, Intake, Lexostatus,
    LexostatusDefinition, LexostatusInput, LexostatusOutcome, ParameterType, Reduction,
};
pub use corpus::regulation_root;
pub use error::{Result, SimulatorError};
pub use scenario::{
    ExpectationFailure, Query, QueryOutcome, Scenario, ScenarioRun, TransportOutcome,
    TransportQuery,
};
pub use security::{Identity, SecurityContext, Signature, SignedAnswer};
pub use transport::{CellTransport, InProcessTransport};
pub use world::{Clock, Fixture, Recording, World};
