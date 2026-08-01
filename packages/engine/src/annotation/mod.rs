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
/// `regelrecht://wet_op_de_zorgtoeslag` and
/// `regelrecht://wet_op_de_zorgtoeslag/hoogte_zorgtoeslag#field` both yield
/// `"wet_op_de_zorgtoeslag"`. Returns `None` if the URI is not a `regelrecht://`
/// reference. Shared by the WASM bindings and `validate-annotations` so the
/// two cannot drift.
pub fn law_id_from_source(source: &str) -> Option<&str> {
    let rest = source.strip_prefix("regelrecht://")?;
    Some(rest.split('/').next().unwrap_or(rest))
}

#[cfg(test)]
mod tests {
    use super::law_id_from_source;

    #[test]
    fn bare_law_uri_yields_the_law_id() {
        assert_eq!(
            law_id_from_source("regelrecht://wet_op_de_zorgtoeslag"),
            Some("wet_op_de_zorgtoeslag")
        );
    }

    #[test]
    fn path_and_fragment_are_stripped_from_the_law_id() {
        assert_eq!(
            law_id_from_source("regelrecht://wet_op_de_zorgtoeslag/hoogte_zorgtoeslag#field"),
            Some("wet_op_de_zorgtoeslag")
        );
    }

    #[test]
    fn non_regelrecht_uri_has_no_law_id() {
        assert_eq!(law_id_from_source("https://example.com/wet"), None);
        assert_eq!(law_id_from_source("wet_op_de_zorgtoeslag"), None);
    }

    #[test]
    fn empty_authority_yields_an_empty_law_id_rather_than_none() {
        // `regelrecht://` without a law is malformed input; it must not be
        // mistaken for "not a regelrecht reference at all".
        assert_eq!(law_id_from_source("regelrecht://"), Some(""));
    }
}
