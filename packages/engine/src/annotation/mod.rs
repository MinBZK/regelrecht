//! Stand-off note resolution (RFC-005, RFC-018).
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

/// Extract the law `$id` from a note's `target.source` URI.
///
/// `regelrecht://zorgtoeslagwet` and
/// `regelrecht://zorgtoeslagwet/hoogte_zorgtoeslag#field` both yield
/// `"zorgtoeslagwet"`. Returns `None` if the URI is not a `regelrecht://`
/// reference. Shared by the WASM bindings and `validate-annotations` so the
/// two cannot drift.
pub fn law_id_from_source(source: &str) -> Option<&str> {
    let rest = source.strip_prefix("regelrecht://")?;
    Some(rest.split('/').next().unwrap_or(rest))
}
