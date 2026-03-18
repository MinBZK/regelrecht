-- Add progress column for live phase tracking of long-running enrich jobs.
-- Stores arbitrary JSON so the worker can relay LLM-reported phase info.
ALTER TABLE jobs ADD COLUMN progress JSONB;
