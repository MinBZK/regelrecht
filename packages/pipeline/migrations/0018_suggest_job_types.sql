-- Add suggest_guidelines and suggest_machine_readable job types for the
-- editor's background AI-suggestion pipeline (suggestions on save).
--
-- Cannot use ALTER TYPE ADD VALUE inside a transaction (sqlx wraps each
-- migration in one), so recreate the enum type with the new values — the
-- same pattern as 0007 for law_status.

-- 1. Rename the existing type to a temporary name. The jobs.job_type column
--    has no default, so no default-drop dance is needed (unlike 0007).
ALTER TYPE job_type RENAME TO job_type_old;

-- 2. Create the new type with the suggest variants added.
CREATE TYPE job_type AS ENUM (
    'harvest',
    'enrich',
    'suggest_guidelines',
    'suggest_machine_readable'
);

-- 3. Move the column to the new type (cast via text).
ALTER TABLE jobs
    ALTER COLUMN job_type TYPE job_type USING job_type::text::job_type;

-- 4. Drop the old type.
DROP TYPE job_type_old;

-- 5. Prevent duplicate active suggest jobs per law + traject + kind. A save
--    enqueues at most one job of each suggest kind per (law, traject); this
--    closes the race where rapid saves pile up redundant suggestion runs
--    while one is still pending/processing. Mirrors idx_unique_active_enrich_job.
CREATE UNIQUE INDEX idx_unique_active_suggest_job
    ON jobs (law_id, job_type, (payload->>'traject_ref'))
    WHERE job_type IN ('suggest_guidelines', 'suggest_machine_readable')
      AND status IN ('pending', 'processing');
