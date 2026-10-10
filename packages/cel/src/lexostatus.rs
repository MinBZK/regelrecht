//! Reading a cell's own chronicle back at a moment: the grams that hold
//! then. A lexostatus is what the cell reads from it for a case: what the
//! application it decides on says, as the law describes the application
//! (see [`crate::Cell::lexostatuses`]), or an article in the policy of the
//! holder that reads the chronicle as a register ([`crate::register`]).
//!
//! A reading is the state at a moment (`peilmoment`, RFC-050): a gram counts
//! when it legally holds by then (`effective_at` on or before the moment), and
//! the grams are in the order they legally hold, then the order the cell
//! recorded them, so a fact recorded late about an earlier moment is not the
//! latest state.

use chrono::{DateTime, FixedOffset};

use crate::chronicle::{Chronicle, Gram};
use crate::error::{setup, Result};

/// A moment of a gram, as the chronicle holds it (RFC 3339).
pub(crate) fn moment(gram: &Gram, value: &str) -> Result<DateTime<FixedOffset>> {
    DateTime::parse_from_rfc3339(value)
        .map_err(|e| setup(format!("gram '{}': moment '{value}': {e}", gram.id)))
}

/// The grams of `chronicle` that hold at `as_of`, in the order they hold:
/// by `effective_at`, then by `recorded_at`, then in recording order.
pub(crate) fn in_force(chronicle: &Chronicle, as_of: DateTime<FixedOffset>) -> Result<Vec<&Gram>> {
    let mut grams = Vec::new();
    for gram in chronicle.grams() {
        let effective = moment(gram, &gram.effective_at)?;
        if effective <= as_of {
            grams.push((effective, moment(gram, &gram.recorded_at)?, gram));
        }
    }
    // A stable sort keeps the recording order between equal moments.
    grams.sort_by_key(|(effective, recorded, _)| (*effective, *recorded));
    Ok(grams.into_iter().map(|(_, _, g)| g).collect())
}
