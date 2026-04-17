//! CVDR XML content downloading.
//!
//! Downloads the regulation XML from the URL resolved during SRU search.

use reqwest::Client;

use crate::error::{HarvesterError, Result};
use crate::http::{bytes_to_string, download_bytes_default};

/// Download CVDR regulation XML content.
///
/// # Arguments
/// * `client` - HTTP client to use
/// * `xml_url` - URL to the XML content (resolved from SRU search)
/// * `cvdr_id` - The CVDR identifier (for error context)
///
/// # Returns
/// Raw XML content as a string
pub async fn download_cvdr_content(
    client: &Client,
    xml_url: &str,
    cvdr_id: &str,
) -> Result<String> {
    let bytes = download_bytes_default(client, xml_url).await.map_err(|e| {
        if let HarvesterError::Http(source) = e {
            HarvesterError::CvdrContentDownload {
                cvdr_id: cvdr_id.to_string(),
                source,
            }
        } else {
            e
        }
    })?;

    Ok(bytes_to_string(
        bytes,
        &format!("CVDR XML content for {cvdr_id}"),
    ))
}
