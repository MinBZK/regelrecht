//! De cel-runtime: cellen die feiten vastleggen als chronolexogram en ze
//! reduceren tot een lexostatus, en processen die lexostatussen van cellen
//! samenvoegen, de engine uitvoeren en een cel laten vastleggen.
//!
//! Cel en proces zijn configuratie, geen code: een map onder `CELLS_PATH` met
//! een `cel.yaml`, en een map onder `PROCESSES_PATH` met een `proces.yaml`
//! (zie README.md). Het lexogram (1) is van niemand; een cel heeft de lagen
//! 2 tot en met 4, elk met een eigen bestand:
//!
//! 1. het lexogram: de regelingen uit `REGULATION_PATH`, ongewijzigd en
//!    gedeeld door alle cellen;
//! 2. de stroomdefinitie: welke feiten de cel vastlegt ([`stroom`]);
//! 3. de reductie tot lexostatus: hoe de kroniek de parameters van een
//!    artikel voedt ([`reductie`]);
//! 4. het gram zelf, append-only in de kroniek ([`kroniek`]).
//!
//! Het proces ([`proces`]) staat daarboven: het informeert (de synthese bij
//! de afnemer, [`synthese`], en de toets), concludeert (het besluit,
//! [`besluit`]) en vraagt een cel vast te leggen, langs dezelfde routes als
//! elke afnemer ([`transport`]). De runtime ([`runtime`]) draait beide. Niets
//! hier noemt een casus.

pub mod api;
pub mod besluit;
pub mod cel;
pub mod config;
pub mod controle;
pub mod eherkenning;
pub mod formulier;
pub mod kroniek;
pub mod mogelijkheid;
pub mod proces;
pub mod reductie;
pub mod regelingen;
pub mod rijen;
pub mod runtime;
pub mod schema;
pub mod sessie;
pub mod startstand;
pub mod stroom;
pub mod synthese;
pub mod toets;
pub mod transport;
pub mod voorbeelden;
