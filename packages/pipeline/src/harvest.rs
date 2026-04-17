use std::collections::HashSet;
use std::path::{Path, PathBuf};

use chrono::Utc;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::error::Result;
use regelrecht_harvester::manifest;

/// Maximum recursion depth for follow-up harvest jobs.
/// Prevents unbounded job creation from circular or deeply nested law references.
pub const MAX_HARVEST_DEPTH: u32 = 1000;

/// Payload for a harvest job, stored as JSON in the job queue.
///
/// Supports both BWB (national) and CVDR (decentral) law sources.
/// Exactly one of `bwb_id` or `cvdr_id` should be set.
///
/// The `bwb_id` field uses `#[serde(default)]` for backward compatibility:
/// existing queued jobs serialized as `{"bwb_id": "BWBR..."}` (a plain String)
/// will deserialize correctly because serde treats a present string value as
/// `Some(...)`, while new payloads omit it entirely when only `cvdr_id` is set.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HarvestPayload {
    /// BWB identifier for national laws (e.g. "BWBR0018451").
    /// Previously a required `String`; now optional to support CVDR sources.
    #[serde(default)]
    pub bwb_id: Option<String>,
    /// CVDR identifier for decentral regulations (e.g. "CVDR681386").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cvdr_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_size_mb: Option<u64>,
    /// Current recursion depth for follow-up harvests. `None` or `0` means this is a root job.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub depth: Option<u32>,
}

impl HarvestPayload {
    /// Returns the law identifier (BWB or CVDR) for this payload.
    pub fn law_id(&self) -> Option<&str> {
        self.bwb_id.as_deref().or(self.cvdr_id.as_deref())
    }
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
    /// Source type: "bwb" or "cvdr".
    #[serde(default = "default_source_type")]
    pub source_type: String,
}

fn default_source_type() -> String {
    "bwb".to_string()
}

/// Status file written alongside the law YAML.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LawStatusFile {
    /// Law identifier (BWB or CVDR).
    pub law_id: String,
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
/// Routes to BWB or CVDR download based on the payload fields:
/// - `cvdr_id` set → CVDR (decentral regulations)
/// - `bwb_id` set → BWB (national laws)
/// - neither set → error
///
/// Returns the harvest result and a list of file paths that were written
/// (for git staging).
pub async fn execute_harvest(
    payload: &HarvestPayload,
    repo_path: &Path,
    output_base: &str,
    http_client: &Client,
) -> Result<(HarvestResult, Vec<PathBuf>)> {
    if let Some(ref cvdr_id) = payload.cvdr_id {
        execute_harvest_cvdr(cvdr_id, payload, repo_path, output_base, http_client).await
    } else if let Some(ref bwb_id) = payload.bwb_id {
        execute_harvest_bwb(bwb_id, payload, repo_path, output_base, http_client).await
    } else {
        Err(crate::error::PipelineError::InvalidInput(
            "harvest payload must have either bwb_id or cvdr_id".into(),
        ))
    }
}

/// Execute a BWB harvest (national laws).
async fn execute_harvest_bwb(
    bwb_id: &str,
    payload: &HarvestPayload,
    repo_path: &Path,
    output_base: &str,
    http_client: &Client,
) -> Result<(HarvestResult, Vec<PathBuf>)> {
    let bwb_manifest = manifest::download_manifest(http_client, bwb_id).await?;
    let effective_date =
        manifest::resolve_consolidation_date(&bwb_manifest, payload.date.as_deref())?;
    tracing::info!(bwb_id = %bwb_id, resolved_date = %effective_date, "resolved consolidation date from manifest");

    tracing::info!(bwb_id = %bwb_id, date = %effective_date, "downloading law XML from BWB");
    let law = if let Some(max_mb) = payload.max_size_mb {
        regelrecht_harvester::download_law_with_max_size(
            http_client,
            bwb_id,
            &effective_date,
            max_mb,
        )
        .await?
    } else {
        regelrecht_harvester::download_law(http_client, bwb_id, &effective_date).await?
    };

    tracing::info!(bwb_id = %bwb_id, title = %law.metadata.title, "law XML downloaded successfully");
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
        .filter(|id| id != bwb_id)
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
        law_id: bwb_id.to_string(),
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
        source_type: "bwb".to_string(),
    };

    let written_files = vec![yaml_path, status_file_path];
    Ok((result, written_files))
}

/// Execute a CVDR harvest (decentral regulations).
async fn execute_harvest_cvdr(
    cvdr_id: &str,
    payload: &HarvestPayload,
    repo_path: &Path,
    output_base: &str,
    http_client: &Client,
) -> Result<(HarvestResult, Vec<PathBuf>)> {
    tracing::info!(cvdr_id = %cvdr_id, "downloading law from CVDR");

    let law =
        regelrecht_harvester::download_cvdr_law(http_client, cvdr_id, payload.date.as_deref())
            .await?;

    tracing::info!(cvdr_id = %cvdr_id, title = %law.metadata.title, "CVDR law downloaded successfully");
    let law_name = law.metadata.title.clone();
    let slug = law.metadata.to_slug();
    let layer = law.metadata.regulatory_layer.as_str().to_string();
    let article_count = law.articles.len();
    let warning_count = law.warning_count();
    let warnings = law.warnings.clone();

    // CVDR laws typically don't have cross-references to BWB laws in the same way,
    // but we still collect any references that exist.
    let mut referenced_bwb_ids: Vec<String> = law
        .articles
        .iter()
        .flat_map(|a| a.references.iter())
        .map(|r| r.bwb_id.clone())
        .filter(|id| id != cvdr_id)
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    referenced_bwb_ids.sort();

    // Determine effective date — use the requested date, metadata date, or today
    let effective_date = payload
        .date
        .clone()
        .or_else(|| law.metadata.effective_date.clone())
        .unwrap_or_else(|| Utc::now().format("%Y-%m-%d").to_string());

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
        law_id: cvdr_id.to_string(),
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
        source_type: "cvdr".to_string(),
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
            bwb_id: Some("BWBR0018451".to_string()),
            cvdr_id: None,
            date: Some("2025-01-01".to_string()),
            max_size_mb: Some(100),
            depth: Some(2),
        };

        let json = serde_json::to_string(&payload).unwrap();
        let deserialized: HarvestPayload = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.bwb_id.as_deref(), Some("BWBR0018451"));
        assert_eq!(deserialized.date.as_deref(), Some("2025-01-01"));
        assert_eq!(deserialized.max_size_mb, Some(100));
        assert_eq!(deserialized.depth, Some(2));
    }

    #[test]
    fn test_harvest_payload_minimal_bwb() {
        let json = r#"{"bwb_id":"BWBR0018451"}"#;
        let payload: HarvestPayload = serde_json::from_str(json).unwrap();

        assert_eq!(payload.bwb_id.as_deref(), Some("BWBR0018451"));
        assert!(payload.cvdr_id.is_none());
        assert!(payload.date.is_none());
        assert!(payload.max_size_mb.is_none());
        assert!(payload.depth.is_none());
    }

    #[test]
    fn test_harvest_payload_cvdr() {
        let json = r#"{"cvdr_id":"CVDR681386"}"#;
        let payload: HarvestPayload = serde_json::from_str(json).unwrap();

        assert!(payload.bwb_id.is_none());
        assert_eq!(payload.cvdr_id.as_deref(), Some("CVDR681386"));
    }

    #[test]
    fn test_harvest_payload_law_id_helper() {
        let bwb_payload = HarvestPayload {
            bwb_id: Some("BWBR0018451".to_string()),
            cvdr_id: None,
            date: None,
            max_size_mb: None,
            depth: None,
        };
        assert_eq!(bwb_payload.law_id(), Some("BWBR0018451"));

        let cvdr_payload = HarvestPayload {
            bwb_id: None,
            cvdr_id: Some("CVDR681386".to_string()),
            date: None,
            max_size_mb: None,
            depth: None,
        };
        assert_eq!(cvdr_payload.law_id(), Some("CVDR681386"));

        let empty_payload = HarvestPayload {
            bwb_id: None,
            cvdr_id: None,
            date: None,
            max_size_mb: None,
            depth: None,
        };
        assert_eq!(empty_payload.law_id(), None);
    }

    /// Backward compatibility: existing queued jobs with `bwb_id` as a plain String
    /// must still deserialize correctly.
    #[test]
    fn test_harvest_payload_backward_compat_plain_string() {
        let json = r#"{"bwb_id":"BWBR0018451","date":"2025-01-01"}"#;
        let payload: HarvestPayload = serde_json::from_str(json).unwrap();
        assert_eq!(payload.bwb_id.as_deref(), Some("BWBR0018451"));
        assert!(payload.cvdr_id.is_none());
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
            source_type: "bwb".to_string(),
        };

        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["law_name"], "Wet op de zorgtoeslag");
        assert_eq!(json["article_count"], 10);
        assert_eq!(json["harvest_date"], "2025-01-01");
        assert_eq!(json["source_type"], "bwb");

        let refs = json["referenced_bwb_ids"].as_array().unwrap();
        assert_eq!(refs.len(), 2);
        assert_eq!(refs[0], "BWBR0002629");
        assert_eq!(refs[1], "BWBR0018450");
    }

    /// Backward compatibility: HarvestResult without source_type defaults to "bwb".
    #[test]
    fn test_harvest_result_default_source_type() {
        let json = r#"{"law_name":"test","slug":"test","layer":"WET","file_path":"test.yaml","article_count":0,"warning_count":0,"warnings":[],"referenced_bwb_ids":[],"harvest_date":"2025-01-01"}"#;
        let result: HarvestResult = serde_json::from_str(json).unwrap();
        assert_eq!(result.source_type, "bwb");
    }

    #[test]
    fn test_law_status_file_serde() {
        let status = LawStatusFile {
            law_id: "BWBR0018451".to_string(),
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
        assert!(yaml.contains("law_id: BWBR0018451"));
        assert!(yaml.contains("status: harvested"));
    }

    #[test]
    fn test_law_status_file_cvdr() {
        let status = LawStatusFile {
            law_id: "CVDR681386".to_string(),
            law_name: "Verordening test".to_string(),
            slug: "verordening_test".to_string(),
            status: "harvested".to_string(),
            last_harvested: "2025-01-01T00:00:00Z".to_string(),
            harvest_date: "2025-01-01".to_string(),
            article_count: 5,
            warning_count: 0,
            warnings: vec![],
        };

        let yaml = serde_yaml_ng::to_string(&status).unwrap();
        assert!(yaml.contains("law_id: CVDR681386"));
    }
}
