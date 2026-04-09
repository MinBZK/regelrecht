---
name: rebase-prs
description: "Rebase all open non-draft PRs onto latest main. Skips branches with local changes in their worktree. Resolves trivial conflicts (lock files, version bumps) automatically; aborts and reports on ambiguous conflicts. Use standalone for a single run, or with /loop for recurring: /loop 10m /rebase-prs"
user_invocable: true
---

# Rebase PRs

Rebase all open non-draft PRs in this repo onto the latest `origin/main`.

## Process

### Step 1: Discover PRs

```bash
gh pr list --author @me --state open --json number,title,isDraft,headRefName,baseRefName
```

Separate into:
- **Non-draft PRs** → candidates for rebase
- **Draft PRs** → skip and report

### Step 2: Fetch latest main

```bash
git fetch origin main
```

### Step 3: For each non-draft PR

#### 3a: Check if rebase is needed

```bash
MAIN_SHA=$(git rev-parse origin/main)
git fetch origin <headRefName>
MERGE_BASE=$(git merge-base origin/main origin/<headRefName>)
```

If `MERGE_BASE == MAIN_SHA` → already up-to-date, skip.

#### 3b: Find the worktree

Look for the branch's worktree in `.worktrees/`. Check all subdirectories:

```bash
for d in .worktrees/*/; do
  branch=$(git -C "$d" branch --show-current)
  echo "$(basename $d): $branch"
done
```

If no worktree exists for the branch, create one:

```bash
git worktree add .worktrees/<safe-name> <headRefName>
```

#### 3c: Check for local changes

```bash
git -C <worktree> status --porcelain
```

- If there are **tracked modifications** (`M`, `A`, `D` in first column, or ` M`, ` D` in second column) → **SKIP** and report "local changes"
- **Untracked files** (`??`) alone do NOT block a rebase — they are unaffected by rebase

#### 3d: Rebase

```bash
git -C <worktree> fetch origin main
git -C <worktree> rebase origin/main
```

#### 3e: Handle conflicts

If rebase hits a conflict, evaluate:

**Trivial (auto-resolve):**
- `package-lock.json` / lock files → accept `--ours` (main's version), regenerate with `npm install`, `git add`, `git rebase --continue`
- `package.json` version bumps → take higher versions from main, keep branch-specific additions (new dependencies), regenerate lock file
- Import ordering, whitespace-only changes
- `Cargo.lock` → accept main's version, run `cargo generate-lockfile` if available

**Ambiguous (abort):**
- Both sides modified the same function/logic
- Structural changes to the same component
- Conflicting business logic

For ambiguous conflicts:
```bash
git -C <worktree> rebase --abort
```
Report the conflict details to the user.

#### 3f: Force push

After successful rebase:

```bash
git -C <worktree> push origin <headRefName> --force-with-lease
```

### Step 4: Report

Print a summary table:

```
| PR | Branch | Status |
|----|--------|--------|
| #123 | feat/foo | Rebased |
| #124 | feat/bar | Up-to-date |
| #125 | feat/baz | Skipped — local changes (N files) |
| #126 | feat/qux | Skipped — draft |
| #127 | feat/quux | Conflict — <details> |
```

## Important Notes

- **Never force-push without `--force-with-lease`** — this protects against overwriting commits pushed by others
- **Never discard local changes** — if a worktree is dirty, skip it entirely
- Lock file conflicts are always safe to resolve by regenerating from the resolved `package.json`/`Cargo.toml`
- When resolving version conflicts in `package.json`, always prefer the higher version number and preserve any new dependencies added by the branch
