-- Traject user management — invite-by-email with pending invites.
--
-- Two changes:
-- 1. Rename traject_role enum values from Dutch (beheerder/lid) to English
--    (owner/contributor). Future roles (reviewer, …) will be added here.
--    ALTER TYPE … RENAME VALUE is in-place: existing traject_members rows
--    are not rewritten, only the label changes.
-- 2. Add traject_invites to hold invitations for emails that don't yet
--    have an accounts row. account_middleware promotes matching invites
--    into traject_members on the next authenticated request from the
--    invited email, so onboarding new collaborators no longer requires
--    them to have logged in before being invited.

ALTER TYPE traject_role RENAME VALUE 'beheerder'  TO 'owner';
ALTER TYPE traject_role RENAME VALUE 'lid'        TO 'contributor';

CREATE TABLE traject_invites (
    traject_id  UUID         NOT NULL REFERENCES trajects(id) ON DELETE CASCADE,
    -- Stored lowercase by the API so the (traject_id, email) PK collides
    -- on case variants of the same address. The promotion query compares
    -- against the user's lowercased email claim.
    email       TEXT         NOT NULL,
    role        traject_role NOT NULL,
    invited_by  UUID         NOT NULL REFERENCES accounts(id),
    invited_at  TIMESTAMPTZ  NOT NULL DEFAULT now(),
    PRIMARY KEY (traject_id, email)
);

-- account_middleware promotes invites by exact email match on every
-- authenticated request; the index keeps the common (empty-result) path
-- sub-millisecond.
CREATE INDEX idx_traject_invites_email ON traject_invites(email);
