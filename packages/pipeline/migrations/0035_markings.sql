-- Markings (schema v0.7.0): constructies die het formaat zelf niet kan
-- uitdrukken. Eén rij per (wet, provider, artikel, constructie), net als de
-- `untranslatables`-tabel die dit vervangt, en op dezelfde manier ververst:
-- delete-and-replace per (law_id, provider) bij het afronden van een enrichrun.
--
-- Waarom een eigen tabel en geen kolommen erbij in `untranslatables`: die tabel
-- heeft `reason TEXT NOT NULL` en `construct TEXT NOT NULL`, en een marking
-- heeft daar geen equivalent van dat je mag invullen. Een marking in die vorm
-- persen zou betekenen dat je tekst verzint die een jurist moet vertrouwen, en
-- dat is precies wat dit project niet doet. De oude tabel blijft bestaan zolang
-- er wetten op schema v0.5.x zijn; de engine leest beide kanalen.
--
-- De velden volgen `CapturedMarking` in packages/pipeline/src/enrich.rs, niet
-- het schema. Dat scheelt: het schema eist `reason` en `resolved_by`, maar de
-- vangststructuur draagt geen `reason` en heeft `resolved_by` als Option. Deze
-- tabel spiegelt wat er werkelijk gevangen wordt, want een kolom NOT NULL maken
-- die de bron niet altijd vult levert een insert die faalt op geldige data.
--
-- `target` is een lijst die leeg mag zijn, en die leegte is een bewering ("dit
-- artikel blijft uitvoerbaar"), geen ontbrekende waarde, dus NOT NULL met een
-- lege array als default.
CREATE TABLE markings (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    law_id              TEXT NOT NULL,
    -- Enrichjob die deze vangst opleverde. ON DELETE CASCADE zodat het
    -- opruimen van een job (of de hele jobs-tabel in tests) deze rijen
    -- meeneemt, gelijk aan `untranslatables`.
    enrich_job_id       UUID NOT NULL REFERENCES jobs(id) ON DELETE CASCADE,
    provider            TEXT NOT NULL,
    article             TEXT NOT NULL,
    -- De constructie die niet past, in de woorden van het artikel zelf.
    about               TEXT NOT NULL,
    -- 'operation' (de bewerking moet gebouwd) of 'model' (het formaat heeft er
    -- geen vorm voor). Gesloten vocabulaire in het schema, dus ook hier.
    resolution          TEXT NOT NULL
                        CHECK (resolution IN ('operation', 'model')),
    -- De wijziging die het zou oplossen, concreet genoeg om werk te worden.
    -- Nullable omdat `CapturedMarking.resolved_by` een Option is.
    resolved_by         TEXT,
    -- De uitvoer die dit artikel niet kan produceren. Leeg = het artikel
    -- blijft uitvoerbaar.
    target              TEXT[] NOT NULL DEFAULT '{}',
    legal_text_excerpt  TEXT NOT NULL,
    accepted            BOOLEAN NOT NULL DEFAULT FALSE,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_markings_law_id ON markings (law_id);
CREATE INDEX idx_markings_accepted ON markings (accepted);

-- De roadmapvraag is "welke ontbrekende bewerking blokkeert de meeste
-- artikelen": een telling over resolution + resolved_by. Die twee samen in de
-- index, want de weergave groepeert op resolution en toont resolved_by erbij.
CREATE INDEX idx_markings_resolution ON markings (resolution, resolved_by);
