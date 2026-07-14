-- De actieve-enrich-uniciteit krijgt een traject-dimensie: een corpus-brede
-- enrich-job (traject_ref NULL) en een traject-taak-aanvraag voor dezelfde
-- wet+provider mogen naast elkaar bestaan, en trajecten blokkeren elkaar
-- niet. Binnen één scope (zelfde traject, of beide corpus-breed) blijft een
-- tweede actieve job een duplicaat. COALESCE omdat NULL's in een unique
-- index anders nooit botsen.
DROP INDEX idx_unique_active_enrich_job;
CREATE UNIQUE INDEX idx_unique_active_enrich_job
    ON jobs (law_id, job_type, (payload->>'provider'), COALESCE(traject_ref, ''))
    WHERE job_type = 'enrich' AND status IN ('pending', 'processing');
