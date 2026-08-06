//! TextQuoteSelector resolution algorithm (RFC-005, RFC-018).
//!
//! Resolution order:
//! 1. Exact match: locate `prefix + exact + suffix` as a substring, with
//!    whitespace-tolerant prefix/suffix checks.
//! 2. Fuzzy match: a sliding window over the text scored
//!    `exact*0.5 + prefix*0.25 + suffix*0.25`, keeping candidates at or above
//!    the threshold (0.7).
//! 3. One match (or a clear winner) is [`MatchStatus::Found`], several
//!    equally-good are [`MatchStatus::Ambiguous`], none is
//!    [`MatchStatus::Orphaned`].
//!
//! A `regelrecht:hint` on the selector does not participate in resolution.
//! RFC-018 is explicit that article numbers are non-authoritative and that
//! the note follows the *text*; any hint-driven shortcut can hide a competing
//! occurrence elsewhere in the law, silently turning a genuine
//! [`MatchStatus::Ambiguous`] (which the editor escalates to a human) into a
//! confident [`MatchStatus::Found`] on a possibly renumbered article. Ruling
//! out that competitor requires the full scan anyway, so the hint cannot
//! shorten the search without being allowed to override it.
//!
//! Structurally ported from the Python proof-of-concept on the
//! `feature/annotation-resolver` branch (resolution order, dedup, tiebreak
//! margin; the PoC's hint fast path was dropped, see above). The scoring function differs deliberately: the
//! PoC used `difflib.SequenceMatcher.ratio()` (Ratcliff-Obershelp); this uses
//! normalised Levenshtein per RFC-018, which is harsher on block moves. The
//! two disagree near the 0.7 threshold, so a boundary BDD scenario guards the
//! behaviour. The Rust port is the single source of truth (it also runs in
//! the browser via WASM).
//!
//! All offsets ([`TextMatch::start`]/`end`, [`SelectorHint`] positions) are
//! **`char` offsets** (Unicode scalar values), not byte or UTF-16 code-unit
//! offsets. JS consumers indexing the law text must account for this; see the
//! WASM binding docs.

use crate::annotation::types::{MatchResult, TextMatch, TextQuoteSelector};
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
/// A present hint never overrides the full-text search: the result is the
/// same with or without it (see the module docs).
pub fn resolve(selector: &TextQuoteSelector, articles: &[Article]) -> MatchResult {
    resolve_with_threshold(selector, articles, DEFAULT_FUZZY_THRESHOLD)
}

/// [`resolve`] with an explicit fuzzy threshold (used by tests).
pub fn resolve_with_threshold(
    selector: &TextQuoteSelector,
    articles: &[Article],
    threshold: f64,
) -> MatchResult {
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
    match clear_winner(&deduped) {
        Some(winner) => MatchResult::found(vec![winner]),
        None => MatchResult::ambiguous(deduped),
    }
}

/// The one candidate that stands out, if there is one.
///
/// A lone candidate wins by default; with several, the best has to beat the
/// runner-up by *more* than [`TIEBREAK_MARGIN`]. `None` means the field is
/// tied, which the caller reports as ambiguous. Candidates arrive sorted by
/// confidence descending (the caller dedupes first, which sorts).
fn clear_winner(deduped: &[TextMatch]) -> Option<TextMatch> {
    match deduped {
        [only] => Some(only.clone()),
        [best, second, ..] if best.confidence - second.confidence > TIEBREAK_MARGIN => {
            Some(best.clone())
        }
        _ => None,
    }
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

        // Window is exactly `len + 1` chars flush against `exact`, so after
        // trimming it must *equal* the prefix/suffix. Equality (not
        // ends_with/starts_with) rejects word-internal false positives, e.g.
        // prefix "op een" must not be satisfied by "rop een". The +1 char of
        // slack plus trim() absorbs a single whitespace difference.
        let prefix_ok = selector.prefix.is_empty() || {
            let p_len = selector.prefix.chars().count();
            let p_start = pos.saturating_sub(p_len + 1);
            let actual: String = chars[p_start..pos].iter().collect();
            actual.trim() == selector.prefix.trim()
        };
        let suffix_ok = selector.suffix.is_empty() || {
            let s_len = selector.suffix.chars().count();
            let s_end = (end + s_len + 1).min(chars.len());
            let actual: String = chars[end..s_end].iter().collect();
            actual.trim() == selector.suffix.trim()
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

    // Constant for the whole scan: compute once, not per window position.
    let exact_words = significant_words(&selector.exact);
    let prefix_len = selector.prefix.chars().count();
    let suffix_len = selector.suffix.chars().count();

    let mut matches: Vec<TextMatch> = Vec::new();
    for window in min_w..=max_w {
        if window > chars.len() {
            break;
        }
        for i in 0..=(chars.len() - window) {
            let candidate: String = chars[i..i + window].iter().collect();
            if !shares_significant_content(&exact_words, &candidate) {
                continue;
            }

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

/// Normalised Levenshtein similarity in `[0.0, 1.0]` (RFC-018).
fn similarity(a: &str, b: &str) -> f64 {
    if a.is_empty() && b.is_empty() {
        return 1.0;
    }
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    strsim::normalized_levenshtein(a, b)
}

/// Significant words (longer than 3 chars, lowercased) of a string.
///
/// Computed once for `exact` before the sliding-window scan; "significant"
/// excludes short function words (articles, prepositions) so the pre-filter
/// keys on content words.
fn significant_words(s: &str) -> std::collections::HashSet<String> {
    s.to_lowercase()
        .split_whitespace()
        .filter(|w| w.chars().count() > 3)
        .map(String::from)
        .collect()
}

/// Cheap pre-filter: does the candidate share a significant word with `exact`?
/// Avoids scoring obviously unrelated windows. `exact_words` is precomputed by
/// the caller so this allocates nothing per window.
fn shares_significant_content(
    exact_words: &std::collections::HashSet<String>,
    candidate: &str,
) -> bool {
    candidate
        .to_lowercase()
        .split_whitespace()
        .any(|w| w.chars().count() > 3 && exact_words.contains(w))
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
    use crate::annotation::types::SelectorHint;

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

    fn hinted(
        exact: &str,
        prefix: &str,
        suffix: &str,
        article_number: &str,
        span: Option<(usize, usize)>,
    ) -> TextQuoteSelector {
        let mut sel = selector(exact, prefix, suffix);
        sel.hint = Some(SelectorHint {
            article_number: article_number.to_string(),
            start: span.map(|(s, _)| s),
            end: span.map(|(_, e)| e),
        });
        sel
    }

    /// A candidate as the fuzzy scan produces them, for the collapsing helpers.
    fn candidate(article_number: &str, start: usize, end: usize, confidence: f64) -> TextMatch {
        TextMatch {
            article_number: article_number.to_string(),
            start,
            end,
            confidence,
            matched_text: String::new(),
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
    fn prefix_rejects_word_internal_false_positive() {
        // "op een" must not be satisfied by the word-internal "...rop een"
        // (the bug an `ends_with` check would let through). Only the genuine
        // occurrence preceded by exactly "op een" should match.
        let arts = vec![article(
            "2",
            "de stroop een zoetstof; recht op een zorgtoeslag",
        )];
        let sel = selector("zorgtoeslag", "op een ", "");
        let r = resolve(&sel, &arts);
        assert!(r.is_found(), "expected the genuine 'op een ' occurrence");
        let m = r.single().unwrap();
        // It must be the second "zorgtoeslag"-context, not anchored via "stroop een".
        assert_eq!(m.matched_text, "zorgtoeslag");
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
    fn a_hint_does_not_change_a_unique_match() {
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

    // === Exact matching: every occurrence, and the context that filters them ===

    #[test]
    fn every_occurrence_is_reported_when_the_context_does_not_disambiguate() {
        // Without prefix/suffix all three occurrences are equally good. They
        // must all be reported: the number of candidates is what tells the
        // editor how ambiguous the anchor really is.
        let arts = vec![article(
            "2",
            "de verzekerde en de verzekerde en nog een verzekerde",
        )];
        let sel = selector("verzekerde", "", "");
        let r = resolve(&sel, &arts);
        assert!(r.is_ambiguous());
        let starts: Vec<usize> = r.matches.iter().map(|m| m.start).collect();
        assert_eq!(starts, vec![3, 20, 42]);
    }

    #[test]
    fn without_context_a_quote_also_matches_inside_a_longer_word() {
        // An empty prefix and suffix impose no context at all, so "toeslag"
        // also matches inside "zorgtoeslagen". That is exactly why an anchor
        // without context comes back ambiguous instead of found.
        let arts = vec![article("2", "de zorgtoeslagen en de aanvullende toeslag")];
        let sel = selector("toeslag", "", "");
        let r = resolve(&sel, &arts);
        assert!(r.is_ambiguous());
        assert_eq!(r.matches.len(), 2);
    }

    #[test]
    fn context_matches_across_a_single_whitespace_difference() {
        // Prefix and suffix are stored without the whitespace that separates
        // them from the quote in the law text. One character of slack plus
        // trimming absorbs that, so this stays an exact hit (confidence 1.0)
        // instead of degrading to a fuzzy one.
        let arts = vec![article(
            "2",
            "heeft recht op een zorgtoeslag ter grootte van dat verschil",
        )];
        let sel = selector("zorgtoeslag", "op een", "ter grootte");
        let r = resolve(&sel, &arts);
        assert!(r.is_found());
        let m = r.single().unwrap();
        assert_eq!(m.start, 19);
        assert_eq!(
            m.confidence, 1.0,
            "a hit on the literal text must not degrade to a fuzzy match"
        );
    }

    #[test]
    fn both_prefix_and_suffix_have_to_match() {
        // The first occurrence has the right prefix but the wrong suffix. Only
        // the occurrence that matches both sides is the anchor.
        let text =
            "recht op een zorgtoeslag krachtens deze wet en recht op een zorgtoeslag van rechtswege";
        let arts = vec![article("2", text)];
        let sel = selector("zorgtoeslag", "op een", "van rechtswege");
        let r = resolve(&sel, &arts);
        assert!(r.is_found(), "got {:?}", r.status);
        let m = r.single().unwrap();
        assert!(text[m.end..].starts_with(" van rechtswege"));
    }

    // === Hints: recorded but never authoritative ===

    #[test]
    fn a_hint_does_not_remove_a_cross_article_ambiguity() {
        // Both articles carry the quote verbatim. After a renumbering the
        // recorded article number may point at the wrong one, so the hint may
        // not decide: the ambiguity goes to a human, exactly as it would
        // without a hint.
        let arts = vec![
            article("2", "Deze verplichting vloeit voort uit de wet."),
            article("3", "De inspecteur handelt overeenkomstig de wet."),
        ];
        let sel = hinted("de wet", "", "", "3", None);
        let r = resolve(&sel, &arts);
        assert!(r.is_ambiguous(), "got {:?}", r.status);
        let articles_hit: Vec<&str> = r
            .matches
            .iter()
            .map(|m| m.article_number.as_str())
            .collect();
        assert_eq!(articles_hit, vec!["2", "3"]);
    }

    #[test]
    fn a_position_hint_does_not_pick_between_identical_occurrences() {
        // The same phrase occurs twice with identical context, so prefix and
        // suffix cannot tell them apart. The recorded position is as stale-
        // prone as the article number (any edit above it shifts the offsets),
        // so it may not silently pick one: both occurrences are reported.
        let text = "recht op een zorgtoeslag van rechtswege en recht op een zorgtoeslag van rechtswege, aldus de toelichting";
        let arts = vec![article("2", text)];
        let start = text.rfind("zorgtoeslag").unwrap();
        let sel = hinted(
            "zorgtoeslag",
            "op een",
            "van rechtswege",
            "2",
            Some((start, start + "zorgtoeslag".len())),
        );
        let r = resolve(&sel, &arts);
        assert!(r.is_ambiguous(), "got {:?}", r.status);
        assert_eq!(r.matches.len(), 2);
    }

    #[test]
    fn a_hinted_fuzzy_match_loses_to_an_exact_match_elsewhere() {
        // The hinted article only fuzzily resembles the quote; another
        // article carries it verbatim with the right context. The full
        // search is authoritative, so the exact occurrence wins even though
        // the hint points elsewhere.
        let arts = vec![
            // "groote" (sic): one letter off, a clear fuzzy hit but not exact.
            article("2", "recht op een zorgtoeslag ter groote van dat verschil"),
            article(
                "7",
                "heeft de verzekerde aanspraak op een zorgtoeslag ter grootte van dat verschil",
            ),
        ];
        let sel = hinted("zorgtoeslag", "op een ", " ter grootte", "2", None);
        let r = resolve(&sel, &arts);
        assert!(r.is_found(), "got {:?}", r.status);
        let m = r.single().unwrap();
        assert_eq!(m.article_number, "7");
        assert_eq!(m.confidence, 1.0);
    }

    #[test]
    fn a_position_hint_with_the_wrong_prefix_is_rejected() {
        // The article was rewritten and the recorded offsets now land on
        // another occurrence: same words, different context. The hint is
        // non-authoritative, so the text search decides.
        let text =
            "een voorschot zorgtoeslag van rechtswege; recht op een zorgtoeslag van rechtswege";
        let arts = vec![article("2", text)];
        let stale = text.find("zorgtoeslag").unwrap();
        let sel = hinted(
            "zorgtoeslag",
            "op een",
            "van rechtswege",
            "2",
            Some((stale, stale + "zorgtoeslag".len())),
        );
        let r = resolve(&sel, &arts);
        assert!(r.is_found(), "got {:?}", r.status);
        assert_eq!(
            r.single().unwrap().start,
            text.rfind("zorgtoeslag").unwrap(),
            "the occurrence with the wrong prefix must not win"
        );
    }

    #[test]
    fn a_position_hint_with_the_wrong_suffix_is_rejected() {
        let text =
            "recht op een zorgtoeslag krachtens deze wet en recht op een zorgtoeslag van rechtswege";
        let arts = vec![article("2", text)];
        let stale = text.find("zorgtoeslag").unwrap();
        let sel = hinted(
            "zorgtoeslag",
            "op een",
            "van rechtswege",
            "2",
            Some((stale, stale + "zorgtoeslag".len())),
        );
        let r = resolve(&sel, &arts);
        assert!(r.is_found(), "got {:?}", r.status);
        assert_eq!(
            r.single().unwrap().start,
            text.rfind("zorgtoeslag").unwrap(),
            "the occurrence with the wrong suffix must not win"
        );
    }

    #[test]
    fn a_position_hint_past_the_end_of_the_article_is_ignored() {
        // The article was shortened; the recorded offsets now point beyond it.
        // That must not panic and must not stop the search.
        let arts = vec![article("2", "heeft recht op een zorgtoeslag hier")];
        let sel = hinted("zorgtoeslag", "op een", "hier", "2", Some((120, 131)));
        let r = resolve(&sel, &arts);
        assert!(r.is_found());
        assert_eq!(r.single().unwrap().start, 19);
    }

    // === Fuzzy matching: which windows, which span, which threshold ===

    #[test]
    fn a_fuzzy_anchor_covers_the_changed_phrase() {
        // The note quotes "aanspraak op een zorgtoeslag"; the article now says
        // "recht op een zorgtoeslag". The anchor has to cover that whole
        // phrase and nothing else: it is the span the editor highlights.
        let arts = vec![article(
            "2",
            "heeft de verzekerde recht op een zorgtoeslag ter grootte van het verschil",
        )];
        let sel = selector(
            "aanspraak op een zorgtoeslag",
            "heeft de verzekerde",
            "ter grootte",
        );
        let r = resolve(&sel, &arts);
        assert!(r.is_found(), "expected a fuzzy match, got {:?}", r.status);
        let m = r.single().unwrap();
        assert_eq!(
            m.matched_text.trim(),
            "recht op een zorgtoeslag",
            "the anchor must cover the changed phrase, not an arbitrary window of about the right size"
        );
        assert!(m.confidence < 1.0);
    }

    #[test]
    fn fuzzy_windows_stay_within_the_length_tolerance() {
        // Candidates are windows of the quote's length ±30%. A window far off
        // that length is a different phrase, not a changed one, and scoring it
        // only invites the resolver to anchor on the wrong text.
        let sel = selector("zorgtoeslag", "", "");
        let text = "de zorgtoeslag wordt jaarlijks vastgesteld door de Belastingdienst";
        let matches = find_fuzzy_matches(text, &sel, DEFAULT_FUZZY_THRESHOLD);
        assert!(!matches.is_empty());
        for m in &matches {
            let len = m.end - m.start;
            assert!(
                (8..=14).contains(&len),
                "window of {len} chars for an 11-char quote: {m:?}"
            );
        }
    }

    #[test]
    fn a_fuzzy_window_may_span_the_whole_article() {
        // The article text is the changed quote and nothing else, so the only
        // window that covers it is the full text.
        let arts = vec![article("2", "recht op een zorgtoeslag")];
        let sel = selector("recht op de zorgtoeslag", "", "");
        let r = resolve(&sel, &arts);
        assert!(r.is_found(), "got {:?}", r.status);
        assert_eq!(r.single().unwrap().matched_text, "recht op een zorgtoeslag");
    }

    #[test]
    fn a_rewrite_that_only_shares_a_word_does_not_anchor() {
        // "belanghebbende" gets these windows past the pre-filter, but none of
        // them scores at the threshold. The note must orphan rather than
        // anchor onto text it is not about.
        let arts = vec![article(
            "2",
            "de belanghebbende dient de aanvraag in bij de Belastingdienst",
        )];
        let sel = selector(
            "stelt het verzamelinkomen van de belanghebbende ambtshalve vast",
            "de inspecteur",
            "voor het jaar",
        );
        let r = resolve(&sel, &arts);
        assert!(r.is_orphaned(), "got {:?} at {:?}", r.status, r.matches);
    }

    #[test]
    fn a_quote_of_only_short_words_does_not_fuzzily_anchor() {
        // "in de wet" has no word longer than three characters, so there is
        // nothing to key a fuzzy match on. Once the phrase is replaced the note
        // orphans instead of drifting onto text that merely looks like it.
        let arts = vec![article("2", "Deze bevoegdheid is opgenomen in het besluit")];
        let sel = selector("in de wet", "", "");
        let r = resolve(&sel, &arts);
        assert!(r.is_orphaned(), "got {:?} at {:?}", r.status, r.matches);
    }

    // === Scoring and collapsing helpers ===

    #[test]
    fn similarity_treats_a_missing_side_as_no_similarity() {
        // A window at the very start of an article has no text before it. That
        // scores zero against a non-empty prefix, not a free 1.0.
        assert_eq!(similarity("op een", ""), 0.0);
        assert_eq!(similarity("", "op een"), 0.0);
        assert_eq!(similarity("", ""), 1.0);
        assert_eq!(similarity("zorgtoeslag", "zorgtoeslag"), 1.0);
        assert!(similarity("aanspraak", "aanzoek") > 0.0);
    }

    #[test]
    fn only_words_longer_than_three_characters_are_significant() {
        let words = significant_words("Recht op een zorgtoeslag van de wet");
        assert!(words.contains("recht"), "lowercased: {words:?}");
        assert!(words.contains("zorgtoeslag"));
        assert!(
            !words.contains("wet"),
            "three characters is too common to key on"
        );
        assert!(!words.contains("op"));
    }

    #[test]
    fn the_prefilter_needs_a_word_shared_with_the_quote() {
        let quote = significant_words("aanspraak op zorgtoeslag");
        assert!(shares_significant_content(&quote, "recht op zorgtoeslag"));
        assert!(
            !shares_significant_content(&quote, "vastgesteld bij ministeriële regeling"),
            "long words that the quote does not use are not shared content"
        );
        assert!(!shares_significant_content(&quote, "op de wet"));
    }

    #[test]
    fn touching_candidates_are_not_overlapping() {
        // Two spans that touch (one ends where the next begins) are separate
        // anchors; only genuinely overlapping spans collapse onto the best one.
        let kept = deduplicate_overlapping(vec![
            candidate("2", 0, 10, 0.80),
            candidate("2", 10, 20, 0.90),
            candidate("2", 20, 30, 0.85),
        ]);
        assert_eq!(kept.len(), 3, "kept {kept:?}");
    }

    #[test]
    fn overlapping_candidates_collapse_onto_the_best() {
        let kept = deduplicate_overlapping(vec![
            candidate("2", 0, 10, 0.80),
            candidate("2", 5, 15, 0.90),
        ]);
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].confidence, 0.90);
    }

    #[test]
    fn the_same_span_in_another_article_is_a_separate_candidate() {
        let kept = deduplicate_overlapping(vec![
            candidate("2", 0, 10, 0.90),
            candidate("3", 0, 10, 0.80),
        ]);
        assert_eq!(kept.len(), 2);
    }

    #[test]
    fn a_winner_has_to_beat_the_runner_up_by_more_than_the_margin() {
        assert_eq!(
            clear_winner(&[candidate("2", 0, 10, 0.80)]).map(|w| w.start),
            Some(0),
            "a lone candidate wins by default"
        );
        assert_eq!(
            clear_winner(&[candidate("2", 0, 10, 0.95), candidate("2", 20, 30, 0.80)])
                .map(|w| w.start),
            Some(0),
            "0.15 ahead is a clear winner"
        );

        // Exactly the margin is not "more than": still a tie. 0.2 and 0.1 are
        // chosen because their difference is exactly 0.1 in binary floating
        // point, which is what puts this case on the boundary.
        let (best, second) = (0.2, 0.1);
        assert_eq!(best - second, TIEBREAK_MARGIN, "fixture sits on the margin");
        assert!(
            clear_winner(&[candidate("2", 0, 10, best), candidate("2", 20, 30, second)]).is_none()
        );

        assert!(
            clear_winner(&[candidate("2", 0, 10, 0.75), candidate("2", 20, 30, 0.72)]).is_none(),
            "within the margin is a tie"
        );
        assert!(clear_winner(&[]).is_none());
    }

    #[test]
    fn find_subslice_reports_the_first_position() {
        let hay: Vec<char> = "zorgtoeslag".chars().collect();
        let needle = |s: &str| s.chars().collect::<Vec<char>>();
        assert_eq!(find_subslice(&hay, &needle("toeslag")), Some(4));
        assert_eq!(find_subslice(&hay, &needle("zorgtoeslag")), Some(0));
        assert_eq!(find_subslice(&hay, &needle("premie")), None);
        assert_eq!(
            find_subslice(&hay, &needle("zorgtoeslagen")),
            None,
            "a needle longer than the haystack cannot be in it"
        );
        assert_eq!(
            find_subslice(&hay, &[]),
            None,
            "an empty quote matches nothing"
        );
    }

    // === Resolving against raw text, without articles ===

    #[test]
    fn raw_text_resolves_a_single_exact_quote() {
        let sel = selector("de wet", "", "");
        let r = resolve_in_text(
            &sel,
            "Deze verplichting vloeit voort uit de wet.",
            DEFAULT_FUZZY_THRESHOLD,
        );
        assert!(r.is_found(), "got {:?}", r.status);
        let m = r.single().unwrap();
        assert_eq!(m.matched_text, "de wet");
        assert_eq!(m.confidence, 1.0);
        assert_eq!(m.article_number, "", "raw text has no article to name");
    }

    #[test]
    fn raw_text_with_a_repeated_quote_is_ambiguous() {
        let sel = selector("de wet", "", "");
        let r = resolve_in_text(
            &sel,
            "de wet verwijst naar de wet.",
            DEFAULT_FUZZY_THRESHOLD,
        );
        assert!(r.is_ambiguous(), "got {:?}", r.status);
        assert_eq!(r.matches.len(), 2);
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
