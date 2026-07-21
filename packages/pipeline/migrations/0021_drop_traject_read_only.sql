-- Drop the `read_only` flag. The `local`/`test-corpus` read-only sentinel
-- that introduced it (migration 0020) has been removed: editing the laws in
-- the regelrecht monorepo now happens through a normal writable GitHub
-- traject pointed at MinBZK/regelrecht (path corpus/regulation/nl), which
-- saves via a session branch + PR. No traject is read-only anymore, so the
-- column is dead.
ALTER TABLE trajects
    DROP COLUMN read_only;
