//! Core data types for the harvester.
//!
//! These types represent Dutch legal documents and their components,
//! matching the Python harvester's `models.py`.

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;
use unicode_normalization::UnicodeNormalization;

use crate::config::wetten_url;

/// Re-export the canonical regulatory layer types from the shared crate.
pub use regelrecht_shared::RegulatoryLayer;

/// Parse a WTI "soort-regeling" text into a `RegulatoryLayer`.
///
/// Returns `(layer, warning)` where the warning is `Some` for ambiguous mappings
/// or unknown types. Direct matches produce no warning.
#[must_use]
pub fn regulatory_layer_from_soort_regeling(text: &str) -> (RegulatoryLayer, Option<String>) {
    match text.to_lowercase().as_str() {
        "grondwet" => (RegulatoryLayer::Grondwet, None),
        "wet" => (RegulatoryLayer::Wet, None),
        "amvb" | "algemene maatregel van bestuur" => (RegulatoryLayer::Amvb, None),
        "ministeriele regeling" | "ministeriële regeling" => {
            (RegulatoryLayer::MinisterieleRegeling, None)
        }
        "beleidsregel" => (RegulatoryLayer::Beleidsregel, None),
        "eu-verordening" => (RegulatoryLayer::EuVerordening, None),
        "eu-richtlijn" => (RegulatoryLayer::EuRichtlijn, None),
        "verdrag" => (RegulatoryLayer::Verdrag, None),
        "uitvoeringsbeleid" => (RegulatoryLayer::Uitvoeringsbeleid, None),
        "gemeentelijke verordening" => (RegulatoryLayer::GemeentelijkeVerordening, None),
        "provinciale verordening" => (RegulatoryLayer::ProvincialeVerordening, None),
        "waterschapsverordening" => (RegulatoryLayer::WaterschapsVerordening, None),
        // Ambiguous mappings - produce warnings
        "koninklijk besluit" | "kb" => (
            RegulatoryLayer::Amvb,
            Some(format!(
                "Mapped soort-regeling '{text}' to AMVB (closest schema match)"
            )),
        ),
        "regeling" => (
            RegulatoryLayer::MinisterieleRegeling,
            Some(format!(
                "Mapped soort-regeling '{text}' to MINISTERIELE_REGELING (closest schema match)"
            )),
        ),
        "verordening" => (
            RegulatoryLayer::GemeentelijkeVerordening,
            Some(format!(
                "Mapped soort-regeling '{text}' to GEMEENTELIJKE_VERORDENING (closest schema match)"
            )),
        ),
        unknown => {
            tracing::warn!(
                soort_regeling = %unknown,
                "Unknown soort-regeling type, defaulting to WET"
            );
            (
                RegulatoryLayer::Wet,
                Some(format!(
                    "Unknown soort-regeling '{unknown}', defaulting to WET"
                )),
            )
        }
    }
}

/// Metadata extracted from WTI file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LawMetadata {
    /// BWB identifier (e.g., "BWBR0018451").
    pub bwb_id: String,

    /// Official title (citeertitel).
    pub title: String,

    /// Type of regulatory document.
    pub regulatory_layer: RegulatoryLayer,

    /// Publication date (optional).
    pub publication_date: Option<String>,

    /// Effective date (optional, usually set from request).
    pub effective_date: Option<String>,
}

/// Regex for slug generation - matches non-word characters.
#[allow(clippy::expect_used)] // Static regex that is guaranteed to be valid
static SLUG_NON_WORD: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[^\w\s-]").expect("valid regex"));

/// Regex for slug generation - matches whitespace and dashes.
#[allow(clippy::expect_used)] // Static regex that is guaranteed to be valid
static SLUG_SPACE_DASH: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[-\s]+").expect("valid regex"));

impl LawMetadata {
    /// Generate a URL-friendly slug from the title.
    ///
    /// Uses NFKD normalization to convert accented characters (like é, ë, ü)
    /// to their ASCII equivalents before filtering.
    ///
    /// # Examples
    /// ```
    /// use regelrecht_harvester::types::{LawMetadata, RegulatoryLayer};
    ///
    /// let metadata = LawMetadata {
    ///     bwb_id: "BWBR0018451".to_string(),
    ///     title: "Wet op de zorgtoeslag".to_string(),
    ///     regulatory_layer: RegulatoryLayer::Wet,
    ///     publication_date: None,
    ///     effective_date: None,
    /// };
    /// assert_eq!(metadata.to_slug(), "wet_op_de_zorgtoeslag");
    /// ```
    #[must_use]
    pub fn to_slug(&self) -> String {
        // Normalize using NFKD to decompose accented characters,
        // then filter to ASCII only (this removes the combining diacritics)
        let text: String = self
            .title
            .to_lowercase()
            .nfkd()
            .filter(|c| c.is_ascii())
            .collect();
        let text = SLUG_NON_WORD.replace_all(&text, "");
        let text = SLUG_SPACE_DASH.replace_all(&text, "_");
        text.trim_matches('_').to_string()
    }
}

/// A reference to another article or law.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reference {
    /// Unique identifier for this reference (e.g., "ref1").
    pub id: String,

    /// BWB identifier of the referenced law.
    pub bwb_id: String,

    /// Article number (optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artikel: Option<String>,

    /// Paragraph number (lid) (optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lid: Option<String>,

    /// Subdivision (onderdeel) (optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub onderdeel: Option<String>,

    /// Chapter (hoofdstuk) (optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hoofdstuk: Option<String>,

    /// Section (paragraaf) (optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub paragraaf: Option<String>,

    /// Division (afdeling) (optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub afdeling: Option<String>,
}

impl Reference {
    /// Create a new reference with just BWB ID.
    #[must_use]
    pub fn new(id: impl Into<String>, bwb_id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            bwb_id: bwb_id.into(),
            artikel: None,
            lid: None,
            onderdeel: None,
            hoofdstuk: None,
            paragraaf: None,
            afdeling: None,
        }
    }

    /// Generate wetten.overheid.nl URL.
    ///
    /// # Arguments
    /// * `date` - Optional date for versioned URL
    #[must_use]
    pub fn to_wetten_url(&self, date: Option<&str>) -> String {
        wetten_url(
            &self.bwb_id,
            date,
            self.artikel.as_deref(),
            self.hoofdstuk.as_deref(),
            self.afdeling.as_deref(),
            self.paragraaf.as_deref(),
        )
    }
}

/// Format references as markdown reference definitions.
///
/// # Arguments
/// * `references` - List of references to format
///
/// # Returns
/// Markdown reference definitions, e.g.:
/// ```text
/// [ref1]: https://wetten.overheid.nl/BWBR0018451#Artikel4
/// [ref2]: https://wetten.overheid.nl/BWBR0018450#Artikel1
/// ```
#[must_use]
pub fn format_reference_definitions(references: &[Reference]) -> String {
    if references.is_empty() {
        return String::new();
    }

    references
        .iter()
        .map(|r| format!("[{}]: {}", r.id, r.to_wetten_url(None)))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Preamble (aanhef) section of a law.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Preamble {
    /// Preamble text content.
    pub text: String,

    /// URL to the preamble on wetten.overheid.nl.
    pub url: String,
}

/// A single article from a law.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Article {
    /// Article number (e.g., "1", "1.1", "1.1.a").
    pub number: String,

    /// Article text content.
    pub text: String,

    /// URL to the article on wetten.overheid.nl.
    pub url: String,

    /// References contained in this article.
    pub references: Vec<Reference>,
}

impl Article {
    /// Create a new article.
    #[must_use]
    pub fn new(number: impl Into<String>, text: impl Into<String>, url: impl Into<String>) -> Self {
        Self {
            number: number.into(),
            text: text.into(),
            url: url.into(),
            references: Vec::new(),
        }
    }

    /// Create an article with references.
    #[must_use]
    pub fn with_references(mut self, references: Vec<Reference>) -> Self {
        self.references = references;
        self
    }
}

/// Complete law with metadata and articles.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Law {
    /// Metadata from WTI file.
    pub metadata: LawMetadata,

    /// Preamble (aanhef) section, if present.
    pub preamble: Option<Preamble>,

    /// List of articles.
    pub articles: Vec<Article>,

    /// Non-fatal warnings encountered during parsing.
    ///
    /// These indicate data that may have been skipped or improperly handled.
    /// Callers can inspect this to assess data quality.
    pub warnings: Vec<String>,
}

impl Law {
    /// Create a new law with metadata.
    #[must_use]
    pub fn new(metadata: LawMetadata) -> Self {
        Self {
            metadata,
            preamble: None,
            articles: Vec::new(),
            warnings: Vec::new(),
        }
    }

    /// Add an article to the law.
    pub fn add_article(&mut self, article: Article) {
        self.articles.push(article);
    }

    /// Add a warning to the law.
    pub fn add_warning(&mut self, warning: impl Into<String>) {
        self.warnings.push(warning.into());
    }

    /// Get the number of warnings.
    #[must_use]
    pub fn warning_count(&self) -> usize {
        self.warnings.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_regulatory_layer_as_str() {
        assert_eq!(RegulatoryLayer::Wet.as_str(), "WET");
        assert_eq!(RegulatoryLayer::Amvb.as_str(), "AMVB");
        assert_eq!(
            RegulatoryLayer::MinisterieleRegeling.as_str(),
            "MINISTERIELE_REGELING"
        );
    }

    #[test]
    fn test_regulatory_layer_as_dir_name() {
        assert_eq!(RegulatoryLayer::Wet.as_dir_name(), "wet");
        assert_eq!(
            RegulatoryLayer::MinisterieleRegeling.as_dir_name(),
            "ministeriele_regeling"
        );
    }

    #[test]
    fn test_regulatory_layer_from_soort_regeling() {
        // Direct matches - no warning
        assert_eq!(
            regulatory_layer_from_soort_regeling("wet"),
            (RegulatoryLayer::Wet, None)
        );
        assert_eq!(
            regulatory_layer_from_soort_regeling("WET"),
            (RegulatoryLayer::Wet, None)
        );
        assert_eq!(
            regulatory_layer_from_soort_regeling("amvb"),
            (RegulatoryLayer::Amvb, None)
        );
        assert_eq!(
            regulatory_layer_from_soort_regeling("algemene maatregel van bestuur"),
            (RegulatoryLayer::Amvb, None)
        );
        assert_eq!(
            regulatory_layer_from_soort_regeling("ministeriële regeling"),
            (RegulatoryLayer::MinisterieleRegeling, None)
        );
        assert_eq!(
            regulatory_layer_from_soort_regeling("grondwet"),
            (RegulatoryLayer::Grondwet, None)
        );
        assert_eq!(
            regulatory_layer_from_soort_regeling("eu-verordening"),
            (RegulatoryLayer::EuVerordening, None)
        );
        assert_eq!(
            regulatory_layer_from_soort_regeling("verdrag"),
            (RegulatoryLayer::Verdrag, None)
        );

        // Ambiguous mappings - produce warnings
        let (layer, warning) = regulatory_layer_from_soort_regeling("koninklijk besluit");
        assert_eq!(layer, RegulatoryLayer::Amvb);
        assert!(warning.is_some());

        let (layer, warning) = regulatory_layer_from_soort_regeling("regeling");
        assert_eq!(layer, RegulatoryLayer::MinisterieleRegeling);
        assert!(warning.is_some());

        let (layer, warning) = regulatory_layer_from_soort_regeling("verordening");
        assert_eq!(layer, RegulatoryLayer::GemeentelijkeVerordening);
        assert!(warning.is_some());

        // Unknown defaults to Wet with warning
        let (layer, warning) = regulatory_layer_from_soort_regeling("unknown");
        assert_eq!(layer, RegulatoryLayer::Wet);
        assert!(warning.is_some());
    }

    #[test]
    fn test_law_metadata_to_slug() {
        let metadata = LawMetadata {
            bwb_id: "BWBR0018451".to_string(),
            title: "Wet op de zorgtoeslag".to_string(),
            regulatory_layer: RegulatoryLayer::Wet,
            publication_date: None,
            effective_date: None,
        };
        assert_eq!(metadata.to_slug(), "wet_op_de_zorgtoeslag");
    }

    #[test]
    fn test_law_metadata_to_slug_special_chars() {
        let metadata = LawMetadata {
            bwb_id: "BWBR0000000".to_string(),
            title: "Wet (test) - special!".to_string(),
            regulatory_layer: RegulatoryLayer::Wet,
            publication_date: None,
            effective_date: None,
        };
        assert_eq!(metadata.to_slug(), "wet_test_special");
    }

    #[test]
    fn test_law_metadata_to_slug_diacritics() {
        let metadata = LawMetadata {
            bwb_id: "BWBR0000000".to_string(),
            title: "Ministeriële regeling".to_string(),
            regulatory_layer: RegulatoryLayer::MinisterieleRegeling,
            publication_date: None,
            effective_date: None,
        };
        // ë should be normalized to e
        assert_eq!(metadata.to_slug(), "ministeriele_regeling");
    }

    #[test]
    fn test_law_metadata_to_slug_various_diacritics() {
        let metadata = LawMetadata {
            bwb_id: "BWBR0000000".to_string(),
            title: "Café résumé naïve".to_string(),
            regulatory_layer: RegulatoryLayer::Wet,
            publication_date: None,
            effective_date: None,
        };
        // é, é, ï should all be normalized to their ASCII equivalents
        assert_eq!(metadata.to_slug(), "cafe_resume_naive");
    }

    #[test]
    fn test_reference_to_wetten_url() {
        let reference = Reference {
            id: "ref1".to_string(),
            bwb_id: "BWBR0018451".to_string(),
            artikel: Some("4".to_string()),
            lid: None,
            onderdeel: None,
            hoofdstuk: None,
            paragraaf: None,
            afdeling: None,
        };

        assert_eq!(
            reference.to_wetten_url(None),
            "https://wetten.overheid.nl/BWBR0018451#Artikel4"
        );
    }

    #[test]
    fn test_reference_to_wetten_url_chapter() {
        let reference = Reference {
            id: "ref1".to_string(),
            bwb_id: "BWBR0009950".to_string(),
            artikel: None,
            lid: None,
            onderdeel: None,
            hoofdstuk: Some("5a".to_string()),
            paragraaf: None,
            afdeling: None,
        };

        assert_eq!(
            reference.to_wetten_url(None),
            "https://wetten.overheid.nl/BWBR0009950#Hoofdstuk5a"
        );
    }

    #[test]
    fn test_reference_to_wetten_url_section() {
        let reference = Reference {
            id: "ref1".to_string(),
            bwb_id: "BWBR0009950".to_string(),
            artikel: None,
            lid: None,
            onderdeel: None,
            hoofdstuk: None,
            paragraaf: None,
            afdeling: Some("3.1".to_string()),
        };

        assert_eq!(
            reference.to_wetten_url(None),
            "https://wetten.overheid.nl/BWBR0009950#Afdeling3.1"
        );
    }

    #[test]
    fn test_reference_to_wetten_url_paragraph() {
        let reference = Reference {
            id: "ref1".to_string(),
            bwb_id: "BWBR0009950".to_string(),
            artikel: None,
            lid: None,
            onderdeel: None,
            hoofdstuk: None,
            paragraaf: Some("2".to_string()),
            afdeling: None,
        };

        assert_eq!(
            reference.to_wetten_url(None),
            "https://wetten.overheid.nl/BWBR0009950#Paragraaf2"
        );
    }

    #[test]
    fn test_reference_to_wetten_url_priority() {
        // Article takes priority over chapter
        let reference = Reference {
            id: "ref1".to_string(),
            bwb_id: "BWBR0009950".to_string(),
            artikel: Some("1".to_string()),
            lid: None,
            onderdeel: None,
            hoofdstuk: Some("5a".to_string()),
            paragraaf: None,
            afdeling: None,
        };

        assert_eq!(
            reference.to_wetten_url(None),
            "https://wetten.overheid.nl/BWBR0009950#Artikel1"
        );
    }

    #[test]
    fn test_format_reference_definitions() {
        let references = vec![
            Reference::new("ref1", "BWBR0018451"),
            Reference {
                id: "ref2".to_string(),
                bwb_id: "BWBR0018450".to_string(),
                artikel: Some("1".to_string()),
                ..Reference::new("", "")
            },
        ];

        let result = format_reference_definitions(&references);
        assert!(result.contains("[ref1]: https://wetten.overheid.nl/BWBR0018451"));
        assert!(result.contains("[ref2]: https://wetten.overheid.nl/BWBR0018450#Artikel1"));
    }

    #[test]
    fn test_format_reference_definitions_empty() {
        let references: Vec<Reference> = vec![];
        assert_eq!(format_reference_definitions(&references), "");
    }

    #[test]
    fn test_article_creation() {
        let article = Article::new("1", "Test text", "https://example.com");
        assert_eq!(article.number, "1");
        assert_eq!(article.text, "Test text");
        assert!(article.references.is_empty());
    }

    #[test]
    fn test_law_add_article() {
        let metadata = LawMetadata {
            bwb_id: "BWBR0018451".to_string(),
            title: "Test Law".to_string(),
            regulatory_layer: RegulatoryLayer::Wet,
            publication_date: None,
            effective_date: None,
        };

        let mut law = Law::new(metadata);
        assert!(law.articles.is_empty());
        assert!(law.warnings.is_empty());

        law.add_article(Article::new("1", "Text", "url"));
        assert_eq!(law.articles.len(), 1);

        law.add_warning("Test warning");
        assert_eq!(law.warning_count(), 1);
        assert_eq!(law.warnings[0], "Test warning");
    }

    #[test]
    fn test_regulatory_layer_serialization() {
        assert_eq!(
            serde_json::to_string(&RegulatoryLayer::Wet).unwrap(),
            "\"WET\""
        );
        assert_eq!(
            serde_json::to_string(&RegulatoryLayer::MinisterieleRegeling).unwrap(),
            "\"MINISTERIELE_REGELING\""
        );
    }
}
