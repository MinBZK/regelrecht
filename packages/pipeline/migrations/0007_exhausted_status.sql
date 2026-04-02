-- ALTER TYPE ... ADD VALUE cannot run inside a transaction block.
-- @see https://www.postgresql.org/docs/current/sql-altertype.html
-- no-transaction

ALTER TYPE law_status ADD VALUE IF NOT EXISTS 'harvest_exhausted' AFTER 'harvest_failed';
ALTER TYPE law_status ADD VALUE IF NOT EXISTS 'enrich_exhausted' AFTER 'enrich_failed';
