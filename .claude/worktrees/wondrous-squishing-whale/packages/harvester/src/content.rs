//! Content (consolidated legal text) file downloading.
//!
//! Content files contain the actual legal text of Dutch laws in XML format.

use reqwest::blocking::Client;

use crate::config::content_url;
use crate::error::{HarvesterError, Result};
use crate::http::{bytes_to_string, download_bytes};

/// Download content XML for a law at a specific date.
///
/// Uses the consolidated version URL pattern with _0 suffix to get the
/// most recent version of the law as of the specified date.
///
/// # Arguments
/// * `client` - HTTP client to use
/// * `bwb_id` - The BWB identifier (e.g., "BWBR0018451")
/// * `date` - The effective date in YYYY-MM-DD format
/// * `max_size` - Maximum response size in bytes
///
/// # Returns
/// Raw XML content as a string
pub fn download_content_xml(
    client: &Client,
    bwb_id: &str,
    date: &str,
    max_size: u64,
) -> Result<String> {
    let url = content_url(bwb_id, date);
    let bytes = download_bytes(client, &url, max_size).map_err(|e| {
        if let HarvesterError::Http(source) = e {
            HarvesterError::ContentDownload {
                bwb_id: bwb_id.to_string(),
                date: date.to_string(),
                source,
            }
        } else {
            e
        }
    })?;

    Ok(bytes_to_string(bytes, &format!("content XML for {bwb_id}")))
}

#[cfg(test)]
mod tests {
    // Integration tests would require mock server or real network access
    // See tests/integration/ for those
}
