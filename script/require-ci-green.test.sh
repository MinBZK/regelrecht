#!/usr/bin/env bash
# Dekt elk pad dat groen of rood beslist in script/require-ci-green.sh, met een
# `gh`-stub op PATH. Zonder deze test is een poort die per ongeluk altijd
# doorlaat niet te onderscheiden van een die werkt.
set -uo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
gate="$here/require-ci-green.sh"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

pass=0
fail=0

# Bouwt een `gh`-stub die één runs-antwoord teruggeeft. $1 is de JSON-array met
# workflow_runs; de stub voert de meegegeven --jq erop uit met de echte jq.
stub_gh() {
    cat >"$tmp/gh" <<STUB
#!/usr/bin/env bash
# Alleen de aanroep die de poort doet: api ... --jq <filter>
filter=""
for ((i = 1; i <= \$#; i++)); do
    if [ "\${!i}" = "--jq" ]; then
        j=\$((i + 1))
        filter="\${!j}"
    fi
done
printf '%s' '{"workflow_runs": $1}' | jq -r "\$filter"
STUB
    chmod +x "$tmp/gh"
}

# $1 = naam, $2 = verwachte exitcode, $3 = workflow_runs-JSON,
# $4 = optioneel: patroon dat in de uitvoer moet staan.
check() {
    local name="$1" want="$2" runs="$3" needle="${4:-}"
    stub_gh "$runs"
    local out status
    out="$(PATH="$tmp:$PATH" REPO=o/r SHA=deadbeef MAX_WAIT_SECONDS=0 POLL_SECONDS=0 \
        bash "$gate" 2>&1)"
    status=$?

    if [ "$status" -ne "$want" ]; then
        echo "FAIL: $name — exit $status, verwacht $want"
        echo "$out" | sed 's/^/    /'
        fail=$((fail + 1))
        return
    fi
    if [ -n "$needle" ] && ! grep -qF "$needle" <<<"$out"; then
        echo "FAIL: $name — '$needle' niet in de uitvoer"
        echo "$out" | sed 's/^/    /'
        fail=$((fail + 1))
        return
    fi
    echo "ok: $name"
    pass=$((pass + 1))
}

run() { # status, conclusion, gestart-op
    printf '{"name":"CI","status":"%s","conclusion":%s,"run_started_at":"%s","html_url":"u"}' \
        "$1" "$2" "$3"
}

check "geslaagde CI laat de deploy door" 0 \
    "[$(run completed '"success"' 2026-08-07T10:00:00Z)]" \
    "is geslaagd"

check "gefaalde CI blokkeert" 1 \
    "[$(run completed '"failure"' 2026-08-07T10:00:00Z)]" \
    "eindigde als 'failure'"

check "afgebroken CI blokkeert" 1 \
    "[$(run completed '"cancelled"' 2026-08-07T10:00:00Z)]" \
    "eindigde als 'cancelled'"

# Een overgeslagen run betekent dat de workflow niet van toepassing was, niet
# dat het werk is nagekeken.
check "overgeslagen CI blokkeert" 1 \
    "[$(run completed '"skipped"' 2026-08-07T10:00:00Z)]" \
    "eindigde als 'skipped'"

check "geen CI-run blokkeert" 1 "[]" \
    "geen enkele CI-run gevonden"

# Alleen runs van deze commit tellen; een andere workflow op dezelfde SHA is
# geen uitspraak over CI.
check "andere workflow op dezelfde SHA telt niet" 1 \
    '[{"name":"Build and Deploy","status":"completed","conclusion":"success","run_started_at":"2026-08-07T10:00:00Z","html_url":"u"}]' \
    "geen enkele CI-run gevonden"

# Nog niet klaar: de poort wacht, en met MAX_WAIT_SECONDS=0 valt hij meteen om
# in plaats van door te rollen.
check "lopende CI laat de deploy niet door" 1 \
    "[$(run in_progress null 2026-08-07T10:00:00Z)]" \
    "nog niet klaar"

# De nieuwste run is de laatste uitspraak; een oudere groene run mag een
# nieuwere rode niet overstemmen.
check "de nieuwste run beslist, niet de eerste" 1 \
    "[$(run completed '"success"' 2026-08-07T10:00:00Z),$(run completed '"failure"' 2026-08-07T11:00:00Z)]" \
    "eindigde als 'failure'"

check "een rerun die groen werd laat alsnog door" 0 \
    "[$(run completed '"failure"' 2026-08-07T10:00:00Z),$(run completed '"success"' 2026-08-07T11:00:00Z)]" \
    "is geslaagd"

# Een mislukte API-aanroep is iets anders dan een ontbrekende run. Zonder dit
# onderscheid meldt de poort een oorzaak die hij niet getoetst heeft.
stub_gh_fails() {
    cat >"$tmp/gh" <<'STUB'
#!/usr/bin/env bash
echo "gh: HTTP 401: Bad credentials" >&2
exit 1
STUB
    chmod +x "$tmp/gh"
}

stub_gh_fails
out=$(PATH="$tmp:$PATH" REPO=o/r SHA=deadbeef MAX_WAIT_SECONDS=0 POLL_SECONDS=0 \
    bash "$gate" 2>&1)
status=$?
if [ "$status" -eq 1 ] && grep -qF "Bad credentials" <<<"$out" \
    && ! grep -qF "geen enkele CI-run" <<<"$out"; then
    echo "ok: een API-fout wordt niet gemeld als een ontbrekende run"
    pass=$((pass + 1))
else
    echo "FAIL: een API-fout wordt niet gemeld als een ontbrekende run — exit $status"
    echo "$out" | sed 's/^/    /'
    fail=$((fail + 1))
fi

echo
echo "$pass geslaagd, $fail mislukt"
[ "$fail" -eq 0 ]
