#!/usr/bin/env bash
# Merge gate: block until the Claude review check-run for this commit is done.
#
# Zero review comments is not evidence of "no findings" — it is equally
# consistent with "the review is still running". So the wait is driven by the
# check-run's status, never by a comment count.
#
# It exits 0 only when the review demonstrably ran to completion for this exact
# commit and left something behind, or when the review demonstrably cannot apply
# (fork, draft, dependabot). Those reasons are read off the pull request itself,
# not off the review's own result, so a review that ran and failed can never
# pass as one that was never meant to run.
set -uo pipefail

: "${REPO:?REPO is verplicht}"
: "${HEAD_SHA:?HEAD_SHA is verplicht}"
: "${PR_NUMBER:?PR_NUMBER is verplicht}"

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

started_at=$(jq -r '.started_at // ""' <<<"$latest")

# Een groene check-run is niet genoeg. De claude-code-action stapt uit met
# conclusie `success` zonder ook maar iets te reviewen zodra het workflowbestand
# afwijkt van dat op de default branch ("Exiting due to workflow validation
# skip"). Gemeten op PR 1157: veertien seconden, groen, geen review. Eis daarom
# ook een spoor van de review zelf, geplaatst na de start van deze run. Dat is
# geen comment-telling vooraf — de status zegt of hij klaar is, dit zegt of hij
# iets heeft opgeleverd.
assert_review_output() {
  local comments reviews newest
  comments=$(gh api "repos/${REPO}/issues/${PR_NUMBER}/comments?per_page=100" --paginate 2>/dev/null || echo '[]')
  reviews=$(gh api "repos/${REPO}/pulls/${PR_NUMBER}/reviews?per_page=100" --paginate 2>/dev/null || echo '[]')
  newest=$(printf '%s\n%s\n' "$comments" "$reviews" |
    jq -rs '[.[][] | select(.user.login == "claude[bot]") | (.updated_at // .submitted_at)] | map(select(. != null)) | sort | last // ""' 2>/dev/null)

  if [ -z "$newest" ] || { [ -n "$started_at" ] && [[ "$newest" < "$started_at" ]]; }; then
    blocked "\`${CHECK_NAME}\` meldt \`${conclusion}\` voor commit \`${HEAD_SHA}\`, maar claude[bot] heeft in die run niets geplaatst. De review-actie slaat zichzelf over (met conclusie success) zodra \`.github/workflows/claude-code-review.yml\` afwijkt van de versie op de default branch. Wijzigt deze PR dat bestand, dan is er geen automatische review en moet een mens de wijziging nalopen. ${url}"
  fi
}

case "$conclusion" in
success | neutral)
  assert_review_output
  echo "::notice title=Claude review gate::review afgerond (${conclusion})"
  summary "### Claude review gate: groen"
  summary ""
  summary "\`${CHECK_NAME}\` is afgerond voor commit \`${HEAD_SHA}\` met conclusie \`${conclusion}\`."
  summary ""
  summary "Wat deze check bewijst: de review is gedraaid, klaar voor precies deze commit, en heeft daadwerkelijk iets geplaatst. Wat hij niet bewijst: dat de bevindingen deugen of dat ze zijn verwerkt. Dat blijft mensenwerk."
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
