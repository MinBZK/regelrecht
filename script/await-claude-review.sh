#!/usr/bin/env bash
# Merge gate: block until the Claude review of this workflow run is done.
#
# Zero review comments is not evidence of "no findings" — it is equally
# consistent with "the review is still running". So the wait is driven by the
# job's status, never by a comment count.
#
# It exits 0 only when the review job ran to completion, or when the review
# demonstrably cannot apply (cross-repo PR, draft, dependabot). Those reasons are
# read off the pull request itself, not off the review's own result, so a review
# that ran and failed can never pass as one that was never meant to run.
#
# The review job is looked up inside *this* workflow run rather than by head SHA.
# A `ready_for_review` or `reopened` event keeps the same SHA, so a SHA lookup can
# race and read the previous run's leftover check-run.
set -uo pipefail

# Tijdstempels worden als string vergeleken; onder een collatie die interpunctie
# negeert klopt die volgorde niet meer.
export LC_ALL=C

: "${REPO:?REPO is verplicht}"
: "${RUN_ID:?RUN_ID is verplicht}"
: "${PR_NUMBER:?PR_NUMBER is verplicht}"

IS_CROSS_REPO="${IS_CROSS_REPO:-false}"
IS_DRAFT="${IS_DRAFT:-false}"
PR_AUTHOR="${PR_AUTHOR:-}"
HEAD_SHA="${HEAD_SHA:-onbekend}"
JOB_NAME="${JOB_NAME:-claude-review}"
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

if [ "$IS_CROSS_REPO" = "true" ]; then
  not_applicable "Deze PR komt uit een andere repository (een fork). Zulke PR's krijgen geen secrets, dus \`CLAUDE_CODE_OAUTH_TOKEN\` ontbreekt en \`${JOB_NAME}\` draait daar niet. Review deze wijziging met de hand voordat je merget."
fi

if [ "$IS_DRAFT" = "true" ]; then
  not_applicable "Deze PR staat op draft. \`${JOB_NAME}\` draait pas bij \"ready for review\", en een draft is niet mergebaar."
fi

if [ "$PR_AUTHOR" = "dependabot[bot]" ]; then
  not_applicable "Deze PR komt van dependabot. Die loopt via de \`claude-dependabot\`-workflow, niet via \`${JOB_NAME}\`."
fi

echo "Wachten op job '${JOB_NAME}' in workflow-run ${RUN_ID} (commit ${HEAD_SHA}, max ${MAX_WAIT_SECONDS}s)."
deadline=$(($(date +%s) + MAX_WAIT_SECONDS))
job=''
status=''
last_error=''

# `filter=all` is essentieel: het run-id blijft gelijk over attempts heen, dus na
# "Re-run this job" op alleen de poort zit `claude-review` niet in de joblijst van
# de nieuwste attempt. Over meerdere attempts is de volgorde niet gespecificeerd,
# vandaar `max_by(.id)`.
while :; do
  if response=$(gh api "repos/${REPO}/actions/runs/${RUN_ID}/jobs?filter=all&per_page=100" 2>/dev/null); then
    last_error=''
  else
    last_error="de jobs van workflow-run ${RUN_ID} waren niet op te halen"
    echo "API-aanroep mislukt, opnieuw proberen."
    response='{"jobs":[]}'
  fi

  job=$(jq -c --arg name "$JOB_NAME" '[.jobs[] | select(.name == $name)] | max_by(.id) // empty' <<<"$response" 2>/dev/null)
  status=''
  [ -n "$job" ] && status=$(jq -r '.status // ""' <<<"$job" 2>/dev/null)

  if [ "$status" = "completed" ]; then
    break
  fi

  if [ "$(date +%s)" -ge "$deadline" ]; then
    if [ -n "$last_error" ]; then
      blocked "Na ${MAX_WAIT_SECONDS}s is nog steeds niet vast te stellen of \`${JOB_NAME}\` klaar is: ${last_error}. Dat is geen uitspraak over de review zelf; draai deze job opnieuw."
    fi
    if [ -z "$job" ]; then
      blocked "Na ${MAX_WAIT_SECONDS}s zit er geen job \`${JOB_NAME}\` in workflow-run ${RUN_ID}. De review is niet gestart; start de hele workflow opnieuw via de Actions-tab (\"Re-run all jobs\")."
    fi
    blocked "Na ${MAX_WAIT_SECONDS}s is \`${JOB_NAME}\` nog niet klaar (status \`${status}\`). Draai deze job opnieuw zodra de review af is."
  fi

  echo "Nog niet klaar (status '${status:-geen job gevonden}'). Volgende poging over ${POLL_SECONDS}s."
  sleep "$POLL_SECONDS"
done

conclusion=$(jq -r '.conclusion // "none"' <<<"$job")
url=$(jq -r '.html_url // ""' <<<"$job")

case "$conclusion" in
success | neutral)
  echo "::notice title=Claude review gate::review afgerond (${conclusion})"
  summary "### Claude review gate: groen"
  summary ""
  summary "\`${JOB_NAME}\` is afgerond voor commit \`${HEAD_SHA}\` met conclusie \`${conclusion}\`."
  summary ""
  summary "Wat deze check bewijst: de job \`${JOB_NAME}\` is in deze run afgerond met conclusie \`${conclusion}\`. Wat hij niet bewijst: dat de review-actie binnen die job werkelijk een review heeft uitgevoerd, dat de bevindingen deugen, of dat ze zijn verwerkt."
  summary ""
  summary "[Bekijk de review-run](${url})"
  exit 0
  ;;
skipped)
  blocked "\`${JOB_NAME}\` is overgeslagen, terwijl deze PR geen fork, geen draft en niet van dependabot is. Er is dus geen review. Zoek uit waarom de job is overgeslagen: ${url}"
  ;;
*)
  blocked "\`${JOB_NAME}\` eindigde op \`${conclusion}\`. Er is geen bruikbare review. Draai hem opnieuw: ${url}"
  ;;
esac
