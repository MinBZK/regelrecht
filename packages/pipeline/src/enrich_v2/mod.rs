//! The parts of the enrichment flow that need no model: the checks, the
//! source gate, the assembler, the reference graph and the context brief.
//!
//! The design constraint that shapes everything here is what the current
//! flow ran into: the agent is spawned without `Bash`, `WebFetch` and
//! `WebSearch`, so it cannot validate, cannot test, and cannot retrieve.
//! Every instruction in this flow is therefore written for an agent that
//! only reads what the worker put in front of it. The worker fetches, the
//! worker validates, the worker checks. The agent reads and writes.
//!
//! [`checks`] is the part that needs no model at all and runs today over any
//! law file, through the `law-check` binary. The worker does not call into
//! this module yet; that wiring, with the capability plan and the closing
//! pass, follows separately.

pub mod assemble;
pub mod checks;
pub mod context;
pub mod refgraph;
pub mod source_gate;
