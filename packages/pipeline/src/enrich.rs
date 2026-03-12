use std::path::{Path, PathBuf};
use std::time::Duration;

use regelrecht_corpus::{CorpusClient, CorpusConfig};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::{PipelineError, Result};

/// Payload for an enrich job, stored as JSON in the job queue.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnrichPayload {
    pub law_id: String,
    /// Relative path to the harvested YAML file within the repo.
    pub yaml_path: String,
}

/// Result of a successful enrichment execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnrichResult {
    pub law_id: String,
    pub yaml_path: String,
    pub articles_total: usize,
    pub articles_enriched: usize,
    pub quality_score: f64,
    pub provider: String,
    pub branch: String,
}

/// Metadata written alongside the enriched law YAML as `.enrichment.yaml`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnrichmentMetadata {
    pub law_id: String,
    pub timestamp: String,
    pub provider: String,
    pub model: String,
    pub prompt_hash: String,
    pub code_commit: String,
    pub quality_score: f64,
    pub articles_total: usize,
    pub articles_enriched: usize,
}

/// Supported LLM providers for enrichment.
///
/// Both providers manage their own authentication:
/// - **OpenCode/VLAM**: reads `~/.local/share/opencode/auth.json` (set via `opencode auth`)
/// - **Claude**: reads `~/.claude/.credentials` or `ANTHROPIC_API_KEY` env var
///
/// In Docker, mount the appropriate auth files or set env vars.
#[derive(Debug, Clone)]
pub enum LlmProvider {
    OpenCode {
        path: PathBuf,
        model: Option<String>,
    },
    Claude {
        path: PathBuf,
        model: Option<String>,
    },
}

impl LlmProvider {
    /// Short name used in branch names and metadata.
    pub fn name(&self) -> &str {
        match self {
            LlmProvider::OpenCode { .. } => "opencode",
            LlmProvider::Claude { .. } => "claude",
        }
    }

    /// Model string for metadata (provider-specific default if not set).
    pub fn model_str(&self) -> String {
        match self {
            LlmProvider::OpenCode { model, .. } => {
                model.clone().unwrap_or_else(|| "default".into())
            }
            LlmProvider::Claude { model, .. } => model.clone().unwrap_or_else(|| "default".into()),
        }
    }
}

/// Configuration for enrichment execution.
#[derive(Debug, Clone)]
pub struct EnrichConfig {
    pub provider: LlmProvider,
    pub timeout: Duration,
    pub code_commit: String,
}

impl EnrichConfig {
    pub fn from_env() -> Self {
        let provider_name = std::env::var("LLM_PROVIDER").unwrap_or_else(|_| "opencode".into());

        let provider = match provider_name.as_str() {
            "claude" => {
                let path = std::env::var("LLM_PATH")
                    .unwrap_or_else(|_| "claude".into())
                    .into();
                let model = std::env::var("LLM_MODEL").ok();
                LlmProvider::Claude { path, model }
            }
            _ => {
                let path = std::env::var("LLM_PATH")
                    .unwrap_or_else(|_| "opencode".into())
                    .into();
                let model = std::env::var("LLM_MODEL").ok();
                LlmProvider::OpenCode { path, model }
            }
        };

        let timeout = std::env::var("LLM_TIMEOUT_SECS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(600);

        let code_commit = std::env::var("CODE_COMMIT").unwrap_or_default();

        Self {
            provider,
            timeout: Duration::from_secs(timeout),
            code_commit,
        }
    }
}

/// Build the enrichment branch name for a given provider and law.
///
/// Format: `enrich/{provider}/{law_slug}` where law_slug is derived from
/// the YAML path (e.g. `wet_op_de_zorgtoeslag`).
pub fn enrich_branch_name(provider_name: &str, yaml_path: &str) -> String {
    let slug = Path::new(yaml_path)
        .parent()
        .and_then(|p| p.file_name())
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown".into());

    format!("enrich/{provider_name}/{slug}")
}

/// Build the prompt that tells the LLM to follow the skill pipeline.
fn build_prompt(yaml_path: &str) -> String {
    format!(
        r#"You are interpreting a Dutch law to make it machine-executable.

The law YAML file is: {yaml_path}

Follow this pipeline in order. For each step, read the referenced skill file
and follow its instructions completely.

## Step 1: MvT Research
Read .claude/skills/law-mvt-research/SKILL.md and follow its instructions to
search for Memorie van Toelichting documents and generate Gherkin test scenarios.
If no MvT documents are found, proceed to step 2 anyway.

## Step 2: Generate machine_readable
Read .claude/skills/law-generate/SKILL.md and its reference.md and examples.md.
Follow the generate→validate→test loop to create machine_readable sections for
each executable article.

## Step 3: Reverse Validation
Read .claude/skills/law-reverse-validate/SKILL.md and follow its instructions
to verify every element in machine_readable traces back to the original legal text.

Write all changes to disk. Do not ask questions — proceed autonomously."#
    )
}

/// Build the command for the configured LLM provider.
///
/// The subprocess inherits the parent environment. Both providers manage their
/// own authentication:
/// - OpenCode/VLAM reads `~/.local/share/opencode/auth.json`
/// - Claude reads `~/.claude/.credentials` or `ANTHROPIC_API_KEY` env var
///
/// In Docker, mount the relevant auth files or set env vars on the container.
fn build_command(
    provider: &LlmProvider,
    prompt: &str,
    yaml_abs: &Path,
    repo_path: &Path,
) -> tokio::process::Command {
    match provider {
        LlmProvider::OpenCode { path, model } => {
            let mut cmd = tokio::process::Command::new(path);
            cmd.arg("run")
                .arg(prompt)
                .arg("-f")
                .arg(yaml_abs)
                .arg("--format")
                .arg("json")
                .arg("--dir")
                .arg(repo_path);
            if let Some(ref m) = model {
                cmd.arg("-m").arg(m);
            }
            cmd
        }
        LlmProvider::Claude { path, model } => {
            let mut cmd = tokio::process::Command::new(path);
            cmd.arg("-p")
                .arg(prompt)
                .arg("--allowedTools")
                .arg("Read,Edit,Write,Bash,Grep,Glob")
                .current_dir(repo_path);
            if let Some(ref m) = model {
                cmd.arg("--model").arg(m);
            }
            cmd
        }
    }
}

/// Create a `CorpusClient` for the enrichment branch.
///
/// Clones the base corpus config but sets the branch to the enrichment branch.
/// The client's `ensure_repo()` will auto-create the branch if it doesn't exist.
pub async fn create_enrich_corpus(
    base_config: &CorpusConfig,
    branch: &str,
) -> Result<CorpusClient> {
    let mut config = base_config.clone();
    config.branch = branch.into();

    // Use a separate checkout directory per branch to avoid conflicts
    let branch_dir_name = branch.replace('/', "-");
    config.repo_path = config
        .repo_path
        .parent()
        .unwrap_or(Path::new("/tmp"))
        .join(branch_dir_name);

    let mut client = CorpusClient::new(config);
    client.ensure_repo().await?;
    Ok(client)
}

/// Execute the enrichment: call the configured LLM to generate machine_readable sections.
///
/// Returns the enrichment result and a list of files that were written
/// (for git staging).
pub async fn execute_enrich(
    payload: &EnrichPayload,
    repo_path: &Path,
    config: &EnrichConfig,
) -> Result<(EnrichResult, Vec<PathBuf>)> {
    let yaml_abs = repo_path.join(&payload.yaml_path);
    if !yaml_abs.exists() {
        return Err(PipelineError::Enrich(format!(
            "law YAML file not found: {}",
            yaml_abs.display()
        )));
    }

    // Count articles before enrichment
    let articles_before = count_articles(&yaml_abs).await?;

    let prompt = build_prompt(&payload.yaml_path);
    let provider_name = config.provider.name().to_string();

    let mut cmd = build_command(&config.provider, &prompt, &yaml_abs, repo_path);

    tracing::info!(
        law_id = %payload.law_id,
        yaml_path = %payload.yaml_path,
        provider = %provider_name,
        "starting enrichment"
    );

    let output = tokio::time::timeout(config.timeout, cmd.output())
        .await
        .map_err(|_| {
            PipelineError::Enrich(format!(
                "{} timed out after {:?}",
                provider_name, config.timeout
            ))
        })?
        .map_err(|e| PipelineError::Enrich(format!("failed to run {}: {e}", provider_name)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(PipelineError::Enrich(format!(
            "{} exited with {}: {}",
            provider_name,
            output.status,
            stderr.chars().take(500).collect::<String>()
        )));
    }

    tracing::info!(law_id = %payload.law_id, provider = %provider_name, "enrichment completed");

    // Count articles with machine_readable after enrichment
    let articles_enriched = count_machine_readable_articles(&yaml_abs).await?;
    let quality_score = if articles_before > 0 {
        articles_enriched as f64 / articles_before as f64
    } else {
        0.0
    };

    // Write enrichment metadata
    let metadata = EnrichmentMetadata {
        law_id: payload.law_id.clone(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        provider: provider_name.clone(),
        model: config.provider.model_str(),
        prompt_hash: compute_prompt_hash(repo_path).await,
        code_commit: config.code_commit.clone(),
        quality_score,
        articles_total: articles_before,
        articles_enriched,
    };

    let metadata_path = yaml_abs
        .parent()
        .unwrap_or(Path::new("."))
        .join(".enrichment.yaml");
    let metadata_yaml = serde_yaml_ng::to_string(&metadata)
        .map_err(|e| PipelineError::Enrich(format!("failed to serialize metadata: {e}")))?;
    tokio::fs::write(&metadata_path, &metadata_yaml).await?;

    // Collect written files for corpus staging
    let mut written_files = vec![yaml_abs.clone(), metadata_path];

    // Check if a feature file was generated (MvT research creates these)
    let features_dir = repo_path.join("features");
    if features_dir.exists() {
        if let Ok(mut entries) = tokio::fs::read_dir(&features_dir).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                let path = entry.path();
                if path.extension().is_some_and(|e| e == "feature") {
                    written_files.push(path);
                }
            }
        }
    }

    let branch = enrich_branch_name(&provider_name, &payload.yaml_path);

    let result = EnrichResult {
        law_id: payload.law_id.clone(),
        yaml_path: payload.yaml_path.clone(),
        articles_total: articles_before,
        articles_enriched,
        quality_score,
        provider: provider_name,
        branch,
    };

    Ok((result, written_files))
}

/// Compute a SHA256 hash of the skill files used in the enrichment prompt.
///
/// This lets you detect when skill instructions changed between enrichments.
async fn compute_prompt_hash(repo_path: &Path) -> String {
    let skill_files = [
        ".claude/skills/law-mvt-research/SKILL.md",
        ".claude/skills/law-generate/SKILL.md",
        ".claude/skills/law-generate/reference.md",
        ".claude/skills/law-generate/examples.md",
        ".claude/skills/law-reverse-validate/SKILL.md",
    ];

    let mut hasher = Sha256::new();
    for file in &skill_files {
        let path = repo_path.join(file);
        if let Ok(content) = tokio::fs::read(&path).await {
            hasher.update(&content);
        }
    }

    format!("{:x}", hasher.finalize())
}

/// Count total articles in a law YAML file.
async fn count_articles(path: &Path) -> Result<usize> {
    let content = tokio::fs::read_to_string(path).await?;
    let value: serde_yaml_ng::Value = serde_yaml_ng::from_str(&content)?;
    Ok(count_articles_in_value(&value))
}

/// Count articles that have a `machine_readable` section.
async fn count_machine_readable_articles(path: &Path) -> Result<usize> {
    let content = tokio::fs::read_to_string(path).await?;
    let value: serde_yaml_ng::Value = serde_yaml_ng::from_str(&content)?;
    Ok(count_machine_readable_in_value(&value))
}

fn count_articles_in_value(value: &serde_yaml_ng::Value) -> usize {
    match value {
        serde_yaml_ng::Value::Mapping(map) => {
            if let Some(serde_yaml_ng::Value::Sequence(seq)) = map.get("articles") {
                return seq.len();
            }
            0
        }
        _ => 0,
    }
}

fn count_machine_readable_in_value(value: &serde_yaml_ng::Value) -> usize {
    match value {
        serde_yaml_ng::Value::Mapping(map) => {
            if let Some(serde_yaml_ng::Value::Sequence(articles)) = map.get("articles") {
                return articles
                    .iter()
                    .filter(|article| {
                        if let serde_yaml_ng::Value::Mapping(article_map) = article {
                            article_map.contains_key("machine_readable")
                        } else {
                            false
                        }
                    })
                    .count();
            }
            0
        }
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_enrich_payload_serde_roundtrip() {
        let payload = EnrichPayload {
            law_id: "BWBR0018451".to_string(),
            yaml_path: "regulation/nl/wet/wet_op_de_zorgtoeslag/2025-01-01.yaml".to_string(),
        };

        let json = serde_json::to_string(&payload).unwrap();
        let deserialized: EnrichPayload = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.law_id, "BWBR0018451");
        assert!(deserialized.yaml_path.contains("zorgtoeslag"));
    }

    #[test]
    fn test_enrich_result_serde() {
        let result = EnrichResult {
            law_id: "BWBR0018451".to_string(),
            yaml_path: "regulation/nl/wet/wet_op_de_zorgtoeslag/2025-01-01.yaml".to_string(),
            articles_total: 10,
            articles_enriched: 7,
            quality_score: 0.7,
            provider: "opencode".to_string(),
            branch: "enrich/opencode/wet_op_de_zorgtoeslag".to_string(),
        };

        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["articles_enriched"], 7);
        assert_eq!(json["quality_score"], 0.7);
        assert_eq!(json["provider"], "opencode");
        assert_eq!(json["branch"], "enrich/opencode/wet_op_de_zorgtoeslag");
    }

    #[test]
    fn test_llm_provider_opencode_defaults() {
        let provider = LlmProvider::OpenCode {
            path: "opencode".into(),
            model: None,
        };
        assert_eq!(provider.name(), "opencode");
        assert_eq!(provider.model_str(), "default");
    }

    #[test]
    fn test_llm_provider_claude_with_model() {
        let provider = LlmProvider::Claude {
            path: "/usr/local/bin/claude".into(),
            model: Some("opus".into()),
        };
        assert_eq!(provider.name(), "claude");
        assert_eq!(provider.model_str(), "opus");
    }

    #[test]
    fn test_enrich_config_default_timeout() {
        let config = EnrichConfig {
            provider: LlmProvider::OpenCode {
                path: "opencode".into(),
                model: None,
            },
            timeout: Duration::from_secs(600),
            code_commit: String::new(),
        };
        assert_eq!(config.timeout, Duration::from_secs(600));
        assert_eq!(config.provider.name(), "opencode");
    }

    #[test]
    fn test_build_prompt_contains_skill_paths() {
        let prompt = build_prompt("regulation/nl/wet/test/2025-01-01.yaml");
        assert!(prompt.contains("law-mvt-research/SKILL.md"));
        assert!(prompt.contains("law-generate/SKILL.md"));
        assert!(prompt.contains("law-reverse-validate/SKILL.md"));
        assert!(prompt.contains("regulation/nl/wet/test/2025-01-01.yaml"));
    }

    #[test]
    fn test_enrich_branch_name() {
        assert_eq!(
            enrich_branch_name(
                "opencode",
                "regulation/nl/wet/wet_op_de_zorgtoeslag/2025-01-01.yaml"
            ),
            "enrich/opencode/wet_op_de_zorgtoeslag"
        );
        assert_eq!(
            enrich_branch_name(
                "claude",
                "regulation/nl/ministeriele_regeling/huurtoeslag_param/2025-01-01.yaml"
            ),
            "enrich/claude/huurtoeslag_param"
        );
    }

    #[test]
    fn test_enrichment_metadata_serde() {
        let meta = EnrichmentMetadata {
            law_id: "BWBR0018451".to_string(),
            timestamp: "2026-03-12T10:00:00Z".to_string(),
            provider: "opencode".to_string(),
            model: "vlam/mistral-medium".to_string(),
            prompt_hash: "abc123".to_string(),
            code_commit: "deadbeef".to_string(),
            quality_score: 0.7,
            articles_total: 10,
            articles_enriched: 7,
        };

        let yaml = serde_yaml_ng::to_string(&meta).unwrap();
        assert!(yaml.contains("law_id: BWBR0018451"));
        assert!(yaml.contains("provider: opencode"));

        let deserialized: EnrichmentMetadata = serde_yaml_ng::from_str(&yaml).unwrap();
        assert_eq!(deserialized.articles_enriched, 7);
    }

    #[test]
    fn test_count_articles_in_value() {
        let yaml = r#"
articles:
  - id: art1
    name: Article 1
  - id: art2
    name: Article 2
  - id: art3
    name: Article 3
    machine_readable:
      actions: []
"#;
        let value: serde_yaml_ng::Value = serde_yaml_ng::from_str(yaml).unwrap();
        assert_eq!(count_articles_in_value(&value), 3);
        assert_eq!(count_machine_readable_in_value(&value), 1);
    }
}
