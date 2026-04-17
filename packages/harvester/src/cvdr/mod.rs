//! CVDR (Centrale Voorziening Decentrale Regelgeving) harvester module.
//!
//! Downloads decentrale regelgeving (municipal, provincial, and water board
//! regulations) from lokaleregelgeving.overheid.nl via the SRU search API.
//!
//! # Flow
//!
//! 1. SRU search to get metadata and XML content URL
//! 2. Download XML content from the resolved URL
//! 3. Parse articles from CVDR XML format
//! 4. Return a `Law` object compatible with the existing YAML generation pipeline

pub mod content;
pub mod parse;
pub mod search;

use reqwest::Client;

use crate::config::{lokaleregelgeving_url, validate_cvdr_id, validate_date};
use crate::error::Result;
use crate::types::Law;
use content::download_cvdr_content;
use parse::parse_cvdr_articles;
use search::search_cvdr;

/// Download and parse a CVDR law.
///
/// # Arguments
/// * `client` - HTTP client to use
/// * `cvdr_id` - The CVDR identifier (e.g., "CVDR681386")
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

    // Step 1: SRU search to get metadata
    let metadata_result = search_cvdr(client, cvdr_id).await?;

    // Step 2: Download XML content
    let xml_content = download_cvdr_content(client, &metadata_result.xml_url, cvdr_id).await?;

    // Step 3: Parse articles from CVDR XML
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
