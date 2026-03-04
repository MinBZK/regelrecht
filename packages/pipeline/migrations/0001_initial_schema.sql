-- Pipeline initial schema

-- Enum types
CREATE TYPE job_type AS ENUM ('harvest', 'enrich');
CREATE TYPE job_status AS ENUM ('pending', 'processing', 'completed', 'failed');
CREATE TYPE law_status AS ENUM (
    'unknown', 'queued',
    'harvesting', 'harvested', 'harvest_failed',
    'enriching', 'enriched', 'enrich_failed'
);

-- Jobs table
CREATE TABLE jobs (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    job_type    job_type    NOT NULL,
    law_id      TEXT        NOT NULL,
    status      job_status  NOT NULL DEFAULT 'pending',
    priority    INTEGER     NOT NULL DEFAULT 50 CHECK (priority BETWEEN 0 AND 100),
    payload     JSONB,
    result      JSONB,
    attempts    INTEGER     NOT NULL DEFAULT 0,
    max_attempts INTEGER    NOT NULL DEFAULT 3 CHECK (max_attempts >= 1),
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    started_at  TIMESTAMPTZ,
    completed_at TIMESTAMPTZ
);

-- Partial index for efficient job claiming: only pending jobs, ordered by priority then age
CREATE INDEX idx_jobs_queue ON jobs (priority DESC, created_at ASC) WHERE status = 'pending';

-- Index for looking up jobs by law_id
CREATE INDEX idx_jobs_law_id ON jobs (law_id);

-- Law status table
CREATE TABLE law_entries (
    law_id          TEXT PRIMARY KEY,
    law_name        TEXT,
    status          law_status  NOT NULL DEFAULT 'unknown',
    harvest_job_id  UUID REFERENCES jobs(id) ON DELETE SET NULL,
    enrich_job_id   UUID REFERENCES jobs(id) ON DELETE SET NULL,
    quality_score   DOUBLE PRECISION CHECK (quality_score >= 0 AND quality_score <= 1),
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Trigger to auto-update updated_at
CREATE OR REPLACE FUNCTION update_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = now();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_jobs_updated_at
    BEFORE UPDATE ON jobs
    FOR EACH ROW EXECUTE FUNCTION update_updated_at();

CREATE TRIGGER trg_law_entries_updated_at
    BEFORE UPDATE ON law_entries
    FOR EACH ROW EXECUTE FUNCTION update_updated_at();
