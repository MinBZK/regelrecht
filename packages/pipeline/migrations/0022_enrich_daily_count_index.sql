-- Partial index supporting the per-provider daily enrich-run count
-- (count_enrich_jobs_started_today). The daily-limit gate runs that query on
-- every enrich-worker poll cycle, filtering enrich jobs by their payload
-- provider and started_at within the current UTC day; without an index it scans
-- the whole jobs table each time.
CREATE INDEX IF NOT EXISTS idx_jobs_enrich_provider_started_at
    ON jobs ((payload ->> 'provider'), started_at)
    WHERE job_type = 'enrich' AND started_at IS NOT NULL;
