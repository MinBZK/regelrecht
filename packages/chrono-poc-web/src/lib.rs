//! De HTTP-laag om de chronolexografie-simulator: één wereld per browsersessie,
//! als JSON, achter de login van RegelRecht.
//!
//! Wat deze crate toevoegt aan [`regelrecht_simulator`] is precies één ding: dat
//! een mens de opstelling kan bespelen. De simulator kent geen HTTP, geen sessie
//! en geen casus; hier komt daar een server om, en verder niets — elke route
//! hieronder is één aanroep op [`regelrecht_simulator::World`], en er wordt
//! nergens iets bijgehouden dat niet in een cel ligt.
//!
//! Drie keuzes bepalen de vorm:
//!
//! 1. **Geen database.** De sessies staan in geheugen ([`tower_sessions`]
//!    `MemoryStore`) en de werelden ook. Dat betekent één replica en een
//!    herstart die alles wist, en dat is voor een opstelling om iets aan te tonen
//!    de juiste ruil: een wereld is een gedachte-experiment en geen dossier.
//! 2. **De casus zit niet in de code.** Het wereldbestand en de regelingen komen
//!    bij het starten uit een bron die de omgeving noemt (zie [`sources`]) — een
//!    pad op schijf of een repo met een token. Het image dat hieruit rolt bevat
//!    dus geen casusinhoud, en een andere casus is een andere env-variabele.
//! 3. **Een wereld per thread.** Een [`regelrecht_simulator::World`] is niet
//!    `Send`, dus hij kan niet in de state van een axum-app liggen. Elke sessie
//!    krijgt daarom een eigen thread die haar wereld bezit; zie [`worlds`] voor
//!    waarom dat niet alleen een omweg is maar ook de nette uitkomst.
//!
//! Het opstarten faalt luid: geen wereldbestand, geen leesbare wereld of geen
//! regelingen betekent dat het proces stopt met de reden. Een server die opkomt
//! zonder wereld zou op elke healthcheck groen staan en op elk verzoek dezelfde
//! fout geven.

#![deny(missing_docs)]

pub mod api;
pub mod config;
pub mod error;
pub mod sources;
pub mod state;
pub mod worlds;

pub use api::router;
pub use config::AppConfig;
pub use error::ApiError;
pub use sources::Source;
pub use state::AppState;
pub use worlds::WorldRegistry;
