use std::path::Path;
use std::time::Duration;

use regelrecht_corpus::{CorpusClient, CorpusConfig};
use reqwest::Client;
use sqlx::PgPool;
use tokio::signal::unix::{signal, SignalKind};

use crate::config::WorkerConfig;
use crate::db;
use crate::enrich::{
    create_enrich_corpus, enrich_branch_name, execute_enrich, progress_file_path, EnrichConfig,
    EnrichPayload,
};
use crate::error::{PipelineError, Result};
use crate::harvest::{execute_harvest, HarvestPayload, HarvestResult, MAX_HARVEST_DEPTH};
use crate::job_queue::{self, CreateJobRequest};
use crate::law_status;
use crate::models::{JobType, LawStatusValue, Priority};

/// Outcome of attempting to process a single job, used to drive the
/// resource-exhaustion circuit breaker in the worker loops.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum JobOutcome {
    /// A job was claimed and handled — either completed, or failed for a
    /// reason specific to that job/law (bad data, LLM error, etc.).
    Processed,
    /// No job was available to claim.
    Idle,
    /// The job failed because the container ran out of the OS resources needed
    /// to spawn processes/threads (fork() EAGAIN / OOM). This is environmental,
    /// not the job's fault, and only clears on restart. The job is failed and
    /// requeued through the normal failure path (so its law status is reset and
    /// it isn't left dangling); the loop additionally counts these and exits the
    /// process once too many happen in a row, so the orchestrator restarts the
    /// worker with a clean process table.
    ResourceExhausted,
}

/// Map a job error to the [`JobOutcome`] reported after normal failure handling:
/// `ResourceExhausted` for environmental fork()/EAGAIN/OOM faults (drives the
/// breaker), `Processed` for ordinary per-law failures.
fn outcome_for_error(err: &str) -> JobOutcome {
    if is_resource_exhaustion(err) {
        JobOutcome::ResourceExhausted
    } else {
        JobOutcome::Processed
    }
}

/// Returns true when an error indicates the container has exhausted the OS
/// resources needed to spawn processes or threads — `fork()` returning EAGAIN
/// ("cannot fork() ... Resource temporarily unavailable"), thread-create
/// failures, or OOM. These faults are environmental: a long-running worker can
/// accumulate them (e.g. un-reaped child processes against a low pids limit)
/// and they only clear when the process restarts. Distinguishing them from
/// per-law failures lets the worker bail out for a restart instead of
/// fast-failing every queued job in a tight loop.
fn is_resource_exhaustion(err: &str) -> bool {
    let e = err.to_ascii_lowercase();
    const MARKERS: &[&str] = &[
        "cannot fork",
        "resource temporarily unavailable", // EAGAIN, human-readable form
        "os error 11",                      // EAGAIN
        "cannot allocate memory",
        "os error 12", // ENOMEM
        "unable to create thread",
        "event loop thread panicked",
    ];
    MARKERS.iter().any(|m| e.contains(m))
}

/// Interval between orphaned-job reaper runs.
const REAPER_INTERVAL: Duration = Duration::from_secs(60);

/// Spawn the orphaned-job reaper as an independent interval task.
///
/// Running the reaper inside the worker loop meant a wedged job stopped
/// reaping too, freezing the whole queue until a pod restart. As its own
/// task (with its own pool handle) it keeps resetting jobs stuck in
/// 'processing' even when the main loop is blocked on a job. The reaper
/// query is idempotent, so multiple workers running it concurrently is safe.
///
/// The task runs until the cancellation token fires (on worker shutdown).
fn spawn_reaper(
    pool: PgPool,
    orphan_timeout: Duration,
    cancel: tokio_util::sync::CancellationToken,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            if let Err(e) = job_queue::reap_orphaned_jobs(&pool, orphan_timeout).await {
                tracing::warn!(error = %e, "failed to reap orphaned jobs");
            }
            tokio::select! {
                _ = cancel.cancelled() => break,
                _ = tokio::time::sleep(REAPER_INTERVAL) => {}
            }
        }
    })
}

/// Run the harvest worker loop.
///
/// Polls the job queue for harvest jobs and executes them.
/// Supports graceful shutdown via SIGTERM and SIGINT (ctrl+c).
/// Shutdown is checked between jobs — an in-flight job always runs to completion.
pub async fn run_harvest_worker(config: WorkerConfig) -> Result<()> {
    let pipeline_config = config.pipeline_config();
    let pool = db::create_pool(&pipeline_config).await?;
    db::ensure_schema(&pool).await?;

    // Initialize corpus client if configured
    let corpus = if let Some(ref corpus_config) = config.corpus_config {
        let mut client = CorpusClient::new(corpus_config.clone());
        client.ensure_repo().await?;
        tracing::info!(path = %corpus_config.repo_path.display(), "corpus repo ready");
        Some(client)
    } else {
        tracing::info!("corpus integration disabled (CORPUS_REPO_URL not set)");
        None
    };

    // When corpus is enabled, write output into the corpus repo checkout
    let output_dir = match &corpus {
        Some(client) => client.repo_path().to_path_buf(),
        None => config.output_dir.clone(),
    };

    let http_client = regelrecht_harvester::http::create_client().map_err(|e| {
        crate::error::PipelineError::Worker(format!("failed to create HTTP client: {e}"))
    })?;

    tracing::info!(
        output_dir = %output_dir.display(),
        output_base = %config.regulation_output_base,
        poll_interval = ?config.poll_interval,
        job_timeout = ?config.job_timeout,
        orphan_timeout = ?config.orphan_timeout,
        "starting harvest worker"
    );

    let mut sigterm = signal(SignalKind::terminate()).map_err(|e| {
        crate::error::PipelineError::Worker(format!("failed to register SIGTERM handler: {e}"))
    })?;

    // Reap orphaned jobs on an independent task so a wedged job in the main
    // loop can't also stop the reaper (which would freeze the whole queue).
    let reaper_cancel = tokio_util::sync::CancellationToken::new();
    let reaper_handle = spawn_reaper(pool.clone(), config.orphan_timeout, reaper_cancel.clone());

    let mut current_interval = std::time::Duration::ZERO; // poll immediately on startup
    let mut consecutive_resource_failures: u32 = 0;

    loop {
        // Check for shutdown signals between jobs
        tokio::select! {
            biased;

            _ = tokio::signal::ctrl_c() => {
                tracing::info!("received SIGINT, stopping worker");
                break;
            }
            _ = sigterm.recv() => {
                tracing::info!("received SIGTERM, stopping worker");
                break;
            }
            _ = tokio::time::sleep(current_interval) => {
                // Ready to process next job
            }
        }

        // Process job outside of select! — runs to completion without cancellation
        match process_next_job(&pool, &config, &output_dir, corpus.as_ref(), &http_client).await {
            Ok(JobOutcome::Processed) => {
                consecutive_resource_failures = 0;
                current_interval = config.poll_interval;
            }
            Ok(JobOutcome::Idle) => {
                consecutive_resource_failures = 0;
                current_interval = (current_interval * 2)
                    .max(config.poll_interval)
                    .min(config.max_poll_interval);
                tracing::info!(next_poll = ?current_interval, "no jobs available, backing off");
            }
            Ok(JobOutcome::ResourceExhausted) => {
                handle_resource_exhaustion(
                    &mut consecutive_resource_failures,
                    config.max_consecutive_resource_failures,
                    "harvest",
                );
                current_interval = config.poll_interval;
            }
            Err(e) => {
                tracing::error!(error = %e, "error processing job");
                current_interval = (current_interval * 2)
                    .max(config.poll_interval)
                    .min(config.max_poll_interval);
            }
        }
    }

    reaper_cancel.cancel();
    if let Err(e) = reaper_handle.await {
        tracing::error!(error = %e, "reaper task panicked");
    }

    Ok(())
}

/// Increment the consecutive resource-exhaustion counter and, once it crosses
/// the configured threshold, exit the process so the orchestrator (RIG) restarts
/// the worker with a fresh process table. fork()/EAGAIN faults are environmental
/// and only clear on restart, so bailing out beats fast-failing the whole queue.
fn handle_resource_exhaustion(counter: &mut u32, threshold: u32, worker: &str) {
    *counter += 1;
    tracing::error!(
        worker,
        consecutive = *counter,
        threshold,
        "resource exhaustion (cannot fork / EAGAIN) while processing job"
    );
    if *counter >= threshold {
        tracing::error!(
            worker,
            threshold,
            "resource-exhaustion breaker tripped; exiting so the orchestrator \
             restarts the worker with a clean process table"
        );
        std::process::exit(1);
    }
}

/// Process the next available harvest job.
///
/// Returns the [`JobOutcome`]: `Processed` when a job was handled, `Idle` when
/// none was available, or `ResourceExhausted` when the job failed because the
/// container could not spawn processes/threads (fork()/EAGAIN).
async fn process_next_job(
    pool: &PgPool,
    config: &WorkerConfig,
    output_dir: &Path,
    corpus: Option<&CorpusClient>,
    http_client: &Client,
) -> Result<JobOutcome> {
    let job = match job_queue::claim_job(pool, Some(JobType::Harvest)).await? {
        Some(job) => job,
        None => return Ok(JobOutcome::Idle),
    };

    tracing::info!(
        job_id = %job.id,
        law_id = %job.law_id,
        attempt = job.attempts,
        "processing harvest job"
    );

    // Parse payload — on failure, fail the job so it doesn't stay orphaned
    let payload: HarvestPayload = match &job.payload {
        Some(p) => match serde_json::from_value(p.clone()) {
            Ok(parsed) => parsed,
            Err(e) => {
                tracing::error!(job_id = %job.id, error = %e, "invalid harvest payload");
                let error_json =
                    serde_json::json!({ "error": format!("invalid harvest payload: {e}") });
                if let Err(fail_err) = job_queue::fail_job(pool, job.id, Some(error_json)).await {
                    tracing::error!(job_id = %job.id, error = %fail_err, "failed to mark job as failed");
                }
                return Ok(JobOutcome::Processed);
            }
        },
        None => {
            // Infer source type from law_id prefix when no payload is stored.
            let (bwb_id, cvdr_id) = if job.law_id.starts_with("CVDR") {
                (None, Some(job.law_id.clone()))
            } else {
                (Some(job.law_id.clone()), None)
            };
            HarvestPayload {
                bwb_id,
                cvdr_id,
                date: None,
                max_size_mb: None,
                depth: None,
            }
        }
    };

    if let Err(e) = law_status::upsert_law(pool, &job.law_id, None, None).await {
        tracing::warn!(error = %e, law_id = %job.law_id, "failed to upsert law entry before harvest");
    }
    if let Err(e) = law_status::update_status(pool, &job.law_id, LawStatusValue::Harvesting).await {
        tracing::warn!(error = %e, law_id = %job.law_id, "failed to set status to harvesting");
    }

    // Bound the harvest with the job timeout (mirrors the enrich path): the
    // BWB HTTP fetches and corpus git operations (fetch/pull/push) have no
    // internal deadline, so a hung remote would otherwise stall this
    // sequential worker loop forever. On timeout the error feeds the normal
    // failure path below (retry with backoff / exhausted threshold).
    let harvest_outcome = match tokio::time::timeout(
        config.job_timeout,
        execute_harvest_job(output_dir, config, &payload, corpus, http_client),
    )
    .await
    {
        Ok(outcome) => outcome,
        Err(_elapsed) => {
            tracing::error!(
                job_id = %job.id,
                law_id = %job.law_id,
                timeout = ?config.job_timeout,
                "harvest job timed out"
            );
            Err(PipelineError::Worker(format!(
                "harvest job timed out after {}s",
                config.job_timeout.as_secs()
            )))
        }
    };

    match harvest_outcome {
        Ok(result) => {
            tracing::info!(
                job_id = %job.id,
                law_name = %result.law_name,
                articles = result.article_count,
                warnings = result.warning_count,
                "harvest completed successfully"
            );

            let result_json = match serde_json::to_value(&result) {
                Ok(v) => Some(v),
                Err(e) => {
                    tracing::warn!(error = %e, job_id = %job.id, "failed to serialize harvest result");
                    None
                }
            };

            // Use a transaction so job completion and law status update are atomic.
            // Both operations must succeed — if either fails, the transaction is
            // rolled back to prevent inconsistent state (e.g. job 'completed'
            // while law status is stuck at 'harvesting').
            let mut tx = pool.begin().await?;
            job_queue::complete_job(&mut *tx, job.id, result_json).await?;
            law_status::update_status(&mut *tx, &job.law_id, LawStatusValue::Harvested).await?;
            tx.commit().await?;

            if let Err(e) = law_status::reset_fail_count(pool, &job.law_id, JobType::Harvest).await
            {
                tracing::warn!(error = %e, law_id = %job.law_id, "failed to reset harvest fail count after success");
            }

            // Always store slug and refresh law_name from latest harvest.
            if let Err(e) = law_status::upsert_law(
                pool,
                &job.law_id,
                Some(&result.law_name),
                Some(&result.slug),
            )
            .await
            {
                tracing::warn!(error = %e, law_id = %job.law_id, "failed to upsert law name/slug");
            }

            // When the re-harvest was byte-identical to the existing version
            // (only the harvest timestamp differed), no new version was
            // committed — skip the follow-up work that only makes sense for an
            // actual content change (re-enriching identical content wastes LLM
            // budget; referenced-law harvests were already queued on the run
            // that first produced this content).
            if !result.changed {
                tracing::info!(
                    law_id = %job.law_id,
                    "no changes detected — law content unchanged; skipped commit, enrich, and follow-up harvests"
                );
                return Ok(JobOutcome::Processed);
            }

            // Auto-create enrich jobs after successful harvest — one per provider.
            // Each provider writes to its own branch (`enrich/{provider}`)
            // so results can be compared side-by-side.
            // Uses INSERT ... ON CONFLICT DO NOTHING against the
            // idx_unique_active_enrich_job partial unique index to atomically
            // prevent duplicate enrich jobs — no TOCTOU race possible.

            // Auto-enrich is opt-in. By default, harvesting a law does NOT
            // enqueue enrich jobs — enrichment is requested explicitly via the
            // admin API (POST /api/enrich-jobs). This prevents the recursive
            // "harvest everything → enrich everything" queue from filling up
            // (and burning LLM budget) for laws nobody asked to enrich. Set
            // ENRICH_AUTO_ENQUEUE=true to restore the old recursive behavior.
            let auto_enrich = std::env::var("ENRICH_AUTO_ENQUEUE")
                .map(|v| {
                    matches!(
                        v.trim().to_ascii_lowercase().as_str(),
                        "1" | "true" | "yes" | "on"
                    )
                })
                .unwrap_or(false);
            // Skip auto-enrich if law is exhausted for enrich
            let enrich_exhausted = match law_status::get_law(pool, &job.law_id).await {
                Ok(law) => law.status == LawStatusValue::EnrichExhausted,
                Err(e) => {
                    tracing::warn!(error = %e, law_id = %job.law_id, "failed to check enrich exhausted status, proceeding with enrich");
                    false
                }
            };
            if !auto_enrich {
                // debug, not info: with auto-enrich off by default this fires for
                // every harvested law (~22k on a full corpus harvest) — steady-state
                // noise, not an event worth surfacing at info.
                tracing::debug!(law_id = %job.law_id, "auto-enrich disabled (set ENRICH_AUTO_ENQUEUE=true to enable); not enqueuing enrich jobs");
            } else if enrich_exhausted {
                tracing::info!(law_id = %job.law_id, "skipping auto-enrich: law is enrich_exhausted");
            } else {
                for provider_name in crate::enrich::ENRICH_PROVIDERS {
                    let enrich_payload = EnrichPayload {
                        law_id: job.law_id.clone(),
                        yaml_path: result.file_path.clone(),
                        provider: Some((*provider_name).to_string()),
                    };
                    let payload_json = match serde_json::to_value(&enrich_payload) {
                        Ok(json) => json,
                        Err(e) => {
                            tracing::warn!(
                                error = %e,
                                law_id = %job.law_id,
                                provider = %provider_name,
                                "failed to serialize enrich payload, skipping"
                            );
                            continue;
                        }
                    };
                    let enrich_req = CreateJobRequest::new(JobType::Enrich, &job.law_id)
                        .with_payload(payload_json);
                    match job_queue::create_enrich_job_if_not_exists(pool, enrich_req).await {
                        Ok(Some(enrich_job)) => {
                            // Link the first created enrich job to the law entry.
                            // With dual providers only one enrich_job_id column exists,
                            // so the first provider's job wins.
                            if let Err(e) =
                                law_status::set_enrich_job(pool, &job.law_id, enrich_job.id).await
                            {
                                tracing::warn!(
                                    error = %e,
                                    law_id = %job.law_id,
                                    enrich_job_id = %enrich_job.id,
                                    "failed to link enrich job to law entry"
                                );
                            }
                            tracing::info!(
                                enrich_job_id = %enrich_job.id,
                                law_id = %job.law_id,
                                provider = %provider_name,
                                "auto-created enrich job after harvest"
                            );
                        }
                        Ok(None) => {
                            tracing::info!(
                                law_id = %job.law_id,
                                provider = %provider_name,
                                "skipping enrich job creation: active job already exists"
                            );
                        }
                        Err(e) => {
                            tracing::warn!(
                                error = %e,
                                law_id = %job.law_id,
                                provider = %provider_name,
                                "failed to auto-create enrich job (harvest still succeeded)"
                            );
                        }
                    }
                }
            }

            // Create follow-up harvest jobs for referenced laws (best-effort).
            // Respects a depth limit to prevent unbounded recursive harvesting.
            let current_depth = payload.depth.unwrap_or(0);
            if !result.referenced_bwb_ids.is_empty() && current_depth < MAX_HARVEST_DEPTH {
                let next_depth = current_depth + 1;
                let mut created = 0u32;
                for bwb_id in &result.referenced_bwb_ids {
                    // Skip harvest for exhausted laws
                    match law_status::get_law(pool, bwb_id).await {
                        Ok(law) if law.status == LawStatusValue::HarvestExhausted => {
                            tracing::info!(bwb_id = %bwb_id, "skipping follow-up harvest: law is harvest_exhausted");
                            continue;
                        }
                        _ => {}
                    }

                    // Propagate the original requested date through the chain.
                    // When None (no date specified), each law independently resolves
                    // its own latest consolidation from BWB — this ensures we always
                    // harvest the version that is valid today.
                    // Follow-up jobs from referenced_bwb_ids are always BWB laws.
                    let follow_up_payload = HarvestPayload {
                        bwb_id: Some(bwb_id.clone()),
                        cvdr_id: None,
                        date: payload.date.clone(),
                        max_size_mb: payload.max_size_mb,
                        depth: Some(next_depth),
                    };
                    let payload_json = match serde_json::to_value(&follow_up_payload) {
                        Ok(v) => v,
                        Err(e) => {
                            tracing::warn!(bwb_id = %bwb_id, error = %e, "failed to serialize follow-up payload");
                            continue;
                        }
                    };
                    let req = CreateJobRequest::new(JobType::Harvest, bwb_id.as_str())
                        .with_priority(Priority::new(30))
                        .with_payload(payload_json);
                    let dedup_date = payload.date.as_deref().unwrap_or(&result.harvest_date);
                    match job_queue::create_harvest_job_if_not_exists(pool, req, dedup_date).await {
                        Ok(Some(_)) => created += 1,
                        Ok(None) => {} // already exists, skip
                        Err(e) => tracing::warn!(
                            bwb_id = %bwb_id,
                            error = %e,
                            "failed to create follow-up harvest job"
                        ),
                    }
                }
                if created > 0 {
                    tracing::info!(
                        count = created,
                        total_refs = result.referenced_bwb_ids.len(),
                        parent_job_id = %job.id,
                        parent_law_id = %job.law_id,
                        depth = next_depth,
                        "created follow-up harvest jobs for referenced laws"
                    );
                }
            } else if !result.referenced_bwb_ids.is_empty() {
                tracing::info!(
                    depth = current_depth,
                    max_depth = MAX_HARVEST_DEPTH,
                    refs = result.referenced_bwb_ids.len(),
                    parent_job_id = %job.id,
                    parent_law_id = %job.law_id,
                    "skipping follow-up harvest jobs: max depth reached"
                );
            }

            Ok(JobOutcome::Processed)
        }
        Err(e) => {
            // The work has no consolidated text to harvest (withdrawn / not yet
            // in force / only announced). The skip reason is uniform — there is
            // no text — so the law lands in a single terminal `not_harvestable`
            // status; the precise reason and date are kept in the job result.
            // Complete (don't fail) the job so it is never retried.
            if let PipelineError::Harvester(
                regelrecht_harvester::HarvesterError::NoConsolidatedText { reason, .. },
            ) = &e
            {
                tracing::info!(
                    job_id = %job.id,
                    law_id = %job.law_id,
                    ?reason,
                    "no consolidated text to harvest; marking not_harvestable (terminal)"
                );
                let result_json = serde_json::json!({ "skipped": reason });
                let mut tx = pool.begin().await?;
                job_queue::complete_job(&mut *tx, job.id, Some(result_json)).await?;
                law_status::update_status(&mut *tx, &job.law_id, LawStatusValue::NotHarvestable)
                    .await?;
                tx.commit().await?;
                return Ok(JobOutcome::Processed);
            }

            // Container resource exhaustion (fork()/EAGAIN) is environmental, not
            // the law's fault, but the attempt was already spent at claim time, so
            // we still run the normal failure/requeue path (which resets law
            // status). We additionally report it via the outcome so the loop's
            // breaker can exit the worker for a clean restart — the only thing
            // that actually clears the condition.
            let outcome = outcome_for_error(&e.to_string());

            tracing::error!(
                job_id = %job.id,
                law_id = %job.law_id,
                error = %e,
                "harvest failed"
            );

            let error_json = serde_json::json!({ "error": e.to_string() });
            // Don't `?` here: a failure while recording the failure must still
            // report `outcome` so the breaker counts a resource-exhaustion fault
            // (matches how the enrich paths handle fail_job errors). For a
            // non-resource error `outcome` is `Processed`, so the loop just moves
            // on — a genuine DB outage resurfaces at the next claim_job and backs
            // off there, so this can't tight-loop.
            let failed_job = match job_queue::fail_job(pool, job.id, Some(error_json)).await {
                Ok(j) => j,
                Err(fail_err) => {
                    tracing::error!(job_id = %job.id, error = %fail_err, "failed to mark harvest job as failed");
                    return Ok(outcome);
                }
            };

            // Only mark law as failed when retries are exhausted
            if failed_job.status == crate::models::JobStatus::Failed {
                if let Err(status_err) =
                    law_status::update_status(pool, &job.law_id, LawStatusValue::HarvestFailed)
                        .await
                {
                    tracing::warn!(error = %status_err, law_id = %job.law_id, "failed to set status to harvest_failed");
                }

                // Check exhausted threshold
                match law_status::increment_fail_count(pool, &job.law_id, JobType::Harvest).await {
                    Ok(count) if count >= config.exhausted_threshold => {
                        if let Err(e) =
                            law_status::exhaust_law(pool, &job.law_id, JobType::Harvest).await
                        {
                            tracing::warn!(error = %e, law_id = %job.law_id, "failed to mark law as harvest_exhausted");
                        }
                    }
                    Ok(count) => {
                        // Not yet exhausted — queue a new harvest job so the
                        // fail_count can accumulate toward the threshold. The
                        // job starts with a backoff delay that grows with the
                        // law's fail count, so a transient BWB outage doesn't
                        // mass-exhaust laws within minutes.
                        let retry_delay = job_queue::retry_backoff(count);
                        tracing::info!(
                            law_id = %job.law_id,
                            fail_count = count,
                            threshold = config.exhausted_threshold,
                            delay = ?retry_delay,
                            "scheduling auto-retry harvest job"
                        );
                        match serde_json::to_value(&payload) {
                            Ok(payload_json) => {
                                let date = payload.date.as_deref().unwrap_or("");
                                let req = CreateJobRequest::new(JobType::Harvest, &job.law_id)
                                    .with_priority(Priority::new(job.priority))
                                    .with_payload(payload_json)
                                    .with_initial_delay(retry_delay);
                                match job_queue::create_harvest_job_if_not_exists(pool, req, date)
                                    .await
                                {
                                    Ok(Some(new_job)) => {
                                        tracing::info!(
                                            new_job_id = %new_job.id,
                                            law_id = %job.law_id,
                                            "auto-retry harvest job created"
                                        );
                                    }
                                    Ok(None) => {
                                        tracing::debug!(
                                            law_id = %job.law_id,
                                            "auto-retry harvest job skipped: active job already exists"
                                        );
                                    }
                                    Err(e) => {
                                        tracing::warn!(
                                            error = %e,
                                            law_id = %job.law_id,
                                            "failed to create auto-retry harvest job"
                                        );
                                    }
                                }
                            }
                            Err(e) => {
                                tracing::warn!(
                                    error = %e,
                                    law_id = %job.law_id,
                                    "failed to serialize retry payload, skipping auto-retry harvest job"
                                );
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, law_id = %job.law_id, "failed to increment harvest fail count");
                    }
                }
            } else {
                // Job will be retried — reset law status to queued
                if let Err(status_err) =
                    law_status::update_status(pool, &job.law_id, LawStatusValue::Queued).await
                {
                    tracing::warn!(error = %status_err, law_id = %job.law_id, "failed to reset status to queued for retry");
                }
            }

            Ok(outcome)
        }
    }
}

/// Run the enrich worker loop.
///
/// Polls the job queue for enrich jobs and executes them using the configured
/// LLM provider (opencode or claude). Each enrichment pushes to a separate
/// branch (`enrich/{provider}`) for review before merging.
///
/// Supports graceful shutdown via SIGTERM and SIGINT (ctrl+c).
pub async fn run_enrich_worker(config: WorkerConfig) -> Result<()> {
    let pipeline_config = config.pipeline_config();
    let pool = db::create_pool(&pipeline_config).await?;
    db::ensure_schema(&pool).await?;

    let enrich_config = EnrichConfig::from_env();

    // Corpus config is passed per-job so each enrichment creates its own
    // branch-specific corpus client. We still use the base repo_path as
    // fallback when corpus is not configured.
    let repo_path = config
        .corpus_config
        .as_ref()
        .map(|c| c.repo_path.clone())
        .unwrap_or_else(|| config.output_dir.clone());

    if config.corpus_config.is_some() {
        tracing::info!("corpus integration enabled, enrichments will push to separate branches");
    } else {
        tracing::info!("corpus integration disabled (CORPUS_REPO_URL not set)");
    }

    tracing::info!(
        repo_path = %repo_path.display(),
        provider = %enrich_config.provider.name(),
        poll_interval = ?config.poll_interval,
        job_timeout = ?config.job_timeout,
        orphan_timeout = ?config.orphan_timeout,
        "starting enrich worker"
    );

    let mut sigterm = signal(SignalKind::terminate()).map_err(|e| {
        crate::error::PipelineError::Worker(format!("failed to register SIGTERM handler: {e}"))
    })?;

    // Reap orphaned jobs on an independent task so a wedged job in the main
    // loop can't also stop the reaper (which would freeze the whole queue).
    let reaper_cancel = tokio_util::sync::CancellationToken::new();
    let reaper_handle = spawn_reaper(pool.clone(), config.orphan_timeout, reaper_cancel.clone());

    let mut current_interval = std::time::Duration::ZERO;
    let mut consecutive_resource_failures: u32 = 0;

    loop {
        tokio::select! {
            biased;

            _ = tokio::signal::ctrl_c() => {
                tracing::info!("received SIGINT, stopping enrich worker");
                break;
            }
            _ = sigterm.recv() => {
                tracing::info!("received SIGTERM, stopping enrich worker");
                break;
            }
            _ = tokio::time::sleep(current_interval) => {
                // Ready to process next job
            }
        }

        // Enforce the per-provider daily run cap before claiming a job. Counted
        // from the durable `jobs` table (not an in-memory counter) so the cap
        // holds across worker restarts/redeploys. Primarily protects a personal
        // Claude subscription token from running the whole corpus in one day.
        // Fail-closed: a limit of 0 (the default when ENRICH_DAILY_LIMIT is
        // unset) pauses the worker without even querying.
        //
        // The cap keys on the worker's configured provider (LLM_PROVIDER), not
        // the per-job payload provider. That is exact for a provider-dedicated
        // worker (the intended deployment); a worker serving multiple providers
        // would under-count the non-default provider's runs.
        let limit = config.enrich_daily_limit;
        let provider = enrich_config.provider.name();
        let over_limit = if limit == 0 {
            true
        } else {
            match job_queue::count_enrich_jobs_started_today(&pool, provider).await {
                Ok(ran_today) => ran_today >= i64::from(limit),
                Err(e) => {
                    tracing::warn!(error = %e, "failed to check daily enrich limit, proceeding");
                    false
                }
            }
        };
        if over_limit {
            current_interval = config.max_poll_interval;
            tracing::info!(
                provider,
                limit,
                next_poll = ?current_interval,
                "daily enrich limit reached (or ENRICH_DAILY_LIMIT unset/0), pausing until the UTC day rolls over"
            );
            continue;
        }

        match process_next_enrich_job(
            &pool,
            &repo_path,
            &enrich_config,
            config.corpus_config.as_ref(),
            config.job_timeout,
            config.exhausted_threshold,
        )
        .await
        {
            Ok(JobOutcome::Processed) => {
                consecutive_resource_failures = 0;
                current_interval = config.poll_interval;
            }
            Ok(JobOutcome::Idle) => {
                consecutive_resource_failures = 0;
                current_interval = (current_interval * 2)
                    .max(config.poll_interval)
                    .min(config.max_poll_interval);
                tracing::info!(next_poll = ?current_interval, "no enrich jobs available, backing off");
            }
            Ok(JobOutcome::ResourceExhausted) => {
                handle_resource_exhaustion(
                    &mut consecutive_resource_failures,
                    config.max_consecutive_resource_failures,
                    "enrich",
                );
                current_interval = config.poll_interval;
            }
            Err(e) => {
                tracing::error!(error = %e, "error processing enrich job");
                current_interval = (current_interval * 2)
                    .max(config.poll_interval)
                    .min(config.max_poll_interval);
            }
        }
    }

    reaper_cancel.cancel();
    if let Err(e) = reaper_handle.await {
        tracing::error!(error = %e, "reaper task panicked");
    }

    Ok(())
}

/// Process the next available enrich job.
///
/// Returns the [`JobOutcome`]: `Processed` when a job was handled, `Idle` when
/// none was available, or `ResourceExhausted` when the job failed because the
/// container could not spawn processes/threads (fork()/EAGAIN).
///
/// Each enrichment creates a separate branch (`enrich/{provider}`)
/// so results can be reviewed before merging. A dedicated `CorpusClient` is
/// created per job pointing at the enrichment branch.
async fn process_next_enrich_job(
    pool: &PgPool,
    repo_path: &Path,
    enrich_config: &EnrichConfig,
    corpus_config: Option<&CorpusConfig>,
    job_timeout: Duration,
    exhausted_threshold: i32,
) -> Result<JobOutcome> {
    let job = match job_queue::claim_job(pool, Some(JobType::Enrich)).await? {
        Some(job) => job,
        None => return Ok(JobOutcome::Idle),
    };

    let payload: EnrichPayload = match &job.payload {
        Some(p) => match serde_json::from_value(p.clone()) {
            Ok(parsed) => parsed,
            Err(e) => {
                tracing::error!(job_id = %job.id, error = %e, "invalid enrich payload");
                let error_json =
                    serde_json::json!({ "error": format!("invalid enrich payload: {e}") });
                if let Err(fail_err) = job_queue::fail_job(pool, job.id, Some(error_json)).await {
                    tracing::error!(job_id = %job.id, error = %fail_err, "failed to mark job as failed");
                }
                return Ok(JobOutcome::Processed);
            }
        },
        None => {
            tracing::error!(job_id = %job.id, "enrich job has no payload");
            let error_json = serde_json::json!({ "error": "enrich job requires a payload" });
            if let Err(fail_err) = job_queue::fail_job(pool, job.id, Some(error_json)).await {
                tracing::error!(job_id = %job.id, error = %fail_err, "failed to mark job as failed");
            }
            return Ok(JobOutcome::Processed);
        }
    };

    // Override the provider if the payload specifies one
    let effective_config = match &payload.provider {
        Some(provider_name) => enrich_config.with_provider_override(provider_name),
        None => enrich_config.clone(),
    };

    tracing::info!(
        job_id = %job.id,
        law_id = %job.law_id,
        attempt = job.attempts,
        provider = %effective_config.provider.name(),
        "processing enrich job"
    );

    // Atomically transition to Enriching only if not already Enriched or EnrichExhausted.
    if let Err(e) = sqlx::query(
        "UPDATE law_entries SET status = 'enriching'::law_status, updated_at = now() \
         WHERE law_id = $1 AND status NOT IN ('enriched', 'enrich_exhausted')",
    )
    .bind(&job.law_id)
    .execute(pool)
    .await
    {
        tracing::warn!(error = %e, law_id = %job.law_id, "failed to set status to enriching");
    }

    // Create a branch-specific corpus client for this enrichment.
    // Pass the job ID to get a unique checkout directory per worker.
    let branch = enrich_branch_name(effective_config.provider.name());
    let enrich_corpus = if let Some(base_config) = corpus_config {
        match create_enrich_corpus(base_config, &branch, job.id, &payload.yaml_path).await {
            Ok(client) => {
                tracing::info!(branch = %branch, "created enrichment branch corpus");
                Some(client)
            }
            Err(e) => {
                tracing::warn!(error = %e, branch = %branch, "failed to create enrichment branch corpus, proceeding without");
                None
            }
        }
    } else {
        None
    };

    // Use the enrichment branch repo if available, otherwise the base repo
    let effective_repo = enrich_corpus
        .as_ref()
        .map(|c| c.repo_path().to_path_buf())
        .unwrap_or_else(|| repo_path.to_path_buf());

    // Ensure skill files are available in the repo checkout so the LLM can
    // read them. In the container the skills are baked into /opt/skills/;
    // this symlinks them into the per-job checkout.
    if let Err(e) = crate::enrich::ensure_skills(&effective_repo).await {
        tracing::warn!(error = %e, "failed to set up skill symlinks");
    }

    // Capture the per-job checkout path for cleanup after the job completes.
    let checkout_path = enrich_corpus.as_ref().map(|c| c.repo_path().to_path_buf());

    // Compute the progress file path and spawn a background polling task.
    // The LLM writes phase info to this file; we relay it to the DB every 10s.
    let normalized_yaml_path = crate::enrich::normalize_yaml_path(&payload.yaml_path).ok();
    let progress_path = normalized_yaml_path
        .as_ref()
        .map(|p| progress_file_path(&effective_repo.join(p)));

    let cancel_token = tokio_util::sync::CancellationToken::new();
    let poll_handle = if let Some(ref ppath) = progress_path {
        let token = cancel_token.clone();
        let pool = pool.clone();
        let job_id = job.id;
        let ppath = ppath.clone();
        Some(tokio::spawn(async move {
            poll_progress_file(&pool, job_id, &ppath, token).await;
        }))
    } else {
        None
    };

    // Ensure the LLM's internal timeout fires before the outer job timeout
    // so ProcessLlmRunner can kill the child process cleanly. Without this,
    // if LLM_TIMEOUT_SECS > WORKER_JOB_TIMEOUT_SECS, the outer timeout would
    // drop the future while the OS subprocess keeps running.
    let mut bounded_config = effective_config.clone();
    if bounded_config.timeout >= job_timeout {
        bounded_config.timeout = job_timeout.saturating_sub(Duration::from_secs(30));
        tracing::warn!(
            llm_timeout = ?effective_config.timeout,
            job_timeout = ?job_timeout,
            adjusted_to = ?bounded_config.timeout,
            "LLM timeout >= job timeout, reducing LLM timeout to leave headroom"
        );
    }

    let enrich_outcome = tokio::time::timeout(
        job_timeout,
        execute_enrich(&payload, &effective_repo, &bounded_config),
    )
    .await;

    let job_result = match enrich_outcome {
        Err(_elapsed) => {
            // Job timed out
            tracing::error!(
                job_id = %job.id,
                law_id = %job.law_id,
                timeout = ?job_timeout,
                "enrich job timed out"
            );

            let error_json = serde_json::json!({
                "error": format!("job timed out after {}s", job_timeout.as_secs())
            });
            match job_queue::fail_job(pool, job.id, Some(error_json)).await {
                Ok(failed_job) => {
                    if failed_job.status == crate::models::JobStatus::Failed {
                        // Set EnrichFailed only if not already Enriched or EnrichExhausted.
                        if let Err(e) = sqlx::query(
                            "UPDATE law_entries SET status = 'enrich_failed'::law_status, updated_at = now() \
                             WHERE law_id = $1 AND status NOT IN ('enriched', 'enrich_exhausted')",
                        )
                        .bind(&job.law_id)
                        .execute(pool)
                        .await
                        {
                            tracing::warn!(error = %e, law_id = %job.law_id, "failed to update law status to enrich_failed");
                        }

                        handle_enrich_exhausted_or_retry(
                            pool,
                            &job.law_id,
                            &payload,
                            job.priority,
                            exhausted_threshold,
                        )
                        .await;
                    } else if let Err(e) = law_status::update_status_if(
                        pool,
                        &job.law_id,
                        LawStatusValue::Enriching,
                        LawStatusValue::Harvested,
                    )
                    .await
                    {
                        tracing::warn!(error = %e, law_id = %job.law_id, "failed to reset law status to harvested");
                    }
                }
                Err(fail_err) => {
                    tracing::error!(job_id = %job.id, error = %fail_err, "failed to mark timed-out job as failed");
                }
            }

            Ok(JobOutcome::Processed)
        }
        Ok(Ok((result, written_files))) => {
            tracing::info!(
                job_id = %job.id,
                articles_total = result.articles_total,
                articles_with_machine_readable = result.articles_with_machine_readable,
                coverage_score = result.coverage_score,
                provider = %result.provider,
                branch = %result.branch,
                "enrichment completed successfully"
            );

            // Push to corpus, complete the job in DB, and update law status.
            // If any of these fail, mark the job as failed so it gets retried
            // instead of orphaning it in 'processing' state for 30 minutes.
            let commit_result: std::result::Result<(), PipelineError> = async {
                if let Some(ref corpus) = enrich_corpus {
                    let message = format!(
                        "enrich({}): {} ({})",
                        result.provider, result.law_id, result.yaml_path
                    );
                    corpus
                        .commit_and_push(&written_files, &message)
                        .await
                        .map_err(|e| PipelineError::Enrich(format!("corpus push failed: {e}")))?;
                }

                let result_json = match serde_json::to_value(&result) {
                    Ok(v) => Some(v),
                    Err(e) => {
                        tracing::warn!(error = %e, job_id = %job.id, "failed to serialize enrich result");
                        None
                    }
                };

                let mut tx = pool.begin().await?;
                job_queue::complete_job(&mut *tx, job.id, result_json).await?;
                law_status::update_status(&mut *tx, &job.law_id, LawStatusValue::Enriched).await?;
                tx.commit().await?;
                Ok(())
            }
            .await;

            match commit_result {
                Err(e) => {
                    let outcome = outcome_for_error(&e.to_string());
                    tracing::error!(
                        job_id = %job.id,
                        error = %e,
                        "post-enrichment commit failed, marking job as failed for retry"
                    );
                    let error_json = serde_json::json!({ "error": e.to_string() });
                    match job_queue::fail_job(pool, job.id, Some(error_json)).await {
                        Ok(failed_job) if failed_job.status == crate::models::JobStatus::Failed => {
                            // Set EnrichFailed only if not already Enriched or EnrichExhausted.
                            if let Err(e) = sqlx::query(
                                "UPDATE law_entries SET status = 'enrich_failed'::law_status, updated_at = now() \
                                 WHERE law_id = $1 AND status NOT IN ('enriched', 'enrich_exhausted')",
                            )
                            .bind(&job.law_id)
                            .execute(pool)
                            .await
                            {
                                tracing::warn!(error = %e, law_id = %job.law_id, "failed to update law status to enrich_failed");
                            }

                            handle_enrich_exhausted_or_retry(
                                pool,
                                &job.law_id,
                                &payload,
                                job.priority,
                                exhausted_threshold,
                            )
                            .await;
                        }
                        Ok(_) => {
                            if let Err(e) = law_status::update_status_if(
                                pool,
                                &job.law_id,
                                LawStatusValue::Enriching,
                                LawStatusValue::Harvested,
                            )
                            .await
                            {
                                tracing::warn!(error = %e, law_id = %job.law_id, "failed to reset law status to harvested");
                            }
                        }
                        Err(fail_err) => {
                            tracing::error!(job_id = %job.id, error = %fail_err, "failed to mark job as failed");
                        }
                    }
                    Ok(outcome)
                }
                Ok(()) => {
                    if let Err(e) =
                        law_status::reset_fail_count(pool, &job.law_id, JobType::Enrich).await
                    {
                        tracing::warn!(error = %e, law_id = %job.law_id, "failed to reset enrich fail count after success");
                    }

                    // Set coverage score outside the transaction (non-critical).
                    // With dual providers, whichever finishes last writes the score.
                    if let Err(e) =
                        law_status::set_coverage_score(pool, &job.law_id, result.coverage_score)
                            .await
                    {
                        tracing::warn!(error = %e, provider = %result.provider, "failed to set coverage score");
                    } else {
                        tracing::info!(
                            law_id = %job.law_id,
                            provider = %result.provider,
                            coverage_score = result.coverage_score,
                            "coverage score updated"
                        );
                    }
                    Ok(JobOutcome::Processed)
                }
            }
        }
        Ok(Err(e)) => {
            let outcome = outcome_for_error(&e.to_string());
            tracing::error!(
                job_id = %job.id,
                law_id = %job.law_id,
                error = %e,
                "enrichment failed"
            );

            let error_json = serde_json::json!({ "error": e.to_string() });
            match job_queue::fail_job(pool, job.id, Some(error_json)).await {
                Ok(failed_job) => {
                    if failed_job.status == crate::models::JobStatus::Failed {
                        // Set EnrichFailed only if not already Enriched or EnrichExhausted.
                        if let Err(status_err) = sqlx::query(
                            "UPDATE law_entries SET status = 'enrich_failed'::law_status, updated_at = now() \
                             WHERE law_id = $1 AND status NOT IN ('enriched', 'enrich_exhausted')",
                        )
                        .bind(&job.law_id)
                        .execute(pool)
                        .await
                        {
                            tracing::warn!(error = %status_err, law_id = %job.law_id, "failed to set status to enrich_failed");
                        }

                        handle_enrich_exhausted_or_retry(
                            pool,
                            &job.law_id,
                            &payload,
                            job.priority,
                            exhausted_threshold,
                        )
                        .await;
                    } else {
                        // Job will be retried — atomically reset to Harvested only if
                        // status is currently Enriching. Cannot regress from Enriched.
                        if let Err(status_err) = law_status::update_status_if(
                            pool,
                            &job.law_id,
                            LawStatusValue::Enriching,
                            LawStatusValue::Harvested,
                        )
                        .await
                        {
                            tracing::warn!(error = %status_err, law_id = %job.law_id, "failed to reset status to harvested for retry");
                        }
                    }
                }
                Err(fail_err) => {
                    tracing::error!(job_id = %job.id, error = %fail_err, "failed to mark job as failed");
                }
            }

            Ok(outcome)
        }
    };

    // Stop the progress polling task and clean up the progress file.
    cancel_token.cancel();
    if let Some(handle) = poll_handle {
        let _ = handle.await;
    }
    if let Some(ref ppath) = progress_path {
        let _ = tokio::fs::remove_file(ppath).await;
    }

    // Clean up the per-job corpus checkout directory (regardless of outcome).
    // Each enrich job creates a full git clone; without cleanup these accumulate.
    if let Some(path) = checkout_path {
        if let Err(e) = tokio::fs::remove_dir_all(&path).await {
            tracing::warn!(
                path = %path.display(),
                error = %e,
                "failed to clean up per-job corpus checkout"
            );
        } else {
            tracing::debug!(path = %path.display(), "cleaned up per-job corpus checkout");
        }
    }

    job_result
}

/// Poll the progress file written by the LLM and relay its contents to the DB.
///
/// Runs until the cancellation token is cancelled. Reads the file every 10
/// seconds; parse errors are silently ignored (the file may be half-written).
async fn poll_progress_file(
    pool: &PgPool,
    job_id: uuid::Uuid,
    path: &Path,
    cancel: tokio_util::sync::CancellationToken,
) {
    let interval = Duration::from_secs(10);
    loop {
        tokio::select! {
            _ = cancel.cancelled() => break,
            _ = tokio::time::sleep(interval) => {}
        }

        let content = match tokio::fs::read_to_string(path).await {
            Ok(c) => c,
            Err(_) => continue, // file doesn't exist yet
        };

        let value: serde_json::Value = match serde_json::from_str(&content) {
            Ok(v) => v,
            Err(_) => continue, // half-written or invalid JSON
        };

        if let Err(e) = job_queue::update_progress(pool, job_id, value).await {
            tracing::warn!(job_id = %job_id, error = %e, "failed to update job progress");
        }
    }

    // Final read to capture the last phase the LLM wrote before the job finished.
    if let Ok(content) = tokio::fs::read_to_string(path).await {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&content) {
            let _ = job_queue::update_progress(pool, job_id, value).await;
        }
    }
}

/// Execute the harvest and write results to the output directory.
///
/// When a corpus client is provided, the written files are committed and pushed
/// to the corpus repository.
///
/// # At-least-once semantics
///
/// The corpus push happens before the DB transaction that marks the job as
/// completed. If the process crashes after a successful push but before the
/// DB commit, the job will be retried on restart. This is safe because the
/// commit is content-gated: re-harvesting unchanged law content produces no
/// new commit (`commit_and_push_content` returns `false`), so retries are
/// idempotent even though the `status.yaml` timestamp churns every run.
///
/// Sets `result.changed` to reflect whether a commit was actually made, so the
/// caller can skip follow-up work (enrich, referenced-law harvests) when the
/// re-harvest was a no-op.
async fn execute_harvest_job(
    output_dir: &Path,
    config: &WorkerConfig,
    payload: &HarvestPayload,
    corpus: Option<&CorpusClient>,
    http_client: &Client,
) -> Result<HarvestResult> {
    let (mut result, files) = execute_harvest(
        payload,
        output_dir,
        &config.regulation_output_base,
        http_client,
    )
    .await?;

    if let Some(corpus) = corpus {
        let message = format!("harvest: {} ({})", result.law_name, result.slug);
        // Only a change to the law YAML (content) counts as a real change; the
        // status.yaml timestamp (metadata) alone must not produce a new version.
        result.changed = corpus
            .commit_and_push_content(
                std::slice::from_ref(&files.content),
                std::slice::from_ref(&files.metadata),
                &message,
            )
            .await?;
    }

    Ok(result)
}

/// Increment the enrich fail count and either mark the law as exhausted
/// or schedule a new enrich job for retry.
async fn handle_enrich_exhausted_or_retry(
    pool: &PgPool,
    law_id: &str,
    payload: &EnrichPayload,
    priority: i32,
    exhausted_threshold: i32,
) {
    match law_status::increment_fail_count(pool, law_id, JobType::Enrich).await {
        Ok(count) if count >= exhausted_threshold => {
            if let Err(e) = law_status::exhaust_law(pool, law_id, JobType::Enrich).await {
                tracing::warn!(error = %e, law_id = %law_id, "failed to mark law as enrich_exhausted");
            }
        }
        Ok(count) => {
            // Not yet exhausted — queue a new enrich job so the
            // fail_count can accumulate toward the threshold. The job starts
            // with a backoff delay that grows with the law's fail count.
            let retry_delay = job_queue::retry_backoff(count);
            tracing::info!(
                law_id = %law_id,
                fail_count = count,
                threshold = exhausted_threshold,
                delay = ?retry_delay,
                "scheduling auto-retry enrich job"
            );
            match serde_json::to_value(payload) {
                Ok(payload_json) => {
                    let req = CreateJobRequest::new(JobType::Enrich, law_id)
                        .with_priority(Priority::new(priority))
                        .with_payload(payload_json)
                        .with_initial_delay(retry_delay);
                    match job_queue::create_enrich_job_if_not_exists(pool, req).await {
                        Ok(Some(new_job)) => {
                            tracing::info!(
                                new_job_id = %new_job.id,
                                law_id = %law_id,
                                "auto-retry enrich job created"
                            );
                        }
                        Ok(None) => {
                            tracing::debug!(
                                law_id = %law_id,
                                "auto-retry enrich job skipped: active job already exists"
                            );
                        }
                        Err(e) => {
                            tracing::warn!(
                                error = %e,
                                law_id = %law_id,
                                "failed to create auto-retry enrich job"
                            );
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        law_id = %law_id,
                        "failed to serialize retry payload, skipping auto-retry enrich job"
                    );
                }
            }
        }
        Err(e) => {
            tracing::warn!(error = %e, law_id = %law_id, "failed to increment enrich fail count");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_fork_eagain_errors_as_resource_exhaustion() {
        // Real strings observed from failed harvest jobs.
        assert!(is_resource_exhaustion(
            "corpus error: git command failed: git pull failed: error: cannot fork() for \
             merge-base: Resource temporarily unavailable"
        ));
        assert!(is_resource_exhaustion(
            "corpus error: IO error: Resource temporarily unavailable (os error 11)"
        ));
        assert!(is_resource_exhaustion(
            "task join error: task 5 panicked with message \"event loop thread panicked\""
        ));
        assert!(is_resource_exhaustion(
            "Cannot allocate memory (os error 12)"
        ));
        assert!(is_resource_exhaustion(
            "fatal: unable to create thread: Resource temporarily unavailable"
        ));
    }

    #[test]
    fn does_not_classify_per_law_failures_as_resource_exhaustion() {
        // These are real per-law failures that must still burn the retry budget.
        assert!(!is_resource_exhaustion(
            "harvester error: Missing required XML element: _latestItem attribute in manifest"
        ));
        assert!(!is_resource_exhaustion(
            "harvester error: CVDR SRU search failed for CVDR756485: No records found"
        ));
        assert!(!is_resource_exhaustion(
            "enrichment error: claude exited with exit status: 1"
        ));
        assert!(!is_resource_exhaustion(
            "enrichment error: opencode timed out after 600s"
        ));
        assert!(!is_resource_exhaustion(
            "YAML error: did not find expected key at line 5 column 3"
        ));
    }

    #[test]
    fn handle_resource_exhaustion_increments_until_threshold() {
        // Below the threshold it must not exit (test would abort if it did).
        let mut counter = 0u32;
        handle_resource_exhaustion(&mut counter, 3, "test");
        assert_eq!(counter, 1);
        handle_resource_exhaustion(&mut counter, 3, "test");
        assert_eq!(counter, 2);
    }
}
