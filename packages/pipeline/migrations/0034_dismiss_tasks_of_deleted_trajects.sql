-- Eenmalige opruiming van open taken die een traject-verwijdering hebben
-- overleefd.
--
-- `tasks.traject_id` is `ON DELETE SET NULL` (0028), dus een verwijderd traject
-- liet zijn open taken staan met alleen de koppeling doorgeknipt. Taken krijgen
-- bij aanmaak altijd een traject mee (elke `create_task`-aanroep zit in een
-- traject-gebonden taak-flow, en de aanvraag-endpoints resolven het traject
-- voordat de job in de queue gaat), dus een open taak met `traject_id IS NULL`
-- is per constructie zo'n wees. Beoordelen kan niet meer — `payload.traject_ref`
-- wijst naar een traject dat er niet meer is — terwijl de taak wel in de
-- account-brede takenlijst blijft staan.
--
-- De taakrij blijft bestaan als audit-spoor (patroon 0028); alleen de status
-- gaat dicht. `resolved_by` blijft leeg: niemand heeft de taak beoordeeld.
UPDATE tasks
SET status = 'dismissed',
    resolved_at = now()
WHERE status = 'open'
  AND traject_id IS NULL;

-- De result/input-blobs van de bijbehorende jobs zijn daarmee wees. Zelfde
-- voorwaarde als `delete_blobs_for_finished_job`: alleen opruimen als er op die
-- job geen open taak meer staat. Beperkt tot jobs die zo'n verweesde taak
-- hadden — een blanco opruiming zou ook de input-blobs van nog lopende jobs
-- wissen, want die krijgen hun taak pas bij afronding.
DELETE FROM job_blobs jb
WHERE EXISTS (
    SELECT 1 FROM tasks t
    WHERE t.job_id = jb.job_id AND t.traject_id IS NULL
)
  AND NOT EXISTS (
    SELECT 1 FROM tasks t
    WHERE t.job_id = jb.job_id AND t.status = 'open'
);
