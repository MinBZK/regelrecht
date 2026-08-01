//! A second enrichment flow, built beside the existing one rather than
//! replacing it.
//!
//! `enrich.rs` stays untouched: it drives the `enrich/claude` and
//! `enrich/opencode` lanes that are running today. This module carries a
//! separate chain with its own skills and its own checks, so the two can be
//! compared over the same laws instead of over different sweeps.
//!
//! The design constraint that shapes everything here is what the current
//! flow ran into: the agent is spawned without `Bash`, `WebFetch` and
//! `WebSearch`, so it cannot validate, cannot test, and cannot retrieve.
//! Every instruction in this flow is therefore written for an agent that
//! only reads what the worker put in front of it. The worker fetches, the
//! worker validates, the worker checks. The agent reads and writes.
//!
//! [`checks`] is the part that needs no model at all and runs today over any
//! law file, enriched by either flow. [`capabilities`] is the other half of
//! that constraint: it compares what a step needs against what the runtime
//! grants, so an instruction the agent cannot carry out is left out of the
//! prompt instead of being answered with an invention.

pub mod assemble;
pub mod capabilities;
pub mod checks;
pub mod context;
pub mod source_gate;
