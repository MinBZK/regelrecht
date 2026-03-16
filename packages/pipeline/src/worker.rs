use std::path::Path;
use std::time::Duration;

use regelrecht_corpus::{CorpusClient, CorpusConfig};
use reqwest::blocking::Client;
use sqlx::PgPool;
use tokio::signal::unix::{signal, SignalKind};

use crate::config::WorkerConfig;
use crate::db;
use crate::enrich::{
    create_enrich_corpus, enrich_branch_name, execute_enrich, EnrichConfig, EnrichPayload,
};
use crate::error::{PipelineError, Result};
use crate::harvest::{execute_harvest, HarvestPayload, HarvestResult};
use crate::job_queue::{self, CreateJobRequest};
use crate::law_status;
use crate::models::{JobType, LawStatusValue};

/// Jobs stuck in 'processing' for longer than this are considered orphaned.
const ORPHAN_TIMEOUT: Duration = Duration::from_secs(30 * 60);

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
        "starting harvest worker"
    );

    let mut sigterm = signal(SignalKind::terminate()).map_err(|e| {
        crate::error::PipelineError::Worker(format!("failed to register SIGTERM handler: {e}"))
    })?;

    let mut current_interval = std::time::Duration::ZERO; // poll immediately on startup

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

        // Reap orphaned jobs stuck in 'processing' (cheap single-query check)
        if let Err(e) = job_queue::reap_orphaned_jobs(&pool, ORPHAN_TIMEOUT).await {
            tracing::warn!(error = %e, "failed to reap orphaned jobs");
        }

        // Process job outside of select! — runs to completion without cancellation
        match process_next_job(&pool, &config, &output_dir, corpus.as_ref(), &http_client).await {
            Ok(true) => {
                current_interval = config.poll_interval;
            }
            Ok(false) => {
                current_interval = (current_interval * 2)
                    .max(config.poll_interval)
                    .min(config.max_poll_interval);
                tracing::info!(next_poll = ?current_interval, "no jobs available, backing off");
            }
            Err(e) => {
                tracing::error!(error = %e, "error processing job");
                current_interval = (current_interval * 2)
                    .max(config.poll_interval)
                    .min(config.max_poll_interval);
            }
        }
    }

    Ok(())
}

/// Process the next available harvest job.
///
/// Returns `Ok(true)` if a job was processed, `Ok(false)` if no job was available.
async fn process_next_job(
    pool: &PgPool,
    config: &WorkerConfig,
    output_dir: &Path,
    corpus: Option<&CorpusClient>,
    http_client: &Client,
) -> Result<bool> {
    let job = match job_queue::claim_job(pool, Some(JobType::Harvest)).await? {
        Some(job) => job,
        None => return Ok(false),
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
                return Ok(true);
            }
        },
        None => HarvestPayload {
            bwb_id: job.law_id.clone(),
            date: None,
            max_size_mb: None,
        },
    };

    if let Err(e) = law_status::upsert_law(pool, &job.law_id, None).await {
        tracing::warn!(error = %e, law_id = %job.law_id, "failed to upsert law entry before harvest");
    }
    if let Err(e) = law_status::update_status(pool, &job.law_id, LawStatusValue::Harvesting).await {
        tracing::warn!(error = %e, law_id = %job.law_id, "failed to set status to harvesting");
    }

    match execute_harvest_job(output_dir, config, &payload, corpus, http_client).await {
        Ok(result) => {
            tracing::info!(
                job_id = %job.id,
                law_name = %result.law_name,
                articles = result.article_count,
                warnings = result.warning_count,
                "harvest completed successfully"
            );

            let result_json = serde_json::to_value(&result).ok();

            // Use a transaction so job completion and law status update are atomic.
            // Both operations must succeed — if either fails, the transaction is
            // rolled back to prevent inconsistent state (e.g. job 'completed'
            // while law status is stuck at 'harvesting').
            let mut tx = pool.begin().await?;
            job_queue::complete_job(&mut *tx, job.id, result_json).await?;
            law_status::update_status(&mut *tx, &job.law_id, LawStatusValue::Harvested).await?;
            tx.commit().await?;

            if let Ok(entry) = law_status::get_law(pool, &job.law_id).await {
                if entry.law_name.is_none() {
                    let _ = law_status::upsert_law(pool, &job.law_id, Some(&result.law_name)).await;
                }
            }

            // Auto-create enrich jobs after successful harvest — one per provider.
            // Each provider writes to its own branch (`enrich/{provider}`)
            // so results can be compared side-by-side.
            // Uses INSERT ... ON CONFLICT DO NOTHING against the
            // idx_unique_active_enrich_job partial unique index to atomically
            // prevent duplicate enrich jobs — no TOCTOU race possible.
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
                let enrich_req =
                    CreateJobRequest::new(JobType::Enrich, &job.law_id).with_payload(payload_json);
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

            Ok(true)
        }
        Err(e) => {
            tracing::error!(
                job_id = %job.id,
                law_id = %job.law_id,
                error = %e,
                "harvest failed"
            );

            let error_json = serde_json::json!({ "error": e.to_string() });
            let failed_job = job_queue::fail_job(pool, job.id, Some(error_json)).await?;

            // Only mark law as failed when retries are exhausted
            if failed_job.status == crate::models::JobStatus::Failed {
                if let Err(status_err) =
                    law_status::update_status(pool, &job.law_id, LawStatusValue::HarvestFailed)
                        .await
                {
                    tracing::warn!(error = %status_err, law_id = %job.law_id, "failed to set status to harvest_failed");
                }
            } else {
                // Job will be retried — reset law status to queued
                if let Err(status_err) =
                    law_status::update_status(pool, &job.law_id, LawStatusValue::Queued).await
                {
                    tracing::warn!(error = %status_err, law_id = %job.law_id, "failed to reset status to queued for retry");
                }
            }

            Ok(true)
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
        "starting enrich worker"
    );

    let mut sigterm = signal(SignalKind::terminate()).map_err(|e| {
        crate::error::PipelineError::Worker(format!("failed to register SIGTERM handler: {e}"))
    })?;

    let mut current_interval = std::time::Duration::ZERO;

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

        if let Err(e) = job_queue::reap_orphaned_jobs(&pool, ORPHAN_TIMEOUT).await {
            tracing::warn!(error = %e, "failed to reap orphaned jobs");
        }

        match process_next_enrich_job(
            &pool,
            &repo_path,
            &enrich_config,
            config.corpus_config.as_ref(),
        )
        .await
        {
            Ok(true) => {
                // Reset to zero to drain the queue quickly when jobs are available.
                current_interval = Duration::ZERO;
            }
            Ok(false) => {
                current_interval = (current_interval * 2)
                    .max(config.poll_interval)
                    .min(config.max_poll_interval);
                tracing::info!(next_poll = ?current_interval, "no enrich jobs available, backing off");
            }
            Err(e) => {
                tracing::error!(error = %e, "error processing enrich job");
                current_interval = (current_interval * 2)
                    .max(config.poll_interval)
                    .min(config.max_poll_interval);
            }
        }
    }

    Ok(())
}

/// Process the next available enrich job.
///
/// Returns `Ok(true)` if a job was processed, `Ok(false)` if no job was available.
///
/// Each enrichment creates a separate branch (`enrich/{provider}`)
/// so results can be reviewed before merging. A dedicated `CorpusClient` is
/// created per job pointing at the enrichment branch.
async fn process_next_enrich_job(
    pool: &PgPool,
    repo_path: &Path,
    enrich_config: &EnrichConfig,
    corpus_config: Option<&CorpusConfig>,
) -> Result<bool> {
    let job = match job_queue::claim_job(pool, Some(JobType::Enrich)).await? {
        Some(job) => job,
        None => return Ok(false),
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
                return Ok(true);
            }
        },
        None => {
            tracing::error!(job_id = %job.id, "enrich job has no payload");
            let error_json = serde_json::json!({ "error": "enrich job requires a payload" });
            if let Err(fail_err) = job_queue::fail_job(pool, job.id, Some(error_json)).await {
                tracing::error!(job_id = %job.id, error = %fail_err, "failed to mark job as failed");
            }
            return Ok(true);
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

    // Atomically transition to Enriching only if not already Enriched.
    // Uses a conditional UPDATE to avoid the TOCTOU race of get-then-update.
    if let Err(e) = law_status::update_status_unless(
        pool,
        &job.law_id,
        LawStatusValue::Enriched,
        LawStatusValue::Enriching,
    )
    .await
    {
        tracing::warn!(error = %e, law_id = %job.law_id, "failed to set status to enriching");
    }

    // Create a branch-specific corpus client for this enrichment.
    // Pass the job ID to get a unique checkout directory per worker.
    let branch = enrich_branch_name(effective_config.provider.name());
    let enrich_corpus = if let Some(base_config) = corpus_config {
        match create_enrich_corpus(base_config, &branch, job.id).await {
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

    // Capture the per-job checkout path for cleanup after the job completes.
    let checkout_path = enrich_corpus.as_ref().map(|c| c.repo_path().to_path_buf());

    let job_result = match execute_enrich(&payload, &effective_repo, &effective_config).await {
        Ok((result, written_files)) => {
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

                let result_json = serde_json::to_value(&result).ok();

                let mut tx = pool.begin().await?;
                job_queue::complete_job(&mut *tx, job.id, result_json).await?;
                law_status::update_status(&mut *tx, &job.law_id, LawStatusValue::Enriched).await?;
                tx.commit().await?;
                Ok(())
            }
            .await;

            match commit_result {
                Err(e) => {
                    tracing::error!(
                        job_id = %job.id,
                        error = %e,
                        "post-enrichment commit failed, marking job as failed for retry"
                    );
                    let error_json = serde_json::json!({ "error": e.to_string() });
                    match job_queue::fail_job(pool, job.id, Some(error_json)).await {
                        Ok(failed_job) if failed_job.status == crate::models::JobStatus::Failed => {
                            let _ = law_status::update_status_unless(
                                pool,
                                &job.law_id,
                                LawStatusValue::Enriched,
                                LawStatusValue::EnrichFailed,
                            )
                            .await;
                        }
                        Ok(_) => {
                            let _ = law_status::update_status_if(
                                pool,
                                &job.law_id,
                                LawStatusValue::Enriching,
                                LawStatusValue::Harvested,
                            )
                            .await;
                        }
                        Err(fail_err) => {
                            tracing::error!(job_id = %job.id, error = %fail_err, "failed to mark job as failed");
                        }
                    }
                }
                Ok(()) => {
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
                }
            }

            Ok(true)
        }
        Err(e) => {
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
                        // Atomically set EnrichFailed only if not already Enriched.
                        if let Err(status_err) = law_status::update_status_unless(
                            pool,
                            &job.law_id,
                            LawStatusValue::Enriched,
                            LawStatusValue::EnrichFailed,
                        )
                        .await
                        {
                            tracing::warn!(error = %status_err, law_id = %job.law_id, "failed to set status to enrich_failed");
                        }
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

            Ok(true)
        }
    };

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

/// Execute the harvest and write results to the output directory.
///
/// When a corpus client is provided, the written files are committed and pushed
/// to the corpus repository.
///
/// # At-least-once semantics
///
/// The corpus push happens before the DB transaction that marks the job as
/// completed. If the process crashes after a successful push but before the
/// DB commit, the job will be retried on restart. This is safe because
/// `commit_and_push` is idempotent: re-harvesting produces identical files,
/// and git detects "no changes to commit" when the content matches.
async fn execute_harvest_job(
    output_dir: &Path,
    config: &WorkerConfig,
    payload: &HarvestPayload,
    corpus: Option<&CorpusClient>,
    http_client: &Client,
) -> Result<HarvestResult> {
    let (result, written_files) = execute_harvest(
        payload,
        output_dir,
        &config.regulation_output_base,
        http_client,
    )
    .await?;

    if let Some(corpus) = corpus {
        let message = format!("harvest: {} ({})", result.law_name, result.slug);
        corpus.commit_and_push(&written_files, &message).await?;
    }

    Ok(result)
}
