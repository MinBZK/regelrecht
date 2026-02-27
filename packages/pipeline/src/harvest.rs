use std::path::{Path, PathBuf};

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::error::Result;

/// Payload for a harvest job, stored as JSON in the job queue.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HarvestPayload {
    pub bwb_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_size_mb: Option<u64>,
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
) -> Result<(HarvestResult, Vec<PathBuf>)> {
    let effective_date = payload
        .date
        .clone()
        .unwrap_or_else(|| Utc::now().format("%Y-%m-%d").to_string());
    let bwb_id = payload.bwb_id.clone();
    let date_for_download = effective_date.clone();
    let max_size_mb = payload.max_size_mb;

    let law = tokio::task::spawn_blocking(move || {
        if let Some(max_mb) = max_size_mb {
            regelrecht_harvester::download_law_with_max_size(&bwb_id, &date_for_download, max_mb)
        } else {
            regelrecht_harvester::download_law(&bwb_id, &date_for_download)
        }
    })
    .await??;

    let law_name = law.metadata.title.clone();
    let slug = law.metadata.to_slug();
    let layer = law.metadata.regulatory_layer.as_str().to_string();
    let article_count = law.articles.len();
    let warning_count = law.warning_count();
    let warnings = law.warnings.clone();

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
        harvest_date: effective_date,
        article_count,
        warning_count,
        warnings: warnings.clone(),
    };

    let status_yaml = serde_yaml_ng::to_string(&status)?;
    let status_content = format!("---\n{status_yaml}");
    tokio::fs::write(&status_file_path, status_content).await?;

    let result = HarvestResult {
        law_name,
        slug,
        layer,
        file_path: yaml_path.to_string_lossy().to_string(),
        article_count,
        warning_count,
        warnings,
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
        };

        let json = serde_json::to_string(&payload).unwrap();
        let deserialized: HarvestPayload = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.bwb_id, "BWBR0018451");
        assert_eq!(deserialized.date.as_deref(), Some("2025-01-01"));
        assert_eq!(deserialized.max_size_mb, Some(100));
    }

    #[test]
    fn test_harvest_payload_minimal() {
        let json = r#"{"bwb_id":"BWBR0018451"}"#;
        let payload: HarvestPayload = serde_json::from_str(json).unwrap();

        assert_eq!(payload.bwb_id, "BWBR0018451");
        assert!(payload.date.is_none());
        assert!(payload.max_size_mb.is_none());
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
        };

        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["law_name"], "Wet op de zorgtoeslag");
        assert_eq!(json["article_count"], 10);
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
