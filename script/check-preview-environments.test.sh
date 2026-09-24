#!/usr/bin/env bash
# Dekt elk pad dat groen of rood beslist in script/check-preview-environments.sh,
# met een `gh`-stub op PATH. Zonder deze test is een controle die altijd
# doorlaat niet te onderscheiden van een die werkt, en dat is precies de fout
# die deze controle bij de opruimactie zelf blootlegt.
set -uo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
gate="$here/check-preview-environments.sh"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

pass=0
fail=0

# $1 = environmentnamen (spatiegescheiden), $2 = open PR-nummers.
stub_gh() {
    cat >"$tmp/gh" <<STUB
#!/usr/bin/env bash
case "\$1" in
  api) for e in $1; do echo "\$e"; done ;;
  pr)  for n in $2; do echo "pr\$n"; done ;;
esac
STUB
    chmod +x "$tmp/gh"
}

check() { # naam, verwachte exit, envs, open-prs, patroon
    local name="$1" want="$2" envs="$3" prs="$4" needle="${5:-}"
    stub_gh "$envs" "$prs"
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

check "alleen environments van open PR's is schoon" 0 \
    "pr10 pr11" "10 11" "Niets achtergebleven"

check "een environment van een gesloten PR is rood" 1 \
    "pr10 pr99" "10" "pr99"

check "helemaal geen environments is schoon" 0 \
    "" "10" "Niets achtergebleven"

# De naamvorm doet ertoe: production en github-pages mogen nooit meetellen.
check "niet-preview-environments tellen niet mee" 0 \
    "production github-pages pr10" "10" "Niets achtergebleven"

# Een mislukte gh-aanroep geeft een lege PR-lijst, en dan zou elke environment
# ten onrechte als achtergebleven gelden. Blokkeren in plaats van rood melden
# op de verkeerde grond.
check "geen open PR's opgehaald blokkeert met een eigen reden" 1 \
    "pr10" "" "kon de open pull requests niet opvragen"

echo
echo "$pass geslaagd, $fail mislukt"
[ "$fail" -eq 0 ]
