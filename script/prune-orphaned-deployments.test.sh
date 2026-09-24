#!/usr/bin/env bash
# Dekt elk pad dat bepaalt of er een deployment verdwijnt. De curl-stub schrijft
# elke DELETE weg, zodat de test afgaat op wat er werkelijk zou zijn verwijderd
# en niet op wat het script erover meldt.
set -uo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
script="$here/prune-orphaned-deployments.sh"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

pass=0
fail=0

# $1 = deploymentnamen (spatiegescheiden), $2 = exitcode voor de GET.
stub_curl() {
    local names="$1" rc="${2:-0}" body="" n
    for n in $names; do
        [ -n "$body" ] && body+=","
        body+="{\"name\":\"$n\"}"
    done
    cat >"$tmp/curl" <<STUB
#!/usr/bin/env bash
for arg in "\$@"; do
    if [ "\$arg" = "DELETE" ]; then
        echo "\${@: -1}" >>"$tmp/deleted"
        exit 0
    fi
done
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

check() { # naam, verwachte exit, deployments, open-prs, verwachte-deletes, curl-rc
    local name="$1" want="$2" deps="$3" prs="$4" want_del="$5" crc="${6:-0}"
    : >"$tmp/deleted"
    stub_curl "$deps" "$crc"
    stub_gh "$prs"

    local out status actual
    out="$(
        PATH="$tmp:$PATH" REPO=o/r ZAD_API_KEY=k ZAD_API_BASE=http://x \
            ZAD_PROJECT=proj bash "$script" 2>&1
    )"
    status=$?
    actual="$(tr '\n' ' ' <"$tmp/deleted" | sed 's/ *$//')"

    if [ "$status" -ne "$want" ]; then
        echo "FAIL: $name — exit $status, verwacht $want"
        echo "$out" | sed 's/^/    /'
        fail=$((fail + 1))
        return
    fi
    if [ "$actual" != "$want_del" ]; then
        echo "FAIL: $name — verwijderd '[$actual]', verwacht '[$want_del]'"
        echo "$out" | sed 's/^/    /'
        fail=$((fail + 1))
        return
    fi
    echo "ok: $name"
    pass=$((pass + 1))
}

check "een preview van een gesloten PR gaat weg" 0 \
    "pr99" "10" "http://x/v2/projects/proj/pr99"

check "een preview van een open PR blijft staan" 0 \
    "pr10" "10" ""

check "productie en upload blijven altijd staan" 0 \
    "regelrecht upload" "10" ""

check "geen prN-deployments is niets te doen" 0 \
    "regelrecht" "10" ""

# Zonder de lijst met open PR's zou elk preview-deployment weggaan, ook die van
# pull requests waar op dat moment iemand naar kijkt.
check "geen open PR's opgehaald verwijdert niets" 1 \
    "pr10 pr99" "" ""

check "een onbereikbare ZAD-API verwijdert niets" 1 \
    "pr99" "10" "" 7

echo
echo "geslaagd: $pass, gefaald: $fail"
[ "$fail" -eq 0 ]
