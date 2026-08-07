#!/usr/bin/env bash
# Merge gate: block until the Claude review check-run for this commit is done.
#
# Zero review comments is not evidence of "no findings" — it is equally
# consistent with "the review is still running". So this gate reads the status
# of the check-run, never the comments.
#
# It exits 0 only when the review demonstrably ran to completion for this exact
# commit, or when the review demonstrably cannot apply (fork, draft, dependabot).
# Those reasons are read off the pull request itself, not off the review's own
# result, so a review that ran and failed can never pass as one that was never
# meant to run.
set -uo pipefail

: "${REPO:?REPO is verplicht}"
: "${HEAD_SHA:?HEAD_SHA is verplicht}"

IS_FORK="${IS_FORK:-false}"
IS_DRAFT="${IS_DRAFT:-false}"
PR_AUTHOR="${PR_AUTHOR:-}"
CHECK_NAME="${CHECK_NAME:-claude-review}"
MAX_WAIT_SECONDS="${MAX_WAIT_SECONDS:-2100}"
POLL_SECONDS="${POLL_SECONDS:-20}"
GITHUB_STEP_SUMMARY="${GITHUB_STEP_SUMMARY:-/dev/null}"

summary() { printf '%s\n' "$1" >>"$GITHUB_STEP_SUMMARY"; }

not_applicable() {
  echo "::notice title=Claude review gate::niet van toepassing — $1"
  summary "### Claude review gate: niet van toepassing"
  summary ""
  summary "$1"
  exit 0
}

blocked() {
  echo "::error title=Claude review gate::$1"
  summary "### Claude review gate: geblokkeerd"
  summary ""
  summary "$1"
  exit 1
}

if [ "$IS_FORK" = "true" ]; then
  not_applicable "Deze PR komt van een fork. Fork-PR's krijgen geen secrets, dus \`CLAUDE_CODE_OAUTH_TOKEN\` ontbreekt en \`${CHECK_NAME}\` draait daar niet. Review deze wijziging met de hand voordat je merget."
fi

if [ "$IS_DRAFT" = "true" ]; then
  not_applicable "Deze PR staat op draft. \`${CHECK_NAME}\` draait pas bij \"ready for review\", en een draft is niet mergebaar."
fi

if [ "$PR_AUTHOR" = "dependabot[bot]" ]; then
  not_applicable "Deze PR komt van dependabot. Die loopt via de \`claude-dependabot\`-workflow, niet via \`${CHECK_NAME}\`."
fi

echo "Wachten op check-run '${CHECK_NAME}' voor commit ${HEAD_SHA} (max ${MAX_WAIT_SECONDS}s)."
deadline=$(($(date +%s) + MAX_WAIT_SECONDS))

while :; do
  if ! response=$(gh api "repos/${REPO}/commits/${HEAD_SHA}/check-runs?check_name=${CHECK_NAME}&per_page=100" 2>&1); then
    echo "API-aanroep mislukt, opnieuw proberen: ${response}"
    response='{"check_runs":[]}'
  fi

  total=$(jq '.check_runs | length' <<<"$response" 2>/dev/null || echo 0)
  unfinished=$(jq '[.check_runs[] | select(.status != "completed")] | length' <<<"$response" 2>/dev/null || echo 1)

  if [ "$total" -gt 0 ] && [ "$unfinished" -eq 0 ]; then
    break
  fi

  if [ "$(date +%s)" -ge "$deadline" ]; then
    if [ "$total" -eq 0 ]; then
      blocked "Na ${MAX_WAIT_SECONDS}s bestaat er geen check-run \`${CHECK_NAME}\` voor commit \`${HEAD_SHA}\`. De review is niet gestart; start de workflow opnieuw via de Actions-tab."
    fi
    blocked "Na ${MAX_WAIT_SECONDS}s is \`${CHECK_NAME}\` voor commit \`${HEAD_SHA}\` nog niet klaar. Draai deze job opnieuw zodra de review af is."
  fi

  echo "Nog niet klaar (${total} check-run(s), waarvan ${unfinished} bezig). Volgende poging over ${POLL_SECONDS}s."
  sleep "$POLL_SECONDS"
done

# Na een re-run staan er meerdere check-runs met dezelfde naam op dezelfde sha;
# de hoogste id is de meest recente.
latest=$(jq -c '.check_runs | sort_by(.id) | last' <<<"$response")
conclusion=$(jq -r '.conclusion // "none"' <<<"$latest")
url=$(jq -r '.html_url // ""' <<<"$latest")

case "$conclusion" in
success | neutral)
  echo "::notice title=Claude review gate::review afgerond (${conclusion})"
  summary "### Claude review gate: groen"
  summary ""
  summary "\`${CHECK_NAME}\` is afgerond voor commit \`${HEAD_SHA}\` met conclusie \`${conclusion}\`."
  summary ""
  summary "Wat deze check bewijst: de review is gedraaid en klaar voor precies deze commit. Wat hij niet bewijst: dat de bevindingen deugen of dat ze zijn verwerkt. Dat blijft mensenwerk."
  summary ""
  summary "[Bekijk de review-run](${url})"
  ;;
skipped)
  blocked "\`${CHECK_NAME}\` is overgeslagen voor commit \`${HEAD_SHA}\`, terwijl deze PR geen fork, geen draft en niet van dependabot is. Er is dus geen review. Zoek uit waarom de job is overgeslagen: ${url}"
  ;;
*)
  blocked "\`${CHECK_NAME}\` eindigde op \`${conclusion}\` voor commit \`${HEAD_SHA}\`. Er is geen bruikbare review. Draai hem opnieuw: ${url}"
  ;;
esac
