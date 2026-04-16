-- The composite PK (person_sub, law_id) already covers lookups by person_sub,
-- making this standalone index redundant.
DROP INDEX IF EXISTS idx_user_favorites_person;
