//! The parts of the enrichment flow that need no model: the checks, the
//! capability plan, the closure, the reference graph, the closing pass.
//!
//! It began as a second flow beside the existing one, so that two chains
//! could be compared over the same laws. That is not what it is any more and
//! the name is the only thing left of it. `enrich.rs` drives one chain and
//! calls into this module at some twenty places; there is no second chain to
//! hold it against. A reader who takes that first paragraph at face value
//! goes looking for a lane that does not exist.
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
pub mod closure;
pub mod context;
pub mod reconcile;
pub mod refgraph;
pub mod source_gate;
