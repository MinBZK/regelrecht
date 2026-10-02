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
# verbinding (curl faalt). $2 (optioneel) = regels "url code" die voor precies
# die URL een andere code geven, zodat een test kan zien wélk pad bevraagd is.
stub_curl() {
    printf '%s\n' "${2:-}" >"$tmp/per-url"
    cat >"$tmp/curl" <<STUB
#!/usr/bin/env bash
url="\${*: -1}"
while read -r u c; do
    if [ "\$u" = "\$url" ]; then printf '%s' "\$c"; exit 0; fi
done <"$tmp/per-url"
if [ -z "$1" ]; then
    echo "curl: (7) Failed to connect" >&2
    exit 7
fi
printf '%s' "$1"
STUB
    chmod +x "$tmp/curl"
}

check() { # naam, verwachte exit, code, urls-json, patroon, per-url-codes
    local naam="$1" want="$2" code="$3" urls="$4" needle="${5:-}"
    stub_curl "$code" "${6:-}"
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

# harvester-admin serveert niets op `/` (sinds #902 een kale API) en wordt op
# /health bevraagd. De stub geeft precies wat productie geeft: `/` 404, /health
# 200. Bevroeg het script nog `/`, dan kreeg het de standaardcode 404 en rood.
admin='{"editor":"https://editor.example","harvester-admin":"https://admin.example"}'
check "harvester-admin wordt op /health bevraagd" 0 404 "$admin" \
    "harvester-admin: 200  https://admin.example/health" \
    $'https://editor.example 200\nhttps://admin.example/health 200'
check "een afsluitende slash geeft geen dubbele" 0 404 '{"harvester-admin":"https://admin.example/"}' \
    "https://admin.example/health" \
    "https://admin.example/health 200"
# De uitzondering verschuift het pad, niet wat telt: een 404 op /health blijft
# rood, anders valt een verdwenen admin niet meer op.
check "404 op /health van harvester-admin is rood" 1 200 "$admin" \
    "harvester-admin antwoordt niet (404) op https://admin.example/health" \
    "https://admin.example/health 404"
# Alleen harvester-admin krijgt die uitzondering; een ander component met een
# 404 op zijn hoofdadres blijft rood.
check "404 op / van een ander component blijft rood" 1 200 "$admin" \
    "editor antwoordt niet (404) op https://editor.example" \
    "https://editor.example 404"

echo
echo "$pass geslaagd, $fail mislukt"
[ "$fail" -eq 0 ]
