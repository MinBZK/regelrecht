-- Fix harvest job results that stored absolute file_path values.
-- Strip the '/tmp/corpus-repo/' prefix so enrich jobs can use them.
UPDATE jobs
SET result = jsonb_set(
    result,
    '{file_path}',
    to_jsonb(regexp_replace(result->>'file_path', '^/tmp/corpus-repo/', ''))
)
WHERE job_type = 'harvest'
  AND status = 'completed'
  AND result->>'file_path' LIKE '/tmp/%';
