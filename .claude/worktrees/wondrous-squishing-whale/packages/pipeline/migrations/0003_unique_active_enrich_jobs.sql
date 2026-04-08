-- Prevent duplicate active enrich jobs per law + provider.
-- This closes the TOCTOU race where two concurrent harvest completions
-- could both create an enrich job for the same law + provider.
CREATE UNIQUE INDEX idx_unique_active_enrich_job
    ON jobs (law_id, job_type, (payload->>'provider'))
    WHERE job_type = 'enrich' AND status IN ('pending', 'processing');
