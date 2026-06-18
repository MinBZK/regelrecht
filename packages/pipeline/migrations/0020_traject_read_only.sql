-- Read-only trajecten. A local-test-corpus traject (created via the
-- reserved `local`/`test-corpus` repo sentinel) reads the local corpus
-- but must never accept writes — saving a local law would only edit the
-- ephemeral baked-in container copy. The save handlers reject writes
-- when this flag is set. Normal trajecten are writable (default FALSE).
ALTER TABLE trajects
    ADD COLUMN read_only BOOLEAN NOT NULL DEFAULT FALSE;
