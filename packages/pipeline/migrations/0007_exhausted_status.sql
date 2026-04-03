-- Add exhausted statuses and fail count columns.
-- Cannot use ALTER TYPE ADD VALUE inside a transaction, so we recreate
-- the enum type with the new values.

-- 1. Drop the column default (it references the old type and blocks ALTER TYPE).
ALTER TABLE law_entries ALTER COLUMN status DROP DEFAULT;

-- 2. Rename the existing type to a temporary name.
ALTER TYPE law_status RENAME TO law_status_old;

-- 3. Create the new type with exhausted statuses added.
CREATE TYPE law_status AS ENUM (
    'unknown', 'queued',
    'harvesting', 'harvested', 'harvest_failed', 'harvest_exhausted',
    'enriching', 'enriched', 'enrich_failed', 'enrich_exhausted'
);

-- 4. Alter the column to use the new type (cast via text).
ALTER TABLE law_entries
    ALTER COLUMN status TYPE law_status USING status::text::law_status;

-- 5. Restore the default.
ALTER TABLE law_entries ALTER COLUMN status SET DEFAULT 'unknown'::law_status;

-- 6. Drop the old type.
DROP TYPE law_status_old;

-- 7. Add fail count columns.
ALTER TABLE law_entries ADD COLUMN harvest_fail_count INTEGER NOT NULL DEFAULT 0;
ALTER TABLE law_entries ADD COLUMN enrich_fail_count INTEGER NOT NULL DEFAULT 0;
