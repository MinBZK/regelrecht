use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use regelrecht_corpus::{CorpusClient, CorpusConfig};
use regelrecht_law_model::ArticleBasedLaw;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::io::AsyncReadExt;
use uuid::Uuid;

use crate::error::{PipelineError, Result};

/// Per-process cache of branch names already confirmed to exist on the
/// corpus remote. Branches are never deleted once created, so a positive
/// probe is permanent for the life of the worker — caching skips the
/// ls-remote round-trip on every subsequent enrich job for the same PR.
fn known_remote_branches() -> &'static Mutex<HashSet<String>> {
    static CACHE: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashSet::new()))
}

fn branch_is_known(branch: &str) -> bool {
    known_remote_branches()
        .lock()
        .map(|cache| cache.contains(branch))
        .unwrap_or(false)
}

fn remember_branch(branch: &str) {
    if let Ok(mut cache) = known_remote_branches().lock() {
        cache.insert(branch.to_string());
    }
}

/// Pick the base branch to check out the law YAML from, given the worker's
/// preferred branch and whether that branch exists on the remote. Pure
/// function so the branch-selection contract can be pinned by unit tests
/// without a live git remote.
fn pick_enrich_base(preferred: &str, preferred_exists: bool) -> &str {
    if preferred == "development" || preferred_exists {
        preferred
    } else {
        "development"
    }
}

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

/// Max bytes of the LLM subprocess's stderr to retain for diagnostics. The tail
/// (most recent output) is kept and appended to the error on a non-zero exit, so
/// a failure reports the real cause (e.g. an auth `401`) instead of a bare code.
const MAX_STDERR_CAPTURE: usize = 4096;

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

        // Both streams are piped and drained. stdout is drained-and-discarded: a
        // verbose agent (e.g. opencode `--format json`) inlines the full body of
        // every fetched page into its event stream, which would flood container
        // logs. stderr is drained too — we MUST keep reading both or a full 64 KB
        // OS pipe buffer blocks the child — but for stderr we also keep a bounded
        // tail and re-log previews, so the LLM's real error (e.g. an auth 401) is
        // both visible in the logs and attached to the job's failure.
        cmd.stderr(std::process::Stdio::piped());
        cmd.stdout(std::process::Stdio::piped());
        let mut child = cmd.spawn().map_err(|e| {
            PipelineError::Enrich(format!("failed to spawn {}: {e}", provider_name))
        })?;

        // Capture the PID before any wait reaps the child; the memory watchdog
        // and process-group kill both need it.
        let pid = child.id();

        // Drain stderr, retaining the last `MAX_STDERR_CAPTURE` bytes for the
        // error message and re-logging bounded previews so it stays visible.
        let stderr_tail = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
        let stderr_task = child.stderr.take().map(|mut stderr| {
            let tail = stderr_tail.clone();
            let stderr_provider = provider_name.clone();
            tokio::spawn(async move {
                let mut buf = [0u8; 8192];
                loop {
                    match stderr.read(&mut buf).await {
                        Ok(0) | Err(_) => break, // EOF or pipe gone
                        Ok(n) => {
                            let text = String::from_utf8_lossy(&buf[..n]);
                            let preview: String = text.trim().chars().take(500).collect();
                            if !preview.is_empty() {
                                tracing::warn!(provider = %stderr_provider, %preview, "agent stderr");
                            }
                            if let Ok(mut t) = tail.lock() {
                                t.push_str(&text);
                                if t.len() > MAX_STDERR_CAPTURE {
                                    let mut cut = t.len() - MAX_STDERR_CAPTURE;
                                    while cut < t.len() && !t.is_char_boundary(cut) {
                                        cut += 1;
                                    }
                                    *t = t[cut..].to_string();
                                }
                            }
                        }
                    }
                }
            })
        });
        // Read the retained stderr tail, formatted as a "; stderr: …" suffix.
        let stderr_suffix = || {
            stderr_tail
                .lock()
                .ok()
                .map(|t| t.trim().to_string())
                .filter(|t| !t.is_empty())
                .map(|t| format!("; stderr: {t}"))
                .unwrap_or_default()
        };

        // Drain the agent's stdout so it never reaches container logs. We MUST
        // keep reading it: if the OS pipe buffer (64 KB) fills, the child blocks
        // indefinitely — the same deadlock the stderr comment above warns about.
        // The task ends on EOF when the process exits or is killed.
        if let Some(mut stdout) = child.stdout.take() {
            let drain_provider = provider_name.clone();
            tokio::spawn(async move {
                // Drain in fixed-size chunks rather than whole lines: opencode
                // inlines multi-MB page bodies as a single JSON line, so a
                // line reader would allocate the entire body just to log a
                // 200-char preview — a heap spike on the very worker this
                // watchdog exists to protect. Reading into a fixed buffer keeps
                // the pipe empty without ever holding more than `buf`.
                let mut buf = [0u8; 8192];
                loop {
                    match stdout.read(&mut buf).await {
                        Ok(0) | Err(_) => break, // EOF or pipe gone (process exited/killed)
                        Ok(n) => {
                            // Bounded preview at debug only (off under the
                            // default "info" subscriber). The leading bytes of a
                            // read carry the event type and ids, not the large
                            // inlined bodies; lossy is fine for a log preview.
                            let preview = String::from_utf8_lossy(&buf[..n.min(200)]);
                            let preview = preview.trim_end();
                            if !preview.is_empty() {
                                tracing::debug!(provider = %drain_provider, %preview, "agent stdout");
                            }
                        }
                    }
                }
            });
        }

        let status = tokio::select! {
            result = child.wait() => {
                result.map_err(|e| {
                    PipelineError::Enrich(format!("failed to wait for {}: {e}", provider_name))
                })?
            }
            _ = tokio::time::sleep(config.timeout) => {
                terminate(&mut child, pid).await;
                // Abort the drain task rather than leaving it detached: if a
                // grandchild inherited fd 2 and survived terminate(), the task
                // would otherwise leak. The tail read below is already populated.
                if let Some(t) = &stderr_task {
                    t.abort();
                }
                return Err(PipelineError::Enrich(format!(
                    "{} timed out after {:?}{}",
                    provider_name, config.timeout, stderr_suffix()
                )));
            }
            observed_mb = watch_memory(pid, config.max_rss_mb) => {
                tracing::error!(
                    provider = %provider_name,
                    pid = ?pid,
                    observed_mb,
                    limit_mb = config.max_rss_mb,
                    "LLM subprocess exceeded memory limit, killing to protect the container"
                );
                terminate(&mut child, pid).await;
                if let Some(t) = &stderr_task {
                    t.abort();
                }
                return Err(PipelineError::Enrich(format!(
                    "{provider_name} exceeded memory limit of {} MB (RSS {observed_mb} MB), killed{}",
                    config.max_rss_mb, stderr_suffix()
                )));
            }
        };

        if !status.success() {
            // Give the stderr drain a moment to finish so the tail is complete,
            // but bound the wait: the child has exited, yet a leaked grandchild
            // that inherited fd 2 could keep the pipe open and never EOF. Without
            // a bound this await (outside the timeout/memory select!) would wedge
            // the worker loop. Best-effort, like the tail in the other paths.
            if let Some(task) = stderr_task {
                let _ = tokio::time::timeout(Duration::from_secs(2), task).await;
            }
            return Err(PipelineError::Enrich(format!(
                "{} exited with {}{}",
                provider_name,
                status,
                stderr_suffix()
            )));
        }

        // Success: abort the drain task instead of leaving it detached — same
        // fd-2/grandchild-leak guard as the timeout/OOM paths. Normally it has
        // already finished (the child closed stderr on exit); aborting a finished
        // task is a no-op.
        if let Some(t) = &stderr_task {
            t.abort();
        }
        Ok(())
    }
}

/// Kill the LLM subprocess and reap it.
///
/// Signals the whole process group (negative pid) so any helpers the agent
/// forked (node workers, git) die too — not just the direct child — then
/// falls back to `child.kill()` (covers a missing pid) and waits to avoid a
/// zombie.
async fn terminate(child: &mut tokio::process::Child, pid: Option<u32>) {
    kill_process_group(pid);
    if let Err(e) = child.kill().await {
        // After the group SIGKILL above the direct child is usually already
        // gone, so `ESRCH` here is the expected benign race — only a different
        // error is worth flagging.
        if e.raw_os_error() != Some(libc::ESRCH) {
            tracing::warn!(error = %e, "failed to kill LLM process");
        }
    }
    let _ = child.wait().await;
}

/// Send `SIGKILL` to the entire process group led by `pid`.
///
/// The subprocess is spawned as its own process-group leader
/// (`process_group(0)` in `build_command`), so `kill(-pid, …)` reaps the agent
/// and everything it forked. A failure (e.g. `ESRCH`) just means the group has
/// already exited and is ignored.
fn kill_process_group(pid: Option<u32>) {
    if let Some(pid) = pid {
        // Guard against pid 0: `kill(-0, …)` collapses to `kill(0, …)`, which
        // POSIX routes to the *caller's own* process group — it would SIGKILL
        // the worker itself. `child.id()` never yields 0 in practice, but the
        // consequence is catastrophic enough to refuse it defensively.
        if pid == 0 {
            return;
        }
        // SAFETY: `kill(2)` with a negative pid targets a process group and has
        // no memory-safety implications; the return value is intentionally
        // ignored (the group may already be gone). Linux PIDs are capped well
        // below `i32::MAX` (`pid_max` <= 2^22), so `pid as i32` cannot overflow.
        unsafe {
            libc::kill(-(pid as i32), libc::SIGKILL);
        }
    }
}

/// Memory watchdog: resolve with the observed RSS (in MB) once the subprocess
/// exceeds `max_rss_mb`.
///
/// The LLM agent accumulates fetched-page bodies in memory across a run; left
/// unchecked it climbs to the container's cgroup limit and triggers an OOM kill
/// of the whole pod. Polling RSS and killing the offender instead turns that
/// into a clean, retryable job failure.
///
/// Disabled (never resolves, letting the `child.wait()`/timeout branches win)
/// when `pid` is `None` or `max_rss_mb` is 0. We poll `/proc` directly rather
/// than capping virtual memory with `RLIMIT_AS`: V8 reserves a huge virtual
/// address space, so a virtual-memory ceiling would crash Node outright.
async fn watch_memory(pid: Option<u32>, max_rss_mb: u64) -> u64 {
    let pid = match pid {
        Some(p) if max_rss_mb > 0 => p,
        _ => {
            tracing::debug!("enrich memory watchdog disabled (no pid or zero limit)");
            std::future::pending::<()>().await;
            unreachable!("pending future never resolves");
        }
    };

    // RSS polling reads `/proc/<pid>/status`, which only exists on Linux (the
    // deploy target). On a non-Linux dev machine `read_vmrss_kb` would return
    // `None` every poll and the watchdog would never fire — silently. Make that
    // degradation explicit and skip the pointless poll loop rather than letting
    // a developer believe a ceiling is enforced when it is not.
    if !cfg!(target_os = "linux") {
        tracing::debug!(
            "enrich memory watchdog inactive: RSS polling needs /proc (Linux only); \
             agent runs without a memory ceiling on this platform"
        );
        std::future::pending::<()>().await;
        unreachable!("pending future never resolves");
    }

    let interval = Duration::from_secs(5);
    loop {
        tokio::time::sleep(interval).await;
        // A missing/unparsable status file means the process is gone; keep
        // looping (harmless) and let `child.wait()` win the select.
        if let Some(rss_kb) = read_vmrss_kb(pid).await {
            let rss_mb = rss_kb / 1024;
            if rss_mb > max_rss_mb {
                return rss_mb;
            }
        }
    }
}

/// Read a process's resident set size (RSS) from `/proc/<pid>/status`, in kB.
/// Returns `None` if the file is missing or unparsable (e.g. the process exited).
///
/// This measures only the direct child PID, not the whole process group that
/// `kill_process_group` SIGKILLs. That asymmetry is intentional: for the current
/// opencode/Claude agent topology the runaway memory is the accumulated
/// fetched-page bodies held in the main node process we spawn, so its RSS is the
/// signal that matters. If a future agent moved that growth into a forked
/// subprocess, this would need to sum the group's RSS (e.g. walk
/// `/proc/<pid>/task` / children) to stay accurate.
async fn read_vmrss_kb(pid: u32) -> Option<u64> {
    let status = tokio::fs::read_to_string(format!("/proc/{pid}/status"))
        .await
        .ok()?;
    parse_vmrss_kb(&status)
}

/// Parse the `VmRSS` value (in kB) out of `/proc/<pid>/status` contents.
fn parse_vmrss_kb(status: &str) -> Option<u64> {
    status
        .lines()
        .find_map(|line| line.strip_prefix("VmRSS:"))
        .and_then(|rest| rest.split_whitespace().next())
        .and_then(|kb| kb.parse::<u64>().ok())
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
    /// Recursion depth for related-legislation follow-up harvests. Inherited
    /// from the harvest job that spawned this enrichment. `None` or `0` means a
    /// root enrichment; the child harvests it enqueues get `depth + 1`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub depth: Option<u32>,
}

/// All known provider names. Used to create one enrich job per provider
/// after a successful harvest.
pub const ENRICH_PROVIDERS: &[&str] = &["opencode", "claude"];

/// A related-legislation reference returned by the enrichment agent in the
/// `.enrichment-result.yaml` sidecar (the "result envelope").
///
/// The extref-only recursive harvester only follows explicit BWB cross-links in
/// the source text, so it misses delegated regelingen and other laws a
/// machine-readable model actually depends on (a `source.regulation`, a
/// `legal_basis`, or an `open_term` delegation). The enrichment agent knows
/// these because it just modeled them, so it declares them here and the worker
/// enqueues follow-up harvests — letting the dependency graph fill itself in.
///
/// This lives OUTSIDE the law schema on purpose: the law YAML stays
/// schema-conformant, and this provenance/routing metadata rides alongside it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RelatedLegislation {
    /// Human-readable name of the related law/regeling (used for SRU fallback
    /// resolution when no `bwb_id`/`slug` is supplied).
    pub name: String,
    /// How this law relates: `source_regulation`, `legal_basis`, or
    /// `delegated_regeling`. Informational; the worker treats all the same.
    #[serde(default)]
    pub relation: String,
    /// Best-effort BWB identifier (e.g. "BWBR0018451"). Preferred resolution.
    #[serde(default)]
    pub bwb_id: Option<String>,
    /// Best-effort corpus slug (e.g. "wet_op_de_zorgtoeslag"). Second-choice
    /// resolution, looked up against `law_entries`.
    #[serde(default)]
    pub slug: Option<String>,
    /// The `open_term` id this delegation fills, when `relation` is a delegation.
    #[serde(default)]
    pub open_term: Option<String>,
}

/// The `.enrichment-result.yaml` result envelope written next to an enriched
/// law YAML. Deliberately NOT a law-schema change — see [`RelatedLegislation`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct EnrichmentResultEnvelope {
    #[serde(default)]
    pub law_id: Option<String>,
    #[serde(default)]
    pub related_legislation: Vec<RelatedLegislation>,
}

/// Read the sibling `.enrichment-result.yaml` result envelope for a law YAML.
///
/// Never errors, so it can never fail an otherwise-successful enrichment:
/// - absent file → empty list;
/// - unparseable file → logged at `warn` and empty list.
async fn read_enrichment_result_envelope(yaml_abs: &Path) -> Vec<RelatedLegislation> {
    let envelope_path = enrichment_result_path(yaml_abs);
    let content = match tokio::fs::read_to_string(&envelope_path).await {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    match serde_yaml_ng::from_str::<EnrichmentResultEnvelope>(&content) {
        Ok(envelope) => envelope.related_legislation,
        Err(e) => {
            tracing::warn!(
                path = %envelope_path.display(),
                error = %e,
                "failed to parse .enrichment-result.yaml; ignoring related legislation"
            );
            Vec::new()
        }
    }
}

/// Path of the `.enrichment-result.yaml` sidecar next to a law YAML file.
fn enrichment_result_path(yaml_abs: &Path) -> PathBuf {
    yaml_abs
        .parent()
        .unwrap_or(Path::new("."))
        .join(".enrichment-result.yaml")
}

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
    /// Related legislation the enrichment agent declared this law depends on,
    /// read from the `.enrichment-result.yaml` sidecar. The worker uses these to
    /// enqueue follow-up harvests. Empty when no sidecar was written.
    #[serde(default)]
    pub related_legislation: Vec<RelatedLegislation>,
    /// Untranslatable constructs captured from the enriched YAML (RFC-012):
    /// legal constructs the agent could not express with the engine's current
    /// operation set. The worker persists these to the `untranslatables` table;
    /// they also ride here in `jobs.result`. `#[serde(default)]` keeps older
    /// stored results deserializable.
    #[serde(default)]
    pub untranslatables: Vec<CapturedUntranslatable>,
}

/// A single untranslatable captured from an enriched article, flattened for
/// persistence. DB-free by design: it rides in `jobs.result` JSON and is written
/// to the `untranslatables` table by the worker.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapturedUntranslatable {
    /// The owning article's number (`Article.number`).
    pub article: String,
    pub construct: String,
    pub reason: String,
    pub suggestion: Option<String>,
    pub legal_text_excerpt: Option<String>,
    pub accepted: bool,
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
/// - **Claude**: authenticates with a **personal Claude subscription** via
///   `CLAUDE_CODE_OAUTH_TOKEN` (from `claude setup-token`), read directly from the
///   environment; no credentials file is written. `ANTHROPIC_API_KEY` is intentionally NOT
///   used — it is not on `LLM_ENV_ALLOWLIST`, so it is never forwarded to `claude` and can
///   never take precedence over the OAuth token.
///
/// In Docker, set the appropriate env var (forwarded to the subprocess via
/// `LLM_ENV_ALLOWLIST`).
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
    /// RSS ceiling (MB) for the LLM subprocess. When it is exceeded the worker
    /// kills the process and fails the job instead of letting the agent OOM the
    /// whole container. 0 disables the watchdog.
    pub max_rss_mb: u64,
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

        // RSS ceiling for the LLM subprocess. Default 3500 MB leaves headroom
        // under the 4096Mi container limit for the worker, git, and node's
        // baseline plus the ~5s watchdog poll lag.
        let max_rss_mb = std::env::var("ENRICH_MAX_RSS_MB")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(3500);

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
            max_rss_mb,
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
            max_rss_mb: self.max_rss_mb,
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
    // Provider-specific auth.
    //
    // NOTE: ANTHROPIC_API_KEY is deliberately NOT forwarded. The claude provider
    // authenticates only with a personal subscription via CLAUDE_CODE_OAUTH_TOKEN.
    // Keeping ANTHROPIC_API_KEY out of the subprocess env means that even if it is
    // still set on the worker (e.g. a leftover), it can never reach `claude` and
    // silently take precedence over the OAuth token — the exact footgun that
    // makes claude fail auth at startup.
    "CLAUDE_CODE_OAUTH_TOKEN",
    "VLAM_API_KEY",
    "OPENCODE_",
];

/// Check whether an environment variable name is on the allowlist.
fn env_allowed(key: &str) -> bool {
    LLM_ENV_ALLOWLIST
        .iter()
        .any(|prefix| key == *prefix || key.starts_with(prefix))
}

/// Select one Claude OAuth token from a comma-separated list, rotating by a
/// time `bucket` so consecutive runs spread across several personal
/// subscriptions (each token has its own usage/rate limits).
///
/// `CLAUDE_CODE_OAUTH_TOKEN` may hold multiple tokens separated by commas. The
/// chosen index is `bucket % n`; callers pass `unix_secs / 100`, so the active
/// token rotates roughly every 100 seconds. Returns `(index, count, token)`, or
/// `None` when there are no non-empty tokens. Pure so it can be unit-tested.
fn select_claude_token(raw: &str, bucket: u64) -> Option<(usize, usize, &str)> {
    let tokens: Vec<&str> = raw
        .split(',')
        .map(|t| t.trim())
        .filter(|t| !t.is_empty())
        .collect();
    if tokens.is_empty() {
        return None;
    }
    let idx = (bucket % tokens.len() as u64) as usize;
    Some((idx, tokens.len(), tokens[idx]))
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

    // Diagnostic logging: record exactly what will be spawned and which env vars
    // are forwarded (NAMES only — never values). Classify the OAuth token by its
    // non-secret prefix so a misconfiguration (an API key pasted into the OAuth
    // slot) is obvious, and flag whether ANTHROPIC_API_KEY is still present in the
    // worker env even though it is deliberately never forwarded.
    let forwarded_env: Vec<&str> = safe_env.iter().map(|(k, _)| k.as_str()).collect();
    let oauth_token_kind = std::env::var("CLAUDE_CODE_OAUTH_TOKEN").ok().map(|t| {
        if t.is_empty() {
            "empty"
        } else if t.starts_with("sk-ant-oat") {
            "oauth-token (sk-ant-oat…)"
        } else if t.starts_with("sk-ant-api") {
            "WRONG: looks like an API key (sk-ant-api…)"
        } else {
            "unrecognized-prefix"
        }
    });
    let model = match provider {
        LlmProvider::OpenCode { model, .. } | LlmProvider::Claude { model, .. } => model.as_deref(),
    };
    tracing::info!(
        provider = provider.name(),
        model = ?model,
        prompt_chars = prompt.len(),
        claude_oauth_token_kind = ?oauth_token_kind,
        anthropic_api_key_present_in_worker_env = std::env::var_os("ANTHROPIC_API_KEY").is_some(),
        "spawning LLM subprocess"
    );
    // The forwarded env var NAMES are static between spawns — keep them at debug
    // so they don't add a long line to every job's info logs.
    tracing::debug!(provider = provider.name(), forwarded_env = ?forwarded_env, "forwarded env to LLM subprocess");

    let mut cmd = match provider {
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
            // If CLAUDE_CODE_OAUTH_TOKEN holds several comma-separated tokens,
            // override the forwarded value with a single one chosen by a
            // time-rotating index, so load spreads across the subscriptions.
            if let Ok(raw) = std::env::var("CLAUDE_CODE_OAUTH_TOKEN") {
                let bucket = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs() / 100)
                    .unwrap_or(0);
                if let Some((idx, count, token)) = select_claude_token(&raw, bucket) {
                    // Always apply the selected (trimmed) token — even for a
                    // single token — so stray whitespace or a trailing comma
                    // never reaches claude verbatim. Never log the token value;
                    // only the 1-based position and count.
                    cmd.env("CLAUDE_CODE_OAUTH_TOKEN", token);
                    if count > 1 {
                        tracing::info!(
                            using_token = idx + 1,
                            of_tokens = count,
                            "selected claude oauth token (rotating by ~100s)"
                        );
                    }
                }
            }
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
    };

    // Run the agent as its own process-group leader so a timeout/memory kill can
    // signal the whole tree (the CLI plus any node workers or git it forks), not
    // just the direct child. `kill_on_drop` is a backstop: if the worker future
    // is dropped (panic, early return) the child is reaped rather than orphaned.
    cmd.process_group(0);
    cmd.kill_on_drop(true);
    cmd
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

    let law_dir = Path::new(&normalized)
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .filter(|d| !d.is_empty());

    // Sparse checkout: only the law directory + features/
    if let Some(ref dir) = law_dir {
        config.sparse_paths = Some(vec![dir.clone(), "features".to_string()]);
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

    // Prefer the worker's own base branch (e.g. `pr574`) so PR deployments
    // enrich their own harvested YAML, not production's. Probe the remote
    // first and fall back to `development` only when the branch doesn't
    // exist yet — which covers a fresh PR whose harvester hasn't pushed.
    // Probing explicitly (instead of try-then-fallback on any error)
    // prevents an unrelated `checkout` or `reset` failure from silently
    // dropping the enrichment back to production's branch.
    //
    // Pass the exact file path (not the directory) so the `ls-files` guard
    // inside `checkout_from_branch` doesn't match sibling files and skip
    // fetching a newly harvested version of an already-known law.
    let preferred_base = base_config.branch.as_str();
    let preferred_exists = if preferred_base == "development" || branch_is_known(preferred_base) {
        true
    } else {
        let exists = client.remote_branch_exists(preferred_base).await?;
        if exists {
            remember_branch(preferred_base);
        }
        exists
    };
    let base_branch = pick_enrich_base(preferred_base, preferred_exists);
    if !preferred_exists {
        tracing::info!(
            branch = %preferred_base,
            "base branch not yet published on remote, using development for first enrichment"
        );
    }

    client
        .checkout_from_branch(base_branch, &[&normalized])
        .await?;

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

    // Read the related-legislation result envelope the agent may have written.
    // Never fails: absent/malformed → empty (see read_enrichment_result_envelope).
    let related_legislation = read_enrichment_result_envelope(&yaml_abs).await;

    // Capture the untranslatables the agent flagged in the enriched YAML (RFC-012).
    let untranslatables = collect_untranslatables(&yaml_abs).await?;

    // Collect written files for corpus staging
    let mut written_files = vec![yaml_abs.clone(), metadata_path];

    // Stage the result envelope as provenance when the agent wrote one.
    let envelope_path = enrichment_result_path(&yaml_abs);
    if envelope_path.exists() {
        written_files.push(envelope_path);
    }

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
        related_legislation,
        untranslatables,
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

/// Count total articles and articles with a `machine_readable` section in one
/// parse pass.
///
/// The law is parsed into the canonical [`ArticleBasedLaw`] model
/// (`regelrecht-law-model`) rather than walked as an untyped YAML value, so the
/// field access is type-checked against the single source of truth for the law
/// format. A structurally-invalid law now surfaces as a parse error here instead
/// of being silently undercounted — acceptable because this only ever runs on
/// real harvested/enriched corpus files, where a corruption is worth failing on.
///
/// An article counts as enriched when it carries a `machine_readable` mapping,
/// including the empty `{}` an LLM may insert before filling it; an explicit
/// `machine_readable: null` is treated as un-enriched. No corpus file uses the
/// bare/null form, so this matches the previous key-presence behavior in practice.
async fn count_article_stats(path: &Path) -> Result<(usize, usize)> {
    let content = tokio::fs::read_to_string(path).await?;
    let law: ArticleBasedLaw = serde_yaml_ng::from_str(&content)?;
    let total = law.articles.len();
    let with_machine_readable = law
        .articles
        .iter()
        .filter(|article| article.machine_readable.is_some())
        .count();
    Ok((total, with_machine_readable))
}

/// Collect all untranslatables from an enriched law YAML, flattened to
/// [`CapturedUntranslatable`] with the owning article number attached.
///
/// Parses the law into the canonical [`ArticleBasedLaw`] model, mirroring
/// [`count_article_stats`]. Returns an empty vec when no article declares any.
async fn collect_untranslatables(path: &Path) -> Result<Vec<CapturedUntranslatable>> {
    let content = tokio::fs::read_to_string(path).await?;
    let law: ArticleBasedLaw = serde_yaml_ng::from_str(&content)?;
    let mut out = Vec::new();
    for article in &law.articles {
        let Some(machine_readable) = &article.machine_readable else {
            continue;
        };
        let Some(entries) = &machine_readable.untranslatables else {
            continue;
        };
        for entry in entries {
            out.push(CapturedUntranslatable {
                article: article.number.clone(),
                construct: entry.construct.clone(),
                reason: entry.reason.clone(),
                suggestion: entry.suggestion.clone(),
                legal_text_excerpt: entry.legal_text_excerpt.clone(),
                accepted: entry.accepted,
            });
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pick_enrich_base_uses_preferred_when_remote_has_it() {
        assert_eq!(pick_enrich_base("pr574", true), "pr574");
    }

    #[test]
    fn pick_enrich_base_falls_back_when_preferred_missing() {
        // Fresh PR deployment whose harvester hasn't pushed its branch yet:
        // enrichment must fall back to development instead of failing.
        assert_eq!(pick_enrich_base("pr574", false), "development");
    }

    #[test]
    fn pick_enrich_base_short_circuits_for_development() {
        // When the worker's own base is already `development`, the
        // remote-exists bool is moot and we always use `development`.
        assert_eq!(pick_enrich_base("development", true), "development");
        assert_eq!(pick_enrich_base("development", false), "development");
    }

    #[test]
    fn test_enrich_payload_serde_roundtrip() {
        let payload = EnrichPayload {
            law_id: "BWBR0018451".to_string(),
            yaml_path: "regulation/nl/wet/wet_op_de_zorgtoeslag/2025-01-01.yaml".to_string(),
            provider: Some("claude".to_string()),
            depth: Some(2),
        };

        let json = serde_json::to_string(&payload).unwrap();
        let deserialized: EnrichPayload = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.provider.as_deref(), Some("claude"));
        assert_eq!(deserialized.depth, Some(2));

        // Verify backward compatibility: provider and depth are optional and
        // skipped when None (old queued payloads omit them entirely).
        let payload_no_provider = EnrichPayload {
            law_id: "BWBR0018451".to_string(),
            yaml_path: "regulation/nl/wet/wet_op_de_zorgtoeslag/2025-01-01.yaml".to_string(),
            provider: None,
            depth: None,
        };
        let json_no_provider = serde_json::to_string(&payload_no_provider).unwrap();
        assert!(!json_no_provider.contains("provider"));
        assert!(!json_no_provider.contains("depth"));
        let deserialized_no_provider: EnrichPayload =
            serde_json::from_str(&json_no_provider).unwrap();
        assert!(deserialized_no_provider.provider.is_none());
        assert!(deserialized_no_provider.depth.is_none());

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
            related_legislation: Vec::new(),
            untranslatables: Vec::new(),
        };

        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["articles_with_machine_readable"], 7);
        assert_eq!(json["coverage_score"], 0.7);
        assert_eq!(json["provider"], "opencode");
        assert_eq!(json["branch"], "enrich/opencode");
    }

    #[test]
    fn test_envelope_full_deserialization() {
        let yaml = r#"
law_id: wet_op_de_zorgtoeslag
related_legislation:
  - name: Regeling vaststelling standaardpremie en bestuursrechtelijke premie
    relation: delegated_regeling
    bwb_id: BWBR0037841
    slug: regeling_standaardpremie
    open_term: standaardpremie
  - name: Algemene wet inkomensafhankelijke regelingen
    relation: source_regulation
"#;
        let envelope: EnrichmentResultEnvelope = serde_yaml_ng::from_str(yaml).unwrap();
        assert_eq!(envelope.law_id.as_deref(), Some("wet_op_de_zorgtoeslag"));
        assert_eq!(envelope.related_legislation.len(), 2);
        let first = &envelope.related_legislation[0];
        assert_eq!(first.relation, "delegated_regeling");
        assert_eq!(first.bwb_id.as_deref(), Some("BWBR0037841"));
        assert_eq!(first.slug.as_deref(), Some("regeling_standaardpremie"));
        assert_eq!(first.open_term.as_deref(), Some("standaardpremie"));
        // Second entry omits every optional field.
        let second = &envelope.related_legislation[1];
        assert_eq!(second.relation, "source_regulation");
        assert!(second.bwb_id.is_none());
        assert!(second.slug.is_none());
        assert!(second.open_term.is_none());
    }

    #[test]
    fn test_envelope_missing_fields_default() {
        // Only `name` is required; everything else defaults.
        let yaml = "related_legislation:\n  - name: Some Law\n";
        let envelope: EnrichmentResultEnvelope = serde_yaml_ng::from_str(yaml).unwrap();
        assert!(envelope.law_id.is_none());
        assert_eq!(envelope.related_legislation.len(), 1);
        let entry = &envelope.related_legislation[0];
        assert_eq!(entry.name, "Some Law");
        assert_eq!(entry.relation, "");
        assert!(entry.bwb_id.is_none());
    }

    #[tokio::test]
    async fn test_read_envelope_absent_file_is_empty() {
        let dir = tempfile::tempdir().unwrap();
        let yaml_abs = dir.path().join("2025-01-01.yaml");
        // No sidecar exists next to it.
        assert!(read_enrichment_result_envelope(&yaml_abs).await.is_empty());
    }

    #[tokio::test]
    async fn test_read_envelope_malformed_is_empty() {
        let dir = tempfile::tempdir().unwrap();
        let yaml_abs = dir.path().join("2025-01-01.yaml");
        std::fs::write(
            enrichment_result_path(&yaml_abs),
            "related_legislation: [this is: not valid: yaml",
        )
        .unwrap();
        // Malformed sidecar must never error — it degrades to empty.
        assert!(read_enrichment_result_envelope(&yaml_abs).await.is_empty());
    }

    #[tokio::test]
    async fn test_read_envelope_present_parses() {
        let dir = tempfile::tempdir().unwrap();
        let yaml_abs = dir.path().join("2025-01-01.yaml");
        std::fs::write(
            enrichment_result_path(&yaml_abs),
            "related_legislation:\n  - name: Delegated Regeling\n    bwb_id: BWBR0037841\n",
        )
        .unwrap();
        let related = read_enrichment_result_envelope(&yaml_abs).await;
        assert_eq!(related.len(), 1);
        assert_eq!(related[0].bwb_id.as_deref(), Some("BWBR0037841"));
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
            max_rss_mb: 3500,
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
        // The memory ceiling must survive a provider override.
        assert_eq!(claude_config.max_rss_mb, 3500);

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
    fn test_select_claude_token() {
        // empty / whitespace-only -> None
        assert_eq!(select_claude_token("", 0), None);
        assert_eq!(select_claude_token("  , ,", 5), None);

        // single token -> always that token, index 0
        assert_eq!(select_claude_token("tokA", 0), Some((0, 1, "tokA")));
        assert_eq!(select_claude_token(" tokA ", 999), Some((0, 1, "tokA")));

        // multiple tokens -> rotate by bucket % n, whitespace trimmed
        assert_eq!(select_claude_token("a, b , c", 0), Some((0, 3, "a")));
        assert_eq!(select_claude_token("a, b , c", 1), Some((1, 3, "b")));
        assert_eq!(select_claude_token("a, b , c", 2), Some((2, 3, "c")));
        assert_eq!(select_claude_token("a, b , c", 3), Some((0, 3, "a")));
        // large bucket (e.g. unix_secs/100) still wraps correctly
        assert_eq!(select_claude_token("a,b", 17_000_001), Some((1, 2, "b")));
    }

    #[test]
    fn test_parse_vmrss_kb_extracts_value() {
        let status = "Name:\tnode\nVmPeak:\t 4194304 kB\nVmRSS:\t  2097152 kB\nThreads:\t12\n";
        assert_eq!(parse_vmrss_kb(status), Some(2_097_152));
    }

    #[test]
    fn test_parse_vmrss_kb_missing_or_malformed() {
        // No VmRSS line.
        assert_eq!(parse_vmrss_kb("Name:\tnode\nThreads:\t12\n"), None);
        // VmRSS present but value not numeric.
        assert_eq!(parse_vmrss_kb("VmRSS:\t  notanumber kB\n"), None);
        // Empty input.
        assert_eq!(parse_vmrss_kb(""), None);
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

    #[tokio::test]
    async fn test_count_article_stats() {
        // A realistic minimal article-based law: typed counting requires the
        // canonical top-level fields ($id/regulatory_layer/publication_date) and
        // articles with number+text, so the fixture mirrors a real harvested law.
        let yaml = r#"---
$id: test_law
regulatory_layer: WET
publication_date: '2025-01-01'
valid_from: '2025-01-01'
articles:
  - number: '1'
    text: Article one.
  - number: '2'
    text: Article two.
  - number: '3'
    text: Article three.
    machine_readable:
      execution:
        actions: []
"#;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("law.yaml");
        tokio::fs::write(&path, yaml).await.unwrap();

        let (total, with_mr) = count_article_stats(&path).await.unwrap();
        assert_eq!(total, 3);
        assert_eq!(with_mr, 1);
    }

    #[tokio::test]
    async fn test_collect_untranslatables() {
        // Two articles carry untranslatables (one accepted, one not); a third
        // article has a machine_readable section without any. The collector must
        // flatten every entry, attach the owning article number, and preserve the
        // optional fields + accepted flag.
        let yaml = r#"---
$id: test_law
regulatory_layer: WET
publication_date: '2025-01-01'
valid_from: '2025-01-01'
articles:
  - number: '1'
    text: Article one.
    machine_readable:
      untranslatables:
        - construct: rounding
          reason: Engine cannot round yet.
          suggestion: Add a ROUND operation.
          legal_text_excerpt: naar boven afgerond op hele euro's
          accepted: false
  - number: '2'
    text: Article two.
    machine_readable:
      execution:
        actions: []
  - number: '3'
    text: Article three.
    machine_readable:
      untranslatables:
        - construct: table_lookup
          reason: Table lookup unsupported.
          accepted: true
"#;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("law.yaml");
        tokio::fs::write(&path, yaml).await.unwrap();

        let collected = collect_untranslatables(&path).await.unwrap();
        assert_eq!(collected.len(), 2);

        let rounding = collected
            .iter()
            .find(|u| u.construct == "rounding")
            .expect("rounding entry");
        assert_eq!(rounding.article, "1");
        assert_eq!(rounding.reason, "Engine cannot round yet.");
        assert_eq!(
            rounding.suggestion.as_deref(),
            Some("Add a ROUND operation.")
        );
        assert_eq!(
            rounding.legal_text_excerpt.as_deref(),
            Some("naar boven afgerond op hele euro's")
        );
        assert!(!rounding.accepted);

        let lookup = collected
            .iter()
            .find(|u| u.construct == "table_lookup")
            .expect("table_lookup entry");
        assert_eq!(lookup.article, "3");
        assert!(lookup.suggestion.is_none());
        assert!(lookup.legal_text_excerpt.is_none());
        assert!(lookup.accepted);
    }

    #[tokio::test]
    async fn test_collect_untranslatables_none() {
        let yaml = r#"---
$id: test_law
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Article one.
"#;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("law.yaml");
        tokio::fs::write(&path, yaml).await.unwrap();

        assert!(collect_untranslatables(&path).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_count_article_stats_empty_vs_null_machine_readable() {
        // An empty `machine_readable: {}` mapping counts as enriched — this
        // matches the old key-presence semantics that `FakeLlmRunner` (and the
        // enrichment delta) rely on when the LLM inserts a bare section. An
        // explicit `machine_readable: null` deserializes to None and is treated
        // as un-enriched; no corpus file uses the bare/null form, so the typed
        // count matches the previous `contains_key` behavior in practice.
        let yaml = r#"---
$id: test_law
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Empty section, enriched.
    machine_readable: {}
  - number: '2'
    text: Null section, not enriched.
    machine_readable: null
  - number: '3'
    text: No section at all.
"#;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("law.yaml");
        tokio::fs::write(&path, yaml).await.unwrap();

        let (total, with_mr) = count_article_stats(&path).await.unwrap();
        assert_eq!(total, 3);
        assert_eq!(with_mr, 1);
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

        let yaml_content = r#"---
$id: test_law
regulatory_layer: WET
publication_date: '2025-01-01'
valid_from: '2025-01-01'
articles:
  - number: '1'
    text: Article one.
  - number: '2'
    text: Article two.
    machine_readable:
      execution:
        actions: []
  - number: '3'
    text: Article three.
"#;
        let yaml_path = "regulation/nl/wet/test_law/2025-01-01.yaml";
        tokio::fs::write(dir.path().join(yaml_path), yaml_content)
            .await
            .unwrap();

        let payload = EnrichPayload {
            law_id: "BWBR0000001".into(),
            yaml_path: yaml_path.into(),
            provider: Some("opencode".into()),
            depth: None,
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

        let yaml_content = "---\n$id: test_law\nregulatory_layer: WET\npublication_date: '2025-01-01'\narticles:\n  - number: '1'\n    text: Article one.\n";
        let yaml_path = "regulation/nl/wet/test_law/2025-01-01.yaml";
        tokio::fs::write(dir.path().join(yaml_path), yaml_content)
            .await
            .unwrap();

        let payload = EnrichPayload {
            law_id: "BWBR0000001".into(),
            yaml_path: yaml_path.into(),
            provider: None,
            depth: None,
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

        let yaml_content = "---\n$id: test_law\nregulatory_layer: WET\npublication_date: '2025-01-01'\narticles:\n  - number: '1'\n    text: Article one.\n";
        let yaml_path = "regulation/nl/wet/test_law/2025-01-01.yaml";
        tokio::fs::write(dir.path().join(yaml_path), yaml_content)
            .await
            .unwrap();

        let payload = EnrichPayload {
            law_id: "BWBR0000001".into(),
            yaml_path: yaml_path.into(),
            provider: None,
            depth: None,
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
