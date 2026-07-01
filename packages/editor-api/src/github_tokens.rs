//! Storage for per-user GitHub OAuth tokens (encrypted at rest).
//!
//! One row per linked account in `github_user_tokens`. The access/refresh
//! tokens are sealed with [`TokenCipher`] before they touch the database, and
//! only ever decrypted in-process when a write needs to authenticate to
//! GitHub *as the user*. Expiry is evaluated in SQL (`expires_at <= now()`)
//! so the code never has to map a `TIMESTAMPTZ` into a Rust time type — the
//! editor-api's sqlx build intentionally has no `time`/`chrono` feature.

use sqlx::PgPool;
use uuid::Uuid;

use crate::crypto::TokenCipher;

/// Raw column tuple for a stored-token row: `(access_enc, refresh_enc,
/// github_login, scopes, expired)`. Named to keep `query_as` readable and
/// satisfy `clippy::type_complexity`.
type TokenRow = (Vec<u8>, Option<Vec<u8>>, String, String, bool);

/// A decrypted, ready-to-use token plus the metadata a caller needs to decide
/// whether it's still usable.
pub struct StoredToken {
    pub access_token: String,
    #[allow(dead_code)] // stored for the (out-of-scope) refresh flow; not read yet.
    pub refresh_token: Option<String>,
    pub github_login: String,
    #[allow(dead_code)] // surfaced via get_status(); kept on the row model here too.
    pub scopes: String,
    /// `true` when the provider gave an expiry that has now passed. Classic
    /// OAuth `repo` tokens never expire, so this stays `false` for them.
    pub expired: bool,
}

/// The non-secret link status surfaced to the frontend (`/auth/github/status`).
pub struct LinkStatus {
    pub github_login: String,
    pub scopes: String,
    pub expired: bool,
}

/// Insert or replace the stored token for `account_id`.
///
/// `expires_in_secs` is the provider's `expires_in` (seconds from now), or
/// `None` for a non-expiring token; it's turned into an absolute `expires_at`
/// in SQL so the stored value is comparable against `now()` on read.
#[allow(clippy::too_many_arguments)]
pub async fn upsert(
    pool: &PgPool,
    cipher: &TokenCipher,
    account_id: Uuid,
    access_token: &str,
    refresh_token: Option<&str>,
    expires_in_secs: Option<i64>,
    github_login: &str,
    scopes: &str,
) -> Result<(), String> {
    let access_enc = cipher.encrypt(access_token)?;
    let refresh_enc = match refresh_token {
        Some(rt) => Some(cipher.encrypt(rt)?),
        None => None,
    };

    sqlx::query(
        "INSERT INTO github_user_tokens
             (account_id, access_token_enc, refresh_token_enc, expires_at, github_login, scopes, updated_at)
         VALUES ($1, $2, $3,
                 CASE WHEN $4::bigint IS NULL THEN NULL
                      ELSE now() + ($4::bigint * interval '1 second') END,
                 $5, $6, now())
         ON CONFLICT (account_id) DO UPDATE
            SET access_token_enc  = EXCLUDED.access_token_enc,
                refresh_token_enc = EXCLUDED.refresh_token_enc,
                expires_at        = EXCLUDED.expires_at,
                github_login      = EXCLUDED.github_login,
                scopes            = EXCLUDED.scopes,
                updated_at        = now()",
    )
    .bind(account_id)
    .bind(access_enc)
    .bind(refresh_enc)
    .bind(expires_in_secs)
    .bind(github_login)
    .bind(scopes)
    .execute(pool)
    .await
    .map_err(|e| format!("failed to store github token: {e}"))?;
    Ok(())
}

/// Non-secret link status for the UI. Does not decrypt the token.
pub async fn get_status(pool: &PgPool, account_id: Uuid) -> Result<Option<LinkStatus>, String> {
    let row: Option<(String, String, bool)> = sqlx::query_as(
        "SELECT github_login, scopes,
                (expires_at IS NOT NULL AND expires_at <= now()) AS expired
           FROM github_user_tokens WHERE account_id = $1",
    )
    .bind(account_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("failed to read github token status: {e}"))?;
    Ok(row.map(|(github_login, scopes, expired)| LinkStatus {
        github_login,
        scopes,
        expired,
    }))
}

/// Fetch and decrypt the stored token for use on a write. Returns `None` when
/// the account has no linked token.
pub async fn get_token(
    pool: &PgPool,
    cipher: &TokenCipher,
    account_id: Uuid,
) -> Result<Option<StoredToken>, String> {
    let row: Option<TokenRow> = sqlx::query_as(
        "SELECT access_token_enc, refresh_token_enc, github_login, scopes,
                (expires_at IS NOT NULL AND expires_at <= now()) AS expired
           FROM github_user_tokens WHERE account_id = $1",
    )
    .bind(account_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("failed to read github token: {e}"))?;

    let Some((access_enc, refresh_enc, github_login, scopes, expired)) = row else {
        return Ok(None);
    };
    let access_token = cipher.decrypt(&access_enc)?;
    let refresh_token = match refresh_enc {
        Some(blob) => Some(cipher.decrypt(&blob)?),
        None => None,
    };
    Ok(Some(StoredToken {
        access_token,
        refresh_token,
        github_login,
        scopes,
        expired,
    }))
}

/// Remove the stored token for `account_id` (disconnect). Idempotent.
pub async fn delete(pool: &PgPool, account_id: Uuid) -> Result<(), String> {
    sqlx::query("DELETE FROM github_user_tokens WHERE account_id = $1")
        .bind(account_id)
        .execute(pool)
        .await
        .map_err(|e| format!("failed to delete github token: {e}"))?;
    Ok(())
}
