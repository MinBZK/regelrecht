-- Retry backoff: scheduled_at delays when a pending job becomes claimable.
-- NULL means the job is claimable immediately (the default for new jobs).
-- fail_job sets it to now() + exponential backoff when re-queueing a job
-- for retry, so transient outages (e.g. BWB downtime) no longer burn all
-- attempts within seconds.
ALTER TABLE jobs ADD COLUMN scheduled_at TIMESTAMPTZ;

-- Recreate the claim index with scheduled_at as a trailing key so the claim
-- query's due-filter (scheduled_at IS NULL OR scheduled_at <= now()) can be
-- evaluated from the index without heap fetches.
DROP INDEX idx_jobs_queue;
CREATE INDEX idx_jobs_queue ON jobs (priority DESC, created_at ASC, scheduled_at)
    WHERE status = 'pending';
