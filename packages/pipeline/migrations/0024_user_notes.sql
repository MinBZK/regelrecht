-- Persoonlijke notities: private per-user notes on a law, stored server-side
-- so they never leave the database (unlike traject annotations, which are
-- shared sidecar YAML in git — RFC-018 Decision 1).
--
-- The columns mirror the W3C Web Annotation model of RFC-005 / RFC-018
-- (schema/v0.5.2/annotation-schema.json): a note is an `Annotation` with a
-- `motivation` and a `TextualBody` (`value` + `format`, markdown by default).
-- The law-level target (`regelrecht://{law_id}`) is derived from `law_id`
-- when the row is serialized as an annotation.
CREATE TABLE user_notes (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    account_id   UUID NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    law_id       TEXT NOT NULL,
    motivation   TEXT NOT NULL DEFAULT 'commenting',
    body_value   TEXT NOT NULL,
    body_format  TEXT NOT NULL DEFAULT 'text/markdown',
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Every read is "this user's notes for this law"; the composite index also
-- serves the account-only lookups (cascade delete, future /api/me exports).
CREATE INDEX idx_user_notes_account_law ON user_notes (account_id, law_id);

CREATE TRIGGER trg_user_notes_updated_at
    BEFORE UPDATE ON user_notes
    FOR EACH ROW EXECUTE FUNCTION update_updated_at();
