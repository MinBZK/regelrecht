ALTER TYPE law_status ADD VALUE IF NOT EXISTS 'harvest_exhausted' AFTER 'harvest_failed';
ALTER TYPE law_status ADD VALUE IF NOT EXISTS 'enrich_exhausted' AFTER 'enrich_failed';

ALTER TABLE law_entries ADD COLUMN harvest_fail_count INTEGER NOT NULL DEFAULT 0;
ALTER TABLE law_entries ADD COLUMN enrich_fail_count INTEGER NOT NULL DEFAULT 0;
