-- no-transaction
-- Add the `document_convert` job type: an uploaded PDF/Word document is
-- converted to markdown by the LLM agent (same subprocess machinery as
-- enrich) and written back as a `.md` werkdocument in the traject.
--
-- `ALTER TYPE ... ADD VALUE` cannot run inside a transaction block (Postgres
-- error 25001), so this migration is marked `-- no-transaction` (must be the
-- first line — sqlx reads that directive) and lives in its own file, ahead of
-- any migration/code that references 'document_convert'.
ALTER TYPE job_type ADD VALUE IF NOT EXISTS 'document_convert';
