-- Seed: harvest job for Wet op de zorgtoeslag (real BWB ID)
INSERT INTO law_entries (law_id, status)
VALUES ('BWBR0018451', 'queued')
ON CONFLICT (law_id) DO NOTHING;

INSERT INTO jobs (job_type, law_id, priority, payload)
VALUES (
    'harvest',
    'BWBR0018451',
    50,
    '{"bwb_id": "BWBR0018451", "date": "2026-01-01"}'::jsonb
);
