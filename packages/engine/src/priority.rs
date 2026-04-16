//! Priority resolution for competing implementations.
//!
//! When multiple regulations implement the same open term, the engine
//! must pick a winner. This module provides the resolution logic:
//!
//! 1. **Lex superior**: Higher regulatory layers take precedence
//! 2. **Lex posterior**: Among equal layers, later effective dates win
//!
//! ## TODO: Resolve `#` internal references at load time
//!
//! Laws may declare `valid_from: '#datum_inwerkingtreding'` — an internal
//! reference to an article output. Currently, priority comparison rejects
//! these with an error. The proper fix is to resolve `#` references when
//! loading laws into the resolver, so that `valid_from` always contains a
//! concrete date by the time priority comparison runs.

use crate::article::ArticleBasedLaw;
use crate::error::EngineError;
use crate::types::RegulatoryLayer;

type Result<T> = std::result::Result<T, EngineError>;

/// Returns the priority rank for a regulatory layer (lower = higher authority).
///
/// The hierarchy follows the Dutch legal system:
/// - International/EU law at the top
/// - Constitution
/// - Formal law
/// - Delegated legislation (AMvB, ministerial regulation)
/// - Policy rules and local ordinances at the bottom
#[must_use]
pub fn layer_rank(layer: &RegulatoryLayer) -> u8 {
    match layer {
        RegulatoryLayer::Verdrag => 0,
        RegulatoryLayer::EuVerordening => 1,
        RegulatoryLayer::EuRichtlijn => 2,
        RegulatoryLayer::Grondwet => 3,
        RegulatoryLayer::Wet => 4,
        RegulatoryLayer::KoninklijkBesluit => 5,
        RegulatoryLayer::Amvb => 6,
        RegulatoryLayer::MinisterieleRegeling => 7,
        RegulatoryLayer::ProvincialeVerordening => 8,
        RegulatoryLayer::WaterschapsVerordening => 9,
        RegulatoryLayer::GemeentelijkeVerordening => 10,
        RegulatoryLayer::Beleidsregel => 11,
        RegulatoryLayer::Uitvoeringsbeleid => 12,
    }
}

/// A candidate implementation with its source law and article number.
pub struct Candidate<'a> {
    pub law: &'a ArticleBasedLaw,
    pub article_number: String,
}

/// Validate that a law's `valid_from` is a concrete YYYY-MM-DD date suitable
/// for lex posterior comparison. Rejects missing dates and unresolved `#`
/// internal references.
fn validate_date_for_comparison<'a>(
    law_id: &str,
    valid_from: &'a Option<String>,
) -> Result<&'a str> {
    let date = valid_from.as_deref().ok_or_else(|| {
        EngineError::ResolutionError(format!(
            "Cannot resolve priority: law '{law_id}' has no valid_from date — \
             lex posterior comparison requires valid_from on all candidates"
        ))
    })?;

    if date.starts_with('#') {
        return Err(EngineError::ResolutionError(format!(
            "Cannot resolve priority: law '{law_id}' has unresolved valid_from \
             reference '{date}' — internal references must be resolved before \
             priority comparison"
        )));
    }

    Ok(date)
}

/// Compare two laws by priority: lex superior (layer rank), then lex posterior (valid_from).
///
/// Returns `Ordering::Greater` if `a` outranks `b`, `Ordering::Less` if `b` outranks `a`.
/// Returns `Err` if both have the same layer and date (ambiguous).
pub fn compare_law_priority(
    a: &ArticleBasedLaw,
    b: &ArticleBasedLaw,
) -> Result<std::cmp::Ordering> {
    let a_rank = layer_rank(&a.regulatory_layer);
    let b_rank = layer_rank(&b.regulatory_layer);

    if a_rank < b_rank {
        return Ok(std::cmp::Ordering::Greater); // a has higher authority
    }
    if a_rank > b_rank {
        return Ok(std::cmp::Ordering::Less); // b has higher authority
    }

    // Same layer: compare valid_from dates (lex posterior)
    let a_date = validate_date_for_comparison(&a.id, &a.valid_from)?;
    let b_date = validate_date_for_comparison(&b.id, &b.valid_from)?;

    if a_date > b_date {
        Ok(std::cmp::Ordering::Greater)
    } else if a_date < b_date {
        Ok(std::cmp::Ordering::Less)
    } else {
        Err(EngineError::ResolutionError(format!(
            "Ambiguous priority: '{}' and '{}' both have regulatory layer {:?} and valid_from '{}' \
             — cannot determine winner",
            a.id, b.id, a.regulatory_layer, a_date
        )))
    }
}

/// Pick the winning candidate from a list of implementations.
///
/// Resolution rules:
/// 1. Lex superior: the candidate from the highest regulatory layer wins
/// 2. Lex posterior: among candidates at the same layer, the one with the
///    latest `valid_from` date wins
///
/// Returns `None` if the candidate list is empty.
/// Returns `Some((winner, reason))` with a human-readable reason string.
/// Returns `Err` if two candidates have the same layer and date (ambiguous).
pub fn resolve_candidate<'a>(
    candidates: &[Candidate<'a>],
) -> Result<Option<(&'a ArticleBasedLaw, String)>> {
    if candidates.is_empty() {
        return Ok(None);
    }

    let mut best = &candidates[0];
    let mut reason = format!("only candidate ({})", best.law.id);

    for candidate in &candidates[1..] {
        match compare_law_priority(candidate.law, best.law)? {
            std::cmp::Ordering::Greater => {
                let prev_id = best.law.id.clone();
                let best_rank = layer_rank(&best.law.regulatory_layer);
                let cand_rank = layer_rank(&candidate.law.regulatory_layer);
                best = candidate;
                reason = if cand_rank < best_rank {
                    format!(
                        "lex superior: {} ({:?}) outranks {} ({:?})",
                        candidate.law.id,
                        candidate.law.regulatory_layer,
                        prev_id,
                        best.law.regulatory_layer,
                    )
                } else {
                    format!(
                        "lex posterior: {} (valid_from {}) is newer than {} (valid_from {})",
                        candidate.law.id,
                        candidate.law.valid_from.as_deref().unwrap_or("?"),
                        prev_id,
                        best.law.valid_from.as_deref().unwrap_or("?"),
                    )
                };
            }
            std::cmp::Ordering::Less | std::cmp::Ordering::Equal => {
                // Equal is unreachable: compare_law_priority returns Err for ambiguous
            }
        }
    }

    Ok(Some((best.law, reason)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_layer_priority_ordering() {
        // International > Constitution > Law > AMvB > Ministerial > Policy
        assert!(layer_rank(&RegulatoryLayer::Verdrag) < layer_rank(&RegulatoryLayer::Grondwet));
        assert!(layer_rank(&RegulatoryLayer::Grondwet) < layer_rank(&RegulatoryLayer::Wet));
        assert!(layer_rank(&RegulatoryLayer::Wet) < layer_rank(&RegulatoryLayer::Amvb));
        assert!(
            layer_rank(&RegulatoryLayer::Amvb) < layer_rank(&RegulatoryLayer::MinisterieleRegeling)
        );
        assert!(
            layer_rank(&RegulatoryLayer::MinisterieleRegeling)
                < layer_rank(&RegulatoryLayer::Beleidsregel)
        );
        assert!(
            layer_rank(&RegulatoryLayer::ProvincialeVerordening)
                < layer_rank(&RegulatoryLayer::WaterschapsVerordening)
        );
        assert!(
            layer_rank(&RegulatoryLayer::WaterschapsVerordening)
                < layer_rank(&RegulatoryLayer::GemeentelijkeVerordening)
        );
    }

    #[test]
    fn test_eu_law_outranks_national() {
        assert!(layer_rank(&RegulatoryLayer::EuVerordening) < layer_rank(&RegulatoryLayer::Wet));
        assert!(layer_rank(&RegulatoryLayer::EuRichtlijn) < layer_rank(&RegulatoryLayer::Wet));
    }

    #[test]
    fn test_resolve_candidate_empty() {
        let candidates: Vec<Candidate> = vec![];
        assert!(resolve_candidate(&candidates).unwrap().is_none());
    }

    #[test]
    fn test_resolve_candidate_single() {
        let law = ArticleBasedLaw::from_yaml_str(
            r#"
$id: test_regulation
regulatory_layer: MINISTERIELE_REGELING
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Test
"#,
        )
        .unwrap();

        let candidates = vec![Candidate {
            law: &law,
            article_number: "1".to_string(),
        }];

        let (winner, reason) = resolve_candidate(&candidates).unwrap().unwrap();
        assert_eq!(winner.id, "test_regulation");
        assert!(reason.contains("only candidate"));
    }

    #[test]
    fn test_resolve_candidate_lex_superior() {
        let wet = ArticleBasedLaw::from_yaml_str(
            r#"
$id: higher_law
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Higher law article
"#,
        )
        .unwrap();

        let regeling = ArticleBasedLaw::from_yaml_str(
            r#"
$id: lower_regulation
regulatory_layer: MINISTERIELE_REGELING
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Lower regulation article
"#,
        )
        .unwrap();

        let candidates = vec![
            Candidate {
                law: &regeling,
                article_number: "1".to_string(),
            },
            Candidate {
                law: &wet,
                article_number: "1".to_string(),
            },
        ];

        let (winner, reason) = resolve_candidate(&candidates).unwrap().unwrap();
        assert_eq!(winner.id, "higher_law");
        assert!(reason.contains("lex superior"));
    }

    #[test]
    fn test_resolve_candidate_lex_posterior() {
        let older = ArticleBasedLaw::from_yaml_str(
            r#"
$id: older_regulation
regulatory_layer: MINISTERIELE_REGELING
publication_date: '2024-01-01'
valid_from: '2024-01-01'
articles:
  - number: '1'
    text: Older regulation
"#,
        )
        .unwrap();

        let newer = ArticleBasedLaw::from_yaml_str(
            r#"
$id: newer_regulation
regulatory_layer: MINISTERIELE_REGELING
publication_date: '2025-01-01'
valid_from: '2025-01-01'
articles:
  - number: '1'
    text: Newer regulation
"#,
        )
        .unwrap();

        let candidates = vec![
            Candidate {
                law: &older,
                article_number: "1".to_string(),
            },
            Candidate {
                law: &newer,
                article_number: "1".to_string(),
            },
        ];

        let (winner, reason) = resolve_candidate(&candidates).unwrap().unwrap();
        assert_eq!(winner.id, "newer_regulation");
        assert!(reason.contains("lex posterior"));
    }

    #[test]
    fn test_resolve_candidate_missing_valid_from_is_error() {
        let without_date = ArticleBasedLaw::from_yaml_str(
            r#"
$id: no_date_regulation
regulatory_layer: MINISTERIELE_REGELING
publication_date: '2024-01-01'
articles:
  - number: '1'
    text: No valid_from
"#,
        )
        .unwrap();

        let with_date = ArticleBasedLaw::from_yaml_str(
            r#"
$id: dated_regulation
regulatory_layer: MINISTERIELE_REGELING
publication_date: '2025-01-01'
valid_from: '2025-01-01'
articles:
  - number: '1'
    text: Has valid_from
"#,
        )
        .unwrap();

        let candidates = vec![
            Candidate {
                law: &without_date,
                article_number: "1".to_string(),
            },
            Candidate {
                law: &with_date,
                article_number: "1".to_string(),
            },
        ];

        let err = resolve_candidate(&candidates).unwrap_err();
        assert!(
            err.to_string().contains("no valid_from date"),
            "Expected error about missing valid_from, got: {}",
            err
        );
    }

    #[test]
    fn test_resolve_candidate_unresolved_reference_is_error() {
        let with_ref = ArticleBasedLaw::from_yaml_str(
            r#"
$id: ref_regulation
regulatory_layer: MINISTERIELE_REGELING
publication_date: '2025-01-01'
valid_from: '#datum_inwerkingtreding'
articles:
  - number: '1'
    text: Has unresolved reference
"#,
        )
        .unwrap();

        let with_date = ArticleBasedLaw::from_yaml_str(
            r#"
$id: dated_regulation
regulatory_layer: MINISTERIELE_REGELING
publication_date: '2025-01-01'
valid_from: '2025-01-01'
articles:
  - number: '1'
    text: Has valid_from
"#,
        )
        .unwrap();

        let candidates = vec![
            Candidate {
                law: &with_ref,
                article_number: "1".to_string(),
            },
            Candidate {
                law: &with_date,
                article_number: "1".to_string(),
            },
        ];

        let err = resolve_candidate(&candidates).unwrap_err();
        assert!(
            err.to_string().contains("unresolved valid_from reference"),
            "Expected error about unresolved reference, got: {}",
            err
        );
    }
}
