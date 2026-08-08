#!/usr/bin/env bash
# Dekt elk pad in script/report-advisories.sh dat beslist of er een melding
# komt, met een `gh`-stub op PATH.
#
# Deze melding is het enige dat een advisory nog onder de aandacht brengt nu de
# PR-poort er niet meer op omvalt. Een meldscript dat stilletjes niets doet is
# van een werkend meldscript alleen te onderscheiden door het hier vast te
# leggen: schoon sluit, dezelfde set zwijgt, een andere set opent, en oud wordt
# herinnerd.
set -uo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
script="$here/report-advisories.sh"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

pass=0
fail=0
NU="2026-08-20T12:00:00Z"

maak_stub() { # issue-list-tsv, laatste-nag-datum, list-faalt
    cat >"$tmp/bin/gh" <<STUB
#!/usr/bin/env bash
echo "\$*" >>"$tmp/calls"
case "\$1 \$2" in
  "issue list")
    if [ -n "${3:-}" ]; then echo "API-fout" >&2; exit 1; fi
    printf '%s' "$1"
    [ -n "$1" ] && echo
    ;;
  "issue view")   printf '%s\n' "$2" ;;
  "issue create") echo "https://github.com/o/r/issues/77" ;;
esac
exit 0
STUB
    chmod +x "$tmp/bin/gh"
}

geval() { # naam, ids-inhoud, verwachte-exit, issue-list-tsv, laatste-nag, list-faalt, verwachte gh-aanroep (leeg = geen schrijfactie)
    local naam="$1" ids="$2" want="$3" lijst="$4" nag="${5:-}" listfail="${6:-}" verwacht="${7:-}"
    rm -rf "$tmp/out" "$tmp/bin" "$tmp/calls"
    mkdir -p "$tmp/out" "$tmp/bin"
    printf '%s' "$ids" >"$tmp/out/advisories.ids"
    echo "melding" >"$tmp/out/advisories.md"
    touch "$tmp/calls"
    maak_stub "$lijst" "$nag" "$listfail"

    local uit status
    uit="$(PATH="$tmp/bin:$PATH" REPO=o/r ADVISORY_OUT="$tmp/out" NOW="$NU" \
        bash "$script" 2>&1)"
    status=$?

    if [ "$status" -ne "$want" ]; then
        echo "FAIL: $naam — exit $status, verwacht $want"
        sed 's/^/    /' <<<"$uit"
        fail=$((fail + 1))
        return
    fi

    local schrijf
    schrijf=$(grep -E '^(issue (create|close|comment))' "$tmp/calls" || true)
    if [ -z "$verwacht" ]; then
        if [ -n "$schrijf" ]; then
            echo "FAIL: $naam — verwachtte geen schrijfactie, kreeg: $schrijf"
            fail=$((fail + 1))
            return
        fi
    elif ! grep -qF "$verwacht" <<<"$schrijf"; then
        echo "FAIL: $naam — '$verwacht' niet aangeroepen; wel: ${schrijf:-niets}"
        sed 's/^/    /' <<<"$uit"
        fail=$((fail + 1))
        return
    fi
    echo "ok: $naam"
    pass=$((pass + 1))
}

# De vingerafdruk die het script van deze bevindingen maakt, zodat een
# testgeval een issue kan opvoeren dat er al over gaat.
fp_van() {
    printf '%s' "$1" >"$tmp/fp.ids"
    sha256sum "$tmp/fp.ids" | cut -c1-16
}

BEVINDING=$'cargo-deny RUSTSEC-2026-0066\n'
ANDERS=$'npm GHSA-aaaa-bbbb-cccc\n'
FP=$(fp_van "$BEVINDING")

geval "schoon zonder openstaand issue laat alles met rust" "" 0 ""

geval "schoon sluit het openstaande issue" "" 0 \
    $'42\t2026-08-01T00:00:00Z\tdeadbeef' "" "" "issue close 42"

geval "een nieuwe bevinding opent een issue" "$BEVINDING" 0 "" "" "" \
    "issue create"

geval "dezelfde bevinding meldt zich niet nog een keer" "$BEVINDING" 0 \
    "42	2026-08-19T00:00:00Z	$FP"

geval "een bevinding die blijft staan krijgt een herinnering" "$BEVINDING" 0 \
    "42	2026-08-01T00:00:00Z	$FP" "" "" "issue comment 42"

geval "na een verse herinnering blijft het stil" "$BEVINDING" 0 \
    "42	2026-08-01T00:00:00Z	$FP" "2026-08-18T00:00:00Z"

geval "een andere set advisories vervangt het issue" "$ANDERS" 0 \
    "42	2026-08-01T00:00:00Z	$FP" "" "" "issue create"

geval "en sluit het vorige" "$ANDERS" 0 \
    "42	2026-08-01T00:00:00Z	$FP" "" "" "issue close 42"

# Een mislukte lijst-aanroep is niet hetzelfde als "er staat geen issue open".
# Zou het script dat verwarren, dan komt er bij elke storing een issue bij over
# een advisory die er al een heeft.
geval "een mislukte lijst-aanroep blokkeert in plaats van te verdubbelen" \
    "$BEVINDING" 1 "" "" "faal"

# Zonder uitslag van de audit valt er niets te melden; dan blijft een oud issue
# ten onrechte open of komt er een lege melding.
rm -rf "$tmp/out" "$tmp/bin"
mkdir -p "$tmp/out" "$tmp/bin"
if PATH="$tmp/bin:$PATH" REPO=o/r ADVISORY_OUT="$tmp/out" bash "$script" >/dev/null 2>&1; then
    echo "FAIL: ontbrekende uitslag hoort te blokkeren"
    fail=$((fail + 1))
else
    echo "ok: ontbrekende uitslag blokkeert"
    pass=$((pass + 1))
fi

echo
echo "$pass geslaagd, $fail mislukt"
[ "$fail" -eq 0 ]
