-- Transient storage for the raw bytes of an uploaded document (PDF/Word)
-- while a `document_convert` job converts it to markdown. ZAD offers no
-- object/blob storage service, so the binary rides in Postgres (bytea, under
-- an app-enforced size cap). Rows are short-lived: the worker deletes the row
-- after a successful conversion or a terminal failure, so the table never
-- accumulates large blobs.
--
-- The owning job references the upload via `payload->>'upload_id'`; there is
-- deliberately no FK back to `jobs` (the job is created in the same request,
-- and the row is cleaned up by the worker, not by cascade).
CREATE TABLE document_uploads (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    traject_ref   TEXT        NOT NULL,
    filename      TEXT        NOT NULL,
    content_type  TEXT        NOT NULL,
    bytes         BYTEA       NOT NULL,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);
