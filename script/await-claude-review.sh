#!/usr/bin/env bash
# Merge gate: block until the Claude review of this workflow run is done.
#
# Zero review comments is not evidence of "no findings" — it is equally
# consistent with "the review is still running". So the wait is driven by the
# job's status, never by a comment count.
#
# It exits 0 only when the review demonstrably ran to completion and left
# something behind, or when the review demonstrably cannot apply (cross-repo PR,
# draft, dependabot). Those reasons are read off the pull request itself, not off
# the review's own result, so a review that ran and failed can never pass as one
# that was never meant to run.
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

while :; do
  if ! response=$(gh api "repos/${REPO}/actions/runs/${RUN_ID}/jobs?per_page=100" 2>&1); then
    echo "API-aanroep mislukt, opnieuw proberen: ${response}"
    response='{"jobs":[]}'
  fi

  job=$(jq -c --arg name "$JOB_NAME" '[.jobs[] | select(.name == $name)] | last // empty' <<<"$response" 2>/dev/null)
  status=''
  [ -n "$job" ] && status=$(jq -r '.status // ""' <<<"$job" 2>/dev/null)

  if [ "$status" = "completed" ]; then
    break
  fi

  if [ "$(date +%s)" -ge "$deadline" ]; then
    if [ -z "$job" ]; then
      blocked "Na ${MAX_WAIT_SECONDS}s zit er geen job \`${JOB_NAME}\` in workflow-run ${RUN_ID}. De review is niet gestart; start de workflow opnieuw via de Actions-tab."
    fi
    blocked "Na ${MAX_WAIT_SECONDS}s is \`${JOB_NAME}\` nog niet klaar (status \`${status}\`). Draai deze job opnieuw zodra de review af is."
  fi

  echo "Nog niet klaar (status '${status:-geen job gevonden}'). Volgende poging over ${POLL_SECONDS}s."
  sleep "$POLL_SECONDS"
done

conclusion=$(jq -r '.conclusion // "none"' <<<"$job")
url=$(jq -r '.html_url // ""' <<<"$job")
started_at=$(jq -r '.started_at // ""' <<<"$job")

# Een groene job is niet genoeg. De claude-code-action stapt uit met conclusie
# `success` zonder ook maar iets te reviewen zodra het workflowbestand afwijkt
# van dat op de default branch ("Exiting due to workflow validation skip").
# Gemeten op PR 1157: veertien seconden, groen, geen review. Eis daarom ook een
# spoor van de review zelf, geplaatst na de start van die job. Dat is geen
# comment-telling vooraf; de status zegt of hij klaar is, dit zegt of hij iets
# heeft opgeleverd.
assert_review_output() {
  if [ -z "$started_at" ]; then
    blocked "\`${JOB_NAME}\` levert geen \`started_at\`, dus is niet vast te stellen of een spoor van claude[bot] uit deze run komt of van een eerdere. Daar gokt de poort niet op. ${url}"
  fi

  local endpoint payload stamps='' newest
  for endpoint in "issues/${PR_NUMBER}/comments" "pulls/${PR_NUMBER}/reviews"; do
    if ! payload=$(gh api "repos/${REPO}/${endpoint}?per_page=100" --paginate 2>&1); then
      blocked "\`repos/${REPO}/${endpoint}\` is niet te lezen, dus of claude[bot] iets heeft geplaatst is onbekend. Dat is geen uitspraak over de review zelf; draai deze job opnieuw. Foutmelding: ${payload}"
    fi
    stamps+=$(jq -rs '.[] | arrays | .[] | select(.user.login == "claude[bot]") | (.updated_at // .submitted_at) | select(. != null)' <<<"$payload" 2>/dev/null)$'\n'
  done
  newest=$(printf '%s' "$stamps" | grep -v '^$' | sort | tail -1)

  if [ -z "$newest" ] || [[ "$newest" < "$started_at" ]]; then
    blocked "\`${JOB_NAME}\` meldt \`${conclusion}\`, maar claude[bot] heeft in die run niets geplaatst. De review-actie slaat zichzelf over (met conclusie success) zodra \`.github/workflows/claude-code-review.yml\` afwijkt van de versie op de default branch. Wijzigt deze PR dat bestand, dan is er geen automatische review en moet een mens de wijziging nalopen. ${url}"
  fi
}

case "$conclusion" in
success | neutral)
  assert_review_output
  echo "::notice title=Claude review gate::review afgerond (${conclusion})"
  summary "### Claude review gate: groen"
  summary ""
  summary "\`${JOB_NAME}\` is afgerond voor commit \`${HEAD_SHA}\` met conclusie \`${conclusion}\`."
  summary ""
  summary "Wat deze check bewijst: de review is gedraaid, klaar voor precies deze commit, en heeft daadwerkelijk iets geplaatst. Wat hij niet bewijst: dat de bevindingen deugen of dat ze zijn verwerkt. Dat blijft mensenwerk."
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
