-- Prevent duplicate active harvest jobs per law.
-- This closes the TOCTOU race where concurrent harvest requests
-- could both create a harvest job for the same law.
-- Mirrors 0003_unique_active_enrich_jobs.sql for harvest jobs.
CREATE UNIQUE INDEX idx_unique_active_harvest_job
    ON jobs (law_id, job_type)
    WHERE job_type = 'harvest' AND status IN ('pending', 'processing');
