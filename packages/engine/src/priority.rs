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
        RegulatoryLayer::GemeentelijkeVerordening => 9,
        RegulatoryLayer::Beleidsregel => 10,
        RegulatoryLayer::Uitvoeringsbeleid => 11,
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
        let best_priority = layer_rank(&best.law.regulatory_layer);
        let candidate_priority = layer_rank(&candidate.law.regulatory_layer);

        if candidate_priority < best_priority {
            // Lower number = higher authority (lex superior)
            let prev_id = best.law.id.clone();
            let prev_layer = best.law.regulatory_layer;
            best = candidate;
            reason = format!(
                "lex superior: {} ({:?}) outranks {} ({:?})",
                candidate.law.id, candidate.law.regulatory_layer, prev_id, prev_layer,
            );
        } else if candidate_priority == best_priority {
            // Same layer: compare valid_from dates (lex posterior)
            let best_date = validate_date_for_comparison(&best.law.id, &best.law.valid_from)?;
            let candidate_date =
                validate_date_for_comparison(&candidate.law.id, &candidate.law.valid_from)?;

            if candidate_date > best_date {
                let prev_id = best.law.id.clone();
                let prev_date = best_date.to_string();
                best = candidate;
                reason = format!(
                    "lex posterior: {} (valid_from {}) is newer than {} (valid_from {})",
                    candidate.law.id, candidate_date, prev_id, prev_date,
                );
            } else if candidate_date == best_date {
                // Same layer, same date: ambiguous — this is a law authoring error
                return Err(EngineError::ResolutionError(format!(
                    "Ambiguous priority: {} and {} both have regulatory layer {:?} and valid_from '{}' — cannot determine winner",
                    best.law.id, candidate.law.id, best.law.regulatory_layer, best_date
                )));
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
