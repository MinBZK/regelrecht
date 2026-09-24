---
title: "Private-repo trajects"
description: "How a traject in the editor is linked to its own (private) GitHub repository instead of the central corpus repository, and which token writes to it."
---

Since [PR #704](https://github.com/MinBZK/regelrecht/pull/704), a *traject* (a working context in the editor, with its own members and branch) can be linked to **its own GitHub repository** instead of the central `MinBZK/regelrecht-corpus`. This suits organizations and teams that want to keep their regulations in a private repository while keeping the editor and its commit attribution.

What you need to do depends on your role: participant, traject owner, or operator of the RegelRecht deployment. It also depends on which of two write modes the deployment runs in.

## The two write modes

The editor writes to a traject repository with one of two kinds of token.

- **Service token.** A fine-grained GitHub Personal Access Token (PAT) per repository, configured by the operator as an environment variable (or as an entry in the `corpus-auth.yaml` file that `CORPUS_AUTH_FILE` points to). Every participant writes through that one token. This is the default.
- **Personal GitHub account.** Each user links their own GitHub account in the editor (Settings, under "Koppelingen", button "Koppelen") and writes with that. This needs a GitHub OAuth App on the deployment and is switched on for the whole environment, either with `GITHUB_USER_TOKEN_REQUIRED=true` or with the switch "Met eigen GitHub-account schrijven" in the editor settings (the feature flag `github.user_oauth`, off by default).

A configured service token always wins. The personal token is only used for a repository that has no service token. In personal mode a traject on a repository without a service token therefore needs no operator at all: a user who has push rights on the repository and has linked their account can create the traject and save to it. Without a linked account, or with an expired link, a save returns HTTP 428 and the editor sends the user through the linking flow.

Neither token is stored in the database. The service token lives only in the editor's runtime environment. The personal token is sealed (encrypted and authenticated with `GITHUB_TOKEN_ENC_KEY`) into an HttpOnly cookie that lasts for the browser session and is bound to the editor account, so the browser holds it but page scripts cannot read it.

## Who does what

With a service token:

| Role | Action |
|---|---|
| Repository owner (or admin) | Creates a GitHub PAT with write access to the target repository |
| Operator | Configures the PAT as an environment variable on the editor deployment (once per repository) |
| Traject owner | Creates the traject in the editor with the repository fields filled in |
| Participants | Log in through SSO; saves are attributed to their own name |

In personal mode the first two rows fall away for repositories without a service token, and every participant links their own GitHub account instead.

## Step 1: Prepare the GitHub repository

The target repository must meet two conditions before it can be linked:

1. **The base branch already exists**, with at least one commit. When the traject is created, the editor creates its own branch (`traject/<slug>-<id>`) from that base on GitHub. If the base does not exist yet (for example an empty `main`), validation fails. If needed, push an empty README commit to `main` to initialize it.
2. **The token has push rights** on the repository: the PAT account for a service token, or the user's own account in personal mode.

The repository can be public or private. Creating a traject always requires a token with push rights, also for a public repository. Afterwards, without a service token and outside personal mode, reads of a public repository go out unauthenticated, with GitHub's lower rate limits. Writes always need a token.

## Step 2: Create a GitHub PAT (service-token mode)

Generate a **fine-grained PAT** at [github.com/settings/personal-access-tokens/new](https://github.com/settings/personal-access-tokens/new):

- **Resource owner**: the owner of the target repository (a personal account or an organization)
- **Repository access**: "Only select repositories", and pick exactly the target repository. Do not choose "All repositories"; it keeps the damage of a leak small.
- **Repository permissions**:
  - `Contents`: **Read and write**, for commits and for creating the traject branch
  - `Metadata`: **Read** (required and selected automatically)
- **Expiration**: choose a period that fits your security policy. When the PAT expires, the editor refuses reads and writes and the operator has to replace the environment variable.

The editor commits directly to the traject branch through the GitHub Contents API and opens no pull request, so the `Pull requests` permission is not needed.

Store the PAT value somewhere safe right away; GitHub shows it only once.

Classic PATs work too, but they grant far too much (every repository the account can reach). Fine-grained is strongly recommended.

## Step 3: The operator configures the environment variable

Give your operator three things:

1. The `owner/repo` of the target repository (for example `acme/regelrecht-lokaal`)
2. The PAT value
3. A short description of its purpose, for the audit trail

The operator sets the PAT as an environment variable on the editor deployment, named:

```
CORPUS_AUTH_<OWNER>_<REPO>_TOKEN
```

where `<OWNER>_<REPO>` is a **deterministic slug** of the coordinates: lowercased, every run of non-alphanumeric characters becomes a single `-` (leading and trailing dashes are dropped), and the result is uppercased with dashes turned into underscores. Examples:

| owner/repo | Environment variable |
|---|---|
| `MinBZK/regelrecht-corpus` (the central repository) | `CORPUS_AUTH_MINBZK_CENTRAL_TOKEN` |
| `acme/regels` | `CORPUS_AUTH_ACME_REGELS_TOKEN` |
| `acme/regelrecht-private-test` | `CORPUS_AUTH_ACME_REGELRECHT_PRIVATE_TEST_TOKEN` |

The central writable repository (`MinBZK/regelrecht-corpus`) does **not** use the derived slug but the fixed auth ref `minbzk-central`, so its variable is `CORPUS_AUTH_MINBZK_CENTRAL_TOKEN`, not `CORPUS_AUTH_MINBZK_REGELRECHT_CORPUS_TOKEN`. A user repository whose name would slug to `minbzk-central` is refused.

How to set an environment variable depends on the platform (ZAD, Kubernetes, docker-compose). After the change the editor pod has to restart to pick up the new variable. **One variable per repository**: when several trajects point at the same repository, one configuration covers them all.

If you create the traject before the variable exists, and the deployment is not in personal mode, the error message shows the exact variable name the editor expects. Pass that name to your operator verbatim.

## Step 4: Create the traject

In the editor:

1. Choose **"Nieuw traject…"** in the traject menu.
2. Fill in **Naam** (name) and, optionally, **Beschrijving** (description).
3. Turn on the switch **"Eigen GitHub-repo (i.p.v. standaard MinBZK-repo)"**.
4. Fill in **Repo owner**, **Repo** and **Base branch** (prefilled with `main`). Set **Subpath** if the YAML files live in a subdirectory; leave it empty for the repository root.
5. Click **"Maak traject aan"**.

Before anything is stored, the editor runs a **preflight check** against the GitHub API:

- Does the repository exist, and can the token see it?
- Does the token have push rights?
- Does the base branch exist?

It then creates the traject branch on GitHub. If any of these steps fails you get a specific error (see [error messages](#error-messages-and-what-they-mean) below), no row is written to the database, and you can retry straight after fixing the cause. If the database write fails after the branch was created, the branch stays behind on GitHub; the editor logs it but does not remove it.

## How commit attribution works

Every save in a traject produces a commit on the traject branch. Who it is attributed to depends on the token:

- **Service token**: author and committer are both set to your name and email from the SSO session (Keycloak). GitHub still records the token's account as the pusher.
- **Personal token**: the editor sets no author or committer, so GitHub attributes the commit to your linked GitHub account.

For the display on GitHub: if the email from your SSO session is a verified address on your GitHub account, GitHub shows your avatar and name on the commit and links to your profile. If it is not, GitHub shows the email as plain text, without a profile link, but still with the correct name.

The editor refuses saves when Keycloak's `email_verified` claim is not `true`. This prevents a user with an unverified email claim from committing under that name. If you get a 403 for this, log in again (which refreshes the claim) or ask your administrator to check your account settings.

## Error messages and what they mean

When creating a traject fails, the editor shows a specific message. The messages are in Dutch; the most important ones:

| Status | Message | What to do |
|---|---|---|
| 503 | "deze repo is nog niet door je beheerder geconfigureerd (verwacht env var X)" | No service token and not in personal mode. Ask the operator to configure the variable named in the message |
| 428 | "Koppel je GitHub-account …" or "Je GitHub-koppeling is verlopen …" | Personal mode: link (or relink) your GitHub account |
| 502 | "het token van je beheerder wordt door GitHub geweigerd" | The token is expired or invalid; ask the operator to replace it |
| 403 | "het geconfigureerde token heeft geen schrijftoegang tot deze repo" | The PAT lacks `Contents: write`, or its account is not a collaborator on the repository |
| 404 | "repo X bestaat niet of het token kan 'm niet zien" | A typo in owner or repo, or the fine-grained PAT is not linked to this repository |
| 404 | "branch 'X' bestaat niet op owner/repo" | The base branch does not exist yet; initialize it with at least one commit |
| 400 | "repo_owner / repo_name mogen alleen letters, cijfers, en de tekens '-', '_' en '.' bevatten" | Owner or repository name contains invalid characters (the subpath has a similar rule, and may not contain `..`) |
| 400 | "base_branch bevat tekens die niet zijn toegestaan in een git branch-naam" | The base branch is not a valid git branch name |
| 400 | "… moeten alle drie worden meegegeven …" | The own-repository switch is on but owner, repository or base branch is empty |
| 502 | "kon de traject-branch 'X' niet aanmaken op owner/repo …" | The traject branch could not be created; nothing was stored |
| 502 | "onverwacht antwoord van GitHub bij repo-validatie" | A temporary GitHub problem; try again |
| 503 | "kon GitHub niet bereiken om de repo te valideren" | GitHub could not be reached; try again |

Saves can fail while editing as well. Most messages are the same; for a 403 about a verified email address (*geverifieerd e-mailadres*), logging in again usually fixes it.

## Limitations

- **A service token is per repository, not per user.** All participants in a traject commit through the same token, under their own name. GitHub's push events always show the token's account as the pusher, which is not enough for a strict per-user audit; use the editor API's audit logs or the commit history of the traject branch for that. Personal mode does not have this limitation.
- **Replacing an expired service token is operator work.** The editor refuses reads and writes once GitHub rejects the PAT, and the operator has to replace it. Plan for this. An expired personal link is fixed by the user relinking.
- **Service-token mode is not self-service.** Every new repository needs an operator to configure a variable. Personal mode removes that step, at the cost of every user needing their own GitHub account with push rights.
- **No tokens in the database.** A deliberate design choice: a bug, breach or insider with database access cannot exfiltrate tokens.

## Rollout notes (relevant only for the first deploy of this feature)

The first deploy with this feature tightened two older paths. Check these on your deployment before rolling out the release.

- **The writable-own source uses strict token resolution** (no `CORPUS_GIT_TOKEN` fallback). Existing trajects that commit to the central MinBZK repository therefore need `CORPUS_AUTH_MINBZK_CENTRAL_TOKEN` as a separate variable. Deployments that so far relied only on `CORPUS_GIT_TOKEN` for the central write path see silent push failures after the release when that variable is missing. Set it before deploying and check the editor logs on the first run; the diagnostic log names the expected variable when the resolver finds no token for the writable-own source.
- **Existing SSO sessions lack the new `email_verified` claim.** The first save after the deploy then returns a 403 asking the user to log in again. Logging in once fixes it; no maintenance or migration is needed.
