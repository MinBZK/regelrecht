#!/usr/bin/env bash
# Merge gate: block until the Claude review of this workflow run is done.
#
# Zero review comments is not evidence of "no findings" — it is equally
# consistent with "the review is still running" and with "the review had nothing
# to report". Whether there was a review is therefore never derived from the
# comments. It is read off the review job itself: its status and conclusion, and
# the outcome of the step that only runs when the review action really executed.
#
# Only once that is established does the gate read the comments, and only for
# what is in them: a `🔴 **Critical**` finding blocks the merge. It reads the
# summary comment, the inline comments and the body of the submitted review, and
# ignores anything written before the review job started, because that came from
# an earlier run. At that point zero comments does mean zero findings, because
# the review is known to have run to completion. The order is load-bearing, so it
# is enforced rather than implied: `assert_no_critical_finding` refuses to run
# before `assert_review_ran` has set `review_proven`. Turn the two around and the
# gate reads the empty window between "cleaning up" and "writing the review" as a
# clean bill.
#
# There is no override. The only way past a critical finding is to fix it and push
# again; the review rewrites its comment on every run, so a finding that is gone
# is gone from the gate too. A label or a magic comment would be an escape hatch
# an agent can operate as easily as a person, and it would read as a human
# judgement while being nothing of the sort.
#
# It exits 0 only when the review job ran to completion, that step ran with it and
# no critical finding stands, or when the review demonstrably cannot apply
# (cross-repo PR, draft, dependabot). Those reasons are read off the pull request
# itself, not off the review's own result, so a review that ran and failed can
# never pass as one that was never meant to run.
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
# De markering waar de review zijn kritieke bevindingen mee opschrijft, letterlijk
# zoals de prompt in ${WORKFLOW_FILE} hem voorschrijft. Verandert de een, dan moet
# de ander mee; de testsuite bindt de twee aan elkaar. Alleen Critical blokkeert:
# Significant heeft "waarschijnlijk" in zijn eigen definitie staan en zou vaak
# vals-positief zijn, en een uitzondering die routine wordt houdt niets tegen.
readonly CRITICAL_MARKER='🔴 **Critical**'
readonly REVIEW_AUTHOR='claude[bot]'

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
# De starttijd van de job scheidt wat déze review schreef van wat er van een
# vorige is blijven staan.
job_started=$(jq -r '.started_at // ""' <<<"$job" 2>/dev/null)

# Een groene job is geen bewijs dat er gereviewd is. Dat bewijs komt uit de
# review-job zelf: een stap die alleen draait als de actie `execution_file` heeft
# gezet, en dat doet zij pas nadat de CLI werkelijk gedraaid heeft. Die stap
# staat in het job-object dat hierboven al is opgehaald, dus het bewijs is per
# constructie aan déze run en deze attempt gebonden, en het bestaat ook wanneer
# de review nul bevindingen had.
#
# Precies één stap met die naam, niet de laatste die zo heet: twee stappen met
# dezelfde naam is geen situatie waarin de poort een winnaar hoort te kiezen.
review_proven=no

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
    review_proven=yes
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

# Waar de review zijn bevindingen achterlaat: de samenvattende comment, de inline
# comments, en de body van de review zelf. Die laatste wordt makkelijk vergeten en
# draagt in de praktijk bevindingen die nergens anders staan.
#
# Gekeken wordt alleen naar wat déze run schreef, afgemeten aan de starttijd van
# de review-job. Wat er ouder is komt van een vorige run: die comments worden na
# afloop opgeruimd, maar een submitted review is niet te verwijderen en een
# mislukte opruiming zou een gerepareerde bevinding anders voor altijd blijven
# blokkeren.
#
# Het resultaat komt in een globale variabele terug en niet uit een subshell:
# `blocked` stapt uit het proces, en in `$(...)` zou dat alleen de subshell
# beëindigen en de poort vrolijk verder laten lopen.
critical_at=''
collect_critical() {
  local what="$1" path="$2" stamp="$3" payload found
  if ! payload=$(gh api "repos/${REPO}/${path}?per_page=100" --paginate 2>"$gh_stderr"); then
    blocked "De ${what} van pull request ${PR_NUMBER} zijn niet op te halen, dus of de review een kritieke bevinding heeft achtergelaten is niet vastgesteld. Dat is geen uitspraak over de review zelf; draai deze job opnieuw. Foutmelding: $(cat "$gh_stderr")"
  fi
  # `--paginate` levert één array per pagina; elke daarvan moet een lijst zijn,
  # anders is er niets te doorzoeken en is stilzwijgend groen precies fout.
  if ! jq -e 'type == "array"' <<<"$payload" >/dev/null 2>&1; then
    blocked "Het antwoord op de ${what} van pull request ${PR_NUMBER} is geen leesbare lijst, dus of de review een kritieke bevinding heeft achtergelaten is niet vastgesteld. Dat is geen uitspraak over de review zelf; draai deze job opnieuw."
  fi
  # Een item zonder bruikbaar tijdstempel valt buiten het venster en zou stil
  # worden overgeslagen: dat is dezelfde fail-open als "niet kunnen kijken",
  # alleen onzichtbaar. Een `PENDING` review is de uitzondering; die is nooit
  # ingediend en heeft dus terecht geen `submitted_at`.
  local zonder_tijd
  zonder_tijd=$(jq -r --arg author "$REVIEW_AUTHOR" --arg stamp "$stamp" '
    [.[] | select(.user.login == $author) | select((.state // "") != "PENDING")
         | select((.[$stamp] // "") == "")] | length' <<<"$payload" 2>/dev/null)
  if [ "${zonder_tijd:-onbekend}" != 0 ]; then
    blocked "In de ${what} van pull request ${PR_NUMBER} staat wat \`${REVIEW_AUTHOR}\` schreef zonder bruikbaar tijdstempel (\`${stamp}\`), dus of het van deze review is valt niet vast te stellen. Dat is geen uitspraak over de review zelf; draai deze job opnieuw."
  fi

  if ! found=$(jq -r --arg author "$REVIEW_AUTHOR" --arg since "$job_started" \
    --arg stamp "$stamp" --arg marker "$CRITICAL_MARKER" '
      .[]
      | select(.user.login == $author)
      | select((.[$stamp] // "") >= $since)
      | select((.body // "") | contains($marker))
      | .html_url // "(zonder url)"' <<<"$payload" 2>/dev/null); then
    blocked "De ${what} van pull request ${PR_NUMBER} zijn niet te doorzoeken op \`${CRITICAL_MARKER}\`, dus of de review een kritieke bevinding heeft achtergelaten is niet vastgesteld. Draai deze job opnieuw."
  fi
  [ -n "$found" ] && critical_at="${critical_at}${found}"$'\n'
  return 0
}

# Pas hier telt een leeg antwoord als "niets gevonden". Daarvoor stond nog open of
# de review überhaupt gedraaid had, en dan is nul comments geen uitkomst maar een
# open venster: de review-job schrijft zijn comments aan het eind van de job, en
# daarvóór staat er niets.
assert_no_critical_finding() {
  local waar

  if [ "$review_proven" != yes ]; then
    blocked "Interne fout in de poort: de bevindingen zijn opgevraagd voordat vaststond dat \`${JOB_NAME}\` werkelijk gereviewd heeft. In die volgorde betekent \"geen kritieke bevinding\" niets. Dit is een fout in \`script/await-claude-review.sh\`, geen uitspraak over deze pull request."
  fi

  if [ -z "$job_started" ]; then
    blocked "De starttijd van \`${JOB_NAME}\` staat niet in het antwoord van de jobs-API, dus welke comments van deze review zijn en welke van een vorige is niet uit elkaar te houden. Dat is geen uitspraak over de review zelf; draai deze job opnieuw. ${url}"
  fi

  collect_critical "samenvattende review-comments" "issues/${PR_NUMBER}/comments" updated_at
  collect_critical "inline review-comments" "pulls/${PR_NUMBER}/comments" updated_at
  collect_critical "ingediende reviews" "pulls/${PR_NUMBER}/reviews" submitted_at

  if [ -z "$critical_at" ]; then
    return
  fi
  waar=$(printf '%s' "$critical_at" | paste -sd' ' -)

  blocked "De review heeft op commit \`${head_sha}\` een bevinding met \`${CRITICAL_MARKER}\` achtergelaten: ${waar}. Repareer wat er staat en push opnieuw: de review schrijft zijn comment bij elke run over, dus een bevinding die weg is, is daarna ook uit de poort weg. ${url}"
}

case "$conclusion" in
success | neutral)
  assert_review_ran
  assert_no_critical_finding
  echo "::notice title=Claude review gate::review afgerond (${conclusion})"
  summary "### Claude review gate: groen"
  summary ""
  summary "\`${JOB_NAME}\` is afgerond voor commit \`${head_sha}\` met conclusie \`${conclusion}\`."
  summary ""
  summary "Wat deze check bewijst: \`${WORKFLOW_FILE}\` is gelijk aan de versie op \`${default_branch}\`, de job \`${JOB_NAME}\` is in deze run afgerond met conclusie \`${conclusion}\`, de stap \`${PROOF_STEP}\` is in die job gedraaid, dus de review-actie heeft werkelijk gereviewd, en in wat \`${REVIEW_AUTHOR}\` sinds de start van die job schreef staat geen \`${CRITICAL_MARKER}\`. Wat hij niet bewijst: dat de bevindingen deugen, dat ze zijn verwerkt, of dat een bevinding van lagere ernst er niet toe doet — daar kijkt de poort niet naar. Dat blijft mensenwerk."
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
