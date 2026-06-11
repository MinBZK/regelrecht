-- Retry backoff: scheduled_at delays when a pending job becomes claimable.
-- NULL means the job is claimable immediately (the default for new jobs).
-- fail_job sets it to now() + exponential backoff when re-queueing a job
-- for retry, so transient outages (e.g. BWB downtime) no longer burn all
-- attempts within seconds.
ALTER TABLE jobs ADD COLUMN scheduled_at TIMESTAMPTZ;

-- Recreate the claim index with scheduled_at as a trailing key. The claim
-- query's due-filter (scheduled_at IS NULL OR scheduled_at <= now()) is an
-- OR on a non-leading column, so it is applied as a heap filter rather than
-- an index qual (and FOR UPDATE visits the heap regardless); the partial
-- predicate and the (priority, created_at) ordering are still index-served.
DROP INDEX IF EXISTS idx_jobs_queue;
CREATE INDEX idx_jobs_queue ON jobs (priority DESC, created_at ASC, scheduled_at)
    WHERE status = 'pending';
