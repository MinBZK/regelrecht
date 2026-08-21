#!/usr/bin/env bash
# Dekt elk pad dat groen of rood beslist in script/check-preview-deployments.sh,
# met stubs voor `gh` en `curl` op PATH. Een controle die altijd doorlaat is
# zonder deze test niet te onderscheiden van een die werkt, en dat is precies de
# fout die deze controle bij de opruiming zelf blootlegt.
set -uo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
gate="$here/check-preview-deployments.sh"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

pass=0
fail=0

# $1 = deploymentnamen (spatiegescheiden), $2 = curl-exitcode.
stub_curl() {
    local names="$1" rc="${2:-0}" body="" n
    if [ "$names" = "__ERROR_BODY__" ]; then
        cat >"$tmp/curl" <<'STUB'
#!/usr/bin/env bash
echo '{"detail":"Not Found"}'
STUB
        chmod +x "$tmp/curl"
        return
    fi
    for n in $names; do
        [ -n "$body" ] && body+=","
        body+="{\"name\":\"$n\"}"
    done
    cat >"$tmp/curl" <<STUB
#!/usr/bin/env bash
[ $rc -ne 0 ] && exit $rc
cat <<'JSON'
{"deployments":[$body]}
JSON
STUB
    chmod +x "$tmp/curl"
}

# $1 = open PR-nummers.
stub_gh() {
    cat >"$tmp/gh" <<STUB
#!/usr/bin/env bash
# Toetst de vraag die gesteld wordt. Zonder deze controle overleeft een mutatie
# van --state open naar --state closed elke test, terwijl dat precies de regel
# is die bepaalt wat behouden blijft.
case " \$* " in
    *" --state open "*) ;;
    *) echo "stub: onverwachte gh-aanroep: \$*" >&2; exit 64 ;;
esac
case " \$* " in
    *" --repo o/r "*) ;;
    *) echo "stub: verkeerde repo: \$*" >&2; exit 64 ;;
esac
for n in $1; do echo "pr\$n"; done
STUB
    chmod +x "$tmp/gh"
}

check() { # naam, verwachte exit, deployments, open-prs, curl-rc, patroon
    local name="$1" want="$2" deps="$3" prs="$4" rc="$5" needle="${6:-}"
    stub_curl "$deps" "$rc"
    stub_gh "$prs"
    local out status
    out="$(
        PATH="$tmp:$PATH" REPO=o/r ZAD_API_KEY=k ZAD_API_BASE=http://x \
            ZAD_PROJECT=p WAIT_SECONDS=0 POLL_INTERVAL=0 bash "$gate" 2>&1
    )"
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

check "alleen deployments van open PR's is schoon" 0 \
    "pr10 pr11" "10 11" 0 "Niets achtergebleven"

check "een deployment van een gesloten PR is rood" 1 \
    "pr10 pr99" "10" 0 "pr99"

check "productie en upload tellen niet mee" 0 \
    "regelrecht upload pr10" "10" 0 "Niets achtergebleven"

# Namen die op "pr" lijken maar geen preview zijn; zonder deze zou een te ruime
# grep ongemerkt doorglippen.
check "prod en preview-shared zijn geen previews" 0 \
    "prod preview-shared pr10-db PR10 pr10" "10" 0 "Niets achtergebleven"

check "geen enkel deployment is schoon" 0 \
    "regelrecht" "10" 0 "Niets achtergebleven"

check "een onbereikbare ZAD-API is rood, niet groen" 1 \
    "pr10" "10" 7 "kon de deployments niet opvragen"

# Een 200 met een foutlichaam gaf voorheen een lege lijst en dus "niets
# achtergebleven": een poort die altijd doorlaat.
check "een 200 zonder deployments-lijst is rood" 1 \
    "__ERROR_BODY__" "10" 0 "bevat geen deployments-lijst"

# Zonder de lijst met open PR's zou elk deployment als achtergebleven gelden;
# dat moet een fout zijn en geen lawine van valse meldingen.
check "geen open PR's opgehaald is rood" 1 \
    "pr10" "" 0 "kon de open pull requests niet opvragen"

echo
echo "geslaagd: $pass, gefaald: $fail"
[ "$fail" -eq 0 ]
