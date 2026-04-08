//! Configuration constants and validation functions for the harvester.

use regex::Regex;
use std::sync::LazyLock;

use crate::error::{HarvesterError, Result};

/// Base URL for BWB (Basiswettenbestand) repository.
pub const BWB_REPOSITORY_URL: &str = "https://repository.officiele-overheidspublicaties.nl/bwb";

/// HTTP timeout in seconds.
///
/// Set to 30 seconds to accommodate large XML files and slow connections.
pub const HTTP_TIMEOUT_SECS: u64 = 30;

/// Default maximum HTTP response size in bytes (100 MB).
///
/// This prevents downloading unexpectedly large files that could exhaust memory.
/// Can be overridden via CLI --max-size flag for exceptionally large laws like
/// Wet op het financieel toezicht (52.6 MB).
pub const DEFAULT_MAX_RESPONSE_SIZE: u64 = 100 * 1024 * 1024;

/// Schema URL for regelrecht YAML files.
pub const SCHEMA_URL: &str = "https://raw.githubusercontent.com/MinBZK/regelrecht-mvp/refs/heads/main/schema/v0.3.1/schema.json";

/// Text wrap width for YAML output.
/// Accounts for up to 6 spaces of YAML block scalar indent (125 - 6 = 119, with margin).
pub const TEXT_WRAP_WIDTH: usize = 115;

/// BWB ID pattern: BWBR followed by 7 digits.
#[allow(clippy::expect_used)] // Static regex that is guaranteed to be valid
static BWB_ID_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^BWBR\d{7}$").expect("valid regex"));

/// Date pattern: YYYY-MM-DD.
#[allow(clippy::expect_used)] // Static regex that is guaranteed to be valid
static DATE_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\d{4}-\d{2}-\d{2}$").expect("valid regex"));

/// Validate BWB ID format.
///
/// # Arguments
/// * `bwb_id` - The BWB identifier to validate
///
/// # Returns
/// * `Ok(())` if valid
/// * `Err(HarvesterError::InvalidBwbId)` if invalid
///
/// # Examples
/// ```
/// use regelrecht_harvester::config::validate_bwb_id;
///
/// assert!(validate_bwb_id("BWBR0018451").is_ok());
/// assert!(validate_bwb_id("INVALID").is_err());
/// ```
pub fn validate_bwb_id(bwb_id: &str) -> Result<()> {
    if BWB_ID_PATTERN.is_match(bwb_id) {
        Ok(())
    } else {
        Err(HarvesterError::InvalidBwbId(bwb_id.to_string()))
    }
}

/// Validate date format (YYYY-MM-DD).
///
/// Rejects dates in the future since BWB won't have consolidated versions for them.
///
/// # Arguments
/// * `date_str` - Date string to validate
///
/// # Returns
/// * `Ok(())` if valid format, valid date, and not in the future
/// * `Err(HarvesterError::InvalidDate)` if invalid
///
/// # Examples
/// ```
/// use regelrecht_harvester::config::validate_date;
///
/// assert!(validate_date("2025-01-01").is_ok());
/// assert!(validate_date("invalid").is_err());
/// assert!(validate_date("2025-13-01").is_err()); // Invalid month
/// ```
pub fn validate_date(date_str: &str) -> Result<()> {
    if !DATE_PATTERN.is_match(date_str) {
        return Err(HarvesterError::InvalidDate(date_str.to_string()));
    }

    // Parse and validate it's a real date
    let parsed_date = chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
        .map_err(|_| HarvesterError::InvalidDate(date_str.to_string()))?;

    // Reject future dates - BWB won't have consolidated versions for them
    let today = chrono::Local::now().date_naive();
    if parsed_date > today {
        return Err(HarvesterError::InvalidDate(format!(
            "{date_str} is in the future (today is {today})"
        )));
    }

    Ok(())
}

/// Build manifest URL for a law.
///
/// # Arguments
/// * `bwb_id` - The BWB identifier (should be validated with `validate_bwb_id` first)
///
/// # Returns
/// URL to the manifest.xml file
///
/// # Panics
/// Debug builds panic if bwb_id doesn't match expected format.
pub fn manifest_url(bwb_id: &str) -> String {
    debug_assert!(
        BWB_ID_PATTERN.is_match(bwb_id),
        "bwb_id should be validated before calling manifest_url"
    );
    format!("{BWB_REPOSITORY_URL}/{bwb_id}/manifest.xml")
}

/// Build WTI (metadata) URL for a law.
///
/// # Arguments
/// * `bwb_id` - The BWB identifier (should be validated with `validate_bwb_id` first)
///
/// # Returns
/// URL to the WTI file
///
/// # Panics
/// Debug builds panic if bwb_id doesn't match expected format.
pub fn wti_url(bwb_id: &str) -> String {
    debug_assert!(
        BWB_ID_PATTERN.is_match(bwb_id),
        "bwb_id should be validated before calling wti_url"
    );
    format!("{BWB_REPOSITORY_URL}/{bwb_id}/{bwb_id}.WTI")
}

/// Build content (consolidated XML) URL for a law at a specific date.
///
/// # Arguments
/// * `bwb_id` - The BWB identifier (should be validated with `validate_bwb_id` first)
/// * `date` - The effective date in YYYY-MM-DD format (should be validated with `validate_date` first)
///
/// # Returns
/// URL to the consolidated XML file
///
/// # Panics
/// Debug builds panic if inputs don't match expected formats.
pub fn content_url(bwb_id: &str, date: &str) -> String {
    debug_assert!(
        BWB_ID_PATTERN.is_match(bwb_id),
        "bwb_id should be validated before calling content_url"
    );
    debug_assert!(
        DATE_PATTERN.is_match(date),
        "date should be validated before calling content_url"
    );
    format!("{BWB_REPOSITORY_URL}/{bwb_id}/{date}_0/xml/{bwb_id}_{date}_0.xml")
}

/// Sanitize a URL fragment identifier by removing problematic characters.
///
/// This ensures fragment IDs are safe for use in URLs and don't contain
/// characters that could cause issues (like quotes, angle brackets, etc.).
///
/// # Examples
/// ```
/// use regelrecht_harvester::config::sanitize_fragment;
///
/// assert_eq!(sanitize_fragment("1a"), "1a");
/// assert_eq!(sanitize_fragment("3.1"), "3.1");
/// assert_eq!(sanitize_fragment("1<script>"), "1script");
/// ```
pub fn sanitize_fragment(fragment: &str) -> String {
    fragment
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_' || *c == '.' || *c == '~')
        .collect()
}

/// Build wetten.overheid.nl URL for a law.
///
/// # Arguments
/// * `bwb_id` - The BWB identifier
/// * `date` - Optional effective date
/// * `article` - Optional article number for fragment
/// * `chapter` - Optional chapter (hoofdstuk) for fragment
/// * `section` - Optional section (afdeling) for fragment
/// * `paragraph` - Optional paragraph (paragraaf) for fragment
///
/// Anchor priority: artikel > hoofdstuk > afdeling > paragraaf
///
/// # Returns
/// Public URL to wetten.overheid.nl
pub fn wetten_url(
    bwb_id: &str,
    date: Option<&str>,
    article: Option<&str>,
    chapter: Option<&str>,
    section: Option<&str>,
    paragraph: Option<&str>,
) -> String {
    let mut url = format!("https://wetten.overheid.nl/{bwb_id}");

    if let Some(d) = date {
        url.push('/');
        url.push_str(d);
    }

    // Anchor priority: artikel > hoofdstuk > afdeling > paragraaf
    // Sanitize all fragment values to prevent injection of problematic characters
    if let Some(a) = article {
        url.push_str("#Artikel");
        url.push_str(&sanitize_fragment(&a.replace(' ', "_")));
    } else if let Some(h) = chapter {
        url.push_str("#Hoofdstuk");
        url.push_str(&sanitize_fragment(&h.replace(' ', "_")));
    } else if let Some(a) = section {
        url.push_str("#Afdeling");
        url.push_str(&sanitize_fragment(&a.replace(' ', "_")));
    } else if let Some(p) = paragraph {
        url.push_str("#Paragraaf");
        url.push_str(&sanitize_fragment(&p.replace(' ', "_")));
    }

    url
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_bwb_id_valid() {
        assert!(validate_bwb_id("BWBR0018451").is_ok());
        assert!(validate_bwb_id("BWBR0000001").is_ok());
        assert!(validate_bwb_id("BWBR9999999").is_ok());
    }

    #[test]
    fn test_validate_bwb_id_invalid() {
        assert!(validate_bwb_id("").is_err());
        assert!(validate_bwb_id("BWBR001845").is_err()); // 6 digits
        assert!(validate_bwb_id("BWBR00184512").is_err()); // 8 digits
        assert!(validate_bwb_id("BWBX0018451").is_err()); // Wrong prefix
        assert!(validate_bwb_id("bwbr0018451").is_err()); // Lowercase
    }

    #[test]
    fn test_validate_date_valid() {
        assert!(validate_date("2025-01-01").is_ok());
        assert!(validate_date("2024-12-31").is_ok());
        assert!(validate_date("2000-06-15").is_ok());
    }

    #[test]
    fn test_validate_date_invalid_format() {
        assert!(validate_date("").is_err());
        assert!(validate_date("2025/01/01").is_err());
        assert!(validate_date("01-01-2025").is_err());
        assert!(validate_date("2025-1-1").is_err());
    }

    #[test]
    fn test_validate_date_invalid_date() {
        assert!(validate_date("2025-13-01").is_err()); // Invalid month
        assert!(validate_date("2025-02-30").is_err()); // Invalid day
        assert!(validate_date("2025-00-01").is_err()); // Zero month
    }

    #[test]
    fn test_manifest_url() {
        assert_eq!(
            manifest_url("BWBR0015703"),
            "https://repository.officiele-overheidspublicaties.nl/bwb/BWBR0015703/manifest.xml"
        );
    }

    #[test]
    fn test_wti_url() {
        assert_eq!(
            wti_url("BWBR0018451"),
            "https://repository.officiele-overheidspublicaties.nl/bwb/BWBR0018451/BWBR0018451.WTI"
        );
    }

    #[test]
    fn test_content_url() {
        assert_eq!(
            content_url("BWBR0018451", "2025-01-01"),
            "https://repository.officiele-overheidspublicaties.nl/bwb/BWBR0018451/2025-01-01_0/xml/BWBR0018451_2025-01-01_0.xml"
        );
    }

    #[test]
    fn test_wetten_url() {
        assert_eq!(
            wetten_url("BWBR0018451", None, None, None, None, None),
            "https://wetten.overheid.nl/BWBR0018451"
        );

        assert_eq!(
            wetten_url("BWBR0018451", Some("2025-01-01"), None, None, None, None),
            "https://wetten.overheid.nl/BWBR0018451/2025-01-01"
        );

        assert_eq!(
            wetten_url(
                "BWBR0018451",
                Some("2025-01-01"),
                Some("1"),
                None,
                None,
                None
            ),
            "https://wetten.overheid.nl/BWBR0018451/2025-01-01#Artikel1"
        );

        // Test space replacement in article numbers
        assert_eq!(
            wetten_url(
                "BWBR0018451",
                Some("2025-01-01"),
                Some("A 1"),
                None,
                None,
                None
            ),
            "https://wetten.overheid.nl/BWBR0018451/2025-01-01#ArtikelA_1"
        );
    }

    #[test]
    fn test_wetten_url_chapter() {
        assert_eq!(
            wetten_url("BWBR0009950", None, None, Some("5a"), None, None),
            "https://wetten.overheid.nl/BWBR0009950#Hoofdstuk5a"
        );
    }

    #[test]
    fn test_wetten_url_section() {
        assert_eq!(
            wetten_url("BWBR0009950", None, None, None, Some("3.1"), None),
            "https://wetten.overheid.nl/BWBR0009950#Afdeling3.1"
        );
    }

    #[test]
    fn test_wetten_url_paragraph() {
        assert_eq!(
            wetten_url("BWBR0009950", None, None, None, None, Some("2")),
            "https://wetten.overheid.nl/BWBR0009950#Paragraaf2"
        );
    }

    #[test]
    fn test_wetten_url_anchor_priority() {
        // Article takes priority over chapter
        assert_eq!(
            wetten_url("BWBR0009950", None, Some("1"), Some("5a"), None, None),
            "https://wetten.overheid.nl/BWBR0009950#Artikel1"
        );

        // Chapter takes priority over section
        assert_eq!(
            wetten_url("BWBR0009950", None, None, Some("5a"), Some("3.1"), None),
            "https://wetten.overheid.nl/BWBR0009950#Hoofdstuk5a"
        );

        // Section takes priority over paragraph
        assert_eq!(
            wetten_url("BWBR0009950", None, None, None, Some("3.1"), Some("2")),
            "https://wetten.overheid.nl/BWBR0009950#Afdeling3.1"
        );
    }

    #[test]
    fn test_sanitize_fragment() {
        // Normal text passes through
        assert_eq!(sanitize_fragment("1a"), "1a");
        assert_eq!(sanitize_fragment("Artikel1"), "Artikel1");

        // Dots, underscores, hyphens, tildes are allowed
        assert_eq!(sanitize_fragment("3.1"), "3.1");
        assert_eq!(sanitize_fragment("A_1"), "A_1");
        assert_eq!(sanitize_fragment("test-case"), "test-case");

        // Special characters are stripped
        assert_eq!(sanitize_fragment("1<script>"), "1script");
        assert_eq!(sanitize_fragment("test\"quote"), "testquote");
        assert_eq!(sanitize_fragment("1&amp;2"), "1amp2");
    }

    #[test]
    fn test_wetten_url_sanitizes_fragments() {
        // Article with potentially dangerous characters should be sanitized
        assert_eq!(
            wetten_url(
                "BWBR0009950",
                None,
                Some("1<script>alert('xss')</script>"),
                None,
                None,
                None
            ),
            "https://wetten.overheid.nl/BWBR0009950#Artikel1scriptalertxssscript"
        );
    }
}
