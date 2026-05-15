-- Traject-concept: projecten waarin gebruikers samenwerken aan wetwijzigingen.
-- Elk traject heeft een eigen federatieve corpus-config (DB-spiegeling van
-- corpus-registry.yaml) waarin exact één source de schrijfbare eigen-source is.

-- Stabiele identiteit per gebruiker, geupsert bij OIDC-login op person_sub.
-- `email` is UNIQUE so that `add_member` (invite-by-email) can resolve the
-- target deterministically; if a future OIDC email change ever collides
-- with another account's email the login UPSERT will fail loudly rather
-- than silently let two rows share the same address.
CREATE TABLE accounts (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    person_sub  TEXT NOT NULL UNIQUE,
    email       TEXT NOT NULL UNIQUE,
    name        TEXT NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TRIGGER trg_accounts_updated_at
    BEFORE UPDATE ON accounts
    FOR EACH ROW EXECUTE FUNCTION update_updated_at();

CREATE TYPE traject_status AS ENUM ('bezig', 'afgerond');

CREATE TABLE trajects (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name         TEXT NOT NULL,
    description  TEXT NOT NULL DEFAULT '',
    scope        TEXT NOT NULL DEFAULT '',
    status       traject_status NOT NULL DEFAULT 'bezig',
    created_by   UUID NOT NULL REFERENCES accounts(id),
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TRIGGER trg_trajects_updated_at
    BEFORE UPDATE ON trajects
    FOR EACH ROW EXECUTE FUNCTION update_updated_at();

CREATE TYPE traject_role AS ENUM ('beheerder', 'lid');

CREATE TABLE traject_members (
    traject_id  UUID NOT NULL REFERENCES trajects(id) ON DELETE CASCADE,
    account_id  UUID NOT NULL REFERENCES accounts(id),
    role        traject_role NOT NULL,
    added_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (traject_id, account_id)
);

CREATE INDEX idx_traject_members_account ON traject_members(account_id);

-- Federatieve corpus-config per traject. Spiegelt het Source-model uit
-- packages/corpus/src/models.rs: een Source is óf Local (local_path) óf
-- GitHub (gh_owner/gh_repo/gh_branch, optioneel gh_path en gh_ref).
CREATE TYPE corpus_source_type AS ENUM ('local', 'github');

CREATE TABLE traject_corpus_sources (
    id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    traject_id       UUID NOT NULL REFERENCES trajects(id) ON DELETE CASCADE,
    source_id        TEXT NOT NULL,
    name             TEXT NOT NULL,
    source_type      corpus_source_type NOT NULL,
    gh_owner         TEXT,
    gh_repo          TEXT,
    gh_branch        TEXT,
    gh_path          TEXT,
    gh_ref           TEXT,
    local_path       TEXT,
    priority         INTEGER NOT NULL,
    auth_ref         TEXT,
    scopes           JSONB NOT NULL DEFAULT '[]'::jsonb,
    is_writable_own  BOOLEAN NOT NULL DEFAULT FALSE,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (traject_id, source_id),
    CHECK (source_type <> 'github' OR (gh_owner IS NOT NULL AND gh_repo IS NOT NULL AND gh_branch IS NOT NULL)),
    CHECK (source_type <> 'local'  OR local_path IS NOT NULL)
);

-- Exact één schrijfbare eigen-source per traject.
CREATE UNIQUE INDEX idx_traject_one_writable_own
    ON traject_corpus_sources(traject_id)
    WHERE is_writable_own = TRUE;
