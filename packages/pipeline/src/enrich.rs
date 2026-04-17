use std::path::{Path, PathBuf};
use std::time::Duration;

use regelrecht_corpus::{CorpusClient, CorpusConfig};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::error::{PipelineError, Result};

/// Trait abstracting the LLM invocation so `execute_enrich` can be tested
/// with a fake provider that doesn't spawn real processes.
#[async_trait::async_trait]
pub trait LlmRunner: Send + Sync {
    /// Run the LLM on the given YAML file and return its exit status.
    ///
    /// Implementations should respect the timeout in `config`.
    async fn run(
        &self,
        payload: &EnrichPayload,
        yaml_abs: &Path,
        repo_path: &Path,
        config: &EnrichConfig,
    ) -> Result<()>;
}

/// Default runner that spawns a real CLI process.
pub struct ProcessLlmRunner;

#[async_trait::async_trait]
impl LlmRunner for ProcessLlmRunner {
    async fn run(
        &self,
        payload: &EnrichPayload,
        yaml_abs: &Path,
        repo_path: &Path,
        config: &EnrichConfig,
    ) -> Result<()> {
        let progress_path = progress_file_path(yaml_abs);
        let prompt = build_prompt(&payload.yaml_path, &progress_path.to_string_lossy());
        let provider_name = config.provider.name().to_string();

        let mut cmd = build_command(&config.provider, &prompt, yaml_abs, repo_path);

        // stderr is inherited so the LLM's logging goes to the worker's stderr.
        // This avoids a deadlock: if stderr were piped, a verbose LLM (e.g. Claude CLI)
        // could fill the OS pipe buffer (64 KB) and block indefinitely.
        cmd.stderr(std::process::Stdio::inherit());
        let mut child = cmd.spawn().map_err(|e| {
            PipelineError::Enrich(format!("failed to spawn {}: {e}", provider_name))
        })?;

        let status = tokio::select! {
            result = child.wait() => {
                result.map_err(|e| {
                    PipelineError::Enrich(format!("failed to wait for {}: {e}", provider_name))
                })?
            }
            _ = tokio::time::sleep(config.timeout) => {
                if let Err(e) = child.kill().await {
                    tracing::warn!(error = %e, "failed to kill timed-out LLM process");
                }
                let _ = child.wait().await;
                return Err(PipelineError::Enrich(format!(
                    "{} timed out after {:?}",
                    provider_name, config.timeout
                )));
            }
        };

        if !status.success() {
            return Err(PipelineError::Enrich(format!(
                "{} exited with {}",
                provider_name, status,
            )));
        }

        Ok(())
    }
}

/// Payload for an enrich job, stored as JSON in the job queue.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnrichPayload {
    pub law_id: String,
    /// Relative path to the harvested YAML file within the repo.
    pub yaml_path: String,
    /// LLM provider to use for this enrichment ("opencode" or "claude").
    /// When set, overrides the worker's `LLM_PROVIDER` env var.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
}

/// All known provider names. Used to create one enrich job per provider
/// after a successful harvest.
pub const ENRICH_PROVIDERS: &[&str] = &["opencode", "claude"];

/// Result of a successful enrichment execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnrichResult {
    pub law_id: String,
    pub yaml_path: String,
    pub articles_total: usize,
    /// Total articles with a `machine_readable` section after enrichment
    /// (includes pre-existing ones). Not the count of newly enriched articles.
    pub articles_with_machine_readable: usize,
    /// Fraction of previously-unenriched articles that the LLM enriched
    /// in this session. 1.0 means every article that was missing a
    /// `machine_readable` section now has one; says nothing about correctness.
    pub coverage_score: f64,
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
    pub coverage_score: f64,
    pub articles_total: usize,
    /// Total articles with a `machine_readable` section after enrichment.
    pub articles_with_machine_readable: usize,
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
///
/// All env vars are read once at startup and stored. `with_provider_override()`
/// selects from pre-built provider configs without re-reading the environment.
#[derive(Debug, Clone)]
pub struct EnrichConfig {
    pub provider: LlmProvider,
    pub timeout: Duration,
    pub code_commit: String,
    /// Pre-built provider configs keyed by name, populated at startup.
    provider_configs: std::collections::HashMap<String, LlmProvider>,
}

impl EnrichConfig {
    pub fn from_env() -> Self {
        let provider_name = std::env::var("LLM_PROVIDER").unwrap_or_else(|_| "opencode".into());

        let timeout = std::env::var("LLM_TIMEOUT_SECS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(600);

        let code_commit = std::env::var("CODE_COMMIT").unwrap_or_default();

        // Build all provider configs once from env vars
        let opencode_provider = LlmProvider::OpenCode {
            path: std::env::var("OPENCODE_PATH")
                .or_else(|_| std::env::var("LLM_PATH"))
                .unwrap_or_else(|_| "opencode".into())
                .into(),
            model: std::env::var("OPENCODE_MODEL")
                .or_else(|_| std::env::var("LLM_MODEL"))
                .ok(),
        };
        let claude_provider = LlmProvider::Claude {
            path: std::env::var("CLAUDE_PATH")
                .or_else(|_| std::env::var("LLM_PATH"))
                .unwrap_or_else(|_| "claude".into())
                .into(),
            model: std::env::var("CLAUDE_MODEL")
                .or_else(|_| std::env::var("LLM_MODEL"))
                .ok(),
        };

        let provider = match provider_name.as_str() {
            "claude" => claude_provider.clone(),
            _ => opencode_provider.clone(),
        };

        let mut provider_configs = std::collections::HashMap::new();
        provider_configs.insert("opencode".to_string(), opencode_provider);
        provider_configs.insert("claude".to_string(), claude_provider);

        Self {
            provider,
            timeout: Duration::from_secs(timeout),
            code_commit,
            provider_configs,
        }
    }

    /// Return a config with the provider overridden if the payload specifies one.
    ///
    /// Selects from pre-built provider configs — no env vars are re-read.
    pub fn with_provider_override(&self, provider_name: &str) -> Self {
        let provider = if let Some(cfg) = self.provider_configs.get(provider_name) {
            cfg.clone()
        } else {
            tracing::warn!(
                requested = %provider_name,
                fallback = %self.provider.name(),
                "unknown provider in payload, falling back to default"
            );
            self.provider.clone()
        };

        Self {
            provider,
            timeout: self.timeout,
            code_commit: self.code_commit.clone(),
            provider_configs: self.provider_configs.clone(),
        }
    }
}

/// Build the enrichment branch name for a given provider.
///
/// All enriched laws for a provider live on a single shared branch
/// (`enrich/{provider}`), so results can be compared with main and
/// between providers without branch-per-law proliferation.
pub fn enrich_branch_name(provider_name: &str) -> String {
    format!("enrich/{provider_name}")
}

/// Build the prompt that tells the LLM to follow the skill pipeline.
fn build_prompt(yaml_path: &str, progress_file_path: &str) -> String {
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

Write all changes to disk. Do not ask questions — proceed autonomously.

## Progress tracking
Before starting each step, write a JSON progress file to report your current phase.
Write to: {progress_file_path}

Write this file at these moments:
- Before Step 1: {{"phase": "mvt_research", "step": 1, "total_steps": 3}}
- Before Step 2: {{"phase": "generating", "step": 2, "total_steps": 3, "article_count": N}}
- After validation in Step 2: {{"phase": "validating", "step": 2, "total_steps": 3, "iteration": M}}
- Before Step 3: {{"phase": "reverse_validating", "step": 3, "total_steps": 3}}

Use the Write tool. Keep it brief — just one write per phase transition."#
    )
}

/// Compute the path of the progress file for a given law YAML file.
///
/// The progress file sits next to the YAML (e.g.
/// `regulation/nl/wet/foo/.enrichment-progress.json`).
pub fn progress_file_path(yaml_abs: &Path) -> PathBuf {
    yaml_abs
        .parent()
        .unwrap_or(Path::new("."))
        .join(".enrichment-progress.json")
}

/// Allowlisted environment variable prefixes/names that are safe to pass to the
/// LLM subprocess.  Everything else (DATABASE_URL, etc.) is stripped.
const LLM_ENV_ALLOWLIST: &[&str] = &[
    "HOME",
    "PATH",
    "TERM",
    "LANG",
    "USER",
    "SHELL",
    "TMPDIR",
    "XDG_",
    // Provider-specific auth
    "ANTHROPIC_API_KEY",
    "VLAM_API_KEY",
    "OPENCODE_",
];

/// Check whether an environment variable name is on the allowlist.
fn env_allowed(key: &str) -> bool {
    LLM_ENV_ALLOWLIST
        .iter()
        .any(|prefix| key == *prefix || key.starts_with(prefix))
}

/// Build the command for the configured LLM provider.
///
/// The subprocess gets a stripped environment: only variables on
/// `LLM_ENV_ALLOWLIST` are forwarded.  This prevents leaking DATABASE_URL
/// and other secrets to the LLM process (which may have shell access).
fn build_command(
    provider: &LlmProvider,
    prompt: &str,
    yaml_abs: &Path,
    repo_path: &Path,
) -> tokio::process::Command {
    // Collect allowed env vars before creating the command.
    let safe_env: Vec<(String, String)> =
        std::env::vars().filter(|(k, _)| env_allowed(k)).collect();

    match provider {
        LlmProvider::OpenCode { path, model } => {
            let mut cmd = tokio::process::Command::new(path);
            cmd.env_clear();
            cmd.envs(safe_env);
            cmd.env("NODE_OPTIONS", "--max-old-space-size=512");
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
            cmd.env_clear();
            cmd.envs(safe_env);
            cmd.env("NODE_OPTIONS", "--max-old-space-size=512");
            cmd.arg("-p")
                .arg(prompt)
                .arg("--allowedTools")
                .arg("Read,Edit,Write,Grep,Glob")
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
///
/// Each invocation uses a unique checkout directory (keyed by branch + job ID)
/// to prevent concurrent workers from clobbering each other's checkouts.
///
/// Uses sparse checkout to only materialize the law directory being enriched
/// plus the `features/` directory. This prevents the LLM subprocess from
/// indexing the entire corpus (thousands of files), which would exceed context
/// limits and cause excessive memory usage.
pub async fn create_enrich_corpus(
    base_config: &CorpusConfig,
    branch: &str,
    job_id: Uuid,
    yaml_path: &str,
) -> Result<CorpusClient> {
    let mut config = base_config.clone();
    config.branch = branch.into();

    // Normalize the yaml_path to strip legacy absolute prefixes (e.g.
    // `/tmp/corpus-repo/regulation/…`) before deriving the law directory
    // for sparse checkout. Without this, git sparse-checkout would receive
    // an absolute path it cannot handle.
    let normalized = normalize_yaml_path(yaml_path)?;

    // Sparse checkout: only the law directory + features/
    if let Some(law_dir) = Path::new(&normalized).parent() {
        let law_dir_str = law_dir.to_string_lossy().to_string();
        if !law_dir_str.is_empty() {
            config.sparse_paths = Some(vec![law_dir_str, "features".to_string()]);
        }
    }

    // Use a separate checkout directory per branch + job to avoid conflicts
    // between concurrent workers processing different laws on the same branch.
    let dir_name = format!("{}-{}", branch.replace('/', "-"), job_id);
    let base_dir = config
        .repo_path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("/tmp"));
    config.repo_path = base_dir.join(dir_name);

    let mut client = CorpusClient::new(config);
    client.ensure_repo().await?;

    // Check out the law directory from development so newly harvested laws
    // are available. Without this, laws harvested after the enrichment branch
    // was created would be missing and cause "file not found" errors.
    //
    // Uses checkout (not merge) so the file addition ends up in the same
    // commit as the enrichment — which survives `git rebase` in commit_and_push.
    let law_dir = Path::new(&normalized)
        .parent()
        .map(|p| p.to_string_lossy().to_string());
    if let Some(ref dir) = law_dir {
        client.checkout_from_branch("development", &[dir]).await?;
    }

    Ok(client)
}

/// Ensure `.claude/skills/` exist in the target repo directory.
///
/// If `SKILLS_DIR` is set (default `/opt/skills` in the container image),
/// symlinks each skill subdirectory into `repo_path/.claude/skills/`.
/// This makes baked-in skill files available to the LLM subprocess.
///
/// No-op when `SKILLS_DIR` doesn't exist (e.g. local development where
/// skills are already in the working tree).
pub async fn ensure_skills(repo_path: &Path) -> Result<()> {
    let skills_source =
        PathBuf::from(std::env::var("SKILLS_DIR").unwrap_or_else(|_| "/opt/skills".into()));
    let source_skills_dir = skills_source.join(".claude/skills");

    if !source_skills_dir.exists() {
        tracing::debug!(
            path = %source_skills_dir.display(),
            "skills source directory not found, skipping symlink"
        );
        return Ok(());
    }

    let target_skills_dir = repo_path.join(".claude/skills");
    tokio::fs::create_dir_all(&target_skills_dir).await?;

    let mut entries = tokio::fs::read_dir(&source_skills_dir).await?;
    while let Some(entry) = entries.next_entry().await? {
        let entry_path = entry.path();
        if entry_path.is_dir() {
            let name = entry.file_name();
            let link_path = target_skills_dir.join(&name);
            // Remove existing symlink, file, or directory to ensure a clean link.
            // remove_file handles symlinks and regular files; remove_dir_all
            // handles real directories left by a previous partial run.
            if let Ok(meta) = tokio::fs::symlink_metadata(&link_path).await {
                if meta.is_dir() && !meta.file_type().is_symlink() {
                    let _ = tokio::fs::remove_dir_all(&link_path).await;
                } else {
                    let _ = tokio::fs::remove_file(&link_path).await;
                }
            }
            tokio::fs::symlink(&entry_path, &link_path)
                .await
                .map_err(|e| {
                    PipelineError::Enrich(format!(
                        "failed to symlink skill {:?} -> {:?}: {e}",
                        entry_path, link_path
                    ))
                })?;
            tracing::debug!(skill = ?name, "symlinked skill into repo");
        }
    }

    Ok(())
}

/// Known absolute prefixes that may appear in yaml_path values from
/// older harvest results. Stripped automatically so enrich jobs still work.
const KNOWN_REPO_PREFIXES: &[&str] = &["/tmp/corpus-repo/", "/tmp/regulation-repo/"];

/// Normalize and validate a yaml_path: strip known absolute prefixes,
/// then verify the path contains only safe characters.
///
/// Prevents path traversal and injection via crafted job payloads.
pub(crate) fn normalize_yaml_path(yaml_path: &str) -> Result<String> {
    if yaml_path.is_empty() {
        return Err(PipelineError::Enrich("yaml_path must not be empty".into()));
    }

    // Auto-strip known absolute prefixes from legacy payloads.
    let mut path = yaml_path.to_string();
    for prefix in KNOWN_REPO_PREFIXES {
        if let Some(stripped) = path.strip_prefix(prefix) {
            tracing::warn!(
                original = %yaml_path,
                normalized = %stripped,
                "yaml_path had absolute prefix, stripped automatically"
            );
            path = stripped.to_string();
            break;
        }
    }

    if path.starts_with('/') {
        return Err(PipelineError::Enrich(format!(
            "yaml_path must be relative, not absolute: {yaml_path}"
        )));
    }
    if !path
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '_' | '-' | '.'))
    {
        return Err(PipelineError::Enrich(format!(
            "yaml_path contains invalid characters: {path}"
        )));
    }
    if path.contains("..") {
        return Err(PipelineError::Enrich(format!(
            "yaml_path must not contain '..': {path}"
        )));
    }
    Ok(path)
}

/// Execute the enrichment using the default process-based LLM runner.
///
/// Convenience wrapper around `execute_enrich_with_runner` using `ProcessLlmRunner`.
pub async fn execute_enrich(
    payload: &EnrichPayload,
    repo_path: &Path,
    config: &EnrichConfig,
) -> Result<(EnrichResult, Vec<PathBuf>)> {
    execute_enrich_with_runner(payload, repo_path, config, &ProcessLlmRunner).await
}

/// Execute the enrichment: call the LLM runner to generate machine_readable sections.
///
/// Returns the enrichment result and a list of files that were written
/// (for git staging). Accepts a `runner` to allow testing with a fake LLM.
pub async fn execute_enrich_with_runner(
    payload: &EnrichPayload,
    repo_path: &Path,
    config: &EnrichConfig,
    runner: &dyn LlmRunner,
) -> Result<(EnrichResult, Vec<PathBuf>)> {
    let normalized_path = normalize_yaml_path(&payload.yaml_path)?;

    let yaml_abs = repo_path.join(&normalized_path);
    if !yaml_abs.exists() {
        return Err(PipelineError::Enrich(format!(
            "law YAML file not found: {}",
            yaml_abs.display()
        )));
    }

    // Count articles and existing machine_readable sections before enrichment
    let (articles_before, machine_readable_before) = count_article_stats(&yaml_abs).await?;

    let provider_name = config.provider.name().to_string();

    tracing::info!(
        law_id = %payload.law_id,
        yaml_path = %payload.yaml_path,
        provider = %provider_name,
        articles = articles_before,
        already_enriched = machine_readable_before,
        "starting enrichment"
    );

    let normalized_payload = EnrichPayload {
        yaml_path: normalized_path.clone(),
        ..payload.clone()
    };
    runner
        .run(&normalized_payload, &yaml_abs, repo_path, config)
        .await?;

    tracing::info!(law_id = %payload.law_id, provider = %provider_name, "enrichment completed");

    // Count articles with machine_readable after enrichment.
    // Coverage score measures what the LLM *added* this session, not total coverage.
    let (articles_after, articles_with_machine_readable) = count_article_stats(&yaml_abs).await?;
    if articles_after != articles_before {
        return Err(PipelineError::Enrich(format!(
            "article count changed during enrichment (before={articles_before}, after={articles_after}) — LLM modified YAML structure"
        )));
    }
    let newly_enriched = articles_with_machine_readable.saturating_sub(machine_readable_before);
    let articles_needing_enrichment = articles_before.saturating_sub(machine_readable_before);
    let coverage_score = if articles_needing_enrichment > 0 {
        newly_enriched as f64 / articles_needing_enrichment as f64
    } else if articles_before > 0 {
        // All articles already had machine_readable before — nothing to do
        1.0
    } else {
        0.0
    };

    // If the LLM ran successfully but didn't enrich any articles, treat it as
    // an error so the job gets retried or marked as failed instead of silently
    // committing a zero-coverage result.
    if articles_needing_enrichment > 0 && newly_enriched == 0 {
        return Err(PipelineError::Enrich(format!(
            "LLM produced no machine_readable sections ({articles_needing_enrichment} articles needed enrichment)"
        )));
    }

    // Write enrichment metadata
    let metadata = EnrichmentMetadata {
        law_id: payload.law_id.clone(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        provider: provider_name.clone(),
        model: config.provider.model_str(),
        prompt_hash: compute_prompt_hash(repo_path).await,
        code_commit: config.code_commit.clone(),
        coverage_score,
        articles_total: articles_before,
        articles_with_machine_readable,
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

    // Check if a feature file was generated for this specific law.
    // MvT research creates feature files named after the law slug.
    // Only include files whose name contains the law slug to avoid
    // accidentally staging unrelated feature files.
    let law_slug = Path::new(&normalized_path)
        .parent()
        .and_then(|p| p.file_name())
        .map(|n| n.to_string_lossy().to_string());
    let features_dir = repo_path.join("features");
    if let Some(ref slug) = law_slug {
        if features_dir.exists() {
            if let Ok(mut entries) = tokio::fs::read_dir(&features_dir).await {
                while let Ok(Some(entry)) = entries.next_entry().await {
                    let path = entry.path();
                    if path.extension().is_some_and(|e| e == "feature") {
                        if let Some(name) = path.file_stem() {
                            if name.to_string_lossy().contains(slug.as_str()) {
                                written_files.push(path);
                            }
                        }
                    }
                }
            }
        }
    }

    let branch = enrich_branch_name(&provider_name);

    let result = EnrichResult {
        law_id: payload.law_id.clone(),
        yaml_path: normalized_path,
        articles_total: articles_before,
        articles_with_machine_readable,
        coverage_score,
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
    let mut files_found = 0usize;
    for file in &skill_files {
        let path = repo_path.join(file);
        if let Ok(content) = tokio::fs::read(&path).await {
            hasher.update(&content);
            files_found += 1;
        } else {
            tracing::warn!(file = %file, "skill file not found for prompt hash");
        }
    }

    if files_found == 0 {
        tracing::warn!("no skill files found — prompt hash will be empty");
    }

    format!("{:x}", hasher.finalize())
}

/// Count total articles and articles with `machine_readable` in one parse pass.
async fn count_article_stats(path: &Path) -> Result<(usize, usize)> {
    let content = tokio::fs::read_to_string(path).await?;
    let value: serde_yaml_ng::Value = serde_yaml_ng::from_str(&content)?;
    Ok((
        count_articles_in_value(&value),
        count_machine_readable_in_value(&value),
    ))
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
            provider: Some("claude".to_string()),
        };

        let json = serde_json::to_string(&payload).unwrap();
        let deserialized: EnrichPayload = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.provider.as_deref(), Some("claude"));

        // Verify backward compatibility: provider is optional and skipped when None
        let payload_no_provider = EnrichPayload {
            law_id: "BWBR0018451".to_string(),
            yaml_path: "regulation/nl/wet/wet_op_de_zorgtoeslag/2025-01-01.yaml".to_string(),
            provider: None,
        };
        let json_no_provider = serde_json::to_string(&payload_no_provider).unwrap();
        assert!(!json_no_provider.contains("provider"));
        let deserialized_no_provider: EnrichPayload =
            serde_json::from_str(&json_no_provider).unwrap();
        assert!(deserialized_no_provider.provider.is_none());

        assert_eq!(deserialized.law_id, "BWBR0018451");
        assert!(deserialized.yaml_path.contains("zorgtoeslag"));
    }

    #[test]
    fn test_enrich_result_serde() {
        let result = EnrichResult {
            law_id: "BWBR0018451".to_string(),
            yaml_path: "regulation/nl/wet/wet_op_de_zorgtoeslag/2025-01-01.yaml".to_string(),
            articles_total: 10,
            articles_with_machine_readable: 7,
            coverage_score: 0.7,
            provider: "opencode".to_string(),
            branch: "enrich/opencode".to_string(),
        };

        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["articles_with_machine_readable"], 7);
        assert_eq!(json["coverage_score"], 0.7);
        assert_eq!(json["provider"], "opencode");
        assert_eq!(json["branch"], "enrich/opencode");
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

    fn test_config(provider: LlmProvider) -> EnrichConfig {
        let mut provider_configs = std::collections::HashMap::new();
        provider_configs.insert(
            "opencode".to_string(),
            LlmProvider::OpenCode {
                path: "opencode".into(),
                model: None,
            },
        );
        provider_configs.insert(
            "claude".to_string(),
            LlmProvider::Claude {
                path: "claude".into(),
                model: Some("opus".into()),
            },
        );
        EnrichConfig {
            provider,
            timeout: Duration::from_secs(600),
            code_commit: "abc123".to_string(),
            provider_configs,
        }
    }

    #[test]
    fn test_with_provider_override() {
        let base_config = test_config(LlmProvider::OpenCode {
            path: "opencode".into(),
            model: None,
        });

        let claude_config = base_config.with_provider_override("claude");
        assert_eq!(claude_config.provider.name(), "claude");
        assert_eq!(claude_config.timeout, Duration::from_secs(600));
        assert_eq!(claude_config.code_commit, "abc123");

        let opencode_config = base_config.with_provider_override("opencode");
        assert_eq!(opencode_config.provider.name(), "opencode");

        // Unknown provider falls back to current provider
        let unknown_config = base_config.with_provider_override("unknown");
        assert_eq!(unknown_config.provider.name(), "opencode");
    }

    #[test]
    fn test_enrich_providers_list() {
        assert!(ENRICH_PROVIDERS.contains(&"opencode"));
        assert!(ENRICH_PROVIDERS.contains(&"claude"));
        assert_eq!(ENRICH_PROVIDERS.len(), 2);
    }

    #[test]
    fn test_enrich_config_default_timeout() {
        let config = test_config(LlmProvider::OpenCode {
            path: "opencode".into(),
            model: None,
        });
        assert_eq!(config.timeout, Duration::from_secs(600));
        assert_eq!(config.provider.name(), "opencode");
    }

    #[test]
    fn test_build_prompt_contains_skill_paths() {
        let prompt = build_prompt(
            "regulation/nl/wet/test/2025-01-01.yaml",
            "/tmp/repo/regulation/nl/wet/test/.enrichment-progress.json",
        );
        assert!(prompt.contains("law-mvt-research/SKILL.md"));
        assert!(prompt.contains("law-generate/SKILL.md"));
        assert!(prompt.contains("law-reverse-validate/SKILL.md"));
        assert!(prompt.contains("regulation/nl/wet/test/2025-01-01.yaml"));
        assert!(prompt.contains(".enrichment-progress.json"));
    }

    #[test]
    fn test_enrich_branch_name() {
        assert_eq!(enrich_branch_name("opencode"), "enrich/opencode");
        assert_eq!(enrich_branch_name("claude"), "enrich/claude");
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
            coverage_score: 0.7,
            articles_total: 10,
            articles_with_machine_readable: 7,
        };

        let yaml = serde_yaml_ng::to_string(&meta).unwrap();
        assert!(yaml.contains("law_id: BWBR0018451"));
        assert!(yaml.contains("provider: opencode"));

        let deserialized: EnrichmentMetadata = serde_yaml_ng::from_str(&yaml).unwrap();
        assert_eq!(deserialized.articles_with_machine_readable, 7);
    }

    #[test]
    fn test_normalize_yaml_path_valid() {
        assert_eq!(
            normalize_yaml_path("regulation/nl/wet/zorgtoeslag/2025-01-01.yaml").unwrap(),
            "regulation/nl/wet/zorgtoeslag/2025-01-01.yaml"
        );
        assert_eq!(
            normalize_yaml_path("regulation/nl/ministeriele_regeling/test/file.yaml").unwrap(),
            "regulation/nl/ministeriele_regeling/test/file.yaml"
        );
    }

    #[test]
    fn test_normalize_yaml_path_strips_known_prefixes() {
        assert_eq!(
            normalize_yaml_path("/tmp/corpus-repo/regulation/nl/wet/test/2025-01-01.yaml").unwrap(),
            "regulation/nl/wet/test/2025-01-01.yaml"
        );
        assert_eq!(
            normalize_yaml_path("/tmp/regulation-repo/regulation/nl/wet/test/2025-01-01.yaml")
                .unwrap(),
            "regulation/nl/wet/test/2025-01-01.yaml"
        );
    }

    #[test]
    fn test_normalize_yaml_path_rejects_unknown_absolute() {
        assert!(normalize_yaml_path("/etc/passwd").is_err());
        assert!(normalize_yaml_path("/other/path/file.yaml").is_err());
    }

    #[test]
    fn test_normalize_yaml_path_rejects_traversal() {
        assert!(normalize_yaml_path("../etc/passwd").is_err());
        assert!(normalize_yaml_path("regulation/../../etc/passwd").is_err());
    }

    #[test]
    fn test_normalize_yaml_path_rejects_special_chars() {
        assert!(normalize_yaml_path("regulation/nl/wet/test; rm -rf /").is_err());
        assert!(normalize_yaml_path("regulation/nl/wet/test$(whoami)").is_err());
        assert!(normalize_yaml_path("").is_err());
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

    /// Fake LLM runner that simulates enrichment by adding `machine_readable`
    /// sections to articles that don't already have them.
    struct FakeLlmRunner;

    #[async_trait::async_trait]
    impl LlmRunner for FakeLlmRunner {
        async fn run(
            &self,
            _payload: &EnrichPayload,
            yaml_abs: &Path,
            _repo_path: &Path,
            _config: &EnrichConfig,
        ) -> Result<()> {
            let content = tokio::fs::read_to_string(yaml_abs).await?;
            let mut value: serde_yaml_ng::Value = serde_yaml_ng::from_str(&content)?;

            if let serde_yaml_ng::Value::Mapping(ref mut map) = value {
                if let Some(serde_yaml_ng::Value::Sequence(ref mut articles)) =
                    map.get_mut("articles")
                {
                    for article in articles.iter_mut() {
                        if let serde_yaml_ng::Value::Mapping(ref mut article_map) = article {
                            if !article_map.contains_key("machine_readable") {
                                article_map.insert(
                                    serde_yaml_ng::Value::String("machine_readable".into()),
                                    serde_yaml_ng::Value::Mapping(Default::default()),
                                );
                            }
                        }
                    }
                }
            }

            let output = serde_yaml_ng::to_string(&value)?;
            tokio::fs::write(yaml_abs, output).await?;
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_execute_enrich_with_fake_runner() {
        let dir = tempfile::tempdir().unwrap();
        let law_dir = dir.path().join("regulation/nl/wet/test_law");
        tokio::fs::create_dir_all(&law_dir).await.unwrap();

        let yaml_content = r#"articles:
  - id: art1
    name: Article 1
  - id: art2
    name: Article 2
    machine_readable:
      actions: []
  - id: art3
    name: Article 3
"#;
        let yaml_path = "regulation/nl/wet/test_law/2025-01-01.yaml";
        tokio::fs::write(dir.path().join(yaml_path), yaml_content)
            .await
            .unwrap();

        let payload = EnrichPayload {
            law_id: "BWBR0000001".into(),
            yaml_path: yaml_path.into(),
            provider: Some("opencode".into()),
        };

        let config = test_config(LlmProvider::OpenCode {
            path: "fake".into(),
            model: None,
        });

        let (result, written_files) =
            execute_enrich_with_runner(&payload, dir.path(), &config, &FakeLlmRunner)
                .await
                .unwrap();

        assert_eq!(result.articles_total, 3);
        assert_eq!(result.articles_with_machine_readable, 3);
        // 2 out of 2 articles needing enrichment were enriched
        assert!((result.coverage_score - 1.0).abs() < f64::EPSILON);
        assert_eq!(result.provider, "opencode");
        assert_eq!(result.branch, "enrich/opencode");

        // Should have written the YAML file + metadata file
        assert!(written_files.len() >= 2);

        // Verify metadata file was written
        let metadata_path = law_dir.join(".enrichment.yaml");
        assert!(metadata_path.exists());
        let meta_content = tokio::fs::read_to_string(&metadata_path).await.unwrap();
        let meta: EnrichmentMetadata = serde_yaml_ng::from_str(&meta_content).unwrap();
        assert_eq!(meta.law_id, "BWBR0000001");
        assert_eq!(meta.provider, "opencode");
        assert_eq!(meta.articles_with_machine_readable, 3);
    }

    /// Fake runner that fails, to test error path.
    struct FailingLlmRunner;

    #[async_trait::async_trait]
    impl LlmRunner for FailingLlmRunner {
        async fn run(
            &self,
            _payload: &EnrichPayload,
            _yaml_abs: &Path,
            _repo_path: &Path,
            _config: &EnrichConfig,
        ) -> Result<()> {
            Err(PipelineError::Enrich("simulated LLM failure".into()))
        }
    }

    #[tokio::test]
    async fn test_execute_enrich_with_failing_runner() {
        let dir = tempfile::tempdir().unwrap();
        let law_dir = dir.path().join("regulation/nl/wet/test_law");
        tokio::fs::create_dir_all(&law_dir).await.unwrap();

        let yaml_content = "articles:\n  - id: art1\n    name: Article 1\n";
        let yaml_path = "regulation/nl/wet/test_law/2025-01-01.yaml";
        tokio::fs::write(dir.path().join(yaml_path), yaml_content)
            .await
            .unwrap();

        let payload = EnrichPayload {
            law_id: "BWBR0000001".into(),
            yaml_path: yaml_path.into(),
            provider: None,
        };

        let config = test_config(LlmProvider::OpenCode {
            path: "fake".into(),
            model: None,
        });

        let err = execute_enrich_with_runner(&payload, dir.path(), &config, &FailingLlmRunner)
            .await
            .unwrap_err();

        assert!(err.to_string().contains("simulated LLM failure"));
    }

    /// Runner that succeeds but doesn't modify the file — should fail with
    /// zero-coverage error.
    struct NoopLlmRunner;

    #[async_trait::async_trait]
    impl LlmRunner for NoopLlmRunner {
        async fn run(
            &self,
            _payload: &EnrichPayload,
            _yaml_abs: &Path,
            _repo_path: &Path,
            _config: &EnrichConfig,
        ) -> Result<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_execute_enrich_zero_coverage_is_error() {
        let dir = tempfile::tempdir().unwrap();
        let law_dir = dir.path().join("regulation/nl/wet/test_law");
        tokio::fs::create_dir_all(&law_dir).await.unwrap();

        let yaml_content = "articles:\n  - id: art1\n    name: Article 1\n";
        let yaml_path = "regulation/nl/wet/test_law/2025-01-01.yaml";
        tokio::fs::write(dir.path().join(yaml_path), yaml_content)
            .await
            .unwrap();

        let payload = EnrichPayload {
            law_id: "BWBR0000001".into(),
            yaml_path: yaml_path.into(),
            provider: None,
        };

        let config = test_config(LlmProvider::OpenCode {
            path: "fake".into(),
            model: None,
        });

        let err = execute_enrich_with_runner(&payload, dir.path(), &config, &NoopLlmRunner)
            .await
            .unwrap_err();

        assert!(err.to_string().contains("no machine_readable sections"));
    }
}
