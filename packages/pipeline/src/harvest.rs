use std::collections::HashSet;
use std::path::{Path, PathBuf};

use chrono::Utc;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};

use crate::error::Result;
use regelrecht_harvester::manifest;

/// Maximum recursion depth for follow-up harvest jobs.
/// Prevents unbounded job creation from circular or deeply nested law references.
pub const MAX_HARVEST_DEPTH: u32 = 1000;

/// Payload for a harvest job, stored as JSON in the job queue.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HarvestPayload {
    pub bwb_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_size_mb: Option<u64>,
    /// Current recursion depth for follow-up harvests. `None` or `0` means this is a root job.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub depth: Option<u32>,
    /// User-configured maximum harvest depth. When not set, falls back to `MAX_HARVEST_DEPTH`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_depth: Option<u32>,
}

/// Result of a successful harvest execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HarvestResult {
    pub law_name: String,
    pub slug: String,
    pub layer: String,
    pub file_path: String,
    pub article_count: usize,
    pub warning_count: usize,
    pub warnings: Vec<String>,
    /// Unique BWB IDs referenced by this law's articles (excluding self-references).
    pub referenced_bwb_ids: Vec<String>,
    /// The resolved effective date used for this harvest.
    pub harvest_date: String,
}

/// Status file written alongside the law YAML.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LawStatusFile {
    pub bwb_id: String,
    pub law_name: String,
    pub slug: String,
    pub status: String,
    pub last_harvested: String,
    pub harvest_date: String,
    pub article_count: usize,
    pub warning_count: usize,
    pub warnings: Vec<String>,
}

/// Execute a harvest: download, parse, and save a law as YAML.
///
/// Returns the harvest result and a list of file paths that were written
/// (for git staging).
pub async fn execute_harvest(
    payload: &HarvestPayload,
    repo_path: &Path,
    output_base: &str,
    http_client: &Client,
) -> Result<(HarvestResult, Vec<PathBuf>)> {
    let bwb_id_for_manifest = payload.bwb_id.clone();
    let date_for_manifest = payload.date.clone();
    let client_for_manifest = http_client.clone();
    let effective_date = tokio::task::spawn_blocking(move || {
        let bwb_manifest = manifest::download_manifest(&client_for_manifest, &bwb_id_for_manifest)?;
        manifest::resolve_consolidation_date(&bwb_manifest, date_for_manifest.as_deref())
    })
    .await??;
    tracing::info!(bwb_id = %payload.bwb_id, resolved_date = %effective_date, "resolved consolidation date from manifest");
    let bwb_id = payload.bwb_id.clone();
    let date_for_download = effective_date.clone();
    let max_size_mb = payload.max_size_mb;

    tracing::info!(bwb_id = %payload.bwb_id, date = %effective_date, "downloading law XML from BWB");
    let law = tokio::task::spawn_blocking(move || {
        if let Some(max_mb) = max_size_mb {
            regelrecht_harvester::download_law_with_max_size(&bwb_id, &date_for_download, max_mb)
        } else {
            regelrecht_harvester::download_law(&bwb_id, &date_for_download)
        }
    })
    .await??;

    tracing::info!(bwb_id = %payload.bwb_id, title = %law.metadata.title, "law XML downloaded successfully");
    let law_name = law.metadata.title.clone();
    let slug = law.metadata.to_slug();
    let layer = law.metadata.regulatory_layer.as_str().to_string();
    let article_count = law.articles.len();
    let warning_count = law.warning_count();
    let warnings = law.warnings.clone();

    let mut referenced_bwb_ids: Vec<String> = law
        .articles
        .iter()
        .flat_map(|a| a.references.iter())
        .map(|r| r.bwb_id.clone())
        .filter(|id| id != &payload.bwb_id)
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    referenced_bwb_ids.sort();

    let output_base_path = repo_path.join(output_base);
    let law_for_save = law;
    let date_for_save = effective_date.clone();
    let yaml_path = tokio::task::spawn_blocking(move || {
        regelrecht_harvester::yaml::save_yaml(
            &law_for_save,
            &date_for_save,
            Some(&output_base_path),
        )
    })
    .await??;

    let status_file_path = yaml_path
        .parent()
        .map(|p| p.join("status.yaml"))
        .unwrap_or_else(|| PathBuf::from("status.yaml"));

    let status = LawStatusFile {
        bwb_id: payload.bwb_id.clone(),
        law_name: law_name.clone(),
        slug: slug.clone(),
        status: "harvested".to_string(),
        last_harvested: Utc::now().to_rfc3339(),
        harvest_date: effective_date.clone(),
        article_count,
        warning_count,
        warnings: warnings.clone(),
    };

    let status_yaml = serde_yaml_ng::to_string(&status)?;
    let status_content = format!("---\n{status_yaml}");
    tokio::fs::write(&status_file_path, status_content).await?;

    let relative_path = yaml_path
        .strip_prefix(repo_path)
        .unwrap_or(&yaml_path)
        .to_string_lossy()
        .to_string();

    let result = HarvestResult {
        law_name,
        slug,
        layer,
        file_path: relative_path,
        article_count,
        warning_count,
        warnings,
        referenced_bwb_ids,
        harvest_date: effective_date,
    };

    let written_files = vec![yaml_path, status_file_path];
    Ok((result, written_files))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_harvest_payload_serde_roundtrip() {
        let payload = HarvestPayload {
            bwb_id: "BWBR0018451".to_string(),
            date: Some("2025-01-01".to_string()),
            max_size_mb: Some(100),
            depth: Some(2),
            max_depth: Some(5),
        };

        let json = serde_json::to_string(&payload).unwrap();
        let deserialized: HarvestPayload = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.bwb_id, "BWBR0018451");
        assert_eq!(deserialized.date.as_deref(), Some("2025-01-01"));
        assert_eq!(deserialized.max_size_mb, Some(100));
        assert_eq!(deserialized.depth, Some(2));
        assert_eq!(deserialized.max_depth, Some(5));
    }

    #[test]
    fn test_harvest_payload_minimal() {
        let json = r#"{"bwb_id":"BWBR0018451"}"#;
        let payload: HarvestPayload = serde_json::from_str(json).unwrap();

        assert_eq!(payload.bwb_id, "BWBR0018451");
        assert!(payload.date.is_none());
        assert!(payload.max_size_mb.is_none());
        assert!(payload.depth.is_none());
    }

    #[test]
    fn test_harvest_result_serde() {
        let result = HarvestResult {
            law_name: "Wet op de zorgtoeslag".to_string(),
            slug: "wet_op_de_zorgtoeslag".to_string(),
            layer: "WET".to_string(),
            file_path: "regulation/nl/wet/wet_op_de_zorgtoeslag/2025-01-01.yaml".to_string(),
            article_count: 10,
            warning_count: 2,
            warnings: vec!["warning1".to_string(), "warning2".to_string()],
            referenced_bwb_ids: vec!["BWBR0002629".to_string(), "BWBR0018450".to_string()],
            harvest_date: "2025-01-01".to_string(),
        };

        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["law_name"], "Wet op de zorgtoeslag");
        assert_eq!(json["article_count"], 10);
        assert_eq!(json["harvest_date"], "2025-01-01");

        let refs = json["referenced_bwb_ids"].as_array().unwrap();
        assert_eq!(refs.len(), 2);
        assert_eq!(refs[0], "BWBR0002629");
        assert_eq!(refs[1], "BWBR0018450");
    }

    #[test]
    fn test_law_status_file_serde() {
        let status = LawStatusFile {
            bwb_id: "BWBR0018451".to_string(),
            law_name: "Wet op de zorgtoeslag".to_string(),
            slug: "wet_op_de_zorgtoeslag".to_string(),
            status: "harvested".to_string(),
            last_harvested: "2025-01-01T00:00:00Z".to_string(),
            harvest_date: "2025-01-01".to_string(),
            article_count: 10,
            warning_count: 0,
            warnings: vec![],
        };

        let yaml = serde_yaml_ng::to_string(&status).unwrap();
        assert!(yaml.contains("bwb_id: BWBR0018451"));
        assert!(yaml.contains("status: harvested"));
    }
}
