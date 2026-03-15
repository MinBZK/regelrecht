//! Priority resolution for competing implementations.
//!
//! When multiple regulations implement the same open term, the engine
//! must pick a winner. This module provides the resolution logic:
//!
//! 1. **Lex superior**: Higher regulatory layers take precedence
//! 2. **Lex posterior**: Among equal layers, later effective dates win

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
fn layer_priority(layer: &RegulatoryLayer) -> u8 {
    match layer {
        RegulatoryLayer::Verdrag => 0,
        RegulatoryLayer::EuVerordening => 1,
        RegulatoryLayer::EuRichtlijn => 2,
        RegulatoryLayer::Grondwet => 3,
        RegulatoryLayer::Wet => 4,
        RegulatoryLayer::Amvb => 5,
        RegulatoryLayer::MinisterieleRegeling => 6,
        RegulatoryLayer::ProvincialeVerordening => 7,
        RegulatoryLayer::GemeentelijkeVerordening => 8,
        RegulatoryLayer::Beleidsregel => 9,
        RegulatoryLayer::Uitvoeringsbeleid => 10,
    }
}

/// A candidate implementation with its source law and article number.
pub struct Candidate<'a> {
    pub law: &'a ArticleBasedLaw,
    pub article_number: String,
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
        let best_priority = layer_priority(&best.law.regulatory_layer);
        let candidate_priority = layer_priority(&candidate.law.regulatory_layer);

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
            let best_date = best.law.valid_from.as_deref().ok_or_else(|| {
                EngineError::ResolutionError(format!(
                    "Cannot resolve priority: law '{}' has no valid_from date — \
                     lex posterior comparison requires valid_from on all candidates",
                    best.law.id
                ))
            })?;
            let candidate_date = candidate.law.valid_from.as_deref().ok_or_else(|| {
                EngineError::ResolutionError(format!(
                    "Cannot resolve priority: law '{}' has no valid_from date — \
                     lex posterior comparison requires valid_from on all candidates",
                    candidate.law.id
                ))
            })?;

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
        assert!(
            layer_priority(&RegulatoryLayer::Verdrag) < layer_priority(&RegulatoryLayer::Grondwet)
        );
        assert!(layer_priority(&RegulatoryLayer::Grondwet) < layer_priority(&RegulatoryLayer::Wet));
        assert!(layer_priority(&RegulatoryLayer::Wet) < layer_priority(&RegulatoryLayer::Amvb));
        assert!(
            layer_priority(&RegulatoryLayer::Amvb)
                < layer_priority(&RegulatoryLayer::MinisterieleRegeling)
        );
        assert!(
            layer_priority(&RegulatoryLayer::MinisterieleRegeling)
                < layer_priority(&RegulatoryLayer::Beleidsregel)
        );
    }

    #[test]
    fn test_eu_law_outranks_national() {
        assert!(
            layer_priority(&RegulatoryLayer::EuVerordening) < layer_priority(&RegulatoryLayer::Wet)
        );
        assert!(
            layer_priority(&RegulatoryLayer::EuRichtlijn) < layer_priority(&RegulatoryLayer::Wet)
        );
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
}
