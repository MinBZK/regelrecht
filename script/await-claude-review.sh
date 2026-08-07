#!/usr/bin/env bash
# Merge gate: block until the Claude review of this workflow run is done.
#
# Zero review comments is not evidence of "no findings" — it is equally
# consistent with "the review is still running" and with "the review had nothing
# to report". The gate therefore never counts comments. It reads two things off
# the review job itself: its status and conclusion, and the outcome of the step
# that only runs when the review action really executed.
#
# It exits 0 only when the review job ran to completion and that step ran with
# it, or when the review demonstrably cannot apply (cross-repo PR, draft,
# dependabot). Those reasons are read off the pull request itself, not off the
# review's own result, so a review that ran and failed can never pass as one that
# was never meant to run.
#
# What the gate treats as fact it fetches itself. The workflow that invokes it
# lives in the pull request, so anything that workflow passes in is written by
# the author of the change under review: `IS_DRAFT: true` in the env block would
# otherwise be enough to declare the review inapplicable. Only the coordinates of
# the run (repository, run id, pull request number) come from the environment,
# and the gate checks that those three describe one and the same thing.
#
# What this cannot reach: the job that calls this script also lives in the pull
# request's workflow file. A PR that keeps the job name and replaces its steps
# reports green without ever running this script, and no line in here can stop
# that. Closing it takes a rule outside the pull request — a ruleset or CODEOWNERS
# entry over `.github/workflows/**`, or a `workflow_run`-triggered gate that
# judges the review run from the outside.
#
# The review job is looked up inside *this* workflow run rather than by head SHA.
# A `ready_for_review` or `reopened` event keeps the same SHA, so a SHA lookup can
# race and read the previous run's leftover check-run.
set -uo pipefail

: "${REPO:?REPO is verplicht}"
: "${RUN_ID:?RUN_ID is verplicht}"
: "${PR_NUMBER:?PR_NUMBER is verplicht}"

MAX_WAIT_SECONDS="${MAX_WAIT_SECONDS:-2100}"
POLL_SECONDS="${POLL_SECONDS:-20}"
GITHUB_STEP_SUMMARY="${GITHUB_STEP_SUMMARY:-/dev/null}"

# Vaste namen, bewust geen env-variabelen: ze staan in het workflowbestand dat de
# PR meebrengt, en een PR die ze mocht overschrijven kon de poort naar een stap
# wijzen die altijd draait.
readonly JOB_NAME='claude-review'
readonly PROOF_STEP='Record that the review ran'
readonly WORKFLOW_FILE='.github/workflows/claude-code-review.yml'

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

# stderr apart houden: een waarschuwing van `gh` op een verder geslaagde aanroep
# zou de payload anders onparseerbaar maken, en dat kwam dan naar buiten als een
# uitspraak over de review in plaats van als leesprobleem.
gh_stderr=$(mktemp "${TMPDIR:-/tmp}/gate-stderr.XXXXXX")
trap 'rm -f "$gh_stderr"' EXIT

if ! pr=$(gh api "repos/${REPO}/pulls/${PR_NUMBER}" 2>"$gh_stderr") ||
  ! head_sha=$(jq -er '.head.sha' <<<"$pr" 2>/dev/null); then
  blocked "Pull request ${PR_NUMBER} in ${REPO} is niet op te halen, dus er valt niets over de review vast te stellen. Dat is geen uitspraak over de review zelf; draai deze job opnieuw. Foutmelding: $(cat "$gh_stderr")"
fi

pr_field() { jq -r "${1} // \"\"" <<<"$pr"; }

# De koppeling tussen de drie coördinaten: run ${RUN_ID} moet over dezelfde
# commit gaan als pull request ${PR_NUMBER}. Zonder die toets kan een PR de poort
# naar de run van een andere, al gereviewde PR wijzen en is elke uitspraak
# hieronder waar — over die andere PR.
if ! run=$(gh api "repos/${REPO}/actions/runs/${RUN_ID}" 2>"$gh_stderr") ||
  ! run_head=$(jq -er '.head_sha' <<<"$run" 2>/dev/null); then
  blocked "Workflow-run ${RUN_ID} is niet op te halen, dus of deze run over pull request ${PR_NUMBER} gaat is niet vastgesteld. Dat is geen uitspraak over de review zelf; draai deze job opnieuw. Foutmelding: $(cat "$gh_stderr")"
fi
if [ "$run_head" != "$head_sha" ]; then
  blocked "Workflow-run ${RUN_ID} draait op commit \`${run_head}\`, terwijl pull request ${PR_NUMBER} op \`${head_sha}\` staat. De poort zou dan over een andere wijziging rapporteren dan er wordt gemerged. Draai de workflow opnieuw op de actuele commit."
fi

if [ "$(jq -r '.head.repo' <<<"$pr")" = "null" ]; then
  not_applicable "De head-repository van deze PR bestaat niet meer, dus er valt geen commit te reviewen en te mergen valt er ook niets."
fi

if [ "$(pr_field '.head.repo.full_name')" != "$REPO" ]; then
  not_applicable "Deze PR komt uit een andere repository (een fork). Zulke PR's krijgen geen secrets, dus \`CLAUDE_CODE_OAUTH_TOKEN\` ontbreekt en \`${JOB_NAME}\` draait daar niet. Review deze wijziging met de hand voordat je merget."
fi

if [ "$(pr_field '.draft')" = "true" ]; then
  not_applicable "Deze PR staat op draft. \`${JOB_NAME}\` draait pas bij \"ready for review\", en een draft is niet mergebaar."
fi

if [ "$(pr_field '.user.login')" = "dependabot[bot]" ]; then
  not_applicable "Deze PR komt van dependabot. Die loopt via de \`claude-dependabot\`-workflow, niet via \`${JOB_NAME}\`."
fi

# De claude-code-action weigert te draaien zodra het workflowbestand afwijkt van
# dat op de default branch: zij stapt uit met conclusie `success` zonder ook maar
# iets te reviewen ("Exiting due to workflow validation skip"). Gemeten op PR
# 1157: veertien seconden, groen, geen review.
#
# Die vergelijking maakt de poort zelf, vóór ze op de review gaat wachten. Dat
# scheelt niet alleen een halfuur wachten op een review die er niet komt; het is
# ook wat de rest van deze poort draagt. Zijn de twee versies gelijk, dan is het
# workflowbestand dat deze run draait dat van de default branch, en niet iets wat
# met de PR is meegekomen. Pas dan zegt de uitkomst van een stap in die workflow
# iets.
#
# Vergeleken wordt de merge-ref, niet de head-commit: een `pull_request`-run
# draait het workflowbestand zoals het in de test-merge van PR en base staat. Een
# PR die het bestand zelf niet aanraakt heeft daar dus de versie van de base
# staan, ook als de branch al weken achterloopt. Op `head.sha` vergelijken zou
# precies die achterlopers rood zetten terwijl hun review gewoon draaide.
blob_sha() {
  local ref payload
  ref="$1"
  payload=$(gh api "repos/${REPO}/contents/${WORKFLOW_FILE}?ref=${ref}" 2>"$gh_stderr") || return 1
  jq -er '.sha' <<<"$payload" 2>/dev/null
}

missing_file() { grep -qE 'HTTP 404|Not Found' "$gh_stderr"; }

default_branch=$(pr_field '.base.repo.default_branch')
[ -n "$default_branch" ] || blocked "De default branch van ${REPO} staat niet in het antwoord van de pull-request-API, dus \`${WORKFLOW_FILE}\` is niet te vergelijken. Draai deze job opnieuw."

merge_ref="refs/pull/${PR_NUMBER}/merge"
if ! run_workflow=$(blob_sha "$merge_ref"); then
  if missing_file; then
    blocked "\`${WORKFLOW_FILE}\` bestaat niet op \`${merge_ref}\`, de samenvoeging van deze PR met de base-branch. Zonder dat bestand is er geen review-workflow en kan de poort niets vaststellen. Opnieuw draaien helpt hier niet; zet het bestand terug of los het merge-conflict op."
  fi
  blocked "\`${WORKFLOW_FILE}\` is niet op te halen voor \`${merge_ref}\`, dus of de review-actie op deze PR draait is niet vastgesteld. Dat is geen uitspraak over de review zelf; draai deze job opnieuw. Foutmelding: $(cat "$gh_stderr")"
fi
if ! base_workflow=$(blob_sha "$default_branch"); then
  if missing_file; then
    blocked "\`${WORKFLOW_FILE}\` bestaat niet op branch \`${default_branch}\`. De review-actie valideert daartegen, dus zonder dat bestand draait zij nergens. Opnieuw draaien helpt hier niet."
  fi
  blocked "\`${WORKFLOW_FILE}\` is niet op te halen voor branch \`${default_branch}\`, dus of de review-actie op deze PR draait is niet vastgesteld. Dat is geen uitspraak over de review zelf; draai deze job opnieuw. Foutmelding: $(cat "$gh_stderr")"
fi

if [ "$run_workflow" != "$base_workflow" ]; then
  blocked "\`${WORKFLOW_FILE}\` is in deze run een andere versie dan die op \`${default_branch}\`: de blob op \`${merge_ref}\` is \`${run_workflow}\`, die op \`${default_branch}\` is \`${base_workflow}\`. De review-actie valideert het workflowbestand tegen de default branch en stapt bij een verschil uit zonder te reviewen, dus deze PR krijgt geen automatische review en de poort kan er niet voor instaan. Wijzigt deze PR dat bestand, dan moet een mens de wijziging nalopen."
fi

echo "Wachten op job '${JOB_NAME}' in workflow-run ${RUN_ID} (commit ${head_sha}, max ${MAX_WAIT_SECONDS}s)."
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

# Een groene job is geen bewijs dat er gereviewd is. Dat bewijs komt uit de
# review-job zelf: een stap die alleen draait als de actie `execution_file` heeft
# gezet, en dat doet zij pas nadat de CLI werkelijk gedraaid heeft. Die stap
# staat in het job-object dat hierboven al is opgehaald, dus het bewijs is per
# constructie aan déze run en deze attempt gebonden, en het bestaat ook wanneer
# de review nul bevindingen had.
#
# Precies één stap met die naam, niet de laatste die zo heet: twee stappen met
# dezelfde naam is geen situatie waarin de poort een winnaar hoort te kiezen.
assert_review_ran() {
  local matches proof
  matches=$(jq -r --arg step "$PROOF_STEP" '(.steps // []) | map(select(.name == $step)) | length' <<<"$job" 2>/dev/null)

  case "${matches:-onbekend}" in
  0)
    blocked "\`${JOB_NAME}\` meldt \`${conclusion}\`, maar de job kent geen stap \`${PROOF_STEP}\`, terwijl \`${WORKFLOW_FILE}\` gelijk is aan de versie op \`${default_branch}\`. Of er gereviewd is, valt daarmee niet af te lezen. Controleer of die stap nog in \`${WORKFLOW_FILE}\` staat. ${url}"
    ;;
  1) ;;
  onbekend)
    blocked "De stappen van \`${JOB_NAME}\` zijn niet uit het antwoord van de jobs-API te lezen, dus of er gereviewd is, is onbekend. Dat is geen uitspraak over de review zelf; draai deze job opnieuw. ${url}"
    ;;
  *)
    blocked "\`${JOB_NAME}\` kent ${matches} stappen met de naam \`${PROOF_STEP}\`. Welke daarvan het bewijs is, kan de poort niet uitmaken. Laat één zo'n stap staan in \`${WORKFLOW_FILE}\`. ${url}"
    ;;
  esac

  proof=$(jq -r --arg step "$PROOF_STEP" '(.steps // []) | map(select(.name == $step)) | .[0].conclusion // ""' <<<"$job" 2>/dev/null)
  case "$proof" in
  success)
    return
    ;;
  skipped)
    # De workflow-validation-skip is hierboven al uitgesloten: het
    # workflowbestand is gelijk aan dat op de default branch. Blijft over dat de
    # actie geen `execution_file` opleverde, bijvoorbeeld doordat zij vroegtijdig
    # uitstapte of doordat een upgrade die output hernoemde.
    blocked "\`${JOB_NAME}\` meldt \`${conclusion}\`, maar de stap \`${PROOF_STEP}\` is in die job overgeslagen. Die stap draait alleen als de review-actie een uitvoerbestand oplevert, dus dat is uitgebleven. \`${WORKFLOW_FILE}\` is gelijk aan de versie op \`${default_branch}\`, dus de bekende workflow-validation-skip verklaart dit niet. Lees het log van de review-job, en ga na of een upgrade van de review-actie de output \`execution_file\` heeft hernoemd. ${url}"
    ;;
  '')
    blocked "De stap \`${PROOF_STEP}\` in \`${JOB_NAME}\` heeft geen leesbare conclusie, dus of er gereviewd is, is onbekend. Dat is geen uitspraak over de review zelf; draai deze job opnieuw. ${url}"
    ;;
  *)
    blocked "\`${JOB_NAME}\` meldt \`${conclusion}\`, maar de stap \`${PROOF_STEP}\` eindigde op \`${proof}\`. Of er gereviewd is, is daarmee niet vastgesteld. Draai de review opnieuw: ${url}"
    ;;
  esac
}

case "$conclusion" in
success | neutral)
  assert_review_ran
  echo "::notice title=Claude review gate::review afgerond (${conclusion})"
  summary "### Claude review gate: groen"
  summary ""
  summary "\`${JOB_NAME}\` is afgerond voor commit \`${head_sha}\` met conclusie \`${conclusion}\`."
  summary ""
  summary "Wat deze check bewijst: \`${WORKFLOW_FILE}\` is gelijk aan de versie op \`${default_branch}\`, de job \`${JOB_NAME}\` is in deze run afgerond met conclusie \`${conclusion}\`, en de stap \`${PROOF_STEP}\` is in die job gedraaid, dus de review-actie heeft werkelijk gereviewd. Wat hij niet bewijst: dat de review iets gevonden heeft, dat de bevindingen deugen, of dat ze zijn verwerkt. Dat blijft mensenwerk."
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
