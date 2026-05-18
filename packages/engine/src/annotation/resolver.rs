//! TextQuoteSelector resolution algorithm (RFC-005, RFC-016).
//!
//! Resolution order:
//! 1. If a hint is present, try the hinted article first; on failure fall
//!    through to a full search (the hint is non-authoritative).
//! 2. Exact match: locate `prefix + exact + suffix` as a substring, with
//!    whitespace-tolerant prefix/suffix checks.
//! 3. Fuzzy match: a sliding window over the text scored
//!    `exact*0.5 + prefix*0.25 + suffix*0.25` using normalised Levenshtein
//!    similarity, keeping candidates at or above the threshold (0.7).
//! 4. One match → [`MatchStatus::Found`], several equally-good →
//!    [`MatchStatus::Ambiguous`], none → [`MatchStatus::Orphaned`].
//!
//! Ported from the Python proof-of-concept on the `feature/annotation-resolver`
//! branch; the Rust port is the single source of truth (it also runs in the
//! browser via WASM).

use crate::annotation::types::{MatchResult, SelectorHint, TextMatch, TextQuoteSelector};
use crate::article::Article;

/// Default minimum weighted score for a fuzzy match to count.
pub const DEFAULT_FUZZY_THRESHOLD: f64 = 0.7;

/// Allowed window-length variation around `exact.len()` when scanning for
/// fuzzy candidates (30%).
const WINDOW_TOLERANCE: f64 = 0.3;

/// Margin by which the best fuzzy match must beat the second-best to be
/// treated as unambiguous.
const TIEBREAK_MARGIN: f64 = 0.1;

/// Resolve `selector` against the articles of a law.
///
/// Article numbers on the returned matches identify where the text was found.
/// A present hint is tried first but never overrides a full-text search.
pub fn resolve(selector: &TextQuoteSelector, articles: &[Article]) -> MatchResult {
    resolve_with_threshold(selector, articles, DEFAULT_FUZZY_THRESHOLD)
}

/// [`resolve`] with an explicit fuzzy threshold (used by tests).
pub fn resolve_with_threshold(
    selector: &TextQuoteSelector,
    articles: &[Article],
    threshold: f64,
) -> MatchResult {
    if let Some(hint) = &selector.hint {
        let hinted = resolve_hint(selector, articles, threshold, hint);
        if hinted.is_found() {
            return hinted;
        }
        // Hint failed: fall through to a full search.
    }

    // Exact match across all articles.
    let mut exact: Vec<TextMatch> = Vec::new();
    for article in articles {
        for mut m in find_exact_matches(&article.text, selector) {
            m.article_number = article.number.clone();
            exact.push(m);
        }
    }
    if !exact.is_empty() {
        return if exact.len() == 1 {
            MatchResult::found(exact)
        } else {
            MatchResult::ambiguous(exact)
        };
    }

    // Fuzzy match across all articles.
    let mut fuzzy: Vec<TextMatch> = Vec::new();
    for article in articles {
        for mut m in find_fuzzy_matches(&article.text, selector, threshold) {
            m.article_number = article.number.clone();
            fuzzy.push(m);
        }
    }
    finalize_fuzzy(fuzzy)
}

/// Resolve a selector against a single raw text body (no article context).
pub fn resolve_in_text(selector: &TextQuoteSelector, text: &str, threshold: f64) -> MatchResult {
    let exact = find_exact_matches(text, selector);
    if !exact.is_empty() {
        return if exact.len() == 1 {
            MatchResult::found(exact)
        } else {
            MatchResult::ambiguous(exact)
        };
    }
    finalize_fuzzy(find_fuzzy_matches(text, selector, threshold))
}

/// Collapse fuzzy candidates into a final [`MatchResult`].
///
/// Overlapping spans are deduplicated keeping the highest confidence. A single
/// surviving match, or a clear winner (more than [`TIEBREAK_MARGIN`] ahead of
/// the runner-up), is `Found`; otherwise `Ambiguous`; empty is `Orphaned`.
fn finalize_fuzzy(matches: Vec<TextMatch>) -> MatchResult {
    if matches.is_empty() {
        return MatchResult::orphaned();
    }
    let deduped = deduplicate_overlapping(matches);
    if deduped.len() == 1 {
        return MatchResult::found(deduped);
    }
    if deduped.len() > 1 && deduped[0].confidence - deduped[1].confidence > TIEBREAK_MARGIN {
        return MatchResult::found(vec![deduped[0].clone()]);
    }
    MatchResult::ambiguous(deduped)
}

/// Try the hinted article (optionally a hinted position) before any full
/// search. Returns `Orphaned` on failure so the caller falls back.
fn resolve_hint(
    selector: &TextQuoteSelector,
    articles: &[Article],
    threshold: f64,
    hint: &SelectorHint,
) -> MatchResult {
    let Some(article) = articles.iter().find(|a| a.number == hint.article_number) else {
        return MatchResult::orphaned();
    };

    // Exact position hint: verify the exact text sits at the given offsets.
    if let (Some(start), Some(end)) = (hint.start, hint.end) {
        let chars: Vec<char> = article.text.chars().collect();
        if start <= end && end <= chars.len() {
            let at: String = chars[start..end].iter().collect();
            if at == selector.exact {
                if let Some(m) =
                    verify_at_position(&article.text, selector, start, end, &article.number)
                {
                    return MatchResult::found(vec![m]);
                }
            }
        }
    }

    // Search the whole hinted article (exact then fuzzy).
    let mut exact = find_exact_matches(&article.text, selector);
    for m in &mut exact {
        m.article_number = article.number.clone();
    }
    if !exact.is_empty() {
        return if exact.len() == 1 {
            MatchResult::found(exact)
        } else {
            MatchResult::ambiguous(exact)
        };
    }

    let mut fuzzy = find_fuzzy_matches(&article.text, selector, threshold);
    for m in &mut fuzzy {
        m.article_number = article.number.clone();
    }
    if fuzzy.is_empty() {
        return MatchResult::orphaned();
    }
    let deduped = deduplicate_overlapping(fuzzy);
    if deduped.len() == 1 {
        MatchResult::found(deduped)
    } else if deduped.len() > 1 && deduped[0].confidence - deduped[1].confidence > TIEBREAK_MARGIN {
        MatchResult::found(vec![deduped[0].clone()])
    } else {
        MatchResult::orphaned()
    }
}

/// Confirm a candidate at a fixed position has the expected (whitespace-
/// tolerant) prefix and suffix.
fn verify_at_position(
    text: &str,
    selector: &TextQuoteSelector,
    start: usize,
    end: usize,
    article_number: &str,
) -> Option<TextMatch> {
    let chars: Vec<char> = text.chars().collect();

    if !selector.prefix.is_empty() {
        let prefix_start = start.saturating_sub(selector.prefix.chars().count() + 1);
        let actual: String = chars[prefix_start..start].iter().collect();
        if !actual.trim().contains(selector.prefix.trim()) {
            return None;
        }
    }
    if !selector.suffix.is_empty() {
        let suffix_end = (end + selector.suffix.chars().count() + 1).min(chars.len());
        let actual: String = chars[end..suffix_end].iter().collect();
        if !actual.trim().contains(selector.suffix.trim()) {
            return None;
        }
    }

    Some(TextMatch {
        article_number: article_number.to_string(),
        start,
        end,
        confidence: 1.0,
        matched_text: chars[start..end].iter().collect(),
    })
}

/// All exact occurrences of `exact` whose (whitespace-normalised) prefix and
/// suffix match. Offsets are in `char`s, not bytes.
fn find_exact_matches(text: &str, selector: &TextQuoteSelector) -> Vec<TextMatch> {
    let chars: Vec<char> = text.chars().collect();
    let exact: Vec<char> = selector.exact.chars().collect();
    if exact.is_empty() {
        return Vec::new();
    }

    let mut matches = Vec::new();
    let mut from = 0usize;
    while from + exact.len() <= chars.len() {
        let Some(rel) = find_subslice(&chars[from..], &exact) else {
            break;
        };
        let pos = from + rel;
        let end = pos + exact.len();

        let prefix_ok = selector.prefix.is_empty() || {
            let p_len = selector.prefix.chars().count();
            let p_start = pos.saturating_sub(p_len + 1);
            let actual: String = chars[p_start..pos].iter().collect();
            actual.trim().contains(selector.prefix.trim())
        };
        let suffix_ok = selector.suffix.is_empty() || {
            let s_len = selector.suffix.chars().count();
            let s_end = (end + s_len + 1).min(chars.len());
            let actual: String = chars[end..s_end].iter().collect();
            actual.trim().contains(selector.suffix.trim())
        };

        if prefix_ok && suffix_ok {
            matches.push(TextMatch {
                article_number: String::new(),
                start: pos,
                end,
                confidence: 1.0,
                matched_text: selector.exact.clone(),
            });
        }
        from = pos + 1;
    }
    matches
}

/// Fuzzy candidates at or above `threshold`, sorted by confidence descending.
///
/// Mirrors the Python proof-of-concept: collect exact occurrences plus
/// sliding windows of `len(exact) ± 30%` that share a significant word with
/// `exact`, then score each by weighted Levenshtein similarity.
fn find_fuzzy_matches(text: &str, selector: &TextQuoteSelector, threshold: f64) -> Vec<TextMatch> {
    let chars: Vec<char> = text.chars().collect();
    let exact_len = selector.exact.chars().count();
    if exact_len == 0 || chars.is_empty() {
        return Vec::new();
    }

    let tolerance = ((exact_len as f64) * WINDOW_TOLERANCE) as usize;
    let min_w = exact_len.saturating_sub(tolerance).max(1);
    let max_w = exact_len + tolerance;

    let mut matches: Vec<TextMatch> = Vec::new();
    for window in min_w..=max_w {
        if window > chars.len() {
            break;
        }
        for i in 0..=(chars.len() - window) {
            let candidate: String = chars[i..i + window].iter().collect();
            if !shares_significant_content(&selector.exact, &candidate) {
                continue;
            }

            let prefix_len = selector.prefix.chars().count();
            let suffix_len = selector.suffix.chars().count();
            let p_start = i.saturating_sub(prefix_len);
            let actual_prefix: String = chars[p_start..i].iter().collect();
            let s_end = (i + window + suffix_len).min(chars.len());
            let actual_suffix: String = chars[i + window..s_end].iter().collect();

            let exact_score = similarity(&selector.exact, &candidate);
            let prefix_score = if selector.prefix.is_empty() {
                1.0
            } else {
                similarity(&selector.prefix, &actual_prefix)
            };
            let suffix_score = if selector.suffix.is_empty() {
                1.0
            } else {
                similarity(&selector.suffix, &actual_suffix)
            };

            let weighted = exact_score * 0.5 + prefix_score * 0.25 + suffix_score * 0.25;
            if weighted >= threshold {
                matches.push(TextMatch {
                    article_number: String::new(),
                    start: i,
                    end: i + window,
                    confidence: weighted,
                    matched_text: candidate,
                });
            }
        }
    }

    matches.sort_by(|a, b| {
        b.confidence
            .partial_cmp(&a.confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    matches
}

/// Normalised Levenshtein similarity in `[0.0, 1.0]` (RFC-016).
fn similarity(a: &str, b: &str) -> f64 {
    if a.is_empty() && b.is_empty() {
        return 1.0;
    }
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    strsim::normalized_levenshtein(a, b)
}

/// Cheap pre-filter: do the two strings share a word longer than 3 chars?
/// Avoids scoring obviously unrelated windows.
fn shares_significant_content(a: &str, b: &str) -> bool {
    let words_a: std::collections::HashSet<String> = a
        .to_lowercase()
        .split_whitespace()
        .map(String::from)
        .collect();
    b.to_lowercase()
        .split_whitespace()
        .any(|w| w.chars().count() > 3 && words_a.contains(w))
}

/// Keep only the highest-confidence match for each overlapping region.
fn deduplicate_overlapping(mut matches: Vec<TextMatch>) -> Vec<TextMatch> {
    matches.sort_by(|a, b| {
        b.confidence
            .partial_cmp(&a.confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut kept: Vec<TextMatch> = Vec::new();
    for m in matches {
        let overlaps = kept
            .iter()
            .any(|k| m.article_number == k.article_number && m.start < k.end && k.start < m.end);
        if !overlaps {
            kept.push(m);
        }
    }
    kept
}

/// First index of `needle` within `haystack`, comparing `char`s.
fn find_subslice(haystack: &[char], needle: &[char]) -> Option<usize> {
    if needle.is_empty() || needle.len() > haystack.len() {
        return None;
    }
    (0..=haystack.len() - needle.len()).find(|&i| haystack[i..i + needle.len()] == *needle)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn article(number: &str, text: &str) -> Article {
        Article {
            number: number.to_string(),
            text: text.to_string(),
            url: None,
            machine_readable: None,
        }
    }

    fn selector(exact: &str, prefix: &str, suffix: &str) -> TextQuoteSelector {
        TextQuoteSelector {
            exact: exact.to_string(),
            prefix: prefix.to_string(),
            suffix: suffix.to_string(),
            hint: None,
        }
    }

    #[test]
    fn exact_match_single() {
        let arts = vec![article(
            "2",
            "heeft de verzekerde aanspraak op een zorgtoeslag ter grootte van dat verschil",
        )];
        let sel = selector("zorgtoeslag", "op een ", " ter grootte");
        let r = resolve(&sel, &arts);
        assert!(r.is_found());
        assert_eq!(r.single().unwrap().article_number, "2");
        assert_eq!(r.single().unwrap().confidence, 1.0);
    }

    #[test]
    fn article_renumbered_still_resolves() {
        // Same text, different article number: content-addressed lookup wins.
        let arts = vec![
            article("1a", "Een nieuw ingevoegd artikel."),
            article(
                "4a",
                "heeft de verzekerde aanspraak op een zorgtoeslag ter grootte van dat verschil",
            ),
        ];
        let sel = selector("zorgtoeslag", "op een ", " ter grootte");
        let r = resolve(&sel, &arts);
        assert!(r.is_found());
        assert_eq!(r.single().unwrap().article_number, "4a");
    }

    #[test]
    fn ambiguous_without_context() {
        let arts = vec![article(
            "2",
            "de verzekerde en de verzekerde en nog een verzekerde",
        )];
        let sel = selector("verzekerde", "", "");
        let r = resolve(&sel, &arts);
        assert!(r.is_ambiguous());
        assert!(r.matches.len() >= 2);
    }

    #[test]
    fn unique_with_context() {
        let arts = vec![article(
            "2",
            "de verzekerde betaalt; de verzekerde ontvangt; heeft de verzekerde aanspraak op zorgtoeslag",
        )];
        let sel = selector("verzekerde", "heeft de ", " aanspraak");
        let r = resolve(&sel, &arts);
        assert!(r.is_found());
    }

    #[test]
    fn orphaned_when_text_removed() {
        let arts = vec![article("2", "Geheel andere tekst zonder de gezochte zin.")];
        let sel = selector(
            "zorgtoeslag ter grootte van dat verschil",
            "aanspraak op een ",
            "",
        );
        let r = resolve(&sel, &arts);
        assert!(r.is_orphaned());
    }

    #[test]
    fn fuzzy_match_on_minor_change() {
        // "aanspraak op een" -> "recht op een"; suffix slightly changed too.
        let arts = vec![article(
            "2",
            "heeft de verzekerde recht op een zorgtoeslag ter grootte van het verschil",
        )];
        let sel = selector(
            "aanspraak op een zorgtoeslag",
            "heeft de verzekerde ",
            " ter grootte van dat verschil",
        );
        let r = resolve(&sel, &arts);
        assert!(r.is_found(), "expected fuzzy match, got {:?}", r.status);
        assert!(r.single().unwrap().confidence < 1.0);
    }

    #[test]
    fn hint_optimisation_finds_match() {
        let arts = vec![
            article("1", "Onbelangrijke tekst."),
            article("2", "heeft de verzekerde aanspraak op een zorgtoeslag hier"),
        ];
        let mut sel = selector("zorgtoeslag", "op een ", " hier");
        sel.hint = Some(SelectorHint {
            article_number: "2".to_string(),
            start: None,
            end: None,
        });
        let r = resolve(&sel, &arts);
        assert!(r.is_found());
        assert_eq!(r.single().unwrap().article_number, "2");
    }

    #[test]
    fn outdated_hint_falls_back_to_full_search() {
        // Hint points at article 9 which does not contain the text;
        // resolver must still find it in article 2.
        let arts = vec![
            article("2", "heeft de verzekerde aanspraak op een zorgtoeslag hier"),
            article("9", "Niets relevants."),
        ];
        let mut sel = selector("zorgtoeslag", "op een ", " hier");
        sel.hint = Some(SelectorHint {
            article_number: "9".to_string(),
            start: Some(0),
            end: Some(5),
        });
        let r = resolve(&sel, &arts);
        assert!(r.is_found());
        assert_eq!(r.single().unwrap().article_number, "2");
    }
}
