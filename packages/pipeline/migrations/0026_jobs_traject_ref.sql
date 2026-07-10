-- Generic job<->traject association. Nullable because the existing
-- corpus-wide harvest/enrich jobs have no owning traject; only jobs that are
-- scoped to a single traject (starting with document_convert) set it. This is
-- the shared foundation that per-traject harvest/enrich will also build on.
ALTER TABLE jobs ADD COLUMN traject_ref TEXT;

-- Partial index: the werkdocumenten status query filters jobs by traject_ref;
-- the vast majority of rows (harvest/enrich) have it NULL, so index only the
-- populated ones.
CREATE INDEX idx_jobs_traject ON jobs (traject_ref) WHERE traject_ref IS NOT NULL;
