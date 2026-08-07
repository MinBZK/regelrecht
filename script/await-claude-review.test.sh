#!/usr/bin/env bash
# Tests voor script/await-claude-review.sh.
#
# `gh` wordt vervangen door een stub die de jobs van de workflow-run uit een
# reeks antwoordt: één antwoord per aanroep, zodat een lopende review die later
# klaar is te simuleren valt. Zo is elk pad deterministisch te bewijzen zonder
# GitHub. Elk ander endpoint laat de stub falen; de poort hoort niets anders te
# raadplegen dan de jobs van deze run.
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
GATE="${HERE}/await-claude-review.sh"

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

mkdir -p "${WORK}/bin"
cat >"${WORK}/bin/gh" <<'STUB'
#!/usr/bin/env bash
url="$2"
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

job() {
  printf '{"jobs":[{"name":"claude-review","status":"%s","conclusion":%s,"html_url":"http://example.invalid/job"}]}\n' \
    "$1" "$2"
}

RUNNING=$(job in_progress null)
DONE_OK=$(job completed '"success"')
DONE_NEUTRAL=$(job completed '"neutral"')
DONE_FAIL=$(job completed '"failure"')
DONE_CANCELLED=$(job completed '"cancelled"')
DONE_SKIPPED=$(job completed '"skipped"')
NO_JOB='{"jobs":[{"name":"iets-anders","status":"completed","conclusion":"success"}]}'
# Na "Re-run this job" op de poort staan er meerdere attempts van dezelfde job
# op één run-id; de hoogste id is de meest recente.
ATTEMPTS='{"jobs":[{"name":"claude-review","status":"completed","conclusion":"success","id":9,"html_url":"http://example.invalid/9"},{"name":"claude-review","status":"completed","conclusion":"failure","id":1,"html_url":"http://example.invalid/1"}]}'

passed=0
failed=0

jobs_are() { printf '%s\n' "$@" >"$STUB_JOBS"; }

# Elke test start van dezelfde grond: één afgeronde, geslaagde review. Een test
# zet alleen wat hij zelf wil bewijzen en kan niet per ongeluk slagen op de
# fixtures van zijn voorganger.
reset_fixtures() {
  jobs_are "$DONE_OK"
  echo 0 >"$STUB_COUNTER"
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
    HEAD_SHA='0000000000000000000000000000000000000000' \
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
# De review draait hier nog; de poort mag daar niet op gaan wachten.
reset_fixtures
jobs_are "$RUNNING"
check "cross-repo-PR wacht niet en blokkeert niet" 0 "niet van toepassing" "andere repository" -- IS_CROSS_REPO=true MAX_WAIT_SECONDS=0
reset_fixtures
jobs_are "$RUNNING"
check "draft-PR wacht niet en blokkeert niet" 0 "niet van toepassing" "draft" -- IS_DRAFT=true MAX_WAIT_SECONDS=0
reset_fixtures
jobs_are "$RUNNING"
check "dependabot-PR wacht niet en blokkeert niet" 0 "niet van toepassing" "dependabot" -- PR_AUTHOR='dependabot[bot]' MAX_WAIT_SECONDS=0

echo "== de poort sluit =="
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

echo "== de poort opent =="
# Het geval uit issue 1178: de review draaide volledig af en had niets aan te
# merken, dus claude[bot] plaatste geen enkele comment. Dat is een schone PR,
# geen ontbrekende review. De stub faalt op elk endpoint buiten de jobs van deze
# run, dus deze test slaagt alleen als de poort geen comments raadpleegt.
reset_fixtures
check "review af met nul bevindingen: groen" 0 "review afgerond (success)" "Wat hij niet bewijst" -- MAX_WAIT_SECONDS=0
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

echo
echo "${passed} geslaagd, ${failed} gefaald"
[ "$failed" -eq 0 ]
