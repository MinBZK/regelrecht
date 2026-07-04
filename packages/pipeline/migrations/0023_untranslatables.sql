-- Captures RFC-012 "untranslatables": legal constructs the enrichment agent
-- could not express with the engine's current operation set. One row per
-- (law, provider, article, construct). Refreshed wholesale per (law_id,
-- provider) on each successful enrich completion (delete-and-replace), so the
-- table always reflects the latest enrich run for that provider.
CREATE TABLE untranslatables (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    law_id              TEXT NOT NULL,
    -- Enrich job that produced this capture. ON DELETE CASCADE so purging a
    -- job (or the whole jobs table in tests) cleans these up automatically.
    enrich_job_id       UUID NOT NULL REFERENCES jobs(id) ON DELETE CASCADE,
    provider            TEXT NOT NULL,
    article             TEXT NOT NULL,
    construct           TEXT NOT NULL,
    reason              TEXT NOT NULL,
    suggestion          TEXT,
    legal_text_excerpt  TEXT,
    accepted            BOOLEAN NOT NULL DEFAULT FALSE,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_untranslatables_law_id ON untranslatables (law_id);
CREATE INDEX idx_untranslatables_construct ON untranslatables (construct);
CREATE INDEX idx_untranslatables_accepted ON untranslatables (accepted);
