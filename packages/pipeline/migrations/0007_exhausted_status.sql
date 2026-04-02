-- Add exhausted statuses and fail count columns.
-- Cannot use ALTER TYPE ADD VALUE inside a transaction, so we recreate
-- the enum type with the new values.

-- 1. Rename the existing type to a temporary name.
ALTER TYPE law_status RENAME TO law_status_old;

-- 2. Create the new type with exhausted statuses added.
CREATE TYPE law_status AS ENUM (
    'unknown', 'queued',
    'harvesting', 'harvested', 'harvest_failed', 'harvest_exhausted',
    'enriching', 'enriched', 'enrich_failed', 'enrich_exhausted'
);

-- 3. Alter the column to use the new type (cast via text).
ALTER TABLE law_entries
    ALTER COLUMN status TYPE law_status USING status::text::law_status;

-- 4. Drop the old type.
DROP TYPE law_status_old;

-- 5. Add fail count columns.
ALTER TABLE law_entries ADD COLUMN harvest_fail_count INTEGER NOT NULL DEFAULT 0;
ALTER TABLE law_entries ADD COLUMN enrich_fail_count INTEGER NOT NULL DEFAULT 0;
