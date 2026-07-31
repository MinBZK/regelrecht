-- Statusgedreven lookups die tot nu toe op een seq scan uitkwamen (audit #477,
-- sectie Database). Twee indexen, elk gevormd naar de expressie die de query
-- werkelijk gebruikt, niet naar de kolom zoals die in de audit staat.
--
-- De audit noemt ook `law_entries.status`. Die index staat hier bewust niet in:
-- de admin-API filtert met `status::text = $1` (`handlers.rs:150,169`), en een
-- index op `((status::text))` wordt door PostgreSQL geweigerd omdat `enum_out`
-- als STABLE is geregistreerd en niet als IMMUTABLE. Zolang de query op de cast
-- vergelijkt is die kolom niet te indexeren. Zie de issue over het weghalen van
-- die cast.
--
-- Geen CREATE INDEX CONCURRENTLY. Dat kan alleen buiten een transactieblok, en
-- sqlx draait elke migratie in een transactie tenzij het bestand met
-- `-- no-transaction` begint (zie 0025/0030/0031). Die route is hier slechter:
-- een afgebroken CONCURRENTLY-build laat een INVALID index achter die door de
-- `IF NOT EXISTS` bij de volgende start wordt overgeslagen en dus permanent
-- ongebruikt blijft. Een niet-concurrent CREATE INDEX neemt een SHARE-lock:
-- lezers draaien door, schrijvers wachten. De advisory lock in `ensure_schema`
-- serialiseert alleen migrerende starters en houdt draaiende workers niet
-- tegen, dus die wachten kort. Bij deze tabelgroottes is dat acceptabel.

-- 1. jobs.started_at voor draaiende jobs — `reap_orphaned_jobs` doet
--    `WHERE status = 'processing' AND started_at < now() - $1` op een eigen
--    interval-taak. Het aantal processing-rijen is klein en constant, dus een
--    partiële index (huisstijl van `idx_jobs_queue`) houdt de scan bij die
--    handvol rijen in plaats van bij de hele, groeiende jobs-tabel.
CREATE INDEX IF NOT EXISTS idx_jobs_processing_started_at
    ON jobs (started_at)
    WHERE status = 'processing';

-- 2. Gefaalde jobs op hun afrondmoment — de dashboard-query
--    `recent_failures` doet `WHERE status = 'failed'
--    ORDER BY COALESCE(completed_at, created_at) DESC LIMIT 50`. De audit
--    noemt `completed_at`, maar een index op die kale kolom bedient die
--    ORDER BY niet: gereapte en gefaalde jobs kunnen `completed_at` NULL
--    hebben en de query sorteert daarom op de COALESCE-expressie. Deze index
--    volgt de expressie én de sorteerrichting, zodat de top-50 uit de index
--    komt; de partiële predicate beperkt daarnaast `metrics::recently_failed`
--    (aggregatie over alleen de failed-rijen) tot dezelfde deelverzameling.
CREATE INDEX IF NOT EXISTS idx_jobs_failed_completed_at
    ON jobs (COALESCE(completed_at, created_at) DESC)
    WHERE status = 'failed';
