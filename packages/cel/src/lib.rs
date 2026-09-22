//! De cel: een cel die een ingediende aanvraag vastlegt als
//! chronolexogram en er via een reductie een lexostatus van maakt.
//!
//! Vier lagen, elk met een eigen bestand (zie README.md):
//!
//! 1. het lexogram: de regelingen uit `REGULATION_PATH`, ongewijzigd;
//! 2. de stroomdefinitie: welke feiten de cel vastlegt ([`stroom`]);
//! 3. de reductie tot lexostatus: hoe de kroniek de parameters van een
//!    artikel voedt ([`reductie`]);
//! 4. het gram zelf, append-only in de kroniek ([`kroniek`]).
//!
//! Niets hier noemt een casus: stroom, lexostatus-definities en corpus komen
//! uit configuratie.

pub mod api;
pub mod config;
pub mod controle;
pub mod eherkenning;
pub mod formulier;
pub mod kroniek;
pub mod reductie;
pub mod regelingen;
pub mod schema;
pub mod stroom;
pub mod toets;
