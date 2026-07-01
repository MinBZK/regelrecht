-- Per-user GitHub OAuth tokens.
--
-- Spike (see PR): authenticate traject writes to GitHub *as the acting user*
-- via a GitHub OAuth App, instead of a service/App token that can reach every
-- repo. One row per linked account. Tokens are stored **encrypted at rest**
-- (nonce || ciphertext, ChaCha20-Poly1305 with a key from GITHUB_TOKEN_ENC_KEY)
-- because ZAD offers no keyvault — the DB column must never hold a plaintext
-- credential.
CREATE TABLE IF NOT EXISTS github_user_tokens (
    account_id      UUID PRIMARY KEY REFERENCES accounts (id) ON DELETE CASCADE,
    -- ChaCha20-Poly1305 sealed blobs (12-byte nonce prefix + ciphertext+tag).
    access_token_enc  BYTEA NOT NULL,
    refresh_token_enc BYTEA,
    -- Absolute expiry of the access token, when the provider returns one.
    -- Classic OAuth App `repo` tokens don't expire, so this is nullable.
    expires_at      TIMESTAMPTZ,
    -- GitHub login (handle) the token belongs to — shown in the UI so the
    -- user can confirm which account is linked, never used for authorization.
    github_login    TEXT NOT NULL,
    -- Space-separated granted scopes, as reported by the token endpoint.
    scopes          TEXT NOT NULL DEFAULT '',
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);
