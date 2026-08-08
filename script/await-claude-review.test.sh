#!/usr/bin/env bash
# Tests voor script/await-claude-review.sh.
#
# `gh` wordt vervangen door een stub die de endpoints kent die de poort gebruikt:
# de pull request, de blob-sha van het workflowbestand per ref, de jobs van de
# workflow-run en de comments van de PR. De jobs komen uit een reeks
# — één antwoord per aanroep — zodat een lopende review die later klaar is te
# simuleren valt. Zo is elk pad deterministisch te bewijzen zonder GitHub. Elk
# ander endpoint laat de stub falen.
#
# De stub schrijft elke aanroep weg in `$STUB_CALLS`. Daarmee is niet alleen te
# toetsen wát de poort besluit, maar ook in welke volgorde zij kijkt: de comments
# mogen pas worden opgevraagd nadat vaststaat dat er gereviewd is.
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
GATE="${HERE}/await-claude-review.sh"

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

mkdir -p "${WORK}/bin"
cat >"${WORK}/bin/gh" <<'STUB'
#!/usr/bin/env bash
url="$2"

fixture() {
  body=$(cat "$1")
  if [ "$body" = "ERROR" ]; then
    echo "gh: simulated API failure" >&2
    exit 1
  fi
  printf '%s\n' "$body"
}

case "$url" in
*/actions/runs/*/jobs*)
  n=$(cat "$STUB_COUNTER")
  echo $((n + 1)) >"$STUB_COUNTER"
  mapfile -t responses <"$STUB_JOBS"
  idx=$n
  if [ "$idx" -ge "${#responses[@]}" ]; then
    idx=$((${#responses[@]} - 1))
  fi
  line="${responses[$idx]}"
  if [ "$line" = "ERROR" ]; then
    echo "gh: simulated API failure" >&2
    exit 1
  fi
  printf '%s\n' "$line"
  ;;
*/issues/*/comments*)
  echo 'sticky-comments' >>"$STUB_CALLS"
  fixture "$STUB_STICKY_COMMENTS"
  ;;
*/pulls/*/comments*)
  echo 'inline-comments' >>"$STUB_CALLS"
  fixture "$STUB_INLINE_COMMENTS"
  ;;
*/pulls/*/reviews*)
  echo 'reviews' >>"$STUB_CALLS"
  fixture "$STUB_REVIEWS"
  ;;
*/actions/runs/*)
  body=$(cat "$STUB_RUN")
  if [ "$body" = "ERROR" ]; then
    echo "gh: simulated API failure" >&2
    exit 1
  fi
  printf '%s\n' "$body"
  ;;
*/contents/*)
  # De opgevraagde ref wordt vastgelegd, zodat een test kan asserten dat de
  # poort de merge-ref van déze PR vergelijkt en niet zomaar een commit.
  printf '%s\n' "${url#*ref=}" >>"$STUB_REFS"
  case "$url" in
  *ref=main) body=$(cat "$STUB_WORKFLOW_BASE") ;;
  *ref=refs/pull/1/merge) body=$(cat "$STUB_WORKFLOW_RUN") ;;
  *)
    echo "gh: HTTP 404: onverwachte ref in ${url}" >&2
    exit 1
    ;;
  esac
  if [ "$body" = "ERROR" ]; then
    echo "gh: HTTP 404" >&2
    exit 1
  fi
  if [ "$body" = "NETWERK" ]; then
    echo "gh: connection reset by peer" >&2
    exit 1
  fi
  printf '{"sha":"%s"}\n' "$body"
  ;;
*/pulls/*)
  body=$(cat "$STUB_PR")
  if [ "$body" = "ERROR" ]; then
    echo "gh: HTTP 403" >&2
    exit 1
  fi
  # WARN: geslaagde aanroep die ook naar stderr schrijft.
  if [ "$body" = "WARN" ]; then
    echo "gh: warning: this API is deprecated" >&2
    cat "$STUB_PR_WARN_BODY"
    exit 0
  fi
  printf '%s\n' "$body"
  ;;
*)
  echo "gh: onverwachte URL: $url" >&2
  exit 1
  ;;
esac
STUB
chmod +x "${WORK}/bin/gh"
export PATH="${WORK}/bin:${PATH}"

export STUB_COUNTER="${WORK}/counter"
export STUB_JOBS="${WORK}/jobs"
export STUB_PR="${WORK}/pr"
export STUB_PR_WARN_BODY="${WORK}/pr-warn-body"
export STUB_RUN="${WORK}/run"
export STUB_REFS="${WORK}/refs"
export STUB_WORKFLOW_RUN="${WORK}/workflow-run"
export STUB_WORKFLOW_BASE="${WORK}/workflow-base"
export STUB_STICKY_COMMENTS="${WORK}/sticky-comments"
export STUB_INLINE_COMMENTS="${WORK}/inline-comments"
export STUB_REVIEWS="${WORK}/reviews"
export STUB_CALLS="${WORK}/calls"

PROOF='Record that the review ran'
CRITICAL='🔴 **Critical**'
HEAD_SHA='c0ffee0000000000000000000000000000000000'

# pull_request <full_name van de head-repo> <draft> <auteur>
pull_request() {
  printf '{"draft":%s,"user":{"login":"%s"},"head":{"sha":"%s","repo":{"full_name":"%s"}},"base":{"repo":{"default_branch":"main"}}}' \
    "${2:-false}" "${3:-iemand}" "$HEAD_SHA" "${1:-example-org/example-repo}"
}

PR_NORMAAL=$(pull_request)
PR_FORK=$(pull_request 'iemand-anders/example-repo')
PR_DRAFT=$(pull_request 'example-org/example-repo' true)
PR_DEPENDABOT=$(pull_request 'example-org/example-repo' false 'dependabot[bot]')
PR_ZONDER_DEFAULT_BRANCH='{"draft":false,"user":{"login":"iemand"},"head":{"sha":"c0ffee0000000000000000000000000000000000","repo":{"full_name":"example-org/example-repo"}},"base":{"repo":{}}}'
# De head-repository is verwijderd; `head.repo` is dan null.
PR_ZONDER_HEAD_REPO='{"draft":false,"user":{"login":"iemand"},"head":{"sha":"c0ffee0000000000000000000000000000000000","repo":null},"base":{"repo":{"default_branch":"main"}}}'
GARBAGE='dit is geen JSON'

RUN_VAN_DEZE_PR=$(printf '{"head_sha":"%s"}' "$HEAD_SHA")
RUN_VAN_EEN_ANDERE_PR='{"head_sha":"beefbeef000000000000000000000000000000ff"}'

# De stappen van de review-job. `$1` is de conclusie van de bewijsstap; laat hem
# leeg voor een job die die stap helemaal niet kent.
steps_json() {
  if [ -z "${1:-}" ]; then
    printf '[{"name":"Run Claude Code Review","conclusion":"success"}]'
  else
    printf '[{"name":"Run Claude Code Review","conclusion":"success"},{"name":"%s","conclusion":"%s"}]' \
      "$PROOF" "$1"
  fi
}

# Een job zonder `steps`-sleutel; die is in het API-schema optioneel.
JOB_ZONDER_STEPS='{"jobs":[{"name":"claude-review","status":"completed","conclusion":"success","html_url":"http://example.invalid/job"}]}'
# Een `steps`-waarde die geen lijst is: dan valt er niets te tellen.
JOB_ONLEESBARE_STEPS='{"jobs":[{"name":"claude-review","status":"completed","conclusion":"success","steps":"geen lijst","html_url":"http://example.invalid/job"}]}'
# De bewijsstap staat er wel, maar zonder conclusie.
JOB_STAP_ZONDER_CONCLUSIE="{\"jobs\":[{\"name\":\"claude-review\",\"status\":\"completed\",\"conclusion\":\"success\",\"steps\":[{\"name\":\"${PROOF}\",\"conclusion\":null}],\"html_url\":\"http://example.invalid/job\"}]}"
# Twee stappen met dezelfde naam: een decoy naast het echte bewijs.
JOB_DUBBELE_STAP="{\"jobs\":[{\"name\":\"claude-review\",\"status\":\"completed\",\"conclusion\":\"success\",\"steps\":[{\"name\":\"${PROOF}\",\"conclusion\":\"skipped\"},{\"name\":\"${PROOF}\",\"conclusion\":\"success\"}],\"html_url\":\"http://example.invalid/job\"}]}"

# De starttijd van de review-job scheidt wat déze review schreef van wat er van
# een vorige is blijven staan.
JOB_START='2026-08-08T10:00:00Z'
TOEN='2026-08-08T09:00:00Z'
NU='2026-08-08T10:05:00Z'

job() {
  printf '{"jobs":[{"name":"claude-review","status":"%s","conclusion":%s,"steps":%s,"started_at":"%s","html_url":"http://example.invalid/job"}]}\n' \
    "$1" "$2" "$(steps_json "${3-success}")" "$JOB_START"
}

RUNNING=$(job in_progress null)
DONE_OK=$(job completed '"success"')
DONE_NEUTRAL=$(job completed '"neutral"')
DONE_FAIL=$(job completed '"failure"')
DONE_CANCELLED=$(job completed '"cancelled"')
DONE_SKIPPED=$(job completed '"skipped"')
# De review-actie stapte eruit voordat zij iets reviewde: de bewijsstap is
# overgeslagen, terwijl de job groen afsluit.
DONE_NO_REVIEW=$(job completed '"success"' skipped)
# Een workflow van vóór de bewijsstap; de job kent de stap niet.
DONE_NO_PROOF_STEP=$(job completed '"success"' '')
DONE_PROOF_FAILED=$(job completed '"success"' failure)
# Zonder starttijd is niet uit elkaar te houden wat van deze review is.
DONE_ZONDER_STARTTIJD="{\"jobs\":[{\"name\":\"claude-review\",\"status\":\"completed\",\"conclusion\":\"success\",\"steps\":$(steps_json success),\"html_url\":\"http://example.invalid/job\"}]}"
NO_JOB='{"jobs":[{"name":"iets-anders","status":"completed","conclusion":"success"}]}'
# Na "Re-run this job" op de poort staan er meerdere attempts van dezelfde job
# op één run-id; de hoogste id is de meest recente.
ATTEMPTS="{\"jobs\":[{\"name\":\"claude-review\",\"status\":\"completed\",\"conclusion\":\"success\",\"steps\":$(steps_json success),\"started_at\":\"${JOB_START}\",\"id\":9,\"html_url\":\"http://example.invalid/9\"},{\"name\":\"claude-review\",\"status\":\"completed\",\"conclusion\":\"failure\",\"steps\":$(steps_json skipped),\"started_at\":\"${JOB_START}\",\"id\":1,\"html_url\":\"http://example.invalid/1\"}]}"

# De comments zoals de API ze teruggeeft.
# `comment <auteur> <tekst> [tijdstip]`, standaard geschreven tijdens deze run.
comment() {
  printf '[{"user":{"login":"%s"},"body":"%s","updated_at":"%s","html_url":"http://example.invalid/comment"}]' \
    "$1" "$2" "${3:-$NU}"
}
# Een ingediende review draagt `submitted_at` in plaats van `updated_at`.
review() {
  printf '[{"user":{"login":"%s"},"state":"COMMENTED","body":"%s","submitted_at":"%s","html_url":"http://example.invalid/review"}]' \
    "$1" "$2" "${3:-$NU}"
}
GEEN_COMMENTS='[]'
COMMENT_MINOR=$(comment 'claude[bot]' '## Kwaliteit\n\n🟡 **Minor** — deze naam dekt de lading niet.')
COMMENT_CRITICAL=$(comment 'claude[bot]' "## Correctheid\n\n${CRITICAL} — hier gaat een uitkering de verkeerde kant op.")
# Een bevinding uit een vorige run: geschreven vóór deze review-job begon. De
# opruimstap haalt zulke comments weg, maar een ingediende review is niet te
# verwijderen, en een mislukte opruiming mag geen gerepareerde bevinding voor
# altijd blijven blokkeren.
COMMENT_CRITICAL_VAN_TOEN=$(comment 'claude[bot]' "## Correctheid\n\n${CRITICAL} — dit is al gerepareerd." "$TOEN")
REVIEW_CRITICAL=$(review 'claude[bot]' "## Correctheid\n\n${CRITICAL} — dit stond alleen in de body van de review.")
# Zonder tijdstempel valt een comment buiten het venster en zou hij stil worden
# overgeslagen: fail-open, en onzichtbaar.
COMMENT_ZONDER_TIJD='[{"user":{"login":"claude[bot]"},"body":"iets","html_url":"http://example.invalid/comment"}]'
# Een review die nog niet is ingediend heeft terecht geen `submitted_at`.
REVIEW_PENDING='[{"user":{"login":"claude[bot]"},"state":"PENDING","body":"nog niet ingediend","html_url":"http://example.invalid/review"}]'
REVIEW_CRITICAL_VAN_TOEN=$(review 'claude[bot]' "${CRITICAL} — uit een vorige run." "$TOEN")
# Dezelfde markering, maar van een mens die de bevinding bespreekt. Wie erover
# praat blokkeert de merge niet; alleen wat de review zelf schreef telt.
COMMENT_CRITICAL_VAN_EEN_MENS=$(comment 'iemand' "Ik denk dat die ${CRITICAL} onterecht is.")

passed=0
failed=0

jobs_are() { printf '%s\n' "$@" >"$STUB_JOBS"; }
pr_is() { printf '%s\n' "$1" >"$STUB_PR"; }
run_is() { printf '%s\n' "$1" >"$STUB_RUN"; }
workflow_is() { printf '%s\n' "$1" >"$STUB_WORKFLOW_RUN"; }
sticky_is() { printf '%s\n' "$1" >"$STUB_STICKY_COMMENTS"; }
inline_is() { printf '%s\n' "$1" >"$STUB_INLINE_COMMENTS"; }
reviews_are() { printf '%s\n' "$1" >"$STUB_REVIEWS"; }
comments_were_read() { grep -qxF 'sticky-comments' "$STUB_CALLS"; }

# Elke test start van dezelfde grond: een gewone PR waarvan het workflowbestand
# gelijk is aan dat op de default branch, en één afgeronde, geslaagde review
# waarvan de bewijsstap gedraaid heeft. Een test zet alleen wat hij zelf wil
# bewijzen en kan niet per ongeluk slagen op de fixtures van zijn voorganger.
reset_fixtures() {
  jobs_are "$DONE_OK"
  pr_is "$PR_NORMAAL"
  printf '%s\n' "$PR_NORMAAL" >"$STUB_PR_WARN_BODY"
  run_is "$RUN_VAN_DEZE_PR"
  workflow_is 'blob-van-main'
  printf '%s\n' 'blob-van-main' >"$STUB_WORKFLOW_BASE"
  sticky_is "$GEEN_COMMENTS"
  inline_is "$GEEN_COMMENTS"
  reviews_are "$GEEN_COMMENTS"
  echo 0 >"$STUB_COUNTER"
  : >"$STUB_REFS"
  : >"$STUB_CALLS"
  : >"${WORK}/summary.md"
}

# check <naam> <exitcode> <tekst in de output> <tekst in de step summary> -- env=waarde...
check() {
  local name="$1" want_code="$2" want_text="$3" want_summary="$4"
  shift 5

  local out code
  out=$(env "$@" \
    REPO='example-org/example-repo' \
    RUN_ID='42' \
    PR_NUMBER='1' \
    POLL_SECONDS=0 \
    GITHUB_STEP_SUMMARY="${WORK}/summary.md" \
    "$GATE" 2>&1)
  code=$?

  local problem=''
  if [ "$code" -ne "$want_code" ]; then
    problem="exitcode ${code}, verwacht ${want_code}"
  elif ! grep -qF -- "$want_text" <<<"$out"; then
    problem="output mist \"${want_text}\""
  elif ! grep -qF -- "$want_summary" "${WORK}/summary.md"; then
    problem="step summary mist \"${want_summary}\""
  fi

  if [ -n "$problem" ]; then
    echo "FAIL ${name}: ${problem}"
    printf '%s\n' "$out" | sed 's/^/     | /'
    failed=$((failed + 1))
    return
  fi
  echo "ok   ${name}"
  passed=$((passed + 1))
}

echo "== niet van toepassing =="
# De review draait hier nog; de poort mag daar niet op gaan wachten. Fork, draft
# en auteur haalt de poort zelf op bij de pull-request-API, niet uit de omgeving
# die het workflowbestand van de PR meegeeft.
reset_fixtures
jobs_are "$RUNNING"
pr_is "$PR_FORK"
check "cross-repo-PR wacht niet en blokkeert niet" 0 "niet van toepassing" "andere repository" -- MAX_WAIT_SECONDS=0
reset_fixtures
jobs_are "$RUNNING"
pr_is "$PR_DRAFT"
check "draft-PR wacht niet en blokkeert niet" 0 "niet van toepassing" "draft" -- MAX_WAIT_SECONDS=0
reset_fixtures
jobs_are "$RUNNING"
pr_is "$PR_DEPENDABOT"
check "dependabot-PR wacht niet en blokkeert niet" 0 "niet van toepassing" "dependabot" -- MAX_WAIT_SECONDS=0
# Een PR die de poort zelf een uitzondering probeert aan te praten via het
# env-blok van het workflowbestand: de poort leest die variabelen niet.
reset_fixtures
check "env-blok van de PR maakt geen uitzondering" 0 "review afgerond (success)" "groen" \
  -- IS_DRAFT=true IS_CROSS_REPO=true PR_AUTHOR='dependabot[bot]' MAX_WAIT_SECONDS=0
reset_fixtures
jobs_are "$DONE_NO_REVIEW"
check "env-blok van de PR wijst de poort niet naar een andere stap" 1 "overgeslagen" "overgeslagen" \
  -- PROOF_STEP='Run Claude Code Review' JOB_NAME='review-gate' WORKFLOW_FILE='README.md' MAX_WAIT_SECONDS=0

echo "== de poort sluit =="
reset_fixtures
pr_is ERROR
check "PR niet op te halen: rood, niet als 'geen review' gemeld" 1 "is niet op te halen" "draai deze job opnieuw" -- MAX_WAIT_SECONDS=0
reset_fixtures
pr_is "$GARBAGE"
check "onparseerbaar antwoord op de PR: rood met een eigen melding" 1 "is niet op te halen" "draai deze job opnieuw" -- MAX_WAIT_SECONDS=0
reset_fixtures
pr_is "$PR_ZONDER_DEFAULT_BRANCH"
check "PR zonder default branch: rood, want er valt niets te vergelijken" 1 "default branch" "niet te vergelijken" -- MAX_WAIT_SECONDS=0
# De drie coordinaten moeten over hetzelfde gaan: een run-id dat naar de run van
# een andere PR wijst, mag geen groen opleveren op andermans review.
reset_fixtures
run_is "$RUN_VAN_EEN_ANDERE_PR"
check "run hoort bij een andere commit: rood" 1 "draait op commit" "andere wijziging" -- MAX_WAIT_SECONDS=0
reset_fixtures
run_is ERROR
check "workflow-run niet op te halen: rood" 1 "Workflow-run 42 is niet op te halen" "draai deze job opnieuw" -- MAX_WAIT_SECONDS=0
reset_fixtures
pr_is "$PR_ZONDER_HEAD_REPO"
check "verwijderde head-repo: niet van toepassing, met de juiste reden" 0 "bestaat niet meer" "bestaat niet meer" -- MAX_WAIT_SECONDS=0
reset_fixtures
jobs_are "$RUNNING"
check "review loopt nog bij deadline: rood" 1 "nog niet klaar" "nog niet klaar" -- MAX_WAIT_SECONDS=0
reset_fixtures
jobs_are "$NO_JOB"
check "review-job zit niet in de run: rood" 1 "zit er geen job" "zit er geen job" -- MAX_WAIT_SECONDS=0
reset_fixtures
jobs_are "$DONE_FAIL"
check "review gefaald: rood" 1 'eindigde op `failure`' 'eindigde op `failure`' -- MAX_WAIT_SECONDS=0
reset_fixtures
jobs_are "$DONE_CANCELLED"
check "review geannuleerd: rood" 1 'eindigde op `cancelled`' 'eindigde op `cancelled`' -- MAX_WAIT_SECONDS=0
reset_fixtures
jobs_are "$DONE_SKIPPED"
check "review overgeslagen zonder geldige reden: rood" 1 "is overgeslagen" "is overgeslagen" -- MAX_WAIT_SECONDS=0

reset_fixtures
jobs_are ERROR
check "jobs-API blijft onleesbaar tot de deadline: rood, niet als 'job ontbreekt'" 1 "niet op te halen" "niet op te halen" -- MAX_WAIT_SECONDS=0

echo "== een workflowbestand dat afwijkt van de default branch =="
# De claude-code-action weigert te draaien zodra dat bestand afwijkt: zij stapt
# uit met conclusie success zonder te reviewen. De poort stelt die afwijking zelf
# vast en wacht niet eerst een halfuur op een review die niet komt.
reset_fixtures
jobs_are "$RUNNING"
workflow_is 'blob-van-de-pr'
check "workflowbestand wijkt af: rood, zonder te wachten" 1 "is in deze run een andere versie" "een mens de wijziging nalopen" -- MAX_WAIT_SECONDS=0
# Vergeleken wordt de merge-ref van deze PR: dat is het bestand dat de run
# werkelijk draaide. De stub weigert elke andere ref, dus deze test valt om zodra
# de poort op de head-commit gaat vergelijken.
if grep -qxF 'refs/pull/1/merge' "$STUB_REFS" && grep -qxF 'main' "$STUB_REFS"; then
  echo "ok   vergeleken wordt de merge-ref van deze PR tegen de default branch"
  passed=$((passed + 1))
else
  echo "FAIL de poort vroeg de refs $(tr '\n' ' ' <"$STUB_REFS")op, niet refs/pull/1/merge en main"
  failed=$((failed + 1))
fi
reset_fixtures
jobs_are "$RUNNING"
workflow_is ERROR
check "workflowbestand ontbreekt op de merge-ref: rood, opnieuw draaien helpt niet" 1 "bestaat niet op" "helpt hier niet" -- MAX_WAIT_SECONDS=0
reset_fixtures
jobs_are "$RUNNING"
workflow_is NETWERK
check "workflowbestand onleesbaar door een leesfout: rood, draai opnieuw" 1 "niet op te halen voor" "draai deze job opnieuw" -- MAX_WAIT_SECONDS=0
reset_fixtures
jobs_are "$RUNNING"
printf '%s\n' ERROR >"$STUB_WORKFLOW_BASE"
check "workflowbestand ontbreekt op de default branch: rood" 1 "bestaat niet op branch" "helpt hier niet" -- MAX_WAIT_SECONDS=0

echo "== een groene job die niets reviewde =="
# Het workflowbestand is hier gelijk aan dat op de default branch, dus de
# workflow-validation-skip is uitgesloten en de melding zegt dat er ook bij.
reset_fixtures
jobs_are "$DONE_NO_REVIEW"
check "bewijsstap overgeslagen: rood" 1 "is in die job overgeslagen" "verklaart dit niet" -- MAX_WAIT_SECONDS=0
reset_fixtures
jobs_are "$DONE_NO_PROOF_STEP"
check "job zonder bewijsstap: rood" 1 "kent geen stap" "kent geen stap" -- MAX_WAIT_SECONDS=0
reset_fixtures
jobs_are "$JOB_ZONDER_STEPS"
check "job zonder stappenlijst: rood" 1 "kent geen stap" "kent geen stap" -- MAX_WAIT_SECONDS=0
# Twee stappen met dezelfde naam: de poort kiest daar geen winnaar uit, want dan
# zou één toegevoegde stap het bewijs kunnen overstemmen.
reset_fixtures
jobs_are "$JOB_DUBBELE_STAP"
check "twee stappen met de bewijsnaam: rood" 1 "2 stappen met de naam" "kan de poort niet uitmaken" -- MAX_WAIT_SECONDS=0
reset_fixtures
jobs_are "$DONE_PROOF_FAILED"
check "bewijsstap gefaald: rood" 1 'eindigde op `failure`' 'eindigde op `failure`' -- MAX_WAIT_SECONDS=0
# De twee "we weten het niet"-armen horen rood te zijn, niet groen bij gebrek aan
# een leesbare uitkomst.
reset_fixtures
jobs_are "$JOB_ONLEESBARE_STEPS"
check "stappen niet te lezen: rood" 1 "zijn niet uit het antwoord" "draai deze job opnieuw" -- MAX_WAIT_SECONDS=0
reset_fixtures
jobs_are "$JOB_STAP_ZONDER_CONCLUSIE"
check "bewijsstap zonder conclusie: rood" 1 "geen leesbare conclusie" "draai deze job opnieuw" -- MAX_WAIT_SECONDS=0

echo "== een kritieke bevinding =="
reset_fixtures
sticky_is "$COMMENT_CRITICAL"
check "kritieke bevinding in de samenvattende review: rood" 1 "heeft op commit" "Repareer wat er staat" -- MAX_WAIT_SECONDS=0
reset_fixtures
inline_is "$COMMENT_CRITICAL"
check "kritieke bevinding in een inline comment: rood" 1 "heeft op commit" "Repareer wat er staat" -- MAX_WAIT_SECONDS=0
# Er is geen uitweg, en de melding hoort er ook geen te suggereren.
reset_fixtures
sticky_is "$COMMENT_CRITICAL"
check "de melding wijst naar repareren en pushen, niet naar een uitweg" 1 "Repareer wat er staat en push opnieuw" "Repareer wat er staat en push opnieuw" -- MAX_WAIT_SECONDS=0
reset_fixtures
reviews_are "$REVIEW_CRITICAL"
check "kritieke bevinding in de body van de ingediende review: rood" 1 "heeft op commit" "Repareer wat er staat" -- MAX_WAIT_SECONDS=0
# De melding wijst aan waar de bevinding staat, anders is een onterechte rood
# niet te vinden.
reset_fixtures
inline_is "$COMMENT_CRITICAL"
check "de melding noemt waar de bevinding staat" 1 "http://example.invalid/comment" "http://example.invalid/comment" -- MAX_WAIT_SECONDS=0

# Alleen wat déze run schreef telt. Een ingediende review is niet te verwijderen
# en een mislukte opruiming laat comments staan; zonder deze grens zou een
# gerepareerde bevinding voor altijd blijven blokkeren.
reset_fixtures
sticky_is "$COMMENT_CRITICAL_VAN_TOEN"
check "kritieke bevinding van vóór deze review-job blokkeert niet" 0 "review afgerond (success)" "Wat hij niet bewijst" -- MAX_WAIT_SECONDS=0
reset_fixtures
reviews_are "$REVIEW_CRITICAL_VAN_TOEN"
check "review-body uit een vorige run blokkeert niet" 0 "review afgerond (success)" "Wat hij niet bewijst" -- MAX_WAIT_SECONDS=0
reset_fixtures
sticky_is "$COMMENT_ZONDER_TIJD"
check "comment zonder tijdstempel: rood, niet stil overgeslagen" 1 "zonder bruikbaar tijdstempel" "draai deze job opnieuw" -- MAX_WAIT_SECONDS=0
reset_fixtures
reviews_are "$REVIEW_PENDING"
check "een nog niet ingediende review blokkeert niet" 0 "review afgerond (success)" "Wat hij niet bewijst" -- MAX_WAIT_SECONDS=0
reset_fixtures
jobs_are "$DONE_ZONDER_STARTTIJD"
sticky_is "$COMMENT_CRITICAL"
check "job zonder starttijd: rood, want de grens ontbreekt" 1 "starttijd van" "draai deze job opnieuw" -- MAX_WAIT_SECONDS=0

reset_fixtures
sticky_is "$COMMENT_MINOR"
check "bevinding van lagere ernst blokkeert niet" 0 "review afgerond (success)" "Wat hij niet bewijst" -- MAX_WAIT_SECONDS=0
reset_fixtures
sticky_is "$COMMENT_CRITICAL_VAN_EEN_MENS"
check "een mens die de markering citeert blokkeert niet" 0 "review afgerond (success)" "Wat hij niet bewijst" -- MAX_WAIT_SECONDS=0

# Niet kunnen kijken is geen schone uitslag: dat is niet te onderscheiden van
# "er staat niets", en dan hoort de poort dicht te blijven.
reset_fixtures
sticky_is ERROR
check "samenvattende comments niet op te halen: rood" 1 "zijn niet op te halen" "draai deze job opnieuw" -- MAX_WAIT_SECONDS=0
reset_fixtures
inline_is ERROR
check "inline comments niet op te halen: rood" 1 "zijn niet op te halen" "draai deze job opnieuw" -- MAX_WAIT_SECONDS=0
reset_fixtures
reviews_are ERROR
check "ingediende reviews niet op te halen: rood" 1 "zijn niet op te halen" "draai deze job opnieuw" -- MAX_WAIT_SECONDS=0
reset_fixtures
sticky_is "$GARBAGE"
check "onleesbaar antwoord op de samenvattende comments: rood" 1 "geen leesbare lijst" "geen leesbare lijst" -- MAX_WAIT_SECONDS=0
reset_fixtures
inline_is "$GARBAGE"
check "onleesbaar antwoord op de inline comments: rood" 1 "geen leesbare lijst" "geen leesbare lijst" -- MAX_WAIT_SECONDS=0
reset_fixtures
reviews_are "$GARBAGE"
check "onleesbaar antwoord op de ingediende reviews: rood" 1 "geen leesbare lijst" "geen leesbare lijst" -- MAX_WAIT_SECONDS=0

echo "== de bevindingen worden pas gelezen als vaststaat dat er gereviewd is =="
# De review-job wist eerst de comments van de vorige review en schrijft daarna de
# nieuwe. Wie in dat venster gaat tellen leest nul en noemt dat schoon. De poort
# mag de comments dus pas opvragen nadat de bewijsstap is vastgesteld, en deze
# tests bewijzen dat aan de aanroepen die de stub optekent.
reset_fixtures
jobs_are "$DONE_NO_REVIEW"
sticky_is "$COMMENT_CRITICAL"
check "bewijsstap overgeslagen: rood op de bewijsstap, niet op de bevinding" 1 "is in die job overgeslagen" "verklaart dit niet" -- MAX_WAIT_SECONDS=0
if comments_were_read; then
  echo "FAIL de poort las de comments terwijl nog niet vaststond dat er gereviewd was"
  failed=$((failed + 1))
else
  echo "ok   geen bewijs dat er gereviewd is: de comments blijven ongelezen"
  passed=$((passed + 1))
fi
reset_fixtures
jobs_are "$RUNNING"
check "review loopt nog: de comments blijven ongelezen" 1 "nog niet klaar" "nog niet klaar" -- MAX_WAIT_SECONDS=0
if comments_were_read; then
  echo "FAIL de poort las de comments van een review die nog liep"
  failed=$((failed + 1))
else
  echo "ok   review nog niet klaar: de comments blijven ongelezen"
  passed=$((passed + 1))
fi

echo "== de poort opent =="
# Het geval uit issue 1178: de review draaide volledig af en had niets aan te
# merken, dus claude[bot] plaatste geen enkele comment. Dat is een schone PR,
# geen ontbrekende review. Nul comments telt hier wél als nul bevindingen, juist
# omdat de bewijsstap er al voor stond.
reset_fixtures
check "review af met nul bevindingen: groen" 0 "review afgerond (success)" "Wat hij niet bewijst" -- MAX_WAIT_SECONDS=0
if comments_were_read; then
  echo "ok   met bewijs van de review worden de comments wél gelezen"
  passed=$((passed + 1))
else
  echo "FAIL de poort werd groen zonder de comments te lezen"
  failed=$((failed + 1))
fi
reset_fixtures
jobs_are "$DONE_NEUTRAL"
check "conclusie neutral telt als afgerond: groen" 0 "review afgerond (neutral)" "groen" -- MAX_WAIT_SECONDS=0
reset_fixtures
jobs_are "$ATTEMPTS"
check "meerdere attempts: de nieuwste job telt" 0 "review afgerond (success)" "groen" -- MAX_WAIT_SECONDS=0
reset_fixtures
jobs_are "$RUNNING" "$RUNNING" "$DONE_OK"
check "wacht door tot de review klaar is" 0 "review afgerond (success)" "groen" -- MAX_WAIT_SECONDS=60
reset_fixtures
jobs_are ERROR "$DONE_OK"
check "API-fout op de jobs is geen conclusie, er wordt doorgepolld" 0 "review afgerond (success)" "groen" -- MAX_WAIT_SECONDS=60
reset_fixtures
pr_is WARN
check "waarschuwing op stderr bederft een geslaagde aanroep niet: groen" 0 "review afgerond (success)" "groen" -- MAX_WAIT_SECONDS=0

echo "== de workflow levert het bewijs dat de poort zoekt =="
# De poort kent de naam van de bewijsstap als vaste waarde; hernoemen in de
# workflow zonder de poort mee te nemen zet elke PR op rood. Deze test bindt de
# twee aan elkaar.
WORKFLOW="$(cd "${HERE}/.." && pwd)/.github/workflows/claude-code-review.yml"
if grep -qF -- "- name: ${PROOF}" "$WORKFLOW" &&
  grep -qF -- "if: steps.claude-review.outputs.execution_file != ''" "$WORKFLOW"; then
  echo "ok   de bewijsstap staat onder die naam in het workflowbestand"
  passed=$((passed + 1))
else
  echo "FAIL de bewijsstap staat niet onder die naam in ${WORKFLOW}, of niet met de verwachte conditie"
  failed=$((failed + 1))
fi

# De poort zoekt de job op onder de naam `claude-review`. Krijgt die job een
# `name:`, dan heet zij in de API anders en wordt elke PR rood.
if grep -qxF '  claude-review:' "$WORKFLOW" &&
  awk '/^  claude-review:/ {inside = 1; next} /^  [a-z]/ {inside = 0} inside && /^    name:/ {found = 1} END {exit found}' "$WORKFLOW"; then
  echo "ok   de review-job heet in de API nog \`claude-review\`"
  passed=$((passed + 1))
else
  echo "FAIL de job \`claude-review\` ontbreekt in ${WORKFLOW} of heeft een eigen \`name:\`, waardoor de poort haar niet vindt"
  failed=$((failed + 1))
fi

# Een kritieke bevinding kent geen uitweg. Een label, een commentaar-commando of
# een omgevingsvariabele zou een uitweg zijn die een agent net zo makkelijk zet
# als een mens, terwijl hij leest als een menselijk oordeel.
if grep -qF '/labels' "$GATE"; then
  echo "FAIL de poort leest labels, en dat kan alleen om er een uitweg aan op te hangen"
  failed=$((failed + 1))
else
  echo "ok   de poort kent geen uitweg langs een kritieke bevinding"
  passed=$((passed + 1))
fi

# De markering waarop de poort rood wordt, is de markering die de prompt de
# review voorschrijft. Wijkt de een af, dan leest de poort langs elke bevinding
# heen en is dat niet aan de uitslag te zien.
if grep -qF -- "$CRITICAL" "$WORKFLOW" && grep -qF -- "$CRITICAL" "$GATE"; then
  echo "ok   poort en prompt gebruiken dezelfde markering voor een kritieke bevinding"
  passed=$((passed + 1))
else
  echo "FAIL de markering ${CRITICAL} staat niet in zowel ${WORKFLOW} als ${GATE}"
  failed=$((failed + 1))
fi

# Opruimen ná de review. Staat het ervoor, dan is een bevinding uit een geklapte
# run weg en is opnieuw pushen genoeg om de poort te laten vergeten.
snapshot_line=$(grep -n -- '- name: Snapshot the previous review' "$WORKFLOW" | cut -d: -f1)
review_line=$(grep -n -- '- name: Run Claude Code Review' "$WORKFLOW" | cut -d: -f1)
cleanup_line=$(grep -n -- '- name: Clean up the superseded review' "$WORKFLOW" | cut -d: -f1)
if [ -n "$snapshot_line" ] && [ -n "$review_line" ] && [ -n "$cleanup_line" ] &&
  [ "$snapshot_line" -lt "$review_line" ] && [ "$review_line" -lt "$cleanup_line" ]; then
  echo "ok   de momentopname staat vóór de review en het opruimen erna"
  passed=$((passed + 1))
else
  echo "FAIL ${WORKFLOW} ruimt niet in de volgorde momentopname, review, opruimen"
  failed=$((failed + 1))
fi

# Het opruimen mag alleen draaien als de review werkelijk gedraaid heeft; anders
# gooit een geklapte run de vorige bevinding alsnog weg.
if awk '/- name: Clean up the superseded review/ {inside = 1; next} /^      - name:/ {inside = 0} inside && /if: steps.claude-review.outputs.execution_file/ {found = 1} END {exit !found}' "$WORKFLOW"; then
  echo "ok   het opruimen hangt aan hetzelfde bewijs als de bewijsstap"
  passed=$((passed + 1))
else
  echo "FAIL de opruimstap in ${WORKFLOW} draait niet onder de conditie dat de review een uitvoerbestand opleverde"
  failed=$((failed + 1))
fi

# De teksten uit de momentopname gaan als context de prompt in. Zonder die
# koppeling weet de volgende review niets van de vorige bevinding, en dan is
# blijven pushen genoeg om een blokkade kwijt te raken.
if grep -qF -- 'steps.snapshot.outputs.context' "$WORKFLOW"; then
  echo "ok   de vorige bevindingen gaan als context de prompt in"
  passed=$((passed + 1))
else
  echo "FAIL de prompt in ${WORKFLOW} krijgt de bevindingen van de vorige review niet mee"
  failed=$((failed + 1))
fi

echo
echo "${passed} geslaagd, ${failed} gefaald"
[ "$failed" -eq 0 ]
