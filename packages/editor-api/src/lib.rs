//! Editor-API library surface.
//!
//! Only the modules needed by `tests/` are exposed here — the binary
//! (`main.rs`) re-declares the full set, so the lib does not have to
//! pull in modules whose internal items are referenced only from the
//! binary's route registration.

pub mod accounts;
pub mod config;
pub mod corpus_handlers;
pub mod crypto;
pub mod github_oauth;
pub mod github_tokens;
pub mod state;
pub mod traject_corpus;
pub mod trajects;
