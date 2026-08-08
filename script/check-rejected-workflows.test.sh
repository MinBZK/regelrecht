#!/usr/bin/env bash
# Dekt elk pad dat groen of rood beslist in script/check-rejected-workflows.sh,
# met een `gh`-stub op PATH. Een controle die altijd doorlaat is anders niet te
# onderscheiden van een die werkt.
set -uo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
gate="$here/check-rejected-workflows.sh"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

pass=0
fail=0

# $1 = regels "naam<TAB>pad<TAB>staat<TAB>url", $2 = exitcode van gh.
stub_gh() {
    printf '%s' "$1" >"$tmp/regels"
    cat >"$tmp/gh" <<STUB
#!/usr/bin/env bash
cat "$tmp/regels"
exit $2
STUB
    chmod +x "$tmp/gh"
}

check() { # naam, verwachte exit, regels, gh-exit, patroon
    local name="$1" want="$2" regels="$3" ghexit="$4" needle="${5:-}"
    stub_gh "$regels" "$ghexit"
    local out status
    out="$(PATH="$tmp:$PATH" REPO=o/r bash "$gate" 2>&1)"
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

goed="CI	.github/workflows/ci.yml	active	https://x/1
Scheduled Cleanup	.github/workflows/scheduled-cleanup.yml	active	https://x/2
"
afgekeurd="CI	.github/workflows/ci.yml	active	https://x/1
.github/workflows/security-advisories.yml	.github/workflows/security-advisories.yml	active	https://x/2
"
uit="CI	.github/workflows/ci.yml	disabled_inactivity	https://x/1
"

check "alles ingelezen en actief is schoon" 0 "$goed" 0 "Alle workflows zijn ingelezen en actief"

check "een naam gelijk aan het pad is rood" 1 "$afgekeurd" 0 "afgekeurd door Actions"

check "de url staat erbij, anders is hij niet op te zoeken" 1 "$afgekeurd" 0 "https://x/2"

# GitHub zet een geplande workflow na zestig dagen inactiviteit uit. Dezelfde
# faalvorm: hij draait niet en zegt dat nergens.
check "een uitgeschakelde workflow is rood" 1 "$uit" 0 "disabled_inactivity"

check "een lege lijst is schoon" 0 "" 0 "Alle workflows zijn ingelezen en actief"

# Een gefaalde aanroep en een lege uitkomst zien er hetzelfde uit; zonder lijst
# zegt de controle niets en is doorlaten fout.
check "een gefaalde gh-aanroep blokkeert met een eigen reden" 1 "" 1 \
    "kon de workflows niet opvragen"

echo
echo "$pass geslaagd, $fail mislukt"
[ "$fail" -eq 0 ]
