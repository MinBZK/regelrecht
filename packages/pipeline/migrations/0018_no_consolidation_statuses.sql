-- Add terminal "no consolidated text" law statuses for works that BWB has no
-- consolidation for: withdrawn (datum_intrekking), not_yet_in_force (future
-- datum_inwerkingtreding), and announced (WTI-only, no dates). These let the
-- harvester skip such laws with a recorded reason instead of failing/retrying.
--
-- Cannot use ALTER TYPE ADD VALUE inside a transaction, so we recreate the enum
-- type with the new values (same approach as 0007).

-- 1. Drop the column default (it references the old type and blocks ALTER TYPE).
ALTER TABLE law_entries ALTER COLUMN status DROP DEFAULT;

-- 2. Rename the existing type to a temporary name.
ALTER TYPE law_status RENAME TO law_status_old;

-- 3. Create the new type with the no-consolidation statuses added.
CREATE TYPE law_status AS ENUM (
    'unknown', 'queued',
    'harvesting', 'harvested', 'harvest_failed', 'harvest_exhausted',
    'enriching', 'enriched', 'enrich_failed', 'enrich_exhausted',
    'withdrawn', 'not_yet_in_force', 'announced'
);

-- 4. Alter the column to use the new type (cast via text).
ALTER TABLE law_entries
    ALTER COLUMN status TYPE law_status USING status::text::law_status;

-- 5. Restore the default.
ALTER TABLE law_entries ALTER COLUMN status SET DEFAULT 'unknown'::law_status;

-- 6. Drop the old type.
DROP TYPE law_status_old;
