-- no-transaction
-- Nieuw jobtype voor een traject-scoped harvest: haal een wet uit BWB op en
-- keten er een taak-flow-enrich aan (zoals law_convert), zonder de centrale
-- corpus-repo aan te raken. ALTER TYPE ... ADD VALUE kan niet in een
-- transactieblok, vandaar de no-transaction-marker (zelfde patroon als
-- 0025_document_convert_job_type.sql en 0030_law_convert_job_type.sql).
ALTER TYPE job_type ADD VALUE IF NOT EXISTS 'traject_harvest';
