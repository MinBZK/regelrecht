//! BWB manifest parsing for consolidation date resolution.
//!
//! The BWB repository doesn't have a consolidation for every date — the manifest.xml
//! file contains all available consolidation dates with their validity periods.
//! This module downloads and parses the manifest to find the correct consolidation date.

use reqwest::blocking::Client;
use roxmltree::Document;

use crate::config::{manifest_url, DEFAULT_MAX_RESPONSE_SIZE};
use crate::error::{HarvesterError, Result};
use crate::http::{bytes_to_string, download_bytes};

/// Parsed BWB manifest containing available consolidations.
#[derive(Debug)]
pub struct BwbManifest {
    /// The `_latestItem` attribute from the `<work>` element (e.g. "2026-02-04_0/xml/BWBR0015703_2026-02-04_0.xml").
    pub latest_item: String,
    /// All consolidation expressions with their validity periods.
    pub expressions: Vec<Consolidation>,
}

/// A single consolidation expression from the manifest.
#[derive(Debug)]
pub struct Consolidation {
    /// Label attribute (e.g. "2026-02-04_0").
    pub label: String,
    /// Start of validity period (e.g. "2026-02-04").
    pub datum_inwerkingtreding: String,
    /// End of validity period (e.g. "9999-12-31" for current version).
    pub einddatum: String,
}

/// Download and parse the BWB manifest for a law.
///
/// # Arguments
/// * `client` - HTTP client to use
/// * `bwb_id` - The BWB identifier (e.g., "BWBR0015703")
pub fn download_manifest(client: &Client, bwb_id: &str) -> Result<BwbManifest> {
    let url = manifest_url(bwb_id);
    let bytes = download_bytes(client, &url, DEFAULT_MAX_RESPONSE_SIZE).map_err(|e| {
        if let HarvesterError::Http(source) = e {
            HarvesterError::ManifestDownload {
                bwb_id: bwb_id.to_string(),
                source,
            }
        } else {
            e
        }
    })?;

    let xml = bytes_to_string(bytes, &format!("manifest for {bwb_id}"));
    parse_manifest(&xml, bwb_id)
}

/// Parse manifest XML into a `BwbManifest`.
fn parse_manifest(xml: &str, bwb_id: &str) -> Result<BwbManifest> {
    let doc = Document::parse(xml)?;
    let root = doc.root_element();

    // Find the <work> element
    let work = root
        .descendants()
        .find(|n| n.has_tag_name("work"))
        .ok_or_else(|| HarvesterError::MissingElement {
            element: "work".to_string(),
            context: format!("manifest for {bwb_id}"),
        })?;

    let latest_item = work
        .attribute("_latestItem")
        .ok_or_else(|| HarvesterError::MissingElement {
            element: "_latestItem attribute".to_string(),
            context: format!("manifest for {bwb_id}"),
        })?
        .to_string();

    let mut expressions = Vec::new();
    for expr in work.descendants().filter(|n| n.has_tag_name("expression")) {
        let label = expr.attribute("label").unwrap_or_default().to_string();

        let datum_inwerkingtreding = expr
            .descendants()
            .find(|n| n.has_tag_name("datum_inwerkingtreding"))
            .and_then(|n| n.text())
            .unwrap_or_default()
            .to_string();

        let einddatum = expr
            .descendants()
            .find(|n| n.has_tag_name("einddatum"))
            .and_then(|n| n.text())
            .unwrap_or("9999-12-31")
            .to_string();

        if !label.is_empty() && !datum_inwerkingtreding.is_empty() {
            expressions.push(Consolidation {
                label,
                datum_inwerkingtreding,
                einddatum,
            });
        }
    }

    Ok(BwbManifest {
        latest_item,
        expressions,
    })
}

/// Extract the date from a `_latestItem` path or label.
///
/// Handles both full paths like "2026-02-04_0/xml/BWBR0015703_2026-02-04_0.xml"
/// and labels like "2026-02-04_0".
fn extract_date_from_item(item: &str) -> Option<&str> {
    // Get the first path segment (before any '/')
    let segment = item.split('/').next().unwrap_or(item);
    // Strip the trailing "_0" (or "_N") version suffix to get the date
    segment.rsplit_once('_').map(|(date, _)| date)
}

/// Resolve the correct consolidation date from a manifest.
///
/// - `None` date: returns the latest available consolidation date (from `_latestItem`)
/// - `Some(date)`: finds the consolidation where `datum_inwerkingtreding <= date <= einddatum`
///
/// # Arguments
/// * `manifest` - Parsed BWB manifest
/// * `date` - Optional target date in YYYY-MM-DD format
///
/// # Returns
/// The consolidation date to use (YYYY-MM-DD format)
pub fn resolve_consolidation_date(manifest: &BwbManifest, date: Option<&str>) -> Result<String> {
    match date {
        None => {
            // No date specified: use the latest consolidation
            extract_date_from_item(&manifest.latest_item)
                .map(|d| d.to_string())
                .ok_or_else(|| HarvesterError::MissingElement {
                    element: "date in _latestItem".to_string(),
                    context: format!("_latestItem: {}", manifest.latest_item),
                })
        }
        Some(target_date) => {
            // Find the consolidation whose validity period covers the target date
            for consolidation in &manifest.expressions {
                if consolidation.datum_inwerkingtreding.as_str() <= target_date
                    && target_date <= consolidation.einddatum.as_str()
                {
                    return Ok(consolidation.datum_inwerkingtreding.clone());
                }
            }

            Err(HarvesterError::NoConsolidation {
                bwb_id: extract_bwb_id_from_latest(&manifest.latest_item),
                date: target_date.to_string(),
            })
        }
    }
}

/// Extract BWB ID from the `_latestItem` path for error messages.
#[allow(clippy::expect_used)]
fn extract_bwb_id_from_latest(latest_item: &str) -> String {
    use regex::Regex;
    use std::sync::LazyLock;

    static BWB_FINDER: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"BWB[A-Z]\d{7}").expect("valid regex"));

    BWB_FINDER
        .find(latest_item)
        .map(|m| m.as_str().to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_MANIFEST: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<repository>
  <work _latestItem="2026-02-04_0/xml/BWBR0015703_2026-02-04_0.xml">
    <expression label="2024-01-01_0">
      <metadata>
        <datum_inwerkingtreding>2024-01-01</datum_inwerkingtreding>
        <einddatum>2024-12-31</einddatum>
      </metadata>
    </expression>
    <expression label="2025-07-01_0">
      <metadata>
        <datum_inwerkingtreding>2025-07-01</datum_inwerkingtreding>
        <einddatum>2025-12-31</einddatum>
      </metadata>
    </expression>
    <expression label="2026-02-04_0">
      <metadata>
        <datum_inwerkingtreding>2026-02-04</datum_inwerkingtreding>
        <einddatum>9999-12-31</einddatum>
      </metadata>
    </expression>
  </work>
</repository>"#;

    #[test]
    fn test_parse_manifest() {
        let manifest = parse_manifest(SAMPLE_MANIFEST, "BWBR0015703").unwrap();

        assert_eq!(
            manifest.latest_item,
            "2026-02-04_0/xml/BWBR0015703_2026-02-04_0.xml"
        );
        assert_eq!(manifest.expressions.len(), 3);

        assert_eq!(manifest.expressions[0].label, "2024-01-01_0");
        assert_eq!(manifest.expressions[0].datum_inwerkingtreding, "2024-01-01");
        assert_eq!(manifest.expressions[0].einddatum, "2024-12-31");

        assert_eq!(manifest.expressions[2].label, "2026-02-04_0");
        assert_eq!(manifest.expressions[2].einddatum, "9999-12-31");
    }

    #[test]
    fn test_resolve_no_date_returns_latest() {
        let manifest = parse_manifest(SAMPLE_MANIFEST, "BWBR0015703").unwrap();
        let result = resolve_consolidation_date(&manifest, None).unwrap();
        assert_eq!(result, "2026-02-04");
    }

    #[test]
    fn test_resolve_date_within_period() {
        let manifest = parse_manifest(SAMPLE_MANIFEST, "BWBR0015703").unwrap();

        // Date falls in the 2025-07-01 to 2025-12-31 period
        let result = resolve_consolidation_date(&manifest, Some("2025-09-15")).unwrap();
        assert_eq!(result, "2025-07-01");

        // Date falls in the 2024-01-01 to 2024-12-31 period
        let result = resolve_consolidation_date(&manifest, Some("2024-06-15")).unwrap();
        assert_eq!(result, "2024-01-01");

        // Date falls in the current (open-ended) period
        let result = resolve_consolidation_date(&manifest, Some("2026-03-01")).unwrap();
        assert_eq!(result, "2026-02-04");
    }

    #[test]
    fn test_resolve_date_on_boundary() {
        let manifest = parse_manifest(SAMPLE_MANIFEST, "BWBR0015703").unwrap();

        // Exactly on datum_inwerkingtreding
        let result = resolve_consolidation_date(&manifest, Some("2025-07-01")).unwrap();
        assert_eq!(result, "2025-07-01");

        // Exactly on einddatum
        let result = resolve_consolidation_date(&manifest, Some("2024-12-31")).unwrap();
        assert_eq!(result, "2024-01-01");
    }

    #[test]
    fn test_resolve_date_no_match() {
        let manifest = parse_manifest(SAMPLE_MANIFEST, "BWBR0015703").unwrap();

        // Date before any consolidation
        let result = resolve_consolidation_date(&manifest, Some("2023-01-01"));
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert!(err.to_string().contains("2023-01-01"));
    }

    #[test]
    fn test_resolve_date_in_gap() {
        let manifest = parse_manifest(SAMPLE_MANIFEST, "BWBR0015703").unwrap();

        // Date between two periods (2025-01-01 to 2025-06-30 is a gap)
        let result = resolve_consolidation_date(&manifest, Some("2025-03-15"));
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_date_from_item() {
        assert_eq!(
            extract_date_from_item("2026-02-04_0/xml/BWBR0015703_2026-02-04_0.xml"),
            Some("2026-02-04")
        );
        assert_eq!(extract_date_from_item("2025-07-01_0"), Some("2025-07-01"));
        assert_eq!(extract_date_from_item("invalid"), None);
    }

    #[test]
    fn test_extract_bwb_id_from_latest() {
        assert_eq!(
            extract_bwb_id_from_latest("2026-02-04_0/xml/BWBR0015703_2026-02-04_0.xml"),
            "BWBR0015703"
        );
        assert_eq!(extract_bwb_id_from_latest("no_id_here"), "unknown");
    }
}
