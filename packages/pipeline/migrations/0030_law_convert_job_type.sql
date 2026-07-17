-- no-transaction
-- Nieuw jobtype voor het omzetten van een geüpload document naar een
-- basis-wet-YAML (geharveste wet zonder machine_readable). ALTER TYPE ...
-- ADD VALUE kan niet in een transactieblok, vandaar de no-transaction-marker
-- (zelfde patroon als 0025_document_convert_job_type.sql).
ALTER TYPE job_type ADD VALUE IF NOT EXISTS 'law_convert';
