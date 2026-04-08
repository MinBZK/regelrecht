//! Types for the element registry system.

use std::fmt;

use crate::types::Reference;

/// Classification of element types for processing strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElementType {
    /// Container elements (artikel, lid, lijst).
    Structural,
    /// Text-level elements (extref, nadruk).
    Inline,
    /// Elements to ignore completely.
    Skip,
}

/// Result from parsing an element.
#[derive(Debug, Clone, Default)]
pub struct ParseResult {
    /// The extracted text content.
    pub text: String,
    /// Errors encountered during parsing (non-fatal).
    pub errors: Vec<String>,
}

impl ParseResult {
    /// Create a new parse result with text.
    #[must_use]
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            errors: Vec::new(),
        }
    }

    /// Create an empty parse result.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            text: String::new(),
            errors: Vec::new(),
        }
    }

    /// Create a parse result with an error.
    #[must_use]
    pub fn with_error(mut self, error: impl Into<String>) -> Self {
        self.errors.push(error.into());
        self
    }

    /// Merge another parse result's errors into this one.
    pub fn merge_errors(&mut self, other: &ParseResult) {
        self.errors.extend(other.errors.iter().cloned());
    }
}

/// Collector for reference-style links during parsing.
#[derive(Debug, Clone, Default)]
pub struct ReferenceCollector {
    /// Collected references.
    references: Vec<Reference>,
    /// Counter for generating unique reference IDs.
    counter: usize,
}

impl ReferenceCollector {
    /// Create a new empty collector.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a reference and return the markdown reference ID.
    ///
    /// # Arguments
    /// * `bwb_id` - BWB identifier of the referenced law
    /// * `artikel` - Optional article number
    ///
    /// # Returns
    /// Reference ID like "ref1" for use in markdown
    pub fn add_reference(&mut self, bwb_id: String, artikel: Option<String>) -> String {
        self.counter += 1;
        let ref_id = format!("ref{}", self.counter);

        let reference = Reference {
            id: ref_id.clone(),
            bwb_id,
            artikel,
            lid: None,
            onderdeel: None,
            hoofdstuk: None,
            paragraaf: None,
            afdeling: None,
        };

        self.references.push(reference);
        ref_id
    }

    /// Add a full reference object.
    pub fn add_full_reference(&mut self, mut reference: Reference) -> String {
        self.counter += 1;
        let ref_id = format!("ref{}", self.counter);
        reference.id = ref_id.clone();
        self.references.push(reference);
        ref_id
    }

    /// Get the collected references.
    #[must_use]
    pub fn references(&self) -> &[Reference] {
        &self.references
    }

    /// Take ownership of collected references.
    #[must_use]
    pub fn into_references(self) -> Vec<Reference> {
        self.references
    }

    /// Get the current count.
    #[must_use]
    pub fn count(&self) -> usize {
        self.counter
    }
}

/// Context passed through parsing operations.
pub struct ParseContext<'a> {
    /// Collector for reference-style links.
    pub collector: Option<&'a mut ReferenceCollector>,

    /// BWB identifier for the current law.
    pub bwb_id: String,

    /// Effective date in YYYY-MM-DD format.
    pub date: String,

    /// Current article number parts for building dot notation.
    pub number_parts: Vec<String>,

    /// Base URL for the current article.
    pub base_url: String,
}

impl<'a> ParseContext<'a> {
    /// Create a new parse context.
    #[must_use]
    pub fn new(bwb_id: impl Into<String>, date: impl Into<String>) -> Self {
        Self {
            collector: None,
            bwb_id: bwb_id.into(),
            date: date.into(),
            number_parts: Vec::new(),
            base_url: String::new(),
        }
    }

    /// Set the reference collector.
    #[must_use]
    pub fn with_collector(mut self, collector: &'a mut ReferenceCollector) -> Self {
        self.collector = Some(collector);
        self
    }

    /// Set the base URL.
    #[must_use]
    pub fn with_base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into();
        self
    }
}

impl fmt::Debug for ParseContext<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ParseContext")
            .field("bwb_id", &self.bwb_id)
            .field("date", &self.date)
            .field("number_parts", &self.number_parts)
            .field("base_url", &self.base_url)
            .field("has_collector", &self.collector.is_some())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_result_new() {
        let result = ParseResult::new("hello");
        assert_eq!(result.text, "hello");
    }

    #[test]
    fn test_parse_result_empty() {
        let result = ParseResult::empty();
        assert_eq!(result.text, "");
    }

    #[test]
    fn test_reference_collector_add() {
        let mut collector = ReferenceCollector::new();

        let id1 = collector.add_reference("BWBR0018451".to_string(), Some("1".to_string()));
        assert_eq!(id1, "ref1");

        let id2 = collector.add_reference("BWBR0018452".to_string(), None);
        assert_eq!(id2, "ref2");

        assert_eq!(collector.count(), 2);
        assert_eq!(collector.references().len(), 2);
    }

    #[test]
    fn test_parse_context_new() {
        let ctx = ParseContext::new("BWBR0018451", "2025-01-01");
        assert_eq!(ctx.bwb_id, "BWBR0018451");
        assert_eq!(ctx.date, "2025-01-01");
        assert!(ctx.collector.is_none());
    }
}
