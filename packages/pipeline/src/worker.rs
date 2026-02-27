use std::path::Path;

use sqlx::PgPool;
use tokio::signal::unix::{signal, SignalKind};

use crate::config::WorkerConfig;
use crate::db;
use crate::error::Result;
use crate::harvest::{execute_harvest, HarvestPayload, HarvestResult};
use crate::job_queue;
use crate::law_status;
use crate::models::{JobType, LawStatusValue};

/// Run the harvest worker loop.
///
/// Polls the job queue for harvest jobs and executes them.
/// Supports graceful shutdown via SIGTERM and SIGINT (ctrl+c).
/// Shutdown is checked between jobs — an in-flight job always runs to completion.
pub async fn run_harvest_worker(config: WorkerConfig) -> Result<()> {
    let pipeline_config = config.pipeline_config();
    let pool = db::create_pool(&pipeline_config).await?;
    db::run_migrations(&pool).await?;

    tracing::info!(
        output_dir = %config.output_dir.display(),
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

        // Process job outside of select! — runs to completion without cancellation
        match process_next_job(&pool, &config).await {
            Ok(true) => {
                current_interval = config.poll_interval;
            }
            Ok(false) => {
                current_interval = (current_interval * 2)
                    .max(config.poll_interval)
                    .min(config.max_poll_interval);
                tracing::debug!(next_poll = ?current_interval, "no jobs available, backing off");
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
async fn process_next_job(pool: &PgPool, config: &WorkerConfig) -> Result<bool> {
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

    match execute_harvest_job(&config.output_dir, config, &payload).await {
        Ok(result) => {
            tracing::info!(
                job_id = %job.id,
                law_name = %result.law_name,
                articles = result.article_count,
                warnings = result.warning_count,
                "harvest completed successfully"
            );

            let result_json = serde_json::to_value(&result).ok();
            job_queue::complete_job(pool, job.id, result_json).await?;

            if let Err(e) =
                law_status::update_status(pool, &job.law_id, LawStatusValue::Harvested).await
            {
                tracing::warn!(error = %e, law_id = %job.law_id, "failed to set status to harvested");
            }
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
            job_queue::fail_job(pool, job.id, Some(error_json)).await?;
            if let Err(status_err) =
                law_status::update_status(pool, &job.law_id, LawStatusValue::HarvestFailed).await
            {
                tracing::warn!(error = %status_err, law_id = %job.law_id, "failed to set status to harvest_failed");
            }

            Ok(true)
        }
    }
}

/// Execute the harvest and write results to the output directory.
async fn execute_harvest_job(
    output_dir: &Path,
    config: &WorkerConfig,
    payload: &HarvestPayload,
) -> Result<HarvestResult> {
    let (result, _written_files) =
        execute_harvest(payload, output_dir, &config.regulation_output_base).await?;

    Ok(result)
}
