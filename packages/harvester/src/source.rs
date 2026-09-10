//! Strategy trait for downloading laws from different sources.
//!
//! Provides a unified interface over BWB (national laws) and CVDR
//! (decentral regulations), so consumers can work with a single
//! `dyn LawSource` instead of if/else routing.

use async_trait::async_trait;
use reqwest::Client;

use crate::config::{
    lokaleregelgeving_url, validate_bwb_id, validate_cvdr_id, DEFAULT_MAX_RESPONSE_SIZE,
};
use crate::error::{HarvesterError, Result};
use crate::types::Law;

/// Enum identifying the law source type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LawSourceType {
    /// National law from BWB (Basiswettenbestand).
    Bwb,
    /// Decentral regulation from CVDR.
    Cvdr,
    /// EU instrument from EUR-Lex / CELLAR (phase 2 — stub only).
    EurLex,
}

/// Strategy trait for downloading laws from different sources.
#[async_trait]
pub trait LawSource: Send + Sync {
    /// Validate that the given ID is valid for this source.
    fn validate_id(&self, id: &str) -> Result<()>;

    /// Download and parse a law.
    async fn download(&self, client: &Client, id: &str, date: Option<&str>) -> Result<Law>;

    /// Build the public URL for a law.
    fn public_url(&self, id: &str) -> String;

    /// Source name for logging/display.
    fn name(&self) -> &'static str;

    /// Source type enum for type-safe dispatch.
    fn source_type(&self) -> LawSourceType;
}

/// BWB (Basiswettenbestand) source for national laws.
#[derive(Debug, Default)]
pub struct BwbSource {
    /// Maximum response size in megabytes.
    pub max_size_mb: Option<u64>,
}

#[async_trait]
impl LawSource for BwbSource {
    fn validate_id(&self, id: &str) -> Result<()> {
        validate_bwb_id(id)
    }

    async fn download(&self, client: &Client, id: &str, date: Option<&str>) -> Result<Law> {
        // BWB requires a date; default to today if not provided.
        let effective_date = date
            .map(String::from)
            .unwrap_or_else(regelrecht_shared::dates::today_str);

        let max_mb = self
            .max_size_mb
            .unwrap_or(DEFAULT_MAX_RESPONSE_SIZE / (1024 * 1024));

        crate::harvester::download_law_with_max_size(client, id, &effective_date, max_mb).await
    }

    fn public_url(&self, id: &str) -> String {
        format!("https://wetten.overheid.nl/{id}")
    }

    fn name(&self) -> &'static str {
        "BWB"
    }

    fn source_type(&self) -> LawSourceType {
        LawSourceType::Bwb
    }
}

/// CVDR (Centrale Voorziening Decentrale Regelgeving) source for decentral regulations.
#[derive(Debug, Default)]
pub struct CvdrSource;

#[async_trait]
impl LawSource for CvdrSource {
    fn validate_id(&self, id: &str) -> Result<()> {
        validate_cvdr_id(id)
    }

    async fn download(&self, client: &Client, id: &str, date: Option<&str>) -> Result<Law> {
        crate::cvdr::download_cvdr_law(client, id, date).await
    }

    fn public_url(&self, id: &str) -> String {
        lokaleregelgeving_url(id)
    }

    fn name(&self) -> &'static str {
        "CVDR"
    }

    fn source_type(&self) -> LawSourceType {
        LawSourceType::Cvdr
    }
}

/// EUR-Lex / CELLAR source (phase 2 stub).
///
/// Accepts CELEX-style identifiers so callers and the registry can already
/// route EU ids. Download is not implemented yet — see
/// `docs/src/content/docs/concepts/eu-corpus-ingest.md`.
#[derive(Debug, Default)]
pub struct EurLexSource;

/// CELEX numbers look like `32016R0679` (sector+year+type+number) or the
/// prefixed form `CELEX:32016R0679`.
pub fn is_celex_id(id: &str) -> bool {
    let bare = id
        .strip_prefix("CELEX:")
        .or_else(|| id.strip_prefix("celex:"))
        .unwrap_or(id);
    let bytes = bare.as_bytes();
    if bytes.len() < 8 || !bytes[0].is_ascii_digit() {
        return false;
    }
    // Year is positions 1-4; type letter at 5 (R/L/D/…); rest digits.
    bytes[1..5].iter().all(|b| b.is_ascii_digit())
        && bytes[5].is_ascii_alphabetic()
        && bytes[6..].iter().all(|b| b.is_ascii_digit())
}

#[async_trait]
impl LawSource for EurLexSource {
    fn validate_id(&self, id: &str) -> Result<()> {
        if is_celex_id(id) {
            Ok(())
        } else {
            Err(HarvesterError::InvalidLawId(id.to_string()))
        }
    }

    async fn download(&self, _client: &Client, id: &str, _date: Option<&str>) -> Result<Law> {
        Err(HarvesterError::NotImplemented(format!(
            "EUR-Lex download for '{id}' is not implemented yet (phase 2)"
        )))
    }

    fn public_url(&self, id: &str) -> String {
        let bare = id
            .strip_prefix("CELEX:")
            .or_else(|| id.strip_prefix("celex:"))
            .unwrap_or(id);
        format!("https://eur-lex.europa.eu/legal-content/EN/ALL/?uri=CELEX:{bare}")
    }

    fn name(&self) -> &'static str {
        "EUR-Lex"
    }

    fn source_type(&self) -> LawSourceType {
        LawSourceType::EurLex
    }
}

/// Detect the law source from an ID and return the appropriate strategy.
///
/// # Errors
/// Returns `HarvesterError::InvalidLawId` if the ID prefix is not recognized.
///
/// # Examples
/// ```
/// use regelrecht_harvester::source::detect_source;
///
/// let source = detect_source("BWBR0018451").unwrap();
/// assert_eq!(source.name(), "BWB");
///
/// let source = detect_source("CVDR681386").unwrap();
/// assert_eq!(source.name(), "CVDR");
///
/// assert!(detect_source("INVALID").is_err());
/// ```
pub fn detect_source(law_id: &str) -> Result<Box<dyn LawSource>> {
    let source: Box<dyn LawSource> = if law_id.starts_with("BWBR") {
        Box::new(BwbSource::default())
    } else if law_id.starts_with("CVDR") {
        Box::new(CvdrSource)
    } else if is_celex_id(law_id) {
        Box::new(EurLexSource)
    } else {
        return Err(HarvesterError::InvalidLawId(law_id.to_string()));
    };
    source.validate_id(law_id)?;
    Ok(source)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_bwb_source() {
        let source = detect_source("BWBR0018451").unwrap();
        assert_eq!(source.name(), "BWB");
    }

    #[test]
    fn detect_cvdr_source() {
        let source = detect_source("CVDR681386").unwrap();
        assert_eq!(source.name(), "CVDR");
    }

    #[test]
    fn detect_eurlex_source() {
        let source = detect_source("32016R0679").unwrap();
        assert_eq!(source.name(), "EUR-Lex");
        assert_eq!(source.source_type(), LawSourceType::EurLex);
        assert!(source.public_url("32016R0679").contains("CELEX:32016R0679"));
    }

    #[test]
    fn eurlex_rejects_non_celex() {
        let source = EurLexSource;
        assert!(source.validate_id("NOTCELEX").is_err());
    }

    #[test]
    fn detect_invalid_source() {
        assert!(detect_source("INVALID").is_err());
        assert!(detect_source("").is_err());
    }

    #[test]
    fn bwb_source_validates_id() {
        let source = BwbSource::default();
        assert!(source.validate_id("BWBR0018451").is_ok());
        assert!(source.validate_id("INVALID").is_err());
    }

    #[test]
    fn cvdr_source_validates_id() {
        let source = CvdrSource;
        assert!(source.validate_id("CVDR681386").is_ok());
        assert!(source.validate_id("INVALID").is_err());
    }

    #[test]
    fn bwb_source_public_url() {
        let source = BwbSource::default();
        assert_eq!(
            source.public_url("BWBR0018451"),
            "https://wetten.overheid.nl/BWBR0018451"
        );
    }

    #[test]
    fn cvdr_source_public_url() {
        let source = CvdrSource;
        assert_eq!(
            source.public_url("CVDR681386"),
            "https://lokaleregelgeving.overheid.nl/CVDR681386"
        );
    }

    #[test]
    fn bwb_source_default_max_size() {
        let source = BwbSource::default();
        assert!(source.max_size_mb.is_none());
    }

    #[test]
    fn bwb_source_custom_max_size() {
        let source = BwbSource {
            max_size_mb: Some(200),
        };
        assert_eq!(source.max_size_mb, Some(200));
    }
}
