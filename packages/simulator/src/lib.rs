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
//! Deze eerste versie kent één cel per scenario en geen verkeer tussen cellen.
//! De grens ligt er wel al: een cel bezit haar [`ChronicleStore`] privé, en
//! [`Cell::reduce`] is de enige publieke ingang voor een consument.
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
pub mod scenario;
mod values;

pub use cell::{
    Cell, CellConfig, ChronicleEvent, ChronicleStore, ChronicleStream, Lexostatus,
    LexostatusDefinition, LexostatusInput, LexostatusOutcome, ParameterType, Reduction,
};
pub use corpus::regulation_root;
pub use error::{Result, SimulatorError};
pub use scenario::{ExpectationFailure, Query, QueryOutcome, Scenario, ScenarioRun};
