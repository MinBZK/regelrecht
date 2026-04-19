//! CVDR (Centrale Voorziening Decentrale Regelgeving) harvester module.
//!
//! Downloads decentrale regelgeving (municipal, provincial, and water board
//! regulations) from lokaleregelgeving.overheid.nl via the SRU search API.
//!
//! # Flow
//!
//! 1. If the CVDR ID has no version suffix, resolve via manifest
//! 2. SRU search to get metadata (using versioned ID)
//! 3. Download XML content from the manifest-resolved URL (more reliable than SRU)
//! 4. Parse articles from CVDR XML format
//! 5. Return a `Law` object compatible with the existing YAML generation pipeline

pub mod content;
pub mod manifest;
pub mod parse;
pub mod search;

use reqwest::Client;

use crate::config::{lokaleregelgeving_url, validate_cvdr_id, validate_date};
use crate::error::Result;
use crate::types::Law;
use content::download_cvdr_content;
use manifest::resolve_latest_cvdr_version;
use parse::parse_cvdr_articles;
use search::search_cvdr;

/// Download and parse a CVDR law.
///
/// If the CVDR ID has no version suffix (e.g., "CVDR691525"), resolves the
/// latest version via the CVDR manifest first. Then uses the versioned ID
/// for the SRU metadata search and the manifest-resolved XML URL for content
/// download.
///
/// # Arguments
/// * `client` - HTTP client to use
/// * `cvdr_id` - The CVDR identifier (e.g., "CVDR681386" or "CVDR681386_1")
/// * `date` - Optional effective date in YYYY-MM-DD format
///
/// # Returns
/// A `Law` object containing metadata, articles, and any warnings encountered during parsing
pub async fn download_cvdr_law(client: &Client, cvdr_id: &str, date: Option<&str>) -> Result<Law> {
    // Validate inputs
    validate_cvdr_id(cvdr_id)?;
    if let Some(d) = date {
        validate_date(d)?;
    }

    // Step 1: Resolve version via manifest if needed
    let (versioned_id, manifest_xml_url) = if cvdr_id.contains('_') {
        // Already has a version suffix, use as-is
        (cvdr_id.to_string(), None)
    } else {
        // Bare ID — resolve latest version from manifest
        let version_info = resolve_latest_cvdr_version(client, cvdr_id).await?;
        tracing::info!(
            cvdr_id = %cvdr_id,
            versioned_id = %version_info.versioned_id,
            version = version_info.version,
            "Resolved CVDR version from manifest"
        );
        let xml_url = version_info.xml_url.clone();
        (version_info.versioned_id, Some(xml_url))
    };

    // Step 2: SRU search to get metadata (using versioned ID)
    let metadata_result = search_cvdr(client, &versioned_id, manifest_xml_url.as_deref()).await?;

    // Step 3: Download XML content
    // Prefer the manifest-resolved XML URL (more reliable than SRU enrichedData)
    let xml_url = manifest_xml_url
        .as_deref()
        .unwrap_or(&metadata_result.xml_url);
    let xml_content = download_cvdr_content(client, xml_url, &versioned_id).await?;

    // Step 4: Parse articles from CVDR XML
    let effective_date = date
        .map(String::from)
        .or_else(|| metadata_result.effective_date.clone())
        .unwrap_or_else(|| chrono::Local::now().format("%Y-%m-%d").to_string());

    let base_url = lokaleregelgeving_url(cvdr_id);
    let parsed = parse_cvdr_articles(&xml_content, &base_url)?;

    // Build metadata from SRU search result
    let law_metadata = metadata_result.to_law_metadata(&effective_date);

    // Combine warnings
    let mut warnings = metadata_result.warnings;
    warnings.extend(parsed.warnings);

    Ok(Law {
        metadata: law_metadata,
        preamble: None,
        articles: parsed.articles,
        warnings,
    })
}
