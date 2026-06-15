//! Tolerant, line-based extraction of a law's header fields.
//!
//! Several consumers need only a law's top-level metadata (`$id`, `name`, …) and
//! its article count, and must get them even when the document body is **not**
//! valid YAML — editor drafts mid-edit, or files an enrichment step is rewriting.
//! Full deserialization would reject those, so this scans line by line instead.
//!
//! This is the single home for that scan; it replaces the per-crate line
//! scanners that previously lived in the tui and corpus crates. Header keys are
//! matched only at the start of a line (column 0), so an indented `$id:` nested
//! inside an article is never mistaken for the law's id.

/// A law's top-level header fields, extracted tolerantly from raw YAML text.
///
/// Every field is best-effort: absent or unparseable fields are simply `None`
/// (or `0` for [`LawHeader::article_count`]). `name` is returned raw — it may be
/// an output reference like `#wet_naam`; callers that want only literal names
/// should filter those out.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LawHeader {
    /// The `$id` field (law slug).
    pub id: Option<String>,
    /// The `$schema` URL.
    pub schema: Option<String>,
    /// The `regulatory_layer` field.
    pub regulatory_layer: Option<String>,
    /// The `publication_date` field.
    pub publication_date: Option<String>,
    /// The `valid_from` field.
    pub valid_from: Option<String>,
    /// The `bwb_id` field.
    pub bwb_id: Option<String>,
    /// The raw `name` field (may be a `#`-prefixed output reference).
    pub name: Option<String>,
    /// Number of articles, counted by their `- number:` entries.
    pub article_count: usize,
}

/// Trim surrounding whitespace and a single layer of quotes; empty becomes `None`.
fn scalar(rest: &str) -> Option<String> {
    let value = rest.trim().trim_matches('"').trim_matches('\'');
    (!value.is_empty()).then(|| value.to_string())
}

/// Extract a law's [`LawHeader`] from raw YAML text without full deserialization.
///
/// Tolerant of malformed/partial documents: it never fails, returning whatever
/// header fields it could find. Each header field takes its first top-level
/// occurrence.
pub fn parse_law_header(yaml: &str) -> LawHeader {
    let mut header = LawHeader::default();

    for line in yaml.lines() {
        // Header keys are matched at column 0 only (no leading whitespace), so a
        // nested `$id:`/`name:` inside an article is never picked up.
        if let Some(rest) = line.strip_prefix("$id:") {
            header.id = header.id.or_else(|| scalar(rest));
        } else if let Some(rest) = line.strip_prefix("$schema:") {
            header.schema = header.schema.or_else(|| scalar(rest));
        } else if let Some(rest) = line.strip_prefix("regulatory_layer:") {
            header.regulatory_layer = header.regulatory_layer.or_else(|| scalar(rest));
        } else if let Some(rest) = line.strip_prefix("publication_date:") {
            header.publication_date = header.publication_date.or_else(|| scalar(rest));
        } else if let Some(rest) = line.strip_prefix("valid_from:") {
            header.valid_from = header.valid_from.or_else(|| scalar(rest));
        } else if let Some(rest) = line.strip_prefix("bwb_id:") {
            header.bwb_id = header.bwb_id.or_else(|| scalar(rest));
        } else if let Some(rest) = line.strip_prefix("name:") {
            header.name = header.name.or_else(|| scalar(rest));
        } else if line.trim_start().starts_with("- number:") {
            header.article_count += 1;
        }
    }

    header
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_id() {
        assert_eq!(
            parse_law_header("$id: my_law\nfoo: bar").id,
            Some("my_law".to_string())
        );
        assert_eq!(
            parse_law_header("$id: \"quoted_id\"\nfoo: bar").id,
            Some("quoted_id".to_string())
        );
        assert_eq!(parse_law_header("foo: bar\nbaz: qux").id, None);
    }

    #[test]
    fn ignores_indented_id() {
        // A nested `$id:` under an article must not be read as the law's id.
        let yaml = "articles:\n  - number: '1'\n    $id: nested\n";
        assert_eq!(parse_law_header(yaml).id, None);
        let yaml = "$id: top_level\narticles:\n  - number: '1'\n    $id: nested\n";
        assert_eq!(parse_law_header(yaml).id, Some("top_level".to_string()));
    }

    #[test]
    fn raw_name_keeps_reference() {
        assert_eq!(
            parse_law_header("name: '#wet_naam'").name,
            Some("#wet_naam".to_string())
        );
        assert_eq!(
            parse_law_header("name: Kieswet").name,
            Some("Kieswet".to_string())
        );
        assert_eq!(parse_law_header("name:").name, None);
    }

    #[test]
    fn counts_articles_and_reads_metadata() {
        let yaml = "\
$id: test
$schema: https://example/v0.5.0/schema.json
regulatory_layer: WET
publication_date: '2025-01-01'
valid_from: '2025-01-01'
bwb_id: BWBR0000000
articles:
  - number: '1'
    text: a
  - number: '2'
    text: b
";
        let h = parse_law_header(yaml);
        assert_eq!(h.id.as_deref(), Some("test"));
        assert_eq!(h.regulatory_layer.as_deref(), Some("WET"));
        assert_eq!(h.publication_date.as_deref(), Some("2025-01-01"));
        assert_eq!(h.valid_from.as_deref(), Some("2025-01-01"));
        assert_eq!(h.bwb_id.as_deref(), Some("BWBR0000000"));
        assert!(h.schema.as_deref().unwrap().contains("v0.5.0"));
        assert_eq!(h.article_count, 2);
    }

    #[test]
    fn tolerates_malformed_body() {
        // Header is still read even when the body is not valid YAML.
        let yaml = "$id: drafty\nname: Draft\narticles:\n  - number: '1'\n    text: \"unterminated";
        let h = parse_law_header(yaml);
        assert_eq!(h.id.as_deref(), Some("drafty"));
        assert_eq!(h.name.as_deref(), Some("Draft"));
        assert_eq!(h.article_count, 1);
    }
}
