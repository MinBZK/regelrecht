#!/usr/bin/env bash
#
# prose-drift-pr.sh — the scheduled "prose drift" flow.
#
# Regenerates the architecture model on-demand (never from a committed file),
# diffs it against the per-node prose sidecar, and — when they have drifted
# apart — opens a pull request with proposals so the narrative can catch up with
# the code:
#
#   * new / undocumented nodes  -> a scaffolded stub is committed (seeded with the
#                                  node's doc-comment as a starting point),
#   * changed nodes (stale prose) and removed nodes (orphaned prose) -> listed in
#                                  the PR body for a human/agent to resolve.
#
# It is intended to run on a schedule (e.g. nightly) from CI. Wiring the actual
# cron trigger is a `.github/workflows/*.yml` change and is deliberately left to
# a maintainer; this script is the self-contained flow that such a workflow — or
# a local run — invokes. When there is no drift it does nothing and exits 0, so a
# scheduled run is a no-op on a synced repo.
#
# Env:
#   BASE_BRANCH   base branch to open the PR against (default: main)
#   PR_BRANCH     working branch name (default: chore/arch-prose-drift)
#   DRY_RUN       when "true", detect + scaffold but do not commit/push/PR
#
# Requires: cargo, git, and (unless DRY_RUN) the `gh` CLI authenticated.

set -euo pipefail

ROOT="$(git rev-parse --show-toplevel)"
MANIFEST="$ROOT/packages/Cargo.toml"
PROSE_DIR="$ROOT/packages/arch-extract/prose"
BASE_BRANCH="${BASE_BRANCH:-main}"
PR_BRANCH="${PR_BRANCH:-chore/arch-prose-drift}"
DRY_RUN="${DRY_RUN:-false}"

arch() {
  cargo run --quiet --release --manifest-path "$MANIFEST" \
    -p regelrecht-arch-extract -- prose "$@" --manifest-path "$MANIFEST"
}

echo "==> Checking prose drift against the on-demand model"
if arch check >/tmp/prose-drift-report.txt 2>/dev/null; then
  echo "No drift — prose sidecar is in sync with the model. Nothing to do."
  cat /tmp/prose-drift-report.txt
  exit 0
fi

echo "Drift detected:"
cat /tmp/prose-drift-report.txt

echo "==> Scaffolding stubs for undocumented nodes"
arch sync

REPORT="$(cat /tmp/prose-drift-report.txt)"
PR_BODY=$(cat <<EOF
Automatisch voorstel van de geplande prosa-driftcontrole.

Het code-afgeleide architectuurmodel (on-demand gegenereerd) en de per-node
prosa-sidecar (\`packages/arch-extract/prose/\`) zijn uit elkaar gelopen. Deze PR
voegt stub-bestanden toe voor nieuwe/ongedocumenteerde nodes (met het bestaande
doc-commentaar als startpunt) en somt gewijzigde en verweesde teksten op ter
review.

\`\`\`
${REPORT}
\`\`\`

Wat te doen:
- Vul de toegevoegde stubs onder \`packages/arch-extract/prose/\` met "wat/waarom".
- Werk teksten bij die als *stale* gemarkeerd staan en draai
  \`just arch-prose-bless <node-id>\` (of \`--all\`) om de fingerprint te verversen.
- Verwijder teksten die als *orphaned* gemarkeerd staan.
EOF
)

if [ "$DRY_RUN" = "true" ]; then
  echo "==> DRY_RUN: skipping commit/push/PR. Proposed PR body:"
  echo "$PR_BODY"
  exit 0
fi

echo "==> Committing proposals on branch $PR_BRANCH"
git fetch origin "$BASE_BRANCH"
git switch -C "$PR_BRANCH" "origin/$BASE_BRANCH"
git add "$PROSE_DIR"
if git diff --cached --quiet; then
  echo "Drift detected but nothing to commit (only stale/orphaned, no new stubs)."
  echo "Opening a PR would be empty; leaving it to a manual review instead."
  exit 0
fi
git commit -m "chore(dev): voorstel prosa-updates na architectuurdrift"
git push --force-with-lease origin "$PR_BRANCH"

# Reuse an existing open PR for this branch if there is one; otherwise create it.
if gh pr view "$PR_BRANCH" --json number >/dev/null 2>&1; then
  echo "==> Existing PR updated (pushed new commit)."
else
  gh pr create --draft \
    --base "$BASE_BRANCH" \
    --head "$PR_BRANCH" \
    --title "chore(dev): prosa-updates na architectuurdrift" \
    --body "$PR_BODY"
fi
