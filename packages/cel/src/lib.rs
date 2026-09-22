//! De cel-runtime: cellen die feiten vastleggen als chronolexogram, ze
//! reduceren tot een lexostatus, en lexostatussen van andere cellen
//! samenvoegen.
//!
//! Een cel is configuratie, geen code: een map onder `CELLS_PATH` met een
//! `cel.yaml` (zie README.md). Per cel vier lagen, elk met een eigen bestand:
//!
//! 1. het lexogram: de regelingen uit `REGULATION_PATH`, ongewijzigd en
//!    gedeeld door alle cellen;
//! 2. de stroomdefinitie: welke feiten de cel vastlegt ([`stroom`]);
//! 3. de reductie tot lexostatus: hoe de kroniek de parameters van een
//!    artikel voedt ([`reductie`]);
//! 4. het gram zelf, append-only in de kroniek ([`kroniek`]).
//!
//! Daarboven: de runtime ([`runtime`]), het transport tussen cellen
//! ([`transport`]) en de synthese bij de afnemer ([`synthese`]). Niets hier
//! noemt een casus.

pub mod api;
pub mod besluit;
pub mod cel;
pub mod config;
pub mod controle;
pub mod eherkenning;
pub mod formulier;
pub mod kroniek;
pub mod reductie;
pub mod regelingen;
pub mod runtime;
pub mod schema;
pub mod sessie;
pub mod startstand;
pub mod stroom;
pub mod synthese;
pub mod toets;
pub mod transport;
