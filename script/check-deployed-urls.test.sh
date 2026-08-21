#!/usr/bin/env bash
# Dekt elk pad dat groen of rood beslist in script/check-deployed-urls.sh, met
# een `curl`-stub op PATH. Zonder deze test is een controle die alles doorlaat
# niet te onderscheiden van een die werkt.
set -uo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
gate="$here/check-deployed-urls.sh"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

pass=0
fail=0

# $1 = de code die de stub voor elke URL teruggeeft; leeg betekent geen
# verbinding (curl faalt).
stub_curl() {
    cat >"$tmp/curl" <<STUB
#!/usr/bin/env bash
if [ -z "$1" ]; then
    echo "curl: (7) Failed to connect" >&2
    exit 7
fi
printf '%s' "$1"
STUB
    chmod +x "$tmp/curl"
}

check() { # naam, verwachte exit, code, urls-json, patroon
    local naam="$1" want="$2" code="$3" urls="$4" needle="${5:-}"
    stub_curl "$code"
    local out st
    out="$(PATH="$tmp:$PATH" URLS="$urls" ATTEMPTS=1 DELAY=0 bash "$gate" 2>&1)"
    st=$?

    if [ "$st" -ne "$want" ]; then
        echo "FAIL: $naam — exit $st, verwacht $want"
        echo "$out" | sed 's/^/    /'
        fail=$((fail + 1))
        return
    fi
    if [ -n "$needle" ] && ! grep -qF "$needle" <<<"$out"; then
        echo "FAIL: $naam — '$needle' niet in de uitvoer"
        echo "$out" | sed 's/^/    /'
        fail=$((fail + 1))
        return
    fi
    echo "ok: $naam"
    pass=$((pass + 1))
}

twee='{"editor":"https://editor.example","docs":"https://docs.example"}'

check "200 op alles is groen" 0 200 "$twee" "2 component(en) bereikbaar"
check "een omleiding telt als bereikbaar" 0 302 "$twee" "bereikbaar"
check "401 telt als bereikbaar, de dienst leeft" 0 401 "$twee" "bereikbaar"
check "500 is rood" 1 500 "$twee" "antwoordt niet (500)"
check "404 is rood" 1 404 "$twee" "antwoordt niet (404)"
check "geen verbinding is rood" 1 "" "$twee" "geen verbinding"

# Een lege lijst betekent dat de deploy niets opleverde. Dat leest anders als
# "alles in orde", en dat is precies de faalvorm die deze controle moet dichten.
check "een lege lijst is rood" 1 200 '{}' "geen enkele URL"
check "geen JSON-object is rood" 1 200 'null' "geen JSON-object"

echo
echo "$pass geslaagd, $fail mislukt"
[ "$fail" -eq 0 ]
