use std::path::Path;

use sqlx::PgPool;

use crate::config::WorkerConfig;
use crate::db;
use crate::error::Result;
use crate::git_ops::GitRepo;
use crate::harvest::{execute_harvest, HarvestPayload, HarvestResult};
use crate::job_queue;
use crate::law_status;
use crate::models::{JobType, LawStatusValue};

/// Run the harvest worker loop.
///
/// Polls the job queue for harvest jobs, executes them, and commits results
/// to the regulation git repository. Supports graceful shutdown via ctrl+c.
pub async fn run_harvest_worker(config: WorkerConfig) -> Result<()> {
    let pipeline_config = config.pipeline_config();
    let pool = db::create_pool(&pipeline_config).await?;
    db::run_migrations(&pool).await?;

    tracing::info!(
        repo_url = %config.regulation_repo_url,
        repo_path = %config.regulation_repo_path.display(),
        output_base = %config.regulation_output_base,
        poll_interval = ?config.poll_interval,
        push_enabled = config.push_to_git,
        "starting harvest worker"
    );

    let repo = GitRepo::clone_or_open(
        &config.regulation_repo_url,
        &config.regulation_repo_path,
    )
    .await?;

    let mut current_interval = std::time::Duration::ZERO; // poll immediately on startup

    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                tracing::info!("received shutdown signal, stopping worker");
                break;
            }
            _ = tokio::time::sleep(current_interval) => {
                match process_next_job(&pool, &repo, &config).await {
                    Ok(true) => {
                        current_interval = config.poll_interval;
                    }
                    Ok(false) => {
                        current_interval = (current_interval * 2).max(config.poll_interval).min(config.max_poll_interval);
                        tracing::debug!(next_poll = ?current_interval, "no jobs available, backing off");
                    }
                    Err(e) => {
                        tracing::error!(error = %e, "error processing job");
                        current_interval = (current_interval * 2).max(config.poll_interval).min(config.max_poll_interval);
                    }
                }
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
    repo: &GitRepo,
    config: &WorkerConfig,
) -> Result<bool> {
    if let Err(e) = repo.pull().await {
        tracing::warn!(error = %e, "git pull failed, continuing anyway");
    }

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

    let payload: HarvestPayload = match &job.payload {
        Some(p) => serde_json::from_value(p.clone()).map_err(|e| {
            crate::error::PipelineError::Worker(format!("invalid harvest payload: {e}"))
        })?,
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

    match execute_and_commit(repo, config, &payload, &job).await {
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

            law_status::update_status(pool, &job.law_id, LawStatusValue::Harvested).await?;
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
            law_status::update_status(pool, &job.law_id, LawStatusValue::HarvestFailed).await?;

            Ok(true)
        }
    }
}

/// Execute the harvest and commit results to git.
async fn execute_and_commit(
    repo: &GitRepo,
    config: &WorkerConfig,
    payload: &HarvestPayload,
    job: &crate::models::Job,
) -> Result<HarvestResult> {
    let (result, written_files) =
        execute_harvest(payload, repo.path(), &config.regulation_output_base).await?;

    let relative_paths: Vec<&Path> = written_files
        .iter()
        .filter_map(|p| p.strip_prefix(repo.path()).ok())
        .collect();

    if !relative_paths.is_empty() {
        repo.add(&relative_paths).await?;

        let commit_msg = format!(
            "harvest: {} ({}) - {} articles",
            result.law_name, payload.bwb_id, result.article_count
        );

        if repo.commit(&commit_msg).await? {
            tracing::info!(job_id = %job.id, "committed harvest results");
            repo.push(config.push_to_git).await?;
        }
    }

    Ok(result)
}
