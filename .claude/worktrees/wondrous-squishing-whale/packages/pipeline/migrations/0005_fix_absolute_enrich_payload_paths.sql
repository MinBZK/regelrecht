-- Fix enrich job payloads that have absolute yaml_path values.
-- The enrich worker reads yaml_path from its payload, and rejects
-- absolute paths. Strip the '/tmp/corpus-repo/' prefix.
UPDATE jobs
SET payload = jsonb_set(
    payload,
    '{yaml_path}',
    to_jsonb(regexp_replace(payload->>'yaml_path', '^/tmp/corpus-repo/', ''))
)
WHERE job_type = 'enrich'
  AND payload->>'yaml_path' LIKE '/tmp/%';
