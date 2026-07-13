-- Persoonlijke review-taken voor async jobs (spec: taken-mechanisme).
-- Een taak koppelt een afgeronde job aan een account dat het resultaat moet
-- beoordelen. De taakrij blijft na afhandeling bestaan (audit-spoor); alleen
-- de bijbehorende job_blobs worden opgeruimd.
CREATE TYPE task_status AS ENUM ('open', 'approved', 'rejected', 'dismissed');

CREATE TABLE tasks (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    task_type           TEXT NOT NULL           -- 'job_review' | 'job_failed'
                        CHECK (task_type IN ('job_review', 'job_failed')),
    status              task_status NOT NULL DEFAULT 'open',
    -- NULL = (toekomst) taak voor het hele traject i.p.v. één persoon.
    assignee_account_id UUID REFERENCES accounts(id) ON DELETE SET NULL,
    traject_id          UUID REFERENCES trajects(id) ON DELETE SET NULL,
    job_id              UUID REFERENCES jobs(id) ON DELETE SET NULL,
    title               TEXT NOT NULL,
    payload             JSONB,                  -- law_id, traject_ref, source_etag, provider, error
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    resolved_at         TIMESTAMPTZ,
    resolved_by         UUID REFERENCES accounts(id) ON DELETE SET NULL
);

-- De takenlijst-query filtert op assignee + open; de rest is klein.
CREATE INDEX idx_tasks_open_assignee ON tasks (assignee_account_id) WHERE status = 'open';

-- De blob-GC checkt per job of er nog een open taak is; partial op 'open'
-- zodat de index klein blijft terwijl de audit-trail groeit.
CREATE INDEX idx_tasks_job_open ON tasks (job_id) WHERE status = 'open';

-- Transiënte input/result-bestanden van een taak-flow-job (patroon:
-- document_uploads). Bewust geen FK naar jobs: de rijen worden door de
-- worker/resolve opgeruimd, niet door cascade; een GC vangt wezen af.
CREATE TABLE job_blobs (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    job_id     UUID NOT NULL,
    kind       TEXT NOT NULL                    -- 'input' | 'result'
               CHECK (kind IN ('input', 'result')),
    path       TEXT NOT NULL,                   -- repo-relatief pad van het bestand
    content    TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_job_blobs_job ON job_blobs (job_id);
