//! Stand-off note resolution (RFC-005, RFC-016).
//!
//! Notes anchor to legal text via a W3C [`TextQuoteSelector`]: an exact quote
//! plus optional prefix/suffix context. The selector is content-addressed, so a
//! note resolves on any law version where the text exists, surviving article
//! renumbering and minor textual changes (via fuzzy matching).
//!
//! See [`crate::annotation::resolver`] for the resolution algorithm.

pub mod resolver;
pub mod types;

pub use resolver::resolve;
pub use types::{MatchResult, MatchStatus, SelectorHint, TextMatch, TextQuoteSelector};
