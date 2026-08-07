#!/usr/bin/env bash
# Tests voor script/await-claude-review.sh.
#
# `gh` wordt vervangen door een stub die per endpoint antwoordt: check-runs uit
# een reeks (één antwoord per aanroep, zodat een lopende review die later klaar
# is te simuleren valt), comments en reviews uit vaste fixtures. Zo is elk pad
# deterministisch te bewijzen zonder GitHub.
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
GATE="${HERE}/await-claude-review.sh"

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

mkdir -p "${WORK}/bin"
cat >"${WORK}/bin/gh" <<'STUB'
#!/usr/bin/env bash
# Routeert op URL. Check-runs komen uit $STUB_RESPONSES, één antwoord per
# aanroep, zodat een nog lopende review die later klaar is te simuleren is.
# Comments en reviews komen uit vaste bestanden.
url="$2"
case "$url" in
*check-runs*)
  n=$(cat "$STUB_COUNTER")
  echo $((n + 1)) >"$STUB_COUNTER"
  mapfile -t responses <"$STUB_RESPONSES"
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
*/comments*) cat "$STUB_COMMENTS" ;;
*/reviews*) cat "$STUB_REVIEWS" ;;
*)
  echo "gh: onverwachte URL: $url" >&2
  exit 1
  ;;
esac
STUB
chmod +x "${WORK}/bin/gh"
export PATH="${WORK}/bin:${PATH}"

passed=0
failed=0

# run <naam> <verwachte exitcode> <verwachte tekst in output> -- env=waarde...
run() {
  local name="$1" want_code="$2" want_text="$3"
  shift 4  # naam, code, tekst, "--"

  export STUB_COUNTER="${WORK}/counter"
  export STUB_RESPONSES="${WORK}/responses"
  export STUB_COMMENTS="${WORK}/comments"
  export STUB_REVIEWS="${WORK}/reviews"
  echo 0 >"$STUB_COUNTER"
  [ -f "$STUB_COMMENTS" ] || echo '[]' >"$STUB_COMMENTS"
  [ -f "$STUB_REVIEWS" ] || echo '[]' >"$STUB_REVIEWS"

  local out code
  out=$(env "$@" \
    REPO='example-org/example-repo' \
    HEAD_SHA='0000000000000000000000000000000000000000' \
    PR_NUMBER='1' \
    POLL_SECONDS=0 \
    GITHUB_STEP_SUMMARY="${WORK}/summary.md" \
    STUB_COUNTER="$STUB_COUNTER" \
    STUB_RESPONSES="$STUB_RESPONSES" \
    STUB_COMMENTS="$STUB_COMMENTS" \
    STUB_REVIEWS="$STUB_REVIEWS" \
    "$GATE" 2>&1)
  code=$?

  if [ "$code" -ne "$want_code" ]; then
    echo "FAIL ${name}: exitcode ${code}, verwacht ${want_code}"
    printf '%s\n' "$out" | sed 's/^/     | /'
    failed=$((failed + 1))
    return
  fi
  if ! grep -qF -- "$want_text" <<<"$out"; then
    echo "FAIL ${name}: output mist \"${want_text}\""
    printf '%s\n' "$out" | sed 's/^/     | /'
    failed=$((failed + 1))
    return
  fi
  echo "ok   ${name}"
  passed=$((passed + 1))
}

responses() { printf '%s\n' "$@" >"${WORK}/responses"; }
comments() { printf '%s\n' "$1" >"${WORK}/comments"; }
reviews() { printf '%s\n' "$1" >"${WORK}/reviews"; }

RUNNING='{"check_runs":[{"id":1,"status":"in_progress","conclusion":null,"started_at":"2026-01-01T12:00:00Z","html_url":"http://example.invalid/1"}]}'
DONE_OK='{"check_runs":[{"id":1,"status":"completed","conclusion":"success","started_at":"2026-01-01T12:00:00Z","html_url":"http://example.invalid/1"}]}'
DONE_FAIL='{"check_runs":[{"id":1,"status":"completed","conclusion":"failure","started_at":"2026-01-01T12:00:00Z","html_url":"http://example.invalid/1"}]}'
DONE_CANCELLED='{"check_runs":[{"id":1,"status":"completed","conclusion":"cancelled","started_at":"2026-01-01T12:00:00Z","html_url":"http://example.invalid/1"}]}'
DONE_SKIPPED='{"check_runs":[{"id":1,"status":"completed","conclusion":"skipped","started_at":"2026-01-01T12:00:00Z","html_url":"http://example.invalid/1"}]}'
EMPTY='{"check_runs":[]}'
# Re-run: oude gefaalde run met lage id, nieuwe geslaagde run met hoge id.
RERUN='{"check_runs":[{"id":1,"status":"completed","conclusion":"failure","started_at":"2026-01-01T11:00:00Z","html_url":"http://example.invalid/1"},{"id":9,"status":"completed","conclusion":"success","started_at":"2026-01-01T12:00:00Z","html_url":"http://example.invalid/9"}]}'

NONE='[]'
STICKY='[{"user":{"login":"claude[bot]"},"created_at":"2026-01-01T12:00:30Z","updated_at":"2026-01-01T12:00:30Z"}]'
STICKY_STALE='[{"user":{"login":"claude[bot]"},"created_at":"2026-01-01T09:00:00Z","updated_at":"2026-01-01T09:00:00Z"}]'
HUMAN_ONLY='[{"user":{"login":"someone"},"created_at":"2026-01-01T12:00:30Z","updated_at":"2026-01-01T12:00:30Z"}]'
CLAUDE_REVIEW='[{"user":{"login":"claude[bot]"},"state":"COMMENTED","submitted_at":"2026-01-01T12:00:40Z"}]'

comments "$STICKY"
reviews "$NONE"

echo "== niet van toepassing =="
responses "$EMPTY"
run "fork-PR wacht niet en blokkeert niet" 0 "niet van toepassing" -- IS_FORK=true MAX_WAIT_SECONDS=0
run "draft-PR wacht niet en blokkeert niet" 0 "niet van toepassing" -- IS_DRAFT=true MAX_WAIT_SECONDS=0
run "dependabot-PR wacht niet en blokkeert niet" 0 "niet van toepassing" -- PR_AUTHOR='dependabot[bot]' MAX_WAIT_SECONDS=0

echo "== de poort sluit =="
responses "$RUNNING"
run "review loopt nog bij deadline: rood" 1 "nog niet klaar" -- MAX_WAIT_SECONDS=0
responses "$EMPTY"
run "review nooit gestart: rood" 1 "bestaat er geen check-run" -- MAX_WAIT_SECONDS=0
responses "$DONE_FAIL"
run "review gefaald: rood" 1 "eindigde op \`failure\`" -- MAX_WAIT_SECONDS=0
responses "$DONE_CANCELLED"
run "review geannuleerd: rood" 1 "eindigde op \`cancelled\`" -- MAX_WAIT_SECONDS=0
responses "$DONE_SKIPPED"
run "review overgeslagen zonder geldige reden: rood" 1 "is overgeslagen" -- MAX_WAIT_SECONDS=0

echo "== groen vraagt ook een spoor van de review zelf =="
# De claude-code-action stapt uit met conclusie success, zonder te reviewen en
# zonder iets te plaatsen, zodra het workflowbestand afwijkt van de default
# branch. Een groene check-run alleen is dus geen bewijs.
responses "$DONE_OK"
comments "$NONE"
reviews "$NONE"
run "groen zonder enig spoor van claude[bot]: rood" 1 "heeft in die run niets geplaatst" -- MAX_WAIT_SECONDS=0
comments "$HUMAN_ONLY"
run "alleen comments van mensen: rood" 1 "heeft in die run niets geplaatst" -- MAX_WAIT_SECONDS=0
comments "$STICKY_STALE"
run "spoor van vóór deze run telt niet: rood" 1 "heeft in die run niets geplaatst" -- MAX_WAIT_SECONDS=0

echo "== de poort opent =="
comments "$STICKY"
reviews "$NONE"
responses "$DONE_OK"
run "review afgerond met sticky comment: groen" 0 "review afgerond (success)" -- MAX_WAIT_SECONDS=0
comments "$NONE"
reviews "$CLAUDE_REVIEW"
run "een geplaatste review telt ook als spoor: groen" 0 "review afgerond (success)" -- MAX_WAIT_SECONDS=0
comments "$STICKY"
reviews "$NONE"
responses "$RERUN"
run "na re-run telt de nieuwste check-run" 0 "review afgerond (success)" -- MAX_WAIT_SECONDS=0
responses "$RUNNING" "$RUNNING" "$DONE_OK"
run "wacht door tot de review klaar is" 0 "review afgerond (success)" -- MAX_WAIT_SECONDS=60
responses "ERROR" "$DONE_OK"
run "API-fout is geen conclusie, er wordt doorgepolld" 0 "review afgerond (success)" -- MAX_WAIT_SECONDS=60

echo "== eerlijkheid over wat de check bewijst =="
if grep -qF 'Wat hij niet bewijst' "${WORK}/summary.md"; then
  echo "ok   groene samenvatting zegt erbij wat hij niet bewijst"
  passed=$((passed + 1))
else
  echo "FAIL groene samenvatting claimt te veel"
  failed=$((failed + 1))
fi

echo
echo "${passed} geslaagd, ${failed} gefaald"
[ "$failed" -eq 0 ]
