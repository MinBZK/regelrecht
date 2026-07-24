//! Error type for the GitHub REST client.
//!
//! Deliberately free of `reqwest` types in its public surface: callers see a
//! plain `status: u16` instead of a `reqwest::StatusCode`, so swapping the
//! transport later (or bumping reqwest across a semver break) never ripples
//! into downstream match arms. The corpus crate maps these onto its own
//! `CorpusError`; the editor maps `RepoAccessError` (a separate, preflight-
//! specific enum in `repo_access`) onto HTTP statuses.

use thiserror::Error;

/// Everything the client can fail with. Statuses stay in the `Display`
/// message (`"403"`, `"conflict"`) because downstream error-mapping and a
/// couple of tests match on those substrings.
#[derive(Debug, Error)]
pub enum GithubError {
    /// Transport-level failure (DNS, TLS, timeout, connection reset) — the
    /// request never produced an HTTP response. Worth retrying / surfacing
    /// as a 503 upstream.
    #[error("transport error talking to GitHub: {0}")]
    Transport(String),

    /// A non-success HTTP response we don't special-case. `status` is the
    /// raw HTTP code (kept as `u16` so no `reqwest` type leaks); `message`
    /// carries the call context plus a short slice of the response body for
    /// operator logs — never a token.
    #[error("GitHub API error {status}: {message}")]
    Api { status: u16, message: String },

    /// Optimistic-concurrency conflict on a write (HTTP 409 — the file's
    /// `sha` moved between the read and the PUT/DELETE). Surfaced separately
    /// so a caller can refresh the sha and retry without parsing strings.
    #[error("conflict: {0}")]
    Conflict(String),

    /// GitHub refused a write with a 403 that is a genuine permission
    /// refusal (no push access for the authenticating identity), as opposed
    /// to a transient rate-limit 403. Kept distinct so callers can translate
    /// it into a clear "not allowed" message. Carries the response text for
    /// operator logging — never show it to end users verbatim.
    #[error("write denied by GitHub (403): {0}")]
    WriteDenied(String),

    /// The supplied token contains bytes that aren't valid in an HTTP header
    /// value (BOM, CR/LF, non-ASCII). We refuse up-front rather than drop the
    /// `Authorization` header silently — an unauthenticated request would
    /// surface as a misleading 401 and send the operator chasing "GitHub
    /// rejected the token" while the real cause is a corrupt env var.
    #[error(
        "token contains characters not valid in an HTTP header value \
             — check the env var for whitespace/BOM/non-ASCII: {0}"
    )]
    InvalidToken(String),

    /// Failed to decode a response body (JSON parse, base64, UTF-8).
    #[error("failed to decode GitHub response: {0}")]
    Decode(String),

    /// Client construction / configuration failure (e.g. the reqwest client
    /// builder failed).
    #[error("configuration error: {0}")]
    Config(String),
}

/// Convenience alias for fallible client operations.
pub type Result<T> = std::result::Result<T, GithubError>;
