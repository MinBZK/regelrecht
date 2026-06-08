-- Add a terminal "not_harvestable" law status for works that BWB has no
-- consolidated text for (withdrawn, not yet in force, or only announced). The
-- skip reason is uniform — there is no text to harvest — so a single status
-- suffices; the precise reason and date are recorded in the harvest job result.
--
-- Cannot use ALTER TYPE ADD VALUE inside a transaction, so we recreate the enum
-- type with the new value (same approach as 0007).

-- 1. Drop the column default (it references the old type and blocks ALTER TYPE).
ALTER TABLE law_entries ALTER COLUMN status DROP DEFAULT;

-- 2. Rename the existing type to a temporary name.
ALTER TYPE law_status RENAME TO law_status_old;

-- 3. Create the new type with the not_harvestable status added.
CREATE TYPE law_status AS ENUM (
    'unknown', 'queued',
    'harvesting', 'harvested', 'harvest_failed', 'harvest_exhausted',
    'enriching', 'enriched', 'enrich_failed', 'enrich_exhausted',
    'not_harvestable'
);

-- 4. Alter the column to use the new type (cast via text).
ALTER TABLE law_entries
    ALTER COLUMN status TYPE law_status USING status::text::law_status;

-- 5. Restore the default.
ALTER TABLE law_entries ALTER COLUMN status SET DEFAULT 'unknown'::law_status;

-- 6. Drop the old type.
DROP TYPE law_status_old;
