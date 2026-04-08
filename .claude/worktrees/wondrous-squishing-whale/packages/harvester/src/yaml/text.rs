//! Text wrapping and normalization utilities for YAML output.

use regex::Regex;
use std::sync::LazyLock;
use textwrap::{fill, Options};

use crate::config::TEXT_WRAP_WIDTH;

/// Regex pattern for reference-style links [text][refN].
#[allow(clippy::expect_used)] // Static regex that is guaranteed to be valid
static REFERENCE_LINK_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[[^\]]+\]\[ref\d+\]").expect("valid regex"));

/// Regex pattern for missing space after comma before a word character.
/// Matches "word,word" but not "word, word" or "1,000".
#[allow(clippy::expect_used)] // Static regex that is guaranteed to be valid
static MISSING_SPACE_AFTER_COMMA: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"([a-zA-Z]),([a-zA-Z])").expect("valid regex"));

/// Check if text contains reference-style links that would be broken by wrapping.
fn contains_reference_link(text: &str) -> bool {
    REFERENCE_LINK_PATTERN.is_match(text)
}

/// Normalize common typographical issues in source text.
///
/// Fixes:
/// - Missing space after comma before a word (e.g., "lid,van" → "lid, van")
///
/// This is needed because some official source XML contains typographical errors.
pub fn normalize_text(text: &str) -> String {
    // Loop until no more replacements needed (handles overlapping cases like "a,b,c")
    let mut result = text.to_string();
    loop {
        let replaced = MISSING_SPACE_AFTER_COMMA
            .replace_all(&result, "$1, $2")
            .to_string();
        if replaced == result {
            break;
        }
        result = replaced;
    }
    result
}

/// Wrap text at specified width, preserving paragraph breaks and reference definitions.
///
/// Reference definitions (lines starting with [refN]:) are preserved as-is
/// to maintain valid markdown reference-style links.
pub fn wrap_text(text: &str, width: usize) -> String {
    // Separate reference definitions from main text
    let lines: Vec<&str> = text.lines().collect();
    let mut ref_lines: Vec<&str> = Vec::new();
    let mut content_lines: Vec<&str> = Vec::new();

    // Find where reference definitions start (from end)
    // Only include empty lines that are BETWEEN reference definitions
    let mut in_refs = false;
    let mut pending_empty: Vec<&str> = Vec::new();

    for line in lines.iter().rev() {
        if line.starts_with("[ref") && line.contains("]: ") {
            // Found a reference line - include any pending empty lines
            for empty in pending_empty.drain(..).rev() {
                ref_lines.insert(0, empty);
            }
            ref_lines.insert(0, line);
            in_refs = true;
        } else if in_refs && line.is_empty() {
            // Empty line while in refs - save for later
            // Only add if followed by another ref line
            pending_empty.push(line);
        } else {
            // Non-ref line - move pending empties to content and exit ref mode
            for empty in pending_empty.drain(..) {
                content_lines.insert(0, empty);
            }
            in_refs = false;
            content_lines.insert(0, line);
        }
    }

    // Any remaining pending empties go to content
    for empty in pending_empty {
        content_lines.insert(0, empty);
    }

    // Wrap content paragraphs, but skip paragraphs containing reference-style links
    let content_text = content_lines.join("\n");
    let paragraphs: Vec<&str> = content_text.split("\n\n").collect();

    let options = Options::new(width);
    let wrapped: Vec<String> = paragraphs
        .iter()
        .map(|p| {
            if contains_reference_link(p) {
                wrap_with_protected_refs(p, &options)
            } else {
                fill(p, &options)
            }
        })
        .collect();

    let wrapped_content = wrapped.join("\n\n");

    // Append reference definitions unchanged
    if !ref_lines.is_empty() {
        format!("{}\n\n{}", wrapped_content, ref_lines.join("\n"))
    } else {
        wrapped_content
    }
}

/// Wrap text while protecting reference-style links from being split.
///
/// Replaces `[text][refN]` tokens with single-word placeholders before wrapping,
/// then restores them after. This allows textwrap to break lines around the links
/// without breaking inside them.
fn wrap_with_protected_refs(text: &str, options: &Options<'_>) -> String {
    let mut replacements: Vec<(String, String)> = Vec::new();
    let mut protected = text.to_string();

    // Replace each reference link with a same-width non-breakable placeholder.
    // Width must match so textwrap calculates line breaks correctly.
    for mat in REFERENCE_LINK_PATTERN.find_iter(text) {
        let original = mat.as_str();
        let tag = format!("__REF{:03}__", replacements.len());
        let placeholder = format!(
            "{}{}",
            tag,
            "_".repeat(original.len().saturating_sub(tag.len()))
        );
        replacements.push((placeholder.clone(), original.to_string()));
        protected = protected.replacen(original, &placeholder, 1);
    }

    let wrapped = fill(&protected, options);

    // Restore original reference links
    let mut result = wrapped;
    for (placeholder, original) in &replacements {
        result = result.replacen(placeholder.as_str(), original, 1);
    }

    result
}

/// Check if text should be wrapped for readability.
pub fn should_wrap_text(text: &str) -> bool {
    let has_markdown_links = text.contains('[') && text.contains("](");
    text.len() > 80 || has_markdown_links
}

/// Wrap text with default width.
pub fn wrap_text_default(text: &str) -> String {
    wrap_text(text, TEXT_WRAP_WIDTH)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wrap_text_simple() {
        let text = "This is a simple text that should be wrapped when it exceeds the specified width limit.";
        let wrapped = wrap_text(text, 40);
        assert!(wrapped.contains('\n'));
    }

    #[test]
    fn test_wrap_text_preserves_paragraphs() {
        let text = "First paragraph.\n\nSecond paragraph.";
        let wrapped = wrap_text(text, 100);
        assert!(wrapped.contains("\n\n"));
    }

    #[test]
    fn test_wrap_text_preserves_references() {
        let text = "Some text with a [link][ref1].\n\n[ref1]: https://example.com";
        let wrapped = wrap_text(text, 100);
        assert!(wrapped.contains("[ref1]: https://example.com"));
    }

    #[test]
    fn test_should_wrap_text_long() {
        let long_text = "A".repeat(100);
        assert!(should_wrap_text(&long_text));
    }

    #[test]
    fn test_should_wrap_text_short() {
        let short_text = "Short text";
        assert!(!should_wrap_text(short_text));
    }

    #[test]
    fn test_should_wrap_text_with_links() {
        let text = "Text with [link](url)";
        assert!(should_wrap_text(text));
    }

    #[test]
    fn test_contains_reference_link() {
        assert!(contains_reference_link(
            "See [article 4][ref1] for details."
        ));
        assert!(contains_reference_link(
            "Multiple [ref][ref1] and [other][ref2] links."
        ));
        assert!(!contains_reference_link("No reference links here."));
        assert!(!contains_reference_link(
            "[link](url) is inline, not reference."
        ));
    }

    #[test]
    fn test_wrap_text_wraps_around_reference_links() {
        // Long text with reference links should be wrapped, but links stay intact
        let text = "This is a very long paragraph that contains a reference link like [article 4 of the Zorgverzekeringswet][ref1] which should not be broken across lines because that would invalidate the markdown.";
        let wrapped = wrap_text(text, 60);
        // The reference link should still be intact (not split across lines)
        assert!(wrapped.contains("[article 4 of the Zorgverzekeringswet][ref1]"));
        // But the text should be wrapped (contains newlines)
        assert!(wrapped.contains('\n'), "Text should be wrapped");
        // And no line should exceed the width by much (allowing for unbreakable ref links)
        for line in wrapped.lines() {
            assert!(
                line.len() <= 110,
                "Line too long ({} chars): {}",
                line.len(),
                line
            );
        }
    }

    #[test]
    fn test_normalize_text_missing_space_after_comma() {
        // Real example from Wet op de zorgtoeslag source XML
        assert_eq!(normalize_text("lid,van"), "lid, van");
        assert_eq!(
            normalize_text("eerste of derde lid,van die wet"),
            "eerste of derde lid, van die wet"
        );
    }

    #[test]
    fn test_normalize_text_preserves_correct_spacing() {
        // Should not change text with correct spacing
        assert_eq!(normalize_text("lid, van"), "lid, van");
        assert_eq!(normalize_text("correct, spacing"), "correct, spacing");
    }

    #[test]
    fn test_normalize_text_preserves_numbers() {
        // Should not add space in numbers like "1,000"
        assert_eq!(normalize_text("€ 1,000"), "€ 1,000");
        assert_eq!(normalize_text("bedrag van 1,50"), "bedrag van 1,50");
    }

    #[test]
    fn test_normalize_text_multiple_occurrences() {
        // Should fix multiple occurrences
        assert_eq!(normalize_text("a,b,c,d"), "a, b, c, d");
    }
}
