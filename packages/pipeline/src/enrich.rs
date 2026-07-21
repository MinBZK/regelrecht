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

/// Outcome of comparing a target law's base version against the enrichment's
/// recorded provenance. Pure decision so it can be unit-tested without git.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BaseAction {
    /// Law not yet on the enrich branch — check it out fresh from the base.
    CheckoutFresh,
    /// Law present and its recorded base matches the current base — unchanged,
    /// keep the existing enrichment (no fresh checkout).
    Skip,
    /// Law present but with no usable recorded provenance — a "legacy"
    /// enrichment written before the freshness guard existed. Adopt the current
    /// base blob SHA as its baseline (recorded on the next metadata write) and
    /// proceed without a fresh checkout, rather than failing. This grandfathers
    /// every pre-guard enrichment so introducing the guard does not turn them
    /// all into an immediate `Drift` on first re-enrichment.
    AdoptBaseline,
    /// Law present and its *recorded* base moved — fail loudly rather than
    /// silently re-enriching on top of a base that differs from the one the
    /// enrichment was generated against.
    Drift,
}

/// Decide what to do for a target law given whether it is already tracked on
/// the enrich branch, the `source_hash` recorded in its `.enrichment.yaml`
/// (if any), and the current base-branch blob SHA of the law.
pub(crate) fn decide_base_action(
    tracked: bool,
    stored_source_hash: Option<&str>,
    base_sha: &str,
) -> BaseAction {
    if !tracked {
        return BaseAction::CheckoutFresh;
    }
    match stored_source_hash {
        // No usable provenance recorded (absent or empty) — a pre-guard
        // enrichment. Grandfather it by adopting the current base as its
        // baseline instead of treating the unknown as drift.
        None | Some("") => BaseAction::AdoptBaseline,
        // Recorded base matches the current base — unchanged.
        Some(h) if h == base_sha => BaseAction::Skip,
        // Recorded base differs from the current base — genuine drift.
        Some(_) => BaseAction::Drift,
    }
}

/// The article window one enrich run must process, decided by the worker (not
/// the LLM) from the persisted cursor. Pure decision so the chunking contract
/// can be pinned by unit tests without git or an LLM (pattern:
/// [`decide_base_action`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ChunkPlan {
    /// Chunking disabled (`ENRICH_MAX_ARTICLES_PER_RUN=0`): process the whole
    /// law in one session, exactly the pre-chunking behavior.
    WholeLaw,
    /// Process articles `[start, end)` in document order. `law_complete` is
    /// true when this window reaches the end of the document — the law can be
    /// marked `enriched` after this run.
    Chunk {
        start: usize,
        end: usize,
        law_complete: bool,
    },
}

/// Plan the article window for this run.
///
/// The stored cursor only counts when it was recorded for the *same* law YAML
/// path AND still fits the document (`cursor <= articles_total`); anything else
/// (new law version at another path, corrupt metadata, legacy files without
/// cursor fields) resets to 0. The window is document order from the cursor —
/// deliberately NOT "the first N un-enriched articles": the law-generate skill
/// legitimately skips definition/procedure/transitional articles without
/// `machine_readable`, so an un-enriched-first window would revisit the same
/// skipped articles forever and never terminate. A cursor guarantees
/// termination in `ceil(total / N)` successful runs regardless of LLM behavior.
pub(crate) fn plan_chunk(
    max_articles_per_run: usize,
    articles_total: usize,
    stored_cursor: usize,
    stored_cursor_path: &str,
    yaml_path: &str,
) -> ChunkPlan {
    if max_articles_per_run == 0 {
        return ChunkPlan::WholeLaw;
    }
    let start = if stored_cursor_path == yaml_path && stored_cursor <= articles_total {
        stored_cursor
    } else {
        0
    };
    let end = start
        .saturating_add(max_articles_per_run)
        .min(articles_total);
    ChunkPlan::Chunk {
        start,
        end,
        law_complete: end >= articles_total,
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
        // Chunked runs get the explicit-article-subset prompt; whole-law runs
        // keep the original prompt byte-identical.
        let prompt = match &payload.chunk_articles {
            Some(articles) => build_chunk_prompt(
                &payload.yaml_path,
                &progress_path.to_string_lossy(),
                articles,
                payload.skip_mvt.unwrap_or(false),
            ),
            None => build_prompt(&payload.yaml_path, &progress_path.to_string_lossy()),
        };
        run_llm_subprocess(
            &config.provider,
            &prompt,
            Some(yaml_abs),
            repo_path,
            config,
            // Enrich edits YAML in place; it does not need shell access.
            false,
        )
        .await
    }
}

/// Spawn and supervise an LLM agent subprocess, provider-agnostically.
///
/// This is the reusable core lifted out of [`ProcessLlmRunner::run`]: it builds
/// the command, drains stdout/stderr (retaining a bounded stderr tail for the
/// error message), and races the child against the configured timeout and the
/// RSS memory watchdog, killing the whole process group on either. `cwd` is the
/// working directory the agent runs in (and writes its output into); `file_arg`
/// is the optional single input file (OpenCode's `-f`). Callers supply their own
/// `prompt` — enrich and document-convert differ only in that prompt.
/// `allow_bash` widens the `claude` provider's tool allowlist to include `Bash`
/// (enrich keeps it off; document-convert needs it so the agent can run/install
/// a converter). It has no effect on `opencode`, which has its own tool model.
pub(crate) async fn run_llm_subprocess(
    provider: &LlmProvider,
    prompt: &str,
    file_arg: Option<&Path>,
    cwd: &Path,
    config: &EnrichConfig,
    allow_bash: bool,
) -> Result<()> {
    let provider_name = provider.name().to_string();

    let mut cmd = build_command(provider, prompt, file_arg, cwd, allow_bash);

    // Both streams are piped and drained. stdout is drained-and-discarded: a
    // verbose agent (e.g. opencode `--format json`) inlines the full body of
    // every fetched page into its event stream, which would flood container
    // logs. stderr is drained too — we MUST keep reading both or a full 64 KB
    // OS pipe buffer blocks the child — but for stderr we also keep a bounded
    // tail and re-log previews, so the LLM's real error (e.g. an auth 401) is
    // both visible in the logs and attached to the job's failure.
    cmd.stderr(std::process::Stdio::piped());
    cmd.stdout(std::process::Stdio::piped());
    let mut child = cmd
        .spawn()
        .map_err(|e| PipelineError::Enrich(format!("failed to spawn {}: {e}", provider_name)))?;

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
    /// Account dat de taak-flow-enrichment aanvroeg (gezet wanneer
    /// `deliver == "task"`); bepaalt de assignee van de review-taak.
    /// De taak-flow-gate zelf is `deliver_as_task()`, niet dit veld.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requested_by: Option<Uuid>,
    /// `"task"` ⇒ resultaat als job_blobs + taak, géén push (taak-flow).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deliver: Option<String>,
    /// Eigenaar-traject van de taak-flow (voor de tasks-rij + save-URL's).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub traject_id: Option<Uuid>,
    /// URL-vorm van het traject (`{slug}-{8hex}`), voor de task-payload
    /// zodat de frontend er review-URL's mee kan bouwen.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub traject_ref: Option<String>,
    /// `document_etag()` van de wet-YAML op aanvraagmoment (staleness-check).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_etag: Option<String>,
    /// `true` wanneer deze enrichment een NIEUWE wet betreft die nog niet in
    /// het traject bestaat (geketend vanuit een `law_convert`-job). Stuurt de
    /// review-taak: `kind: "law_create"` + eigen titel, zodat de editor het
    /// voorstel als aan-te-maken wet behandelt i.p.v. als wijziging.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub new_law: Option<bool>,
    /// Article numbers this run must process (chunked enrichment). Computed
    /// per run by `execute_enrich_with_runner` from the persisted cursor and
    /// passed to the [`LlmRunner`] via the normalized payload — never stored
    /// in queue payloads (`skip_serializing_if` keeps old payloads and the
    /// runner trait untouched). `None` means whole-law mode.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chunk_articles: Option<Vec<String>>,
    /// `true` on continuation chunks (cursor > 0): the MvT-research step ran
    /// during the first chunk and its feature file is already on the branch,
    /// so the prompt tells the agent to skip step 1. Transport-only, like
    /// `chunk_articles`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skip_mvt: Option<bool>,
}

impl EnrichPayload {
    /// Taak-flow: resultaat naar Postgres + taak i.p.v. push naar git.
    pub fn deliver_as_task(&self) -> bool {
        self.deliver.as_deref() == Some("task")
    }
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
    /// Per-chunk review report (chunked enrichment only). A chunk may
    /// legitimately add zero `machine_readable` sections (e.g. a
    /// transitional-law chapter); this report is the agent's proof that it
    /// actually reviewed the window, so the run can still count as progress.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chunk_report: Option<ChunkReport>,
}

/// Proof-of-review for one enrichment chunk, written by the agent into the
/// `.enrichment-result.yaml` envelope. See [`EnrichmentResultEnvelope`].
///
/// Only counts as proof when it references at least one article of the chunk
/// window it was written for (checked by the no-op guard in
/// `execute_enrich_with_runner`): an empty or unrelated report must not
/// advance the cursor past an unreviewed window.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct ChunkReport {
    /// Article numbers the agent reviewed this session.
    #[serde(default)]
    pub articles_reviewed: Vec<String>,
    /// Articles deliberately left without `machine_readable`, with the reason
    /// (e.g. "definition article", "transitional law").
    #[serde(default)]
    pub articles_skipped: Vec<SkippedArticle>,
}

/// One deliberately-skipped article in a [`ChunkReport`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SkippedArticle {
    pub number: String,
    #[serde(default)]
    pub reason: String,
}

/// Read the sibling `.enrichment-result.yaml` result envelope for a law YAML.
///
/// Never errors, so it can never fail an otherwise-successful enrichment:
/// - absent file → default (empty) envelope;
/// - unparseable file → logged at `warn` and default envelope.
async fn read_enrichment_result_envelope(yaml_abs: &Path) -> EnrichmentResultEnvelope {
    let envelope_path = enrichment_result_path(yaml_abs);
    let content = match tokio::fs::read_to_string(&envelope_path).await {
        Ok(c) => c,
        Err(_) => return EnrichmentResultEnvelope::default(),
    };
    match serde_yaml_ng::from_str::<EnrichmentResultEnvelope>(&content) {
        Ok(envelope) => envelope,
        Err(e) => {
            tracing::warn!(
                path = %envelope_path.display(),
                error = %e,
                "failed to parse .enrichment-result.yaml; ignoring its contents"
            );
            EnrichmentResultEnvelope::default()
        }
    }
}

/// Strip a stale `chunk_report` from the `.enrichment-result.yaml` sidecar
/// before a chunked LLM run.
///
/// The envelope is committed to the enrich branch as provenance, so the fresh
/// checkout of a continuation chunk still contains the *previous* chunk's
/// `chunk_report`. Left in place, the no-op guard would accept that stale
/// report as proof-of-review for THIS window and silently advance the cursor
/// past an unreviewed window. Removing only the `chunk_report` key (all other
/// envelope contents — e.g. `related_legislation` — stay intact, via a raw
/// `Value` edit so unknown keys survive too) guarantees any report present
/// after the run was written this session.
///
/// Best-effort: an absent, unparseable, or non-mapping file is left alone —
/// `read_enrichment_result_envelope` already degrades those to an empty
/// envelope (no `chunk_report`) after the run, which keeps the guard sound.
async fn clear_stale_chunk_report(yaml_abs: &Path) {
    let envelope_path = enrichment_result_path(yaml_abs);
    let Ok(content) = tokio::fs::read_to_string(&envelope_path).await else {
        return;
    };
    let Ok(mut value) = serde_yaml_ng::from_str::<serde_yaml_ng::Value>(&content) else {
        return;
    };
    let Some(map) = value.as_mapping_mut() else {
        return;
    };
    if map.remove("chunk_report").is_none() {
        return;
    }
    match serde_yaml_ng::to_string(&value) {
        Ok(stripped) => {
            if let Err(e) = tokio::fs::write(&envelope_path, stripped).await {
                tracing::warn!(
                    path = %envelope_path.display(),
                    error = %e,
                    "failed to strip stale chunk_report from result envelope"
                );
            }
        }
        Err(e) => {
            tracing::warn!(
                path = %envelope_path.display(),
                error = %e,
                "failed to re-serialize result envelope while stripping stale chunk_report"
            );
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
    /// `false` when this run was a chunk that did NOT reach the end of the
    /// document: the law must stay `enriching` and the worker enqueues a
    /// continuation job. Defaults to `true` so pre-chunking `jobs.result` JSON
    /// (which lacks the field and always covered the whole law) still
    /// deserializes as complete.
    #[serde(default = "default_law_complete")]
    pub law_complete: bool,
    /// Cursor after this run (index of the first unprocessed article, in
    /// document order). 0 in whole-law mode.
    #[serde(default)]
    pub enrich_cursor: usize,
}

/// Serde default for [`EnrichResult::law_complete`]: results stored before
/// chunking existed always covered the whole law.
fn default_law_complete() -> bool {
    true
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
    /// Git blob SHA of the base-branch law YAML this enrichment was generated
    /// from. Empty when unknown (files written before this field existed).
    #[serde(default)]
    pub source_hash: String,
    /// Chunked-enrichment cursor: index (document order) of the first article
    /// NOT yet processed by the chunk loop. 0 for legacy files (serde default)
    /// and whole-law runs; equal to `articles_total` once the loop finished.
    #[serde(default)]
    pub enrich_cursor: usize,
    /// The normalized law YAML path the cursor was recorded for. The cursor
    /// only applies when this matches the current target path — a new law
    /// version lives at a different path, which resets the cursor to 0.
    #[serde(default)]
    pub enrich_cursor_path: String,
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
    /// Max articles one enrich run may process (`ENRICH_MAX_ARTICLES_PER_RUN`).
    /// 0 disables chunking (whole-law sessions, the pre-chunking behavior).
    /// Default 15: a 600s session empirically enriches ~5–20 articles and the
    /// law-generate skill batches internally per ~15, so one chunk ≈ one
    /// skill batch.
    pub max_articles_per_run: usize,
    /// Pre-built provider configs keyed by name, populated at startup.
    provider_configs: std::collections::HashMap<String, LlmProvider>,
}

#[cfg(test)]
impl EnrichConfig {
    /// Build a config for tests (crate-internal), without reading the
    /// environment. Shared by the enrich and document-convert test suites.
    pub(crate) fn for_test(provider: LlmProvider) -> Self {
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
            // Chunking off by default in tests; chunk tests opt in explicitly.
            max_articles_per_run: 0,
            provider_configs,
        }
    }
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

        // Chunk size for large laws: max articles per enrich run. 0 disables
        // chunking entirely (whole-law sessions, the pre-chunking behavior).
        let max_articles_per_run = std::env::var("ENRICH_MAX_ARTICLES_PER_RUN")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(15);

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
            max_articles_per_run,
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
            max_articles_per_run: self.max_articles_per_run,
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

/// Build the prompt for one enrichment chunk: an explicit article subset.
///
/// Differences from [`build_prompt`] (which stays byte-identical for whole-law
/// runs): the agent must process ONLY the listed articles; the MvT-research
/// step is skipped on continuation chunks (`skip_mvt`, cursor > 0 — the
/// feature file already exists on the branch); reverse validation is limited
/// to the articles edited this session; and the agent must record a
/// `chunk_report` in `.enrichment-result.yaml` so a legitimately-empty chunk
/// (e.g. transitional law) still proves it was reviewed.
fn build_chunk_prompt(
    yaml_path: &str,
    progress_file_path: &str,
    article_numbers: &[String],
    skip_mvt: bool,
) -> String {
    let article_list = article_numbers.join(", ");
    let mvt_step = if skip_mvt {
        "## Step 1: MvT Research — SKIP\n\
         MvT research already ran during an earlier session for this law; its\n\
         feature file is already present. Do NOT redo it. Proceed to step 2."
            .to_string()
    } else {
        "## Step 1: MvT Research\n\
         Read .claude/skills/law-mvt-research/SKILL.md and follow its instructions to\n\
         search for Memorie van Toelichting documents and generate Gherkin test scenarios.\n\
         If no MvT documents are found, proceed to step 2 anyway."
            .to_string()
    };
    format!(
        r#"You are interpreting a Dutch law to make it machine-executable.

The law YAML file is: {yaml_path}

This is one chunk of a larger law. Process ONLY these articles (by their
`number` field) and leave every other article completely untouched:

{article_list}

Follow this pipeline in order. For each step, read the referenced skill file
and follow its instructions completely.

{mvt_step}

## Step 2: Generate machine_readable
Read .claude/skills/law-generate/SKILL.md and its reference.md and examples.md.
Follow the generate→validate→test loop to create machine_readable sections for
each executable article — restricted to the article subset listed above.

## Step 3: Reverse Validation
Read .claude/skills/law-reverse-validate/SKILL.md and follow its instructions
to verify every element in machine_readable traces back to the original legal
text — only for the articles you edited in this session.

## Step 4: Chunk report
Write (or update) the file `.enrichment-result.yaml` next to the law YAML with
a `chunk_report` mapping recording what you did in this session:

```yaml
chunk_report:
  articles_reviewed: ["<number>", ...]
  articles_skipped:
    - number: "<number>"
      reason: "<why no machine_readable, e.g. definition/transitional article>"
```

This report is REQUIRED even when no article in this chunk needed a
machine_readable section. Keep any existing `related_legislation` entries in
that file intact.

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
///
/// `file_arg` is passed to OpenCode as its `-f` input file (the Claude provider
/// ignores it and reads via its own tools from `cwd`). Enrich passes the law
/// YAML; a caller with no single input file passes `None`.
fn build_command(
    provider: &LlmProvider,
    prompt: &str,
    file_arg: Option<&Path>,
    cwd: &Path,
    allow_bash: bool,
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
            cmd.arg("run").arg(prompt);
            if let Some(f) = file_arg {
                cmd.arg("-f").arg(f);
            }
            cmd.arg("--format").arg("json").arg("--dir").arg(cwd);
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
            // `Bash` is only granted when the caller asks for it (document-convert
            // may need to run/install a converter); enrich keeps the shell off.
            let allowed_tools = if allow_bash {
                "Bash,Read,Edit,Write,Grep,Glob"
            } else {
                "Read,Edit,Write,Grep,Glob"
            };
            cmd.arg("-p")
                .arg(prompt)
                .arg("--allowedTools")
                .arg(allowed_tools)
                .current_dir(cwd);
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

/// Result of preparing the per-job enrichment checkout: the client plus the
/// base-branch blob SHA of the target law (recorded into `.enrichment.yaml`
/// as `source_hash`).
pub struct EnrichCorpus {
    pub client: CorpusClient,
    pub source_hash: String,
}

/// Read the `source_hash` recorded in the target law's `.enrichment.yaml`, if
/// present and non-empty. Returns `None` when the file is absent/unparseable
/// or the field is empty (both treated as "unknown provenance").
async fn read_stored_source_hash(repo_path: &Path, normalized_law_path: &str) -> Option<String> {
    #[derive(serde::Deserialize)]
    struct Provenance {
        #[serde(default)]
        source_hash: String,
    }
    let meta_rel = Path::new(normalized_law_path)
        .parent()?
        .join(".enrichment.yaml");
    let content = tokio::fs::read_to_string(repo_path.join(meta_rel))
        .await
        .ok()?;
    let prov: Provenance = serde_yaml_ng::from_str(&content).ok()?;
    (!prov.source_hash.is_empty()).then_some(prov.source_hash)
}

/// Read the chunked-enrichment cursor recorded in the target law's
/// `.enrichment.yaml`: `(enrich_cursor, enrich_cursor_path)`.
///
/// Absent file, unparseable YAML, or missing fields all degrade to `(0, "")` —
/// [`plan_chunk`] then resets to the start, which is the safe default for
/// legacy metadata written before the cursor existed.
async fn read_stored_cursor(repo_path: &Path, normalized_law_path: &str) -> (usize, String) {
    #[derive(serde::Deserialize, Default)]
    struct CursorFields {
        #[serde(default)]
        enrich_cursor: usize,
        #[serde(default)]
        enrich_cursor_path: String,
    }
    let Some(parent) = Path::new(normalized_law_path).parent() else {
        return (0, String::new());
    };
    let meta_rel = parent.join(".enrichment.yaml");
    let Ok(content) = tokio::fs::read_to_string(repo_path.join(meta_rel)).await else {
        return (0, String::new());
    };
    let fields: CursorFields = serde_yaml_ng::from_str(&content).unwrap_or_default();
    (fields.enrich_cursor, fields.enrich_cursor_path)
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
) -> Result<EnrichCorpus> {
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

    // Past `ensure_repo()` a per-job git clone exists on disk. Resolving the
    // base branch, checking freshness, or checking out the law can all fail —
    // including a deliberate `BaseDrift` bail-out. The worker's shared cleanup
    // only captures the checkout path on the success path, so on any error we
    // must remove the clone here; otherwise an errored (and especially a
    // drifted, awaiting-human) job leaks its clone on a disk/OOM-sensitive
    // worker.
    let checkout_dir = client.repo_path().to_path_buf();
    let source_hash = match resolve_enrich_base(&client, base_config, &normalized).await {
        Ok(source_hash) => source_hash,
        Err(e) => {
            if let Err(rm) = tokio::fs::remove_dir_all(&checkout_dir).await {
                tracing::warn!(
                    path = %checkout_dir.display(),
                    error = %rm,
                    "failed to clean up per-job corpus checkout after enrich setup error"
                );
            }
            return Err(e);
        }
    };

    Ok(EnrichCorpus {
        client,
        source_hash,
    })
}

/// Resolve the enrichment base branch and materialize the target law from it,
/// returning the base blob SHA to record as provenance (`source_hash`).
///
/// Split out from [`create_enrich_corpus`] so that every error it can raise — a
/// git probe failure, a checkout failure, or a [`PipelineError::BaseDrift`]
/// bail-out — flows through a single caller-side cleanup that removes the
/// per-job clone. Only `&self` methods are used; the clone already exists.
async fn resolve_enrich_base(
    client: &CorpusClient,
    base_config: &CorpusConfig,
    normalized: &str,
) -> Result<String> {
    // Prefer the worker's own base branch (e.g. `pr574`) so PR deployments
    // enrich their own harvested YAML, not production's. Probe the remote
    // first and fall back to `development` only when the branch doesn't
    // exist yet — which covers a fresh PR whose harvester hasn't pushed.
    // Probing explicitly (instead of try-then-fallback on any error)
    // prevents an unrelated `checkout` or `reset` failure from silently
    // dropping the enrichment back to production's branch.
    //
    // The freshness guard below works on the exact file path (not the
    // directory): `is_tracked` and `fetch_base_blob_sha` resolve a single blob,
    // so a newly harvested version of an already-known law is judged on its own
    // path rather than being masked by a sibling version in the same directory.
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

    // Freshness guard: compare the target law's base version against the
    // provenance recorded in a prior enrichment. New law -> check out fresh;
    // unchanged base -> keep existing enrichment; missing provenance (a legacy,
    // pre-guard enrichment) -> adopt the current base as baseline and proceed;
    // changed base -> fail loudly (do NOT auto-overwrite on a moved base).
    let base_sha = client.fetch_base_blob_sha(base_branch, normalized).await?;
    let tracked = client.is_tracked(normalized).await?;
    let stored = read_stored_source_hash(client.repo_path(), normalized).await;

    match decide_base_action(tracked, stored.as_deref(), &base_sha) {
        BaseAction::CheckoutFresh => {
            client.checkout_path_from_fetch_head(normalized).await?;
            tracing::info!(base = %base_branch, path = %normalized, "checked out law fresh from base");
        }
        BaseAction::Skip => {
            tracing::debug!(path = %normalized, "base unchanged, no fresh checkout needed");
        }
        BaseAction::AdoptBaseline => {
            // Legacy enrichment with no recorded provenance. Keep the existing
            // enrichment (no fresh checkout, like Skip); the current base blob
            // SHA returned below is stamped as `source_hash` on the next
            // `.enrichment.yaml` write, establishing the baseline so subsequent
            // runs can detect real drift.
            tracing::info!(
                path = %normalized,
                base = %base_branch,
                "no recorded provenance for tracked law; adopting current base as baseline (legacy enrichment grandfathered)"
            );
        }
        BaseAction::Drift => {
            return Err(PipelineError::BaseDrift {
                yaml_path: normalized.to_string(),
                base: base_branch.to_string(),
                expected: stored.unwrap_or_else(|| "(none recorded)".to_string()),
                actual: base_sha,
            });
        }
    }

    Ok(base_sha)
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

/// Error-message marker for a chunk that produced no reviewable output at all:
/// no new `machine_readable` sections, no `chunk_report` in the result
/// envelope, and no new untranslatables. This wording deliberately does NOT
/// contain any `is_deterministic_content_failure` marker ("no machine_readable
/// sections" / "yaml error"): the failure must stay retryable — a healthy law
/// whose chunk merely hiccupped must never be terminally exhausted in one
/// step. The worker's `chunk_no_output_is_not_deterministic` test pins this.
pub(crate) const CHUNK_NO_OUTPUT_MARKER: &str = "enrichment chunk produced no reviewable output";

/// Execute the enrichment using the default process-based LLM runner.
///
/// Convenience wrapper around `execute_enrich_with_runner` using `ProcessLlmRunner`.
pub async fn execute_enrich(
    payload: &EnrichPayload,
    repo_path: &Path,
    config: &EnrichConfig,
    source_hash: &str,
) -> Result<(EnrichResult, Vec<PathBuf>)> {
    execute_enrich_with_runner(payload, repo_path, config, source_hash, &ProcessLlmRunner).await
}

/// Execute the enrichment: call the LLM runner to generate machine_readable sections.
///
/// Returns the enrichment result and a list of files that were written
/// (for git staging). Accepts a `runner` to allow testing with a fake LLM.
pub async fn execute_enrich_with_runner(
    payload: &EnrichPayload,
    repo_path: &Path,
    config: &EnrichConfig,
    source_hash: &str,
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

    // Parse the law once for the pre-run stats, the chunk window's article
    // numbers, and the untranslatables baseline of the chunk no-op guard.
    let law = load_law(&yaml_abs).await?;
    let (articles_before, machine_readable_before) = article_stats(&law);

    // Chunk planning: the worker (not the LLM) owns the cursor, read from the
    // `.enrichment.yaml` already present on the enrich branch checkout.
    let (stored_cursor, stored_cursor_path) = read_stored_cursor(repo_path, &normalized_path).await;
    let plan = plan_chunk(
        config.max_articles_per_run,
        articles_before,
        stored_cursor,
        &stored_cursor_path,
        &normalized_path,
    );
    let (chunk_window, law_complete, next_cursor) = match plan {
        ChunkPlan::WholeLaw => (None, true, 0),
        ChunkPlan::Chunk {
            start,
            end,
            law_complete,
        } => {
            let numbers: Vec<String> = law.articles[start..end]
                .iter()
                .map(|a| a.number.clone())
                .collect();
            (Some((start, numbers)), law_complete, end)
        }
    };
    let untranslatables_before = collect_untranslatables_from(&law).len();

    let provider_name = config.provider.name().to_string();

    tracing::info!(
        law_id = %payload.law_id,
        yaml_path = %payload.yaml_path,
        provider = %provider_name,
        articles = articles_before,
        already_enriched = machine_readable_before,
        chunk = ?chunk_window.as_ref().map(|(start, numbers)| (*start, numbers.len())),
        "starting enrichment"
    );

    // An empty window (valid cursor already at the end of the document) means
    // the chunk loop finished this law earlier: nothing to process, no LLM run
    // — complete trivially instead of prompting an agent with zero articles.
    let empty_window = matches!(&chunk_window, Some((_, numbers)) if numbers.is_empty());
    if empty_window {
        tracing::info!(
            law_id = %payload.law_id,
            cursor = stored_cursor,
            "chunk cursor already at end of document; completing without an LLM run"
        );
    } else {
        // A previous chunk's committed envelope must not serve as
        // proof-of-review for this window (see clear_stale_chunk_report).
        if chunk_window.is_some() {
            clear_stale_chunk_report(&yaml_abs).await;
        }
        let normalized_payload = EnrichPayload {
            yaml_path: normalized_path.clone(),
            chunk_articles: chunk_window.as_ref().map(|(_, numbers)| numbers.clone()),
            // MvT research runs once, during the first chunk (cursor 0).
            skip_mvt: chunk_window.as_ref().map(|(start, _)| *start > 0),
            ..payload.clone()
        };
        runner
            .run(&normalized_payload, &yaml_abs, repo_path, config)
            .await?;

        tracing::info!(law_id = %payload.law_id, provider = %provider_name, "enrichment completed");
    }

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

    // Read the result envelope the agent may have written: related legislation
    // plus (chunked) the chunk_report used by the no-op guard below. Never
    // fails: absent/malformed → default (see read_enrichment_result_envelope).
    let envelope = read_enrichment_result_envelope(&yaml_abs).await;

    // Capture the untranslatables the agent flagged in the enriched YAML (RFC-012).
    let untranslatables = collect_untranslatables(&yaml_abs).await?;

    match &chunk_window {
        // Whole-law mode: if the LLM ran successfully but didn't enrich any
        // articles, treat it as an error so the job gets retried or marked as
        // failed instead of silently committing a zero-coverage result.
        // Unchanged from the pre-chunking behavior (and deliberately matched
        // by `is_deterministic_content_failure` in the worker).
        None => {
            if articles_needing_enrichment > 0 && newly_enriched == 0 {
                return Err(PipelineError::Enrich(format!(
                    "LLM produced no machine_readable sections ({articles_needing_enrichment} articles needed enrichment)"
                )));
            }
        }
        // Chunked mode: a window may legitimately yield zero new
        // machine_readable sections (definition/transitional chapters) — but
        // only when the agent proves it reviewed the window (chunk_report) or
        // produced new untranslatables. No output at all fails with a message
        // that deliberately does NOT match `is_deterministic_content_failure`:
        // the failure stays retryable and can never terminally exhaust a
        // healthy law. The empty window skipped the LLM, so it is exempt.
        Some((start, numbers)) if !empty_window => {
            let has_new_untranslatables = untranslatables.len() > untranslatables_before;
            // The chunk_report only counts as proof-of-review when it names at
            // least one article of THIS window: a bare `chunk_report: {}` or
            // one listing unrelated numbers must not advance the cursor past
            // an unreviewed window. Full window coverage is deliberately NOT
            // required — exact-match strictness against agent-written numbers
            // could retry-loop a healthy chunk toward exhaustion.
            let report_references_window = envelope.chunk_report.as_ref().is_some_and(|report| {
                report
                    .articles_reviewed
                    .iter()
                    .chain(report.articles_skipped.iter().map(|s| &s.number))
                    .any(|n| numbers.contains(n))
            });
            if newly_enriched == 0 && !report_references_window && !has_new_untranslatables {
                return Err(PipelineError::Enrich(format!(
                    "{CHUNK_NO_OUTPUT_MARKER}: no new machine_readable additions, no chunk_report \
                     referencing this window, no new untranslatables (articles {}..{} of {})",
                    start,
                    start + numbers.len(),
                    articles_before
                )));
            }
        }
        Some(_) => {}
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
        source_hash: source_hash.to_string(),
        enrich_cursor: next_cursor,
        enrich_cursor_path: normalized_path.clone(),
    };

    let metadata_path = yaml_abs
        .parent()
        .unwrap_or(Path::new("."))
        .join(".enrichment.yaml");
    let metadata_yaml = serde_yaml_ng::to_string(&metadata)
        .map_err(|e| PipelineError::Enrich(format!("failed to serialize metadata: {e}")))?;
    tokio::fs::write(&metadata_path, &metadata_yaml).await?;

    let related_legislation = envelope.related_legislation;

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
        law_complete,
        enrich_cursor: next_cursor,
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
    let law = load_law(path).await?;
    Ok(article_stats(&law))
}

/// Parse a law YAML file into the canonical [`ArticleBasedLaw`] model.
async fn load_law(path: &Path) -> Result<ArticleBasedLaw> {
    let content = tokio::fs::read_to_string(path).await?;
    Ok(serde_yaml_ng::from_str(&content)?)
}

/// `(articles_total, articles_with_machine_readable)` for a parsed law.
fn article_stats(law: &ArticleBasedLaw) -> (usize, usize) {
    let total = law.articles.len();
    let with_machine_readable = law
        .articles
        .iter()
        .filter(|article| article.machine_readable.is_some())
        .count();
    (total, with_machine_readable)
}

/// Collect all untranslatables from an enriched law YAML, flattened to
/// [`CapturedUntranslatable`] with the owning article number attached.
///
/// Parses the law into the canonical [`ArticleBasedLaw`] model, mirroring
/// [`count_article_stats`]. Returns an empty vec when no article declares any.
async fn collect_untranslatables(path: &Path) -> Result<Vec<CapturedUntranslatable>> {
    let law = load_law(path).await?;
    Ok(collect_untranslatables_from(&law))
}

/// Flatten the untranslatables of an already-parsed law. See
/// [`collect_untranslatables`].
fn collect_untranslatables_from(law: &ArticleBasedLaw) -> Vec<CapturedUntranslatable> {
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
    out
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
    fn decide_base_action_new_law_checks_out_fresh() {
        assert_eq!(
            decide_base_action(false, None, "sha_new"),
            BaseAction::CheckoutFresh
        );
        // Even if a stored hash somehow exists, an untracked path is a fresh checkout.
        assert_eq!(
            decide_base_action(false, Some("sha_old"), "sha_new"),
            BaseAction::CheckoutFresh
        );
    }

    #[test]
    fn decide_base_action_unchanged_base_skips() {
        assert_eq!(
            decide_base_action(true, Some("sha"), "sha"),
            BaseAction::Skip
        );
    }

    #[test]
    fn decide_base_action_changed_base_is_drift() {
        assert_eq!(
            decide_base_action(true, Some("sha_old"), "sha_new"),
            BaseAction::Drift
        );
    }

    #[test]
    fn decide_base_action_missing_or_empty_provenance_adopts_baseline() {
        // A tracked law with no recorded provenance is a pre-guard "legacy"
        // enrichment: grandfather it by adopting the current base as baseline,
        // never a terminal drift (that would fail every existing enrichment the
        // moment the guard ships).
        assert_eq!(
            decide_base_action(true, None, "sha_new"),
            BaseAction::AdoptBaseline
        );
        assert_eq!(
            decide_base_action(true, Some(""), "sha_new"),
            BaseAction::AdoptBaseline
        );
    }

    #[test]
    fn test_enrich_payload_serde_roundtrip() {
        let payload = EnrichPayload {
            law_id: "BWBR0018451".to_string(),
            yaml_path: "regulation/nl/wet/wet_op_de_zorgtoeslag/2025-01-01.yaml".to_string(),
            provider: Some("claude".to_string()),
            depth: Some(2),
            requested_by: None,
            deliver: None,
            traject_id: None,
            traject_ref: None,
            source_etag: None,
            new_law: None,
            chunk_articles: None,
            skip_mvt: None,
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
            requested_by: None,
            deliver: None,
            traject_id: None,
            traject_ref: None,
            source_etag: None,
            new_law: None,
            chunk_articles: None,
            skip_mvt: None,
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
            law_complete: true,
            enrich_cursor: 0,
        };

        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["articles_with_machine_readable"], 7);
        assert_eq!(json["coverage_score"], 0.7);
        assert_eq!(json["provider"], "opencode");
        assert_eq!(json["branch"], "enrich/opencode");
        assert_eq!(json["law_complete"], true);
        assert_eq!(json["enrich_cursor"], 0);
    }

    #[test]
    fn enrich_result_law_complete_defaults_true_for_legacy_json() {
        // `jobs.result` rows written before chunking existed lack both fields;
        // they always covered the whole law, so they must deserialize as
        // complete with cursor 0.
        let legacy = serde_json::json!({
            "law_id": "BWBR0018451",
            "yaml_path": "regulation/nl/wet/wet_op_de_zorgtoeslag/2025-01-01.yaml",
            "articles_total": 10,
            "articles_with_machine_readable": 7,
            "coverage_score": 0.7,
            "provider": "opencode",
            "branch": "enrich/opencode",
        });
        let result: EnrichResult = serde_json::from_value(legacy).unwrap();
        assert!(result.law_complete);
        assert_eq!(result.enrich_cursor, 0);
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
        let envelope = read_enrichment_result_envelope(&yaml_abs).await;
        assert!(envelope.related_legislation.is_empty());
        assert!(envelope.chunk_report.is_none());
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
        let envelope = read_enrichment_result_envelope(&yaml_abs).await;
        assert!(envelope.related_legislation.is_empty());
        assert!(envelope.chunk_report.is_none());
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
        let related = read_enrichment_result_envelope(&yaml_abs)
            .await
            .related_legislation;
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
        EnrichConfig::for_test(provider)
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
    fn test_enrich_payload_task_fields_roundtrip_and_backcompat() {
        // Oude payloads (zonder taak-velden) moeten blijven deserialiseren.
        let old = serde_json::json!({"law_id": "x", "yaml_path": "nl/x/2025-01-01.yaml"});
        let parsed: EnrichPayload = serde_json::from_value(old).unwrap();
        assert!(parsed.requested_by.is_none());
        assert!(!parsed.deliver_as_task());

        // Nieuwe payloads dragen de taak-velden mee.
        let account = uuid::Uuid::new_v4();
        let new = EnrichPayload {
            law_id: "x".into(),
            yaml_path: "laws/x/law.yaml".into(),
            provider: Some("claude".into()),
            depth: None,
            requested_by: Some(account),
            deliver: Some("task".into()),
            traject_id: Some(uuid::Uuid::new_v4()),
            traject_ref: Some("testtraject-abcd1234".into()),
            source_etag: Some("\"abc\"".into()),
            new_law: None,
            chunk_articles: None,
            skip_mvt: None,
        };
        let roundtrip: EnrichPayload =
            serde_json::from_value(serde_json::to_value(&new).unwrap()).unwrap();
        assert_eq!(roundtrip.requested_by, Some(account));
        assert!(roundtrip.deliver_as_task());
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
            source_hash: String::new(),
            enrich_cursor: 0,
            enrich_cursor_path: String::new(),
        };

        let yaml = serde_yaml_ng::to_string(&meta).unwrap();
        assert!(yaml.contains("law_id: BWBR0018451"));
        assert!(yaml.contains("provider: opencode"));

        let deserialized: EnrichmentMetadata = serde_yaml_ng::from_str(&yaml).unwrap();
        assert_eq!(deserialized.articles_with_machine_readable, 7);
    }

    #[test]
    fn enrichment_metadata_source_hash_defaults_when_absent() {
        // A .enrichment.yaml written before this field existed.
        let legacy = "law_id: BWBR0001\ntimestamp: '2026-01-01T00:00:00Z'\nprovider: claude\nmodel: m\nprompt_hash: p\ncode_commit: c\ncoverage_score: 1.0\narticles_total: 1\narticles_with_machine_readable: 1\n";
        let meta: EnrichmentMetadata = serde_yaml_ng::from_str(legacy).unwrap();
        assert_eq!(meta.source_hash, "");
        // Cursor fields default too (files written before chunking existed).
        assert_eq!(meta.enrich_cursor, 0);
        assert_eq!(meta.enrich_cursor_path, "");
    }

    #[test]
    fn enrichment_metadata_source_hash_roundtrips() {
        let meta = EnrichmentMetadata {
            law_id: "BWBR0001".into(),
            timestamp: "2026-01-01T00:00:00Z".into(),
            provider: "claude".into(),
            model: "m".into(),
            prompt_hash: "p".into(),
            code_commit: "c".into(),
            coverage_score: 1.0,
            articles_total: 1,
            articles_with_machine_readable: 1,
            source_hash: "abc123".into(),
            enrich_cursor: 30,
            enrich_cursor_path: "regulation/nl/wet/x/2026-01-01.yaml".into(),
        };
        let yaml = serde_yaml_ng::to_string(&meta).unwrap();
        let back: EnrichmentMetadata = serde_yaml_ng::from_str(&yaml).unwrap();
        assert_eq!(back.source_hash, "abc123");
        assert_eq!(back.enrich_cursor, 30);
        assert_eq!(
            back.enrich_cursor_path,
            "regulation/nl/wet/x/2026-01-01.yaml"
        );
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
            requested_by: None,
            deliver: None,
            traject_id: None,
            traject_ref: None,
            source_etag: None,
            new_law: None,
            chunk_articles: None,
            skip_mvt: None,
        };

        let config = test_config(LlmProvider::OpenCode {
            path: "fake".into(),
            model: None,
        });

        let (result, written_files) =
            execute_enrich_with_runner(&payload, dir.path(), &config, "", &FakeLlmRunner)
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
            requested_by: None,
            deliver: None,
            traject_id: None,
            traject_ref: None,
            source_etag: None,
            new_law: None,
            chunk_articles: None,
            skip_mvt: None,
        };

        let config = test_config(LlmProvider::OpenCode {
            path: "fake".into(),
            model: None,
        });

        let err = execute_enrich_with_runner(&payload, dir.path(), &config, "", &FailingLlmRunner)
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
            requested_by: None,
            deliver: None,
            traject_id: None,
            traject_ref: None,
            source_etag: None,
            new_law: None,
            chunk_articles: None,
            skip_mvt: None,
        };

        let config = test_config(LlmProvider::OpenCode {
            path: "fake".into(),
            model: None,
        });

        let err = execute_enrich_with_runner(&payload, dir.path(), &config, "", &NoopLlmRunner)
            .await
            .unwrap_err();

        assert!(err.to_string().contains("no machine_readable sections"));
    }

    // --- Chunked enrichment ---

    #[test]
    fn plan_chunk_zero_disables_chunking() {
        assert_eq!(
            plan_chunk(0, 324, 0, "regulation/a.yaml", "regulation/a.yaml"),
            ChunkPlan::WholeLaw
        );
        // Even a stored cursor is ignored in whole-law mode.
        assert_eq!(
            plan_chunk(0, 324, 100, "regulation/a.yaml", "regulation/a.yaml"),
            ChunkPlan::WholeLaw
        );
    }

    #[test]
    fn plan_chunk_first_run_starts_at_zero() {
        // Legacy metadata (no cursor fields) reads as (0, "") — path mismatch
        // resets to 0, which is also the correct start.
        assert_eq!(
            plan_chunk(15, 324, 0, "", "regulation/a.yaml"),
            ChunkPlan::Chunk {
                start: 0,
                end: 15,
                law_complete: false
            }
        );
    }

    #[test]
    fn plan_chunk_resumes_from_valid_cursor() {
        assert_eq!(
            plan_chunk(15, 324, 30, "regulation/a.yaml", "regulation/a.yaml"),
            ChunkPlan::Chunk {
                start: 30,
                end: 45,
                law_complete: false
            }
        );
    }

    #[test]
    fn plan_chunk_resets_on_path_mismatch() {
        // A new law version lives at a different (date-named) path: the cursor
        // recorded for the old version must not apply.
        assert_eq!(
            plan_chunk(
                15,
                324,
                30,
                "regulation/2025-01-01.yaml",
                "regulation/2026-01-01.yaml"
            ),
            ChunkPlan::Chunk {
                start: 0,
                end: 15,
                law_complete: false
            }
        );
    }

    #[test]
    fn plan_chunk_resets_on_cursor_beyond_total() {
        // Corrupt metadata or a shrunk document: a cursor past the end resets.
        assert_eq!(
            plan_chunk(15, 20, 25, "regulation/a.yaml", "regulation/a.yaml"),
            ChunkPlan::Chunk {
                start: 0,
                end: 15,
                law_complete: false
            }
        );
    }

    #[test]
    fn plan_chunk_final_window_is_complete() {
        // Partial last window.
        assert_eq!(
            plan_chunk(15, 20, 15, "regulation/a.yaml", "regulation/a.yaml"),
            ChunkPlan::Chunk {
                start: 15,
                end: 20,
                law_complete: true
            }
        );
        // Window exactly reaching the end.
        assert_eq!(
            plan_chunk(10, 20, 10, "regulation/a.yaml", "regulation/a.yaml"),
            ChunkPlan::Chunk {
                start: 10,
                end: 20,
                law_complete: true
            }
        );
        // Law smaller than one window: complete in a single run.
        assert_eq!(
            plan_chunk(15, 3, 0, "", "regulation/a.yaml"),
            ChunkPlan::Chunk {
                start: 0,
                end: 3,
                law_complete: true
            }
        );
    }

    #[test]
    fn plan_chunk_cursor_at_end_yields_empty_complete_window() {
        // cursor == total is valid (the loop finished earlier): empty window,
        // trivially complete — execute skips the LLM run entirely.
        assert_eq!(
            plan_chunk(15, 20, 20, "regulation/a.yaml", "regulation/a.yaml"),
            ChunkPlan::Chunk {
                start: 20,
                end: 20,
                law_complete: true
            }
        );
    }

    #[test]
    fn plan_chunk_empty_law_is_complete() {
        assert_eq!(
            plan_chunk(15, 0, 0, "", "regulation/a.yaml"),
            ChunkPlan::Chunk {
                start: 0,
                end: 0,
                law_complete: true
            }
        );
    }

    #[test]
    fn test_build_prompt_is_byte_stable() {
        // The whole-law prompt must stay byte-identical to the pre-chunking
        // prompt: chunking must not change what an N=0 (or legacy) run sends
        // to the LLM. Pin the exact text — an intentional prompt change must
        // update this fixture consciously.
        let expected = r#"You are interpreting a Dutch law to make it machine-executable.

The law YAML file is: regulation/nl/wet/test/2025-01-01.yaml

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
Write to: /tmp/repo/.enrichment-progress.json

Write this file at these moments:
- Before Step 1: {"phase": "mvt_research", "step": 1, "total_steps": 3}
- Before Step 2: {"phase": "generating", "step": 2, "total_steps": 3, "article_count": N}
- After validation in Step 2: {"phase": "validating", "step": 2, "total_steps": 3, "iteration": M}
- Before Step 3: {"phase": "reverse_validating", "step": 3, "total_steps": 3}

Use the Write tool. Keep it brief — just one write per phase transition."#;
        assert_eq!(
            build_prompt(
                "regulation/nl/wet/test/2025-01-01.yaml",
                "/tmp/repo/.enrichment-progress.json"
            ),
            expected
        );
    }

    #[test]
    fn test_build_chunk_prompt_first_chunk_includes_mvt() {
        let numbers = vec!["1".to_string(), "2".to_string(), "3a".to_string()];
        let prompt = build_chunk_prompt(
            "regulation/nl/wet/test/2025-01-01.yaml",
            "/tmp/repo/.enrichment-progress.json",
            &numbers,
            false,
        );
        assert!(prompt.contains("Process ONLY these articles"));
        assert!(prompt.contains("1, 2, 3a"));
        assert!(prompt.contains("law-mvt-research/SKILL.md"));
        assert!(!prompt.contains("SKIP"));
        assert!(prompt.contains("chunk_report"));
        assert!(prompt.contains("articles_skipped"));
        assert!(prompt.contains("law-generate/SKILL.md"));
        assert!(prompt.contains("law-reverse-validate/SKILL.md"));
    }

    #[test]
    fn test_build_chunk_prompt_continuation_skips_mvt() {
        let numbers = vec!["16".to_string(), "17".to_string()];
        let prompt = build_chunk_prompt(
            "regulation/nl/wet/test/2025-01-01.yaml",
            "/tmp/repo/.enrichment-progress.json",
            &numbers,
            true,
        );
        assert!(prompt.contains("MvT Research — SKIP"));
        assert!(!prompt.contains("law-mvt-research/SKILL.md"));
        assert!(prompt.contains("16, 17"));
    }

    #[test]
    fn enrich_payload_chunk_fields_are_transport_only() {
        // Queue payloads never carry the chunk fields: absent when None…
        let bare = EnrichPayload {
            law_id: "x".into(),
            yaml_path: "regulation/a.yaml".into(),
            provider: None,
            depth: None,
            requested_by: None,
            deliver: None,
            traject_id: None,
            traject_ref: None,
            source_etag: None,
            new_law: None,
            chunk_articles: None,
            skip_mvt: None,
        };
        let json = serde_json::to_string(&bare).unwrap();
        assert!(!json.contains("chunk_articles"));
        assert!(!json.contains("skip_mvt"));
        // …and old payload JSON without them still deserializes.
        let old = serde_json::json!({"law_id": "x", "yaml_path": "regulation/a.yaml"});
        let parsed: EnrichPayload = serde_json::from_value(old).unwrap();
        assert!(parsed.chunk_articles.is_none());
        assert!(parsed.skip_mvt.is_none());
    }

    #[test]
    fn test_envelope_chunk_report_roundtrip_and_backcompat() {
        // Old envelopes (without chunk_report) keep parsing.
        let old = "related_legislation:\n  - name: Some Law\n";
        let envelope: EnrichmentResultEnvelope = serde_yaml_ng::from_str(old).unwrap();
        assert!(envelope.chunk_report.is_none());

        let yaml = r#"
related_legislation:
  - name: Some Law
chunk_report:
  articles_reviewed: ["1", "2", "3"]
  articles_skipped:
    - number: "2"
      reason: definition article
"#;
        let envelope: EnrichmentResultEnvelope = serde_yaml_ng::from_str(yaml).unwrap();
        let report = envelope.chunk_report.expect("chunk_report parses");
        assert_eq!(report.articles_reviewed, vec!["1", "2", "3"]);
        assert_eq!(report.articles_skipped.len(), 1);
        assert_eq!(report.articles_skipped[0].number, "2");
        assert_eq!(report.articles_skipped[0].reason, "definition article");
    }

    /// Chunk-aware fake runner: adds `machine_readable` ONLY to the articles
    /// listed in `payload.chunk_articles` and records every invocation's
    /// window + skip_mvt so the loop contract can be asserted.
    struct FakeChunkRunner {
        calls: std::sync::Mutex<Vec<(Vec<String>, Option<bool>)>>,
        /// Also write a `chunk_report` envelope next to the YAML.
        write_report: bool,
    }

    impl FakeChunkRunner {
        fn new(write_report: bool) -> Self {
            Self {
                calls: std::sync::Mutex::new(Vec::new()),
                write_report,
            }
        }
    }

    #[async_trait::async_trait]
    impl LlmRunner for FakeChunkRunner {
        async fn run(
            &self,
            payload: &EnrichPayload,
            yaml_abs: &Path,
            _repo_path: &Path,
            _config: &EnrichConfig,
        ) -> Result<()> {
            let chunk = payload
                .chunk_articles
                .clone()
                .expect("chunked run must pass chunk_articles");
            self.calls
                .lock()
                .unwrap()
                .push((chunk.clone(), payload.skip_mvt));

            let content = tokio::fs::read_to_string(yaml_abs).await?;
            let mut value: serde_yaml_ng::Value = serde_yaml_ng::from_str(&content)?;
            if let serde_yaml_ng::Value::Mapping(ref mut map) = value {
                if let Some(serde_yaml_ng::Value::Sequence(ref mut articles)) =
                    map.get_mut("articles")
                {
                    for article in articles.iter_mut() {
                        if let serde_yaml_ng::Value::Mapping(ref mut article_map) = article {
                            let number = article_map
                                .get("number")
                                .and_then(|n| n.as_str())
                                .unwrap_or_default()
                                .to_string();
                            if chunk.contains(&number)
                                && !article_map.contains_key("machine_readable")
                            {
                                article_map.insert(
                                    serde_yaml_ng::Value::String("machine_readable".into()),
                                    serde_yaml_ng::Value::Mapping(Default::default()),
                                );
                            }
                        }
                    }
                }
            }
            tokio::fs::write(yaml_abs, serde_yaml_ng::to_string(&value)?).await?;

            if self.write_report {
                let report = format!(
                    "chunk_report:\n  articles_reviewed: [{}]\n",
                    chunk
                        .iter()
                        .map(|n| format!("\"{n}\""))
                        .collect::<Vec<_>>()
                        .join(", ")
                );
                tokio::fs::write(enrichment_result_path(yaml_abs), report).await?;
            }
            Ok(())
        }
    }

    fn four_article_law() -> &'static str {
        r#"---
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
  - number: '4'
    text: Article four.
"#
    }

    fn chunk_test_payload(yaml_path: &str) -> EnrichPayload {
        EnrichPayload {
            law_id: "BWBR0000001".into(),
            yaml_path: yaml_path.into(),
            provider: Some("opencode".into()),
            depth: None,
            requested_by: None,
            deliver: None,
            traject_id: None,
            traject_ref: None,
            source_etag: None,
            new_law: None,
            chunk_articles: None,
            skip_mvt: None,
        }
    }

    #[tokio::test]
    async fn test_execute_enrich_chunked_loop_terminates() {
        // 4 articles, 2 per run: the loop must finish in exactly 2 runs, the
        // cursor persisting via .enrichment.yaml between them.
        let dir = tempfile::tempdir().unwrap();
        let law_dir = dir.path().join("regulation/nl/wet/test_law");
        tokio::fs::create_dir_all(&law_dir).await.unwrap();
        let yaml_path = "regulation/nl/wet/test_law/2025-01-01.yaml";
        tokio::fs::write(dir.path().join(yaml_path), four_article_law())
            .await
            .unwrap();

        let mut config = test_config(LlmProvider::OpenCode {
            path: "fake".into(),
            model: None,
        });
        config.max_articles_per_run = 2;
        let payload = chunk_test_payload(yaml_path);
        let runner = FakeChunkRunner::new(false);

        // Run 1: articles 1-2, MvT research included, law not complete.
        let (result, _) =
            execute_enrich_with_runner(&payload, dir.path(), &config, "sha1", &runner)
                .await
                .unwrap();
        assert!(!result.law_complete);
        assert_eq!(result.enrich_cursor, 2);
        assert_eq!(result.articles_with_machine_readable, 2);
        assert!((result.coverage_score - 0.5).abs() < f64::EPSILON);

        // Cursor persisted on disk for the continuation run.
        let meta: EnrichmentMetadata = serde_yaml_ng::from_str(
            &tokio::fs::read_to_string(law_dir.join(".enrichment.yaml"))
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(meta.enrich_cursor, 2);
        assert_eq!(meta.enrich_cursor_path, yaml_path);

        // Run 2: articles 3-4, MvT research skipped, law complete.
        let (result, _) =
            execute_enrich_with_runner(&payload, dir.path(), &config, "sha1", &runner)
                .await
                .unwrap();
        assert!(result.law_complete);
        assert_eq!(result.enrich_cursor, 4);
        assert_eq!(result.articles_with_machine_readable, 4);

        let calls = runner.calls.lock().unwrap();
        assert_eq!(
            *calls,
            vec![
                (vec!["1".to_string(), "2".to_string()], Some(false)),
                (vec!["3".to_string(), "4".to_string()], Some(true)),
            ]
        );
    }

    #[tokio::test]
    async fn test_execute_enrich_chunk_noop_with_report_succeeds() {
        // A chunk that adds no machine_readable but writes a chunk_report is
        // legitimate progress: the cursor advances and the run succeeds.
        let dir = tempfile::tempdir().unwrap();
        let law_dir = dir.path().join("regulation/nl/wet/test_law");
        tokio::fs::create_dir_all(&law_dir).await.unwrap();
        let yaml_path = "regulation/nl/wet/test_law/2025-01-01.yaml";
        tokio::fs::write(dir.path().join(yaml_path), four_article_law())
            .await
            .unwrap();

        /// Writes only a chunk_report — no machine_readable at all.
        struct ReportOnlyRunner;
        #[async_trait::async_trait]
        impl LlmRunner for ReportOnlyRunner {
            async fn run(
                &self,
                _payload: &EnrichPayload,
                yaml_abs: &Path,
                _repo_path: &Path,
                _config: &EnrichConfig,
            ) -> Result<()> {
                tokio::fs::write(
                    enrichment_result_path(yaml_abs),
                    "chunk_report:\n  articles_reviewed: [\"1\", \"2\"]\n  articles_skipped:\n    - number: \"1\"\n      reason: transitional law\n",
                )
                .await?;
                Ok(())
            }
        }

        let mut config = test_config(LlmProvider::OpenCode {
            path: "fake".into(),
            model: None,
        });
        config.max_articles_per_run = 2;

        let (result, _) = execute_enrich_with_runner(
            &chunk_test_payload(yaml_path),
            dir.path(),
            &config,
            "",
            &ReportOnlyRunner,
        )
        .await
        .unwrap();
        assert!(!result.law_complete);
        assert_eq!(result.enrich_cursor, 2);
        assert_eq!(result.articles_with_machine_readable, 0);
    }

    #[tokio::test]
    async fn test_execute_enrich_chunk_noop_without_report_fails_retryable() {
        // No machine_readable, no chunk_report, no untranslatables: the chunk
        // fails — with a message that must NOT be classified as a
        // deterministic content failure (the worker pins that classification).
        let dir = tempfile::tempdir().unwrap();
        let law_dir = dir.path().join("regulation/nl/wet/test_law");
        tokio::fs::create_dir_all(&law_dir).await.unwrap();
        let yaml_path = "regulation/nl/wet/test_law/2025-01-01.yaml";
        tokio::fs::write(dir.path().join(yaml_path), four_article_law())
            .await
            .unwrap();

        let mut config = test_config(LlmProvider::OpenCode {
            path: "fake".into(),
            model: None,
        });
        config.max_articles_per_run = 2;

        let err = execute_enrich_with_runner(
            &chunk_test_payload(yaml_path),
            dir.path(),
            &config,
            "",
            &NoopLlmRunner,
        )
        .await
        .unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains(CHUNK_NO_OUTPUT_MARKER), "got: {msg}");
        assert!(!msg.contains("no machine_readable sections"), "got: {msg}");
    }

    #[tokio::test]
    async fn test_execute_enrich_chunk_empty_or_unrelated_report_is_no_proof() {
        // A bare `chunk_report: {}` — or one naming only articles outside the
        // window — must not count as proof-of-review: presence alone would let
        // a do-nothing run advance the cursor past an unreviewed window (and
        // eventually mark the law enriched with silent gaps).
        /// Writes a fixed chunk_report body, nothing else.
        struct FixedReportRunner(&'static str);
        #[async_trait::async_trait]
        impl LlmRunner for FixedReportRunner {
            async fn run(
                &self,
                _payload: &EnrichPayload,
                yaml_abs: &Path,
                _repo_path: &Path,
                _config: &EnrichConfig,
            ) -> Result<()> {
                tokio::fs::write(enrichment_result_path(yaml_abs), self.0).await?;
                Ok(())
            }
        }

        for report in [
            "chunk_report: {}\n",
            // Articles 3-4 are outside the first window (articles 1-2).
            "chunk_report:\n  articles_reviewed: [\"3\", \"4\"]\n",
        ] {
            let dir = tempfile::tempdir().unwrap();
            let law_dir = dir.path().join("regulation/nl/wet/test_law");
            tokio::fs::create_dir_all(&law_dir).await.unwrap();
            let yaml_path = "regulation/nl/wet/test_law/2025-01-01.yaml";
            tokio::fs::write(dir.path().join(yaml_path), four_article_law())
                .await
                .unwrap();

            let mut config = test_config(LlmProvider::OpenCode {
                path: "fake".into(),
                model: None,
            });
            config.max_articles_per_run = 2;

            let err = execute_enrich_with_runner(
                &chunk_test_payload(yaml_path),
                dir.path(),
                &config,
                "",
                &FixedReportRunner(report),
            )
            .await
            .unwrap_err();
            assert!(
                err.to_string().contains(CHUNK_NO_OUTPUT_MARKER),
                "report {report:?} must not count as proof: {err}"
            );
        }
    }

    #[tokio::test]
    async fn test_execute_enrich_chunk_stale_report_is_no_proof_of_review() {
        // The envelope is committed to the enrich branch as provenance, so a
        // continuation chunk's checkout still contains the PREVIOUS chunk's
        // chunk_report. A run that produces nothing must not pass the no-op
        // guard on that stale report — the worker strips it pre-run, keeping
        // the rest of the envelope (related_legislation) intact.
        let dir = tempfile::tempdir().unwrap();
        let law_dir = dir.path().join("regulation/nl/wet/test_law");
        tokio::fs::create_dir_all(&law_dir).await.unwrap();
        let yaml_path = "regulation/nl/wet/test_law/2025-01-01.yaml";
        tokio::fs::write(dir.path().join(yaml_path), four_article_law())
            .await
            .unwrap();
        // Cursor after chunk 1 (articles 1-2) …
        tokio::fs::write(
            law_dir.join(".enrichment.yaml"),
            format!(
                "law_id: BWBR0000001\ntimestamp: '2026-01-01T00:00:00Z'\nprovider: opencode\nmodel: m\nprompt_hash: p\ncode_commit: c\ncoverage_score: 1.0\narticles_total: 4\narticles_with_machine_readable: 0\nenrich_cursor: 2\nenrich_cursor_path: {yaml_path}\n"
            ),
        )
        .await
        .unwrap();
        // …and chunk 1's committed envelope, report included.
        tokio::fs::write(
            law_dir.join(".enrichment-result.yaml"),
            "related_legislation:\n  - name: Some Law\n    bwb_id: BWBR0037841\nchunk_report:\n  articles_reviewed: [\"1\", \"2\"]\n",
        )
        .await
        .unwrap();

        let mut config = test_config(LlmProvider::OpenCode {
            path: "fake".into(),
            model: None,
        });
        config.max_articles_per_run = 2;

        let err = execute_enrich_with_runner(
            &chunk_test_payload(yaml_path),
            dir.path(),
            &config,
            "",
            &NoopLlmRunner,
        )
        .await
        .unwrap_err();
        assert!(
            err.to_string().contains(CHUNK_NO_OUTPUT_MARKER),
            "stale chunk_report must not count as this session's proof: {err}"
        );

        // The stale report was stripped; the rest of the envelope survived.
        let envelope = read_enrichment_result_envelope(&dir.path().join(yaml_path)).await;
        assert!(envelope.chunk_report.is_none());
        assert_eq!(envelope.related_legislation.len(), 1);
        assert_eq!(
            envelope.related_legislation[0].bwb_id.as_deref(),
            Some("BWBR0037841")
        );
    }

    #[tokio::test]
    async fn test_execute_enrich_chunk_cursor_at_end_completes_without_llm() {
        // A valid cursor at the end of the document (loop already finished)
        // completes trivially: no LLM invocation, law_complete = true.
        let dir = tempfile::tempdir().unwrap();
        let law_dir = dir.path().join("regulation/nl/wet/test_law");
        tokio::fs::create_dir_all(&law_dir).await.unwrap();
        let yaml_path = "regulation/nl/wet/test_law/2025-01-01.yaml";
        tokio::fs::write(dir.path().join(yaml_path), four_article_law())
            .await
            .unwrap();
        // Pre-existing metadata with the cursor at the end.
        tokio::fs::write(
            law_dir.join(".enrichment.yaml"),
            format!(
                "law_id: BWBR0000001\ntimestamp: '2026-01-01T00:00:00Z'\nprovider: opencode\nmodel: m\nprompt_hash: p\ncode_commit: c\ncoverage_score: 1.0\narticles_total: 4\narticles_with_machine_readable: 4\nenrich_cursor: 4\nenrich_cursor_path: {yaml_path}\n"
            ),
        )
        .await
        .unwrap();

        /// Panics when invoked: the empty window must never reach the LLM.
        struct PanickingRunner;
        #[async_trait::async_trait]
        impl LlmRunner for PanickingRunner {
            async fn run(
                &self,
                _payload: &EnrichPayload,
                _yaml_abs: &Path,
                _repo_path: &Path,
                _config: &EnrichConfig,
            ) -> Result<()> {
                panic!("LLM must not run for an empty chunk window");
            }
        }

        let mut config = test_config(LlmProvider::OpenCode {
            path: "fake".into(),
            model: None,
        });
        config.max_articles_per_run = 2;

        let (result, _) = execute_enrich_with_runner(
            &chunk_test_payload(yaml_path),
            dir.path(),
            &config,
            "",
            &PanickingRunner,
        )
        .await
        .unwrap();
        assert!(result.law_complete);
        assert_eq!(result.enrich_cursor, 4);
    }

    #[tokio::test]
    async fn test_execute_enrich_chunk_cursor_resets_for_new_version() {
        // Metadata recorded for another yaml path (older law version): the
        // cursor must reset to 0 and MvT research must run again.
        let dir = tempfile::tempdir().unwrap();
        let law_dir = dir.path().join("regulation/nl/wet/test_law");
        tokio::fs::create_dir_all(&law_dir).await.unwrap();
        let yaml_path = "regulation/nl/wet/test_law/2026-01-01.yaml";
        tokio::fs::write(dir.path().join(yaml_path), four_article_law())
            .await
            .unwrap();
        tokio::fs::write(
            law_dir.join(".enrichment.yaml"),
            "law_id: BWBR0000001\ntimestamp: '2026-01-01T00:00:00Z'\nprovider: opencode\nmodel: m\nprompt_hash: p\ncode_commit: c\ncoverage_score: 1.0\narticles_total: 4\narticles_with_machine_readable: 4\nenrich_cursor: 2\nenrich_cursor_path: regulation/nl/wet/test_law/2025-01-01.yaml\n",
        )
        .await
        .unwrap();

        let mut config = test_config(LlmProvider::OpenCode {
            path: "fake".into(),
            model: None,
        });
        config.max_articles_per_run = 2;
        let runner = FakeChunkRunner::new(false);

        let (result, _) = execute_enrich_with_runner(
            &chunk_test_payload(yaml_path),
            dir.path(),
            &config,
            "",
            &runner,
        )
        .await
        .unwrap();
        assert!(!result.law_complete);
        assert_eq!(result.enrich_cursor, 2);
        let calls = runner.calls.lock().unwrap();
        // Reset to the start: first window, MvT research NOT skipped.
        assert_eq!(
            *calls,
            vec![(vec!["1".to_string(), "2".to_string()], Some(false))]
        );
    }

    #[tokio::test]
    async fn test_execute_enrich_whole_law_has_no_chunk_fields() {
        // N=0: the runner receives a payload without chunk fields, so
        // ProcessLlmRunner builds the byte-identical whole-law prompt.
        let dir = tempfile::tempdir().unwrap();
        let law_dir = dir.path().join("regulation/nl/wet/test_law");
        tokio::fs::create_dir_all(&law_dir).await.unwrap();
        let yaml_path = "regulation/nl/wet/test_law/2025-01-01.yaml";
        tokio::fs::write(dir.path().join(yaml_path), four_article_law())
            .await
            .unwrap();

        struct AssertWholeLawRunner;
        #[async_trait::async_trait]
        impl LlmRunner for AssertWholeLawRunner {
            async fn run(
                &self,
                payload: &EnrichPayload,
                yaml_abs: &Path,
                _repo_path: &Path,
                _config: &EnrichConfig,
            ) -> Result<()> {
                assert!(payload.chunk_articles.is_none());
                assert!(payload.skip_mvt.is_none());
                // Enrich everything so the zero-coverage guard passes.
                FakeLlmRunner
                    .run(payload, yaml_abs, _repo_path, _config)
                    .await
            }
        }

        let config = test_config(LlmProvider::OpenCode {
            path: "fake".into(),
            model: None,
        });
        assert_eq!(config.max_articles_per_run, 0);

        let (result, _) = execute_enrich_with_runner(
            &chunk_test_payload(yaml_path),
            dir.path(),
            &config,
            "",
            &AssertWholeLawRunner,
        )
        .await
        .unwrap();
        assert!(result.law_complete);
        assert_eq!(result.enrich_cursor, 0);
    }
}
