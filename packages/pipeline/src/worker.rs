use std::path::Path;
use std::time::Duration;

use regelrecht_corpus::CorpusClient;
use sqlx::PgPool;
use tokio::signal::unix::{signal, SignalKind};

use crate::config::WorkerConfig;
use crate::db;
use crate::error::Result;
use crate::harvest::{execute_harvest, HarvestPayload, HarvestResult};
use crate::job_queue;
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
    db::wait_for_schema(&pool).await?;

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
        match process_next_job(&pool, &config, &output_dir, corpus.as_ref()).await {
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

    match execute_harvest_job(output_dir, config, &payload, corpus).await {
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
) -> Result<HarvestResult> {
    let (result, written_files) =
        execute_harvest(payload, output_dir, &config.regulation_output_base).await?;

    if let Some(corpus) = corpus {
        let message = format!("harvest: {} ({})", result.law_name, result.slug);
        corpus.commit_and_push(&written_files, &message).await?;
    }

    Ok(result)
}
