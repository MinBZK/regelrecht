-- Fix: re-queue the Zorgtoeslag harvest job with a valid date.
-- The original seed job had no date, causing the harvester to default to
-- today's date which may not exist in the law repository.
UPDATE jobs
SET status = 'pending',
    attempts = 0,
    payload = '{"bwb_id": "BWBR0018451", "date": "2026-01-01"}'::jsonb,
    updated_at = now()
WHERE law_id = 'BWBR0018451'
  AND job_type = 'harvest';

UPDATE law_entries
SET status = 'queued',
    updated_at = now()
WHERE law_id = 'BWBR0018451';
