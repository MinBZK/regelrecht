//! In-band diagnosis of a failed index scan of a traject's own repo.
//!
//! # Why here, and not in an admin screen
//!
//! From the outside these failure modes cannot be told apart: GitHub answers
//! 404 both for "this repository does not exist" and for "it is private and
//! your token cannot see it" (see [`RepoAccessError::RepoNotFound`]). An
//! operator looking in from outside has no token that can resolve that
//! ambiguity — and the more the platform leans on per-user tokens, the less
//! it ever will.
//!
//! The member's own request, however, carries exactly the token that can:
//! it is their repo. So the moment the scan fails, everything needed to
//! classify the cause sharply is already in hand. The diagnosis therefore
//! runs in-band, on the error path of the request that hit the problem.
//!
//! # Shape
//!
//! [`classify_index_failure`] probes GitHub and returns an
//! [`IndexFailureKind`] — a small, stable, closed set of causes.
//! [`index_failure_to_status`] turns that into the HTTP status and the Dutch
//! message the user reads. Splitting the two keeps the probing testable
//! against a mocked GitHub and keeps every user-facing sentence in one place.
//!
//! This is the *read* counterpart of `trajects::repo_access_error_to_status`,
//! which does the same job for the create-traject preflight. They are
//! deliberately separate: the write path talks to the person configuring a
//! repo ("your token has no push access"), the read path talks to a member
//! whose library will not open ("your traject has not been initialised yet").
//! Same underlying `RepoAccessError`, different audience, different remedy.
//!
//! # Cost
//!
//! Nothing here runs on the happy path. The probe costs two GitHub calls
//! (repo + base branch), a third when the traject branch has to be checked
//! or when a refusal has to be pinned down, and a fourth only when the
//! traject branch turns out to be absent — and all of that exclusively after
//! a scan has already failed.

use axum::http::StatusCode;
use regelrecht_github::{GithubClient, GithubError, RepoAccessError};
use uuid::Uuid;

/// Where the token used for the diagnosis came from — the same token the
/// failed scan used, so the classification describes the access path that
/// actually broke.
///
/// It decides whether "GitHub rejects this token" is something the *user*
/// can fix (re-link their account) or something only an operator can.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenOrigin {
    /// The caller's own linked GitHub account.
    User,
    /// A server-side token configured for this source.
    Server,
    /// No token at all was available.
    Absent,
}

impl TokenOrigin {
    /// Stable log value.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Server => "server",
            Self::Absent => "absent",
        }
    }
}

/// The writable-own source of a traject, as GitHub coordinates: enough to
/// ask GitHub every question the classification needs, plus the two ids
/// every log line on this path carries.
#[derive(Debug, Clone)]
pub struct OwnSourceTarget {
    pub traject_id: Uuid,
    pub source_id: String,
    pub owner: String,
    pub repo: String,
    /// The traject's own branch — the one the editor commits to. Read from
    /// the traject's stored source config, never re-derived, so the probe
    /// asks about the branch the scan actually used.
    pub branch: String,
    /// The branch the traject branch is cut from.
    pub base_branch: String,
}

impl OwnSourceTarget {
    /// `owner/name`, the form the Refs and Activity APIs take.
    pub fn full_repo(&self) -> String {
        format!("{}/{}", self.owner, self.repo)
    }
}

/// Why a traject's own index scan failed.
///
/// A closed set on purpose: the value is logged verbatim as the `kind`
/// field, so a fault can be looked up by *sort* across trajects as well as
/// by traject. Adding a variant is a deliberate act; falling into
/// [`Unknown`](Self::Unknown) is the explicit "we could not tell" bucket
/// rather than a silent catch-all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexFailureKind {
    /// Repo and base branch are fine, the traject branch simply does not
    /// exist yet and never did: a fresh traject that has not been written
    /// to. The most common cause by far.
    TrajectBranchMissing,
    /// The traject branch is gone *and* the repository's activity log
    /// records its deletion. Work that only lived there may be lost.
    TrajectBranchGone,
    /// The repository is not at the address the traject has stored:
    /// renamed, transferred or deleted.
    RepoUnavailable,
    /// The repository is readable but the branch the traject branches from
    /// no longer exists.
    BaseBranchMissing,
    /// GitHub rejects the token outright (revoked, expired, wrong).
    LinkRevoked,
    /// The token authenticates but does not carry enough access to this
    /// repository.
    InsufficientScope,
    /// GitHub could not be reached at all (DNS, TLS, timeout, reset).
    GithubUnreachable,
    /// There is no GitHub credential available for this source at all, so
    /// nothing can be probed and nothing can be read.
    NoCredential,
    /// Repo, base branch and traject branch all check out, or GitHub
    /// answered something we cannot place. The scan failed for a reason
    /// this classification does not cover.
    Unknown,
}

impl IndexFailureKind {
    /// Stable log value — the fixed vocabulary of the `kind` field.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::TrajectBranchMissing => "traject_branch_missing",
            Self::TrajectBranchGone => "traject_branch_gone",
            Self::RepoUnavailable => "repo_unavailable",
            Self::BaseBranchMissing => "base_branch_missing",
            Self::LinkRevoked => "link_revoked",
            Self::InsufficientScope => "insufficient_scope",
            Self::GithubUnreachable => "github_unreachable",
            Self::NoCredential => "no_credential",
            Self::Unknown => "unknown",
        }
    }
}

impl std::fmt::Display for IndexFailureKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Classify why the scan of `target` failed, using `token` — the very token
/// the scan used, so the probe walks the same access path.
///
/// Never returns an error: a diagnosis that cannot complete is itself a
/// classification ([`IndexFailureKind::Unknown`]). The caller is already on
/// an error path and must always be able to answer the user.
pub async fn classify_index_failure(
    client: &GithubClient,
    target: &OwnSourceTarget,
    token: Option<&str>,
) -> IndexFailureKind {
    let Some(token) = token else {
        // Nothing to probe with. A private repo is unreadable either way,
        // and probing anonymously would report a 404 as "repo gone" — a
        // wrong answer is worse than an honest one.
        return IndexFailureKind::NoCredential;
    };

    // Repo reachable? Base branch present? Token good enough? One helper,
    // already used by the create-traject preflight, answers all three.
    match client
        .validate_repo_access(&target.owner, &target.repo, &target.base_branch, token)
        .await
    {
        Ok(_) => classify_traject_branch(client, target, token).await,
        // Neither refusal is taken at face value — see [`Refusal`]: the
        // preflight answers in write-path terms and folds statuses that mean
        // very different things to a reader.
        Err(RepoAccessError::Unauthorized) => match probe_refusal(client, target, token).await {
            // GitHub refuses the credential itself. Re-linking is the fix, so
            // this — and only this — earns the connect flow.
            Refusal::Credential => IndexFailureKind::LinkRevoked,
            Refusal::Throttled | Refusal::Unreachable => IndexFailureKind::GithubUnreachable,
            Refusal::Access => IndexFailureKind::InsufficientScope,
            // The read went through, or failed on something unrelated: the
            // refusal is unexplained. Say so, rather than send the member off
            // to re-link a credential that may be perfectly fine.
            Refusal::Unclear => IndexFailureKind::Unknown,
        },
        Err(RepoAccessError::RepoNotFound) => IndexFailureKind::RepoUnavailable,
        Err(RepoAccessError::BranchNotFound) => IndexFailureKind::BaseBranchMissing,
        // The preflight raises this both for `permissions.push == false` and
        // for a 403 on the branch read. On the read path either means the
        // same thing to the user: this account does not have enough access
        // to this repo. Kept apart from `LinkRevoked` because the remedy
        // differs — re-linking the same account changes nothing here.
        Err(RepoAccessError::NoPushAccess) => match probe_refusal(client, target, token).await {
            Refusal::Throttled | Refusal::Unreachable => IndexFailureKind::GithubUnreachable,
            // `push == false` (the read itself goes through), a genuine 403
            // on the branch read, or a credential that fell over between the
            // two calls: for a reader they all say the same thing, and none
            // of them is the connect-flow case — the repo lookup that just
            // succeeded rules that out.
            Refusal::Credential | Refusal::Access | Refusal::Unclear => {
                IndexFailureKind::InsufficientScope
            }
        },
        Err(RepoAccessError::Transport(_)) => IndexFailureKind::GithubUnreachable,
        Err(RepoAccessError::Other(_)) => IndexFailureKind::Unknown,
    }
}

/// What GitHub says when the repository is read once more, with the raw
/// status kept instead of folded.
///
/// `validate_repo_access` is the create-traject preflight, and it answers in
/// write-path terms: [`RepoAccessError::Unauthorized`] covers a 401 *and* a
/// 403 on the repo lookup, [`RepoAccessError::NoPushAccess`] covers both
/// `permissions.push == false` and a 403 on the branch read. Harmless there —
/// each pair ends in one status. Not here: 401 means
/// [`IndexFailureKind::LinkRevoked`] → 428 → `apiAuthGuard.js` → connect
/// flow, while a 403 is also how GitHub reports a spent rate limit, an
/// ungranted SAML/SSO authorisation and an IP-allow-list block. Re-linking
/// fixes none of those, and a rate limit is not a permission problem at all —
/// it passes on its own.
///
/// So both refusals are re-asked through [`GithubClient::branch_exists`],
/// which preserves the status. Strictly on the error path, and only for the
/// two outcomes whose folding actually costs the member a wrong remedy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Refusal {
    /// 401 — GitHub refuses the credential itself.
    Credential,
    /// A rate limit, primary or secondary. Temporary, and nobody's fault.
    Throttled,
    /// 403 — authenticated, but this repository stays shut for this token.
    Access,
    /// GitHub could not be reached at all.
    Unreachable,
    /// The read went through, or failed on something else entirely.
    Unclear,
}

async fn probe_refusal(client: &GithubClient, target: &OwnSourceTarget, token: &str) -> Refusal {
    match client
        .branch_exists(&target.full_repo(), &target.base_branch, Some(token))
        .await
    {
        Err(GithubError::Api { status: 401, .. }) => Refusal::Credential,
        Err(GithubError::Api { status, message })
            if status == 429 || (status == 403 && is_rate_limited(&message)) =>
        {
            Refusal::Throttled
        }
        Err(GithubError::Api { status: 403, .. }) => Refusal::Access,
        Err(GithubError::Transport(_)) => Refusal::Unreachable,
        _ => Refusal::Unclear,
    }
}

/// Whether a GitHub error body is the rate limiter talking.
///
/// Both documented wordings — "API rate limit exceeded for …" (primary) and
/// "You have exceeded a secondary rate limit …" — carry this phrase. A miss
/// only costs precision: the answer lands on the neighbouring classification,
/// never on the connect flow.
fn is_rate_limited(message: &str) -> bool {
    message.to_ascii_lowercase().contains("rate limit")
}

/// Repo, base branch and access all check out — so the remaining question is
/// the traject's own branch.
async fn classify_traject_branch(
    client: &GithubClient,
    target: &OwnSourceTarget,
    token: &str,
) -> IndexFailureKind {
    let full_repo = target.full_repo();
    match client
        .branch_exists(&full_repo, &target.branch, Some(token))
        .await
    {
        // Everything the classification can check is healthy; whatever broke
        // the scan is outside this set.
        Ok(true) => IndexFailureKind::Unknown,
        Ok(false) => {
            // Absent branch: never created, or created and later deleted.
            // The Refs API cannot tell them apart (both are a 404), the
            // activity log can.
            match client
                .branch_deletion_recorded(&full_repo, &target.branch, Some(token))
                .await
            {
                Ok(true) => IndexFailureKind::TrajectBranchGone,
                Ok(false) => IndexFailureKind::TrajectBranchMissing,
                Err(e) => {
                    // Degrade to the far more common cause rather than
                    // claiming a data loss we could not confirm.
                    tracing::debug!(
                        traject = %target.traject_id,
                        source_id = %target.source_id,
                        error = %e,
                        "branch-deletion probe failed; reporting the traject branch as \
                         never created"
                    );
                    IndexFailureKind::TrajectBranchMissing
                }
            }
        }
        Err(GithubError::Transport(_)) => IndexFailureKind::GithubUnreachable,
        Err(_) => IndexFailureKind::Unknown,
    }
}

/// The HTTP status and the Dutch sentence a member reads for `kind`.
///
/// Every message says what is wrong *and* what to do about it, and none of
/// them falls back on "de gegevens konden niet worden opgehaald". They stay
/// short and carry no raw GitHub payload, so the frontend's length cap on
/// the index-error pane can never replace one — see `ID_BUDGET` for the one
/// part of a message whose length is not ours to choose.
///
/// On the statuses: **428 is reserved editor-wide for the GitHub connect
/// flow** (`apiAuthGuard.js` redirects every 428 on `/api/*` into it), so it
/// is used for exactly one case — a link that GitHub itself rejects, which
/// re-linking fixes. Notably *not* for [`IndexFailureKind::InsufficientScope`]:
/// re-linking the same account would land right back here, and a 428 would
/// make that an endless round trip.
pub fn index_failure_to_status(
    kind: IndexFailureKind,
    target: &OwnSourceTarget,
    origin: TokenOrigin,
) -> (StatusCode, String) {
    let repo = clamp_identifier(&target.full_repo());
    let branch = clamp_identifier(&target.branch);
    let base_branch = clamp_identifier(&target.base_branch);
    match kind {
        IndexFailureKind::TrajectBranchMissing => (
            StatusCode::CONFLICT,
            format!(
                "Dit traject is nog niet geïnitialiseerd: de branch '{branch}' bestaat nog niet \
                 in {repo}. Sla één wijziging op, dan wordt de branch aangemaakt en vult de \
                 bibliotheek zich."
            ),
        ),
        IndexFailureKind::TrajectBranchGone => (
            StatusCode::GONE,
            format!(
                "De branch '{branch}' van dit traject is verwijderd uit {repo}. Werk dat alleen \
                 daar stond, is mogelijk verloren. Laat een beheerder de branch herstellen \
                 voordat je verder werkt."
            ),
        ),
        // GitHub answers 404 for a repo that is not there *and* for one this
        // token may not see, and nothing in-band separates the two — so the
        // message names both, with the remedy for each. Claiming only the
        // rename would send a member who was quietly removed from the repo
        // to an administrator to fix an address that is perfectly correct.
        IndexFailureKind::RepoUnavailable => (
            StatusCode::NOT_FOUND,
            format!(
                "GitHub kent {repo} niet op het adres uit dit traject: hernoemd, overgedragen of \
                 verwijderd — of je hebt er geen toegang meer toe. Vraag toegang tot de repo, of \
                 laat een beheerder het adres nakijken."
            ),
        ),
        IndexFailureKind::BaseBranchMissing => (
            StatusCode::CONFLICT,
            format!(
                "De basisbranch '{base_branch}' van dit traject bestaat niet meer in {repo}. \
                 Laat een beheerder die branch herstellen of het traject op een bestaande \
                 basisbranch zetten."
            ),
        ),
        // The only 428 on this path, and only when the rejected token is the
        // caller's own: then the connect flow is the fix. A rejected server
        // token is nothing the member can re-link away.
        IndexFailureKind::LinkRevoked => match origin {
            TokenOrigin::User => (
                StatusCode::PRECONDITION_REQUIRED,
                format!(
                    "Je GitHub-koppeling wordt door GitHub geweigerd, waardoor {repo} niet \
                     gelezen kan worden. Koppel je GitHub-account opnieuw."
                ),
            ),
            TokenOrigin::Server | TokenOrigin::Absent => (
                StatusCode::BAD_GATEWAY,
                format!(
                    "Het GitHub-token van de beheerder wordt geweigerd, waardoor {repo} niet \
                     gelezen kan worden. Meld dit bij je beheerder."
                ),
            ),
        },
        // Split for the same reason as `LinkRevoked`, one step further: a
        // server token that is too narrow is not something the member can
        // re-link away either — a writable-at-rest source never consults a
        // personal link for reads, so pointing at the connect flow would be a
        // dead end. Same 403 either way; only the remedy differs.
        IndexFailureKind::InsufficientScope => match origin {
            TokenOrigin::User => (
                StatusCode::FORBIDDEN,
                format!(
                    "Je GitHub-toegang tot {repo} is niet toereikend voor dit traject. Vraag de \
                     eigenaar van de repo om toegang, of koppel het GitHub-account dat die \
                     toegang wel heeft."
                ),
            ),
            TokenOrigin::Server | TokenOrigin::Absent => (
                StatusCode::FORBIDDEN,
                format!(
                    "Het GitHub-token van de beheerder heeft niet genoeg toegang tot {repo} voor \
                     dit traject. Meld dit bij je beheerder; zelf opnieuw koppelen helpt hier \
                     niet."
                ),
            ),
        },
        IndexFailureKind::GithubUnreachable => (
            StatusCode::SERVICE_UNAVAILABLE,
            "GitHub is nu niet bereikbaar, waardoor de bibliotheek van dit traject niet geladen \
             kan worden. Probeer het over een paar minuten opnieuw."
                .to_string(),
        ),
        IndexFailureKind::NoCredential => (
            StatusCode::BAD_GATEWAY,
            format!(
                "Er is geen GitHub-toegang beschikbaar om {repo} te lezen. Koppel je \
                 GitHub-account, of laat een beheerder een token voor deze repo instellen."
            ),
        ),
        IndexFailureKind::Unknown => (
            StatusCode::BAD_GATEWAY,
            format!(
                "De bibliotheek van dit traject kon niet worden gelezen en de oorzaak is niet \
                 vast te stellen; {repo} gaf geen duidelijk antwoord. Probeer het opnieuw en \
                 meld het als het blijft misgaan."
            ),
        ),
    }
}

/// How many characters a repo address or branch name may take up in a
/// message.
///
/// The library pane (`indexErrorSupportingText`) does not shorten a message
/// it finds too long — it *replaces* it with "De gegevens konden niet worden
/// opgehaald.", the very sentence this module exists to retire. The prose is
/// ours to keep short; the identifiers are not. GitHub allows an owner of 39
/// characters and a repo of 100, and a base branch is typed in by hand, so
/// two long ones spliced into the longest sentence here would push it past
/// the 300-character cap and silently undo the whole diagnosis.
///
/// 64 keeps the worst case (two identifiers in the longest message) under
/// 300 with room to spare, while leaving every realistic `owner/repo` and
/// every generated `traject/<slug>-<id>` branch untouched.
const ID_BUDGET: usize = 64;

/// Shorten `id` to [`ID_BUDGET`] characters, eliding the middle so the parts
/// that identify it best — the owner at the front, the distinguishing tail at
/// the back — both survive.
fn clamp_identifier(id: &str) -> String {
    if id.chars().count() <= ID_BUDGET {
        return id.to_string();
    }
    // One character goes to the ellipsis; the remainder splits head-heavy.
    let keep = ID_BUDGET - 1;
    let tail = keep / 3;
    let head = keep - tail;
    let chars: Vec<char> = id.chars().collect();
    let front: String = chars[..head].iter().collect();
    let back: String = chars[chars.len() - tail..].iter().collect();
    format!("{front}…{back}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_identifiers_are_left_alone() {
        assert_eq!(clamp_identifier("example-org/corpus"), "example-org/corpus");
        assert_eq!(
            clamp_identifier(&"a".repeat(ID_BUDGET)),
            "a".repeat(ID_BUDGET)
        );
    }

    #[test]
    fn long_identifiers_keep_both_ends() {
        let long = format!("{}/{}", "o".repeat(39), "r".repeat(100));
        let short = clamp_identifier(&long);
        assert_eq!(short.chars().count(), ID_BUDGET);
        assert!(short.starts_with("oooo"), "{short}");
        assert!(short.ends_with("rrrr"), "{short}");
        assert!(short.contains('…'), "{short}");
    }

    #[test]
    fn multibyte_identifiers_do_not_split_a_character() {
        // Char-wise, not byte-wise: slicing a multi-byte name by bytes would
        // panic instead of shortening.
        let long = "é".repeat(ID_BUDGET * 2);
        assert_eq!(clamp_identifier(&long).chars().count(), ID_BUDGET);
    }
}
