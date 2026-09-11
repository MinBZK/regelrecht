//! The cell tier of source resolution (RFC-022 §4.2).
//!
//! `source.regulation` names whoever produces a value. Usually that is a
//! regulation the engine loaded and evaluates itself (RFC-007). It can also
//! be another organisation that publishes a reduction over its own facts and
//! answers a query for it.
//!
//! The engine knows nothing about that second case beyond this trait: not what
//! such a producer is, not how it is addressed, not how the query travels, not
//! who signed the answer. It knows only that the value did not come from a
//! regulation it ran, so it was *accepted* rather than recomputed — which is
//! why it lands in the receipt's `accepted_values` (RFC-013) and never in the
//! data-source register.
//!
//! An engine without a resolver has no cell tier at all. That is a capability
//! the caller grants, not a rule the caller is trusted to keep: a
//! configuration that must not reach outside is one that was never handed a
//! [`CellResolver`].

use std::collections::BTreeMap;

use crate::error::Result;
use crate::types::Value;

/// Resolves a `source.regulation` that names a cell instead of a loaded
/// regulation (tier 3 of RFC-022 §4.2).
///
/// Registered on the service with the cell ids it answers for
/// ([`LawExecutionService::set_cell_resolver`](crate::LawExecutionService::set_cell_resolver)).
/// Only those ids reach the resolver; anything else is a regulation or an
/// error, so a cell can never be queried by accident.
pub trait CellResolver {
    /// Answer `output` for `cell_id` as of `reference_date`.
    ///
    /// # Arguments
    /// * `cell_id` — the value of `source.regulation`, one of the ids this
    ///   resolver was registered for.
    /// * `output` — the requested output, `source.output` or, when that is
    ///   absent, the input's own name.
    /// * `parameters` — the arguments built from `source.parameters`, exactly
    ///   as a cross-law call would receive them.
    /// * `reference_date` — the date the execution reasons about
    ///   (`YYYY-MM-DD`), the same one that selects regulation versions.
    ///
    /// # Returns
    /// `Ok(Some(value))` when the cell answered, `Ok(None)` when it has no
    /// answer for this question (the engine then reports an unresolvable
    /// source, step 4 of the order), `Err` when the query itself failed.
    fn resolve(
        &self,
        cell_id: &str,
        output: &str,
        parameters: &BTreeMap<String, Value>,
        reference_date: &str,
    ) -> Result<Option<Value>>;
}
