-- Add slug column to law_entries for reverse lookup (slug → bwb_id).
-- The editor knows laws by their slug ($id like "participatiewet") but the
-- harvester needs the BWB ID. This column bridges that gap.
ALTER TABLE law_entries ADD COLUMN slug TEXT;
CREATE UNIQUE INDEX idx_law_entries_slug ON law_entries (slug) WHERE slug IS NOT NULL;
