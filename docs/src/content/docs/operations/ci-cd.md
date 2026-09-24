---
title: "CI/CD Pipeline"
description: "What runs on every push and pull request, and how only the checks relevant to changed files run."
---

Continuous integration runs on every push to `main` and every pull request via `.github/workflows/ci.yml`. Only checks relevant to changed files run, so CI stays fast.

## What CI checks

### Code quality (on Rust/YAML changes)

- **Formatting** - `just format` (rustfmt check)
- **Linting** - `just lint` (clippy)
- **YAML validation** - yamllint + schema validation on corpus files
- **Pre-commit hooks** - trailing whitespace, end-of-file, merge conflicts

### Tests (on Rust changes)

CI runs `just test`, which is `cargo test --workspace` over every crate. A new
crate is covered without anyone having to add it to a list; the pipeline and
editor-api suites use testcontainers for PostgreSQL, so the runner needs Docker.

`just test-no-docker` is the same coverage minus those container-backed suites,
for a machine without Docker. `just check` runs the full `test`.

The BDD suite (`just bdd`, cucumber-rs with Gherkin scenarios) covers two
buckets and is **not** part of `just test`; the target carries `test = false` so
it only runs when called by name. `BDD_BUCKET` picks the bucket: `all` (the
default, what `just bdd` runs), `corpus` or `conformance`.

### BDD conformance and demo (on relevant changes)

The **BDD conformance** job runs bucket B (`bdd/conformance/*.feature` against
the synthetic `test_*` laws) as `BDD_BUCKET=conformance cargo test --test bdd`.
It hangs on the `Test` gate, so it blocks a merge. That bucket proves the engine
speaks the whole feature language and depends on nothing outside the repo.

The **BDD demo** job validates every law in `corpus/demo/regulation` against the
schema and then runs bucket A over that corpus, with `REGULATION_PATH` pointing
at it (the same run as the first half of `just bdd-demo`). The demo corpus and
its synthetic persona data live entirely in the repository, so this job also
hangs on the `Test` gate and blocks a merge.

Bucket A over the live corpus (`corpus/regulation/**/scenarios/*.feature`) stays out of CI. It asserts
what the live laws currently produce, so a failure there means a law changed or a
scenario went stale; a human decides what that is worth. Run it locally with
`BDD_BUCKET=corpus`.

### WASM build (on engine changes)

Builds the engine for the WebAssembly target to catch compilation issues early.

### Licence and supply-chain audit (always runs)

- **Rust** - `cargo-deny bans licenses sources`: banned crates, licence compliance, and an allowlist of dependency sources
- **Frontend** - `license-checker` over the npm workspace at the repo root, refusing the copyleft licences the project cannot ship

Neither scans for known vulnerabilities. That is `just audit-advisories` (RustSec plus npm advisories), which runs periodically from `security-advisories.yml` rather than on every push.

### Schema protection (on PRs)

Released schema versions in `schema/v*.*.*` are immutable. CI fails if a PR tries to modify or delete a released schema. Only `schema/latest/` can be updated freely.

### Provenance checks (on corpus/engine changes)

The `provenance-checks` job verifies that every corpus YAML file uses a tag-based `$schema` URL (`refs/tags/schema-vX.Y.Z`) and that the referenced schema version is known. This catches files that still use the old `refs/heads/main` format. See [RFC-013](/rfcs/rfc-013) for context.

### Component-specific checks

- **Admin** - format, lint, cargo check, tests, frontend build
- **Editor API** - format, lint, cargo check

## Change detection

CI uses path filters to determine which checks to run:

| Change group | Triggers on changes to |
|---|---|
| `ci` | `packages/`, `frontend/`, `corpus/regulation/`, `corpus/demo/`, `bdd/`, `schema/`, `script/`, the PoC trees, and the root build files (`Justfile`, `package.json`, `rust-toolchain.toml`) |
| `admin` | `packages/admin/` |
| `editor-api` | `packages/editor-api/`, `packages/corpus/`, `packages/pipeline/`, `packages/harvester/` |
| `docs` | `docs/` |

The `ci` group includes `frontend/`, so frontend changes also trigger the Rust checks (the editor is shipped as one image built from `frontend/` plus the `editor-api` Rust binary that serves it). Docs-only changes skip the Rust checks and run just the docs accessibility gate (`just docs-a11y`).

## Merge gates

Passing the checks above is not enough on its own. Four gates weigh in on whether a pull request can merge. Only **Claude review completed** is a required status check on `main`; the other three report and are read by a human.

| Gate | What it requires |
|------|------------------|
| **Werkpakket genoemd** | The PR body ends with a `Werkpakket:` line naming a werkpakket from the roadmap. `geen` is allowed with a reason. See [Contributing](./contributing). |
| **Claude review completed** | The automated review ran to completion for this commit and left no finding marked Critical. There is no override: fix the finding and push again. |
| **Security update approved** | A Dependabot security update needs an approving review from someone with write access, on the commit being merged. Security updates skip the five-day cooldown, so a human looks at them instead. |
| **Mutation Testing (diff)** | Reports on whether the tests pin the behavior the change introduces. |

The full list of required checks on `main` is `Pre-commit`, `WASM Build`, `Protect schema versions`, `Security Audit`, `Test`, `Validate PR title` and `Claude review completed`. That list lives in the branch protection, not in a file; `script/merge-queue-checks.test.mjs` keeps a hand-maintained copy of it (see [the merge queue](#the-merge-queue)).

### The Claude review gate

`claude-code-review.yml` posts its findings as review comments and takes about ten minutes, roughly twice as long as the other required checks. Merging on green used to be possible before the review had said anything. The `review-gate` job (check name **Claude review completed**) closes that window. It waits for the `claude-review` job inside its own workflow run and passes only once that job is `completed` with `success` or `neutral`.

It looks the job up by run id rather than by head SHA. `ready_for_review` and `reopened` keep the same SHA, and a SHA lookup can then read a leftover check-run from the previous run. It also queries with `filter=all`: the run id survives a "Re-run this job", and `claude-review` is then absent from the newest attempt's job list.

The wait is driven by the job's `status`, never by the number of review comments. Zero comments fits "still running" as well as "the review had nothing to report". The gate does read the comments, but only for what they say, and only after it has established that the review ran to completion. That order is enforced: `assert_no_critical_finding` blocks with an internal-error message if it is reached before `assert_review_ran` has set `review_proven`, and the test suite asserts that the comment endpoints stay untouched on every path where the review is unproven.

#### When the review does not apply

`claude-review` itself cannot be a required check. It does not run on cross-repo (fork) PRs, which have no secrets and therefore no `CLAUDE_CODE_OAUTH_TOKEN`, and a required check that never reports blocks such a PR forever. `review-gate` always runs and always reports. It derives "not applicable" from the PR as the API describes it (cross-repo, draft, Dependabot) rather than from the conclusion of `claude-review`, so a failed or skipped review can never pass as an inapplicable one.

Dependabot is a deliberate exemption. Those PRs go through `claude-dependabot.yml`, which decides for itself whether to merge; the gate does not verify that it ran.

#### A green job is not proof of a review

`claude-code-action` exits with conclusion `success` without reviewing anything when the workflow file differs from the version on the default branch ("Exiting due to workflow validation skip"). The gate therefore checks two things before it believes a green job.

First, it compares `.github/workflows/claude-code-review.yml` as this run has it with the copy on the default branch, through the contents API. If they differ there is no automatic review to wait for, and the gate blocks straight away, before the wait. That comparison is also what makes the rest trustworthy: identical files mean the workflow that ran is the one from the default branch, not something the PR brought along. A PR that edits this file needs a human review and an admin merge.

The comparison uses `refs/pull/N/merge`, not the head commit. A `pull_request` run executes the workflow file as it stands in the test merge of PR and base, so a branch that has fallen behind still runs the base's copy. Comparing against the head commit would put exactly those branches on red while their review ran fine.

Second, it reads the proof out of the review job. The step **Record that the review ran** carries `if: steps.claude-review.outputs.execution_file != ''`, and that output is only set once the Claude CLI has actually run. On a self-skip the step is `skipped`. The gate reads that step's conclusion from the `steps[]` array of the job it already looked up, so the proof is bound to this run and this attempt, and it exists just as well when the review had no findings. The gate insists on exactly one step by that name, because two would let an added decoy outvote the real one. Renaming the step means changing `PROOF_STEP` in the gate as well; the test suite asserts that the workflow still carries the step under that name with that `if:`, and the pre-commit hook runs on the workflow file for that reason.

#### What the gate trusts

The gate fetches the facts it relies on itself. The workflow that invokes it lives in the pull request, so anything that workflow passes in is written by the author of the change under review. Setting `IS_DRAFT: true` in the env block used to be enough to declare the review inapplicable. Draft status, fork status, author and the workflow file now all come from the API. The env block carries only the coordinates of the run (repository, run id, PR number), and the gate checks that the run and the PR describe the same commit, so pointing it at another PR's run yields a red.

The gate runs the copy of its own script from the base branch, not the one in the PR; otherwise a PR could turn the script into `exit 0` and be green by construction. If that copy cannot be fetched or checked out, the gate blocks rather than falling back to the PR's version.

What none of this reaches is the `review-gate` job itself, which lives in the workflow file the PR brings along. A PR that keeps the job name **Claude review completed** and replaces its steps with `run: true` reports green without running the script, and nothing inside the script can prevent that. Closing that hole takes a rule outside the pull request: a ruleset or CODEOWNERS entry over `.github/workflows/**`, or a gate triggered by `workflow_run` that judges the review run from the outside. Until then the gate protects against mistakes, not against someone determined to route around it.

Every red outcome states what was established and names no cause it has not tested. An earlier version told every clean PR that the review action had skipped itself over a workflow change, on PRs that changed no workflow at all ([issue #1178](https://github.com/MinBZK/regelrecht/issues/1178)).

#### Critical findings block

A finished review is not the same as an acceptable one. On 8 August 2026 the review put a Critical finding on PR 1234 at 10:11:31 and the PR merged at 10:11:54, green throughout, because the gate only asked whether a review had happened. It now searches what `claude[bot]` wrote for the exact string `🔴 **Critical**` and turns red on a hit, naming the comments it found it in. Three places count: the sticky comment, the inline comments, and the body of the submitted review. The last one is easy to forget, and in practice it carries findings that appear nowhere else.

Only what this run wrote counts, measured against the review job's `started_at`. An item by `claude[bot]` without a usable timestamp blocks rather than silently falling outside the window; a `PENDING` review is the exception, since it was never submitted. Anything older came from an earlier run. Cleanup removes those comments, but a submitted review cannot be deleted at all, and a cleanup that fails would otherwise keep blocking a finding that has long been fixed. Without a readable `started_at` the gate blocks, because it cannot tell the two apart. One edge the window does not cover: `cancel-in-progress` does not stop a run instantly, so a comment written by the run being cancelled can land inside its successor's window and turn that one red. The next push clears it.

Only Critical blocks. Significant carries "likely" in its own definition and would often be a false positive, and an exception that becomes routine stops holding anything back. There is no override: fix the finding and push again. Since the review rewrites its comment on every run, a finding that is gone is gone from the gate too. An escape hatch (a label, a magic comment, an environment variable) is something an agent operates as easily as a person, while it reads as a human judgement, so there is none; if a case ever arises that needs one, it gets decided then.

The marker lives in `CRITICAL_MARKER` and in the review prompt, and the test suite binds the two: change one without the other and the gate reads past every finding without anyone noticing. The marker is free text written by a language model, so the prompt states that a machine parses it and that it must be written exactly as given, and nowhere else. That cannot be closed completely.

#### Findings cannot be pushed away

Blocking on findings only works if a finding cannot be made to disappear. The review job used to delete every `claude[bot]` comment at the start of its run, so a crash after the deletion took the previous finding with it, and a rerun that happened not to notice it again turned red into green. The order is now reversed: `script/claude-review-comments.sh snapshot` records the ids and texts before the review, and `clean-up` removes them afterwards, under the same `execution_file` condition as the proof step. Cleanup deletes only what is still there with an unchanged `updated_at`. `use_sticky_comment` makes the action reuse the previous comment, so its id is in the snapshot, and deleting by id alone would wipe the review that just ran.

Snapshot and cleanup select comments by the line `<!-- claude-review -->`, which the prompt tells the review to end every comment with. The author alone is not enough: `claude.yml` answers `@claude` in the same thread as the same bot, and that conversation is neither a finding to hand to the next review nor something to delete. Cleanup never fails the step. A failed list call counts as a failed cleanup and is reported in the step summary; failing there would put `claude-review` on `failure` and make the gate report that no usable review exists, for a review that ran fine.

The snapshotted texts go into the prompt as context, with the instruction that they describe an earlier version of the diff and that each one is to be re-checked against the current one: nothing carried over that has been fixed, nothing dropped that still stands. Without that, the gate would reward pushing until the review forgets, which is worse than no gate. On the way in the severity markers are rewritten to `[Critical]` and similar, so a review that quotes an old finding to say it has been fixed does not trip the gate with the quote.

The gate needs `issues: read` on top of `pull-requests: read`, because the summary comment is an issue comment and that endpoint sits behind the issues API.

In short, the gate proves that the review ran to completion for this commit and that it left no Critical finding standing. It says nothing about findings of lower severity, whether the findings hold up, or whether they were addressed.

The logic lives in `script/await-claude-review.sh` and `script/claude-review-comments.sh`. Each has a test suite next to it, using a `gh` stub, that covers every path deciding green versus red, or what is kept and what is thrown away. Both run as pre-commit hooks.

### The security-update gate

Dependabot security updates bypass the five-day `cooldown` in `.github/dependabot.yml`: they arrive the moment the fixed version is published. The check **Security update approved** (`.github/workflows/security-update-gate.yml`) turns green on such a PR only once an engineer with write access has approved it.

It is a check rather than branch protection because branch protection offers no other way to express this. `required_approving_review_count` applies to a branch, not to an author or a label, and ruleset conditions only select refs. Turning it on repository-wide would mean nobody could merge their own PR, because GitHub does not let an author approve their own change. The condition therefore lives in the check, as it does for **Claude review completed**.

No API field says whether a PR is a security update. The gate reads three independent signals: an open Dependabot alert for exactly the package the PR bumps, Dependabot's own line "This update includes a security fix", and a GHSA or CVE identifier in the opening of the PR body, up to the first `<details>` block (after that come quoted changelogs, which may just as well be about another package). One signal is enough. A false positive costs an approval that was not needed; a false negative lets a security patch through unseen, so the gate leans toward the former.

An approval counts only on the commit it was given on. `dismiss_stale_reviews` only works when `required_approving_review_count` is above 0, and here it is 0, so the script does the binding itself: after a rebase or a push nothing is approved any more. Approvals from bots do not count, nor do approvals from someone without write access (an `author_association` other than OWNER, MEMBER or COLLABORATOR).

`claude-dependabot.yml` uses the same script to decide whether a PR is a security update and, if so, no longer merges it itself. The review still runs on it, as input for the engineer who approves. That workflow does not enforce anything; the check does.

There is no minimum age. The age threshold exists because nobody looks at a routine bump, and the approval is exactly what removes that concern. Adding it on top would leave an actively exploited vulnerability waiting five days plus the wait for a human. The publication date of the new version is one of the things the approver looks at, not something the gate decides for them.

The Mattermost notification lives in the same workflow and fires when a security PR is opened and when it is merged, through `MATTERMOST_WEBHOOK_URL`. Dependabot starts that run, and in that case `secrets.*` reads from the Dependabot secret store instead of the Actions one. A webhook URL stored only under Actions would make the notification silently disappear, so the step fails hard on an empty URL.

The logic lives in `script/require-security-approval.sh`, with `script/require-security-approval.test.sh` (a `gh` stub) covering every path that decides green or red; the tests run as a pre-commit hook. The gate runs the copy of the script from the base branch, not the one in the pull request. If the base branch has no copy, that is not a read error: it is the pull request that introduces or removes the gate, so nothing on `main` is protected by this check yet, and the job says so and passes. The same limitation as for the review gate applies: the job itself sits in the workflow file the PR brings along.

## The merge queue

`main` merges through a merge queue. Rather than merging your branch directly, GitHub builds a branch of its own (`gh-readonly-queue/main/...`) holding `main` plus the pull requests waiting in line, and runs the checks there. That is what keeps a set of individually green pull requests from landing as a broken `main`, and it removes the rebase-and-wait cycle that a busy repository otherwise runs on. It also means the required checks have to report on that queue branch, not only on the pull request, and most of what follows comes from that.

### Putting a pull request in the queue

```bash
gh pr merge <nr> -R MinBZK/regelrecht --squash
```

gh replies "The merge strategy for main is set by the merge queue". That is a notice, not an error. gh refuses `--delete-branch` while the queue is on, and it is not needed: the repository deletes the head branch itself after the merge.

A command that succeeded does not mean the pull request is in the queue. Ask:

```bash
gh api graphql -f query='{repository(owner:"MinBZK",name:"regelrecht"){
  pullRequest(number:<nr>){state mergeQueueEntry{state position}}}}'
```

A pull request that drops out of the queue stays `OPEN`, so waiting for the state to become something other than `OPEN` waits forever after a drop. Follow `mergeQueueEntry` instead: `MERGED` means done, and `OPEN` with `mergeQueueEntry: null` means it dropped out. The reason is in the timeline:

```bash
gh api graphql -f query='{repository(owner:"MinBZK",name:"regelrecht"){
  pullRequest(number:<nr>){timelineItems(last:5,itemTypes:[REMOVED_FROM_MERGE_QUEUE_EVENT]){
  nodes{... on RemovedFromMergeQueueEvent{createdAt reason}}}}}}'
```

### When something fails in the queue

A failure in the queue does not show on the pull request. Its checks stay green; the red run belongs to the queue branch. This finds it:

```bash
gh run list -R MinBZK/regelrecht --event merge_group --limit 20 \
  --json databaseId,headBranch,workflowName,conclusion \
  --jq '.[] | select(.headBranch | test("pr-<nr>-"))'
```

When a test fails in the queue that the pull request does not touch, while the run on the pull request and the latest run on `main` are green, it is usually a flaky test or a clash with another pull request in the same group. Queue it once more. If it drops out a second time on the same failure it is no longer chance: find the cause instead of queueing it again. A flaky test found this way gets an issue, because every drop costs a full round for everyone behind it.

### Required checks in the queue

The queue has no list of required checks of its own; it uses the list from the branch protection. A check that never reports on the queue branch stays on "Expected" and holds its entry until the status check timeout drops it. That happens quietly, because the message is on the queue branch rather than a red check on the pull request, so it only shows when nothing merges any more. `gh run list --branch 'gh-readonly-queue/main/...'` shows what happened there.

Skipped is not the same as missing. A job skipped by an `if:` reports `skipped`, which GitHub counts as passing. Only a workflow that does not run at all reports nothing. A required job may therefore skip in the queue; its workflow must not drop out.

`Protect schema versions` uses that difference deliberately and skips in the queue. It compares against `origin/main`, and in the queue `HEAD` is not one pull request but the whole group: if one of them touches a released schema version the whole group fails and takes the others with it. The same check already catches that on each pull request.

`Validate PR title` and `Claude review completed` are the other way around: they can only exist on a pull request, since one reads the title and the other reads the review comments, the body and `refs/pull/N/merge`. In the queue both come from `.github/workflows/merge-queue-gates.yml`. What that workflow establishes is that both gates were green at the moment the pull request joined the queue, and nothing more. A finding that arrives after joining no longer stops anything. With "Merge when ready", GitHub joins the queue by itself as soon as the last gate turns green, so no human sits in between at that point.

`script/merge-queue-checks.test.mjs` ties every required check to a job that runs in the queue, with the one deliberate skip listed explicitly. The hook runs on every workflow, because any workflow can carry or lose such a check. Its `REQUIRED_CHECKS` list is maintained by hand, and the two directions are not symmetric: a name too many fails loudly, a name too few is a silent loss of coverage. The branch protection changes with a button rather than a commit, so no review reminds anyone to update that list.

### Branch protection settings

"Require branches to be up to date before merging" is off, and should stay off. It demands that a pull request first be updated to the tip of `main`, which is exactly the manual work the queue takes over. With it on, everyone still rebases by hand, CI runs again afterwards, and the queue only adds an extra run per merge without the benefit. The setting exists in no file, only in the branch protection.

Every merge runs CI twice: once on the queue branch and once on the push to `main` that follows. For a group of several pull requests that gets cheaper per pull request as long as the group passes; if it fails, the work is lost and the rebuilt group builds again. The build concurrency in the queue settings limits that.

Turning the queue off is a single switch in the branch protection. The `merge_group` triggers and `merge-queue-gates.yml` are then never triggered and cost nothing, so no revert is needed. `enforce_admins` is on, so there is no way around a stuck entry: take it out of the queue first, then merge, then switch the queue back on.

## Further reading

- [Contributing](./contributing) - the PR process, including the required trailer
- [Deployment](./deployment) - what happens after CI passes
- [Testing](/guide/testing) - how to run tests locally
