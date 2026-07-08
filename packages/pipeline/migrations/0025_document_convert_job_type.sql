-- Add the `document_convert` job type: an uploaded PDF/Word document is
-- converted to markdown by the LLM agent (same subprocess machinery as
-- enrich) and written back as a `.md` werkdocument in the traject.
--
-- ALTER TYPE ... ADD VALUE cannot run in a transaction together with a
-- statement that *uses* the new value, so this lives in its own migration
-- file, ahead of any migration/code that references 'document_convert'.
ALTER TYPE job_type ADD VALUE IF NOT EXISTS 'document_convert';
