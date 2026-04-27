-- Per-user settings for the editor (e.g. theme).
-- Key/value shape so new settings do not require a migration.
-- The PRIMARY KEY already serves person_sub-prefixed lookups, so no
-- separate single-column index is created — same scrub pattern as
-- 0009_drop_redundant_favorites_index.sql.
CREATE TABLE user_settings (
    person_sub  TEXT        NOT NULL,
    key         TEXT        NOT NULL,
    value       TEXT        NOT NULL,
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (person_sub, key)
);

CREATE TRIGGER trg_user_settings_updated_at
    BEFORE UPDATE ON user_settings
    FOR EACH ROW EXECUTE FUNCTION update_updated_at();
