#!/usr/bin/env bash
# Dekt de herschrijving in script/linkify-werkpakket.sh, met een `gh`-stub die
# de PATCH opvangt zodat de nieuwe body te inspecteren is.
#
# Het belangrijkste dat hier bewaakt wordt is dat de herschrijving idempotent is
# en de regel parseerbaar laat: draait ze twee keer, of leest de poort de
# uitkomst, dan moet er hetzelfde uitkomen. Anders zou deze stap de volgende run
# van de poort rood maken op een PR die niets verkeerd doet.
set -uo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
script="$here/linkify-werkpakket.sh"
gate="$here/require-werkpakket.sh"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

pass=0
fail=0

# De stub geeft de body terug en legt een PATCH vast in patched.json.
cat >"$tmp/gh" <<'STUB'
#!/usr/bin/env bash
method=""
for ((i = 1; i <= $#; i++)); do
    case "${!i}" in
    -X) j=$((i + 1)); method="${!j}" ;;
    esac
done
if [ "$method" = "PATCH" ]; then
    cat >"$FIXTURES/patched.json"
    echo '{}'
    exit 0
fi
jq -r '.body // ""' <"$FIXTURES/pr.json"
STUB
chmod +x "$tmp/gh"

# $1 = naam, $2 = body erin, $3 = verwachte regel eruit (leeg = geen PATCH)
check() {
    local name="$1" invoer="$2" verwacht="$3"
    jq -n --arg b "$invoer" '{body: $b}' >"$tmp/pr.json"
    rm -f "$tmp/patched.json"

    PATH="$tmp:$PATH" FIXTURES="$tmp" REPO=o/r PR_NUMBER=42 \
        bash "$script" >/dev/null 2>&1

    local nieuw=''
    [ -f "$tmp/patched.json" ] && nieuw=$(jq -r '.body' <"$tmp/patched.json")

    if [ -z "$verwacht" ]; then
        if [ -f "$tmp/patched.json" ]; then
            echo "FAIL: $name — er is een PATCH gestuurd terwijl dat niet hoorde"
            sed 's/^/    /' <<<"$nieuw"
            fail=$((fail + 1))
            return
        fi
        echo "ok: $name"
        pass=$((pass + 1))
        return
    fi

    if ! grep -qxF -- "$verwacht" <<<"$nieuw"; then
        echo "FAIL: $name"
        echo "    verwacht: $verwacht"
        sed 's/^/    kreeg:    /' <<<"$nieuw"
        fail=$((fail + 1))
        return
    fi
    echo "ok: $name"
    pass=$((pass + 1))
}

url='https://regelrecht.rijks.app/roadmap/werkpakket'

check "een kale slug wordt een link" \
    'Wat dit doet.

Werkpakket: referentie-casus-i' \
    "Werkpakket: [referentie-casus-i](${url}/referentie-casus-i)"

check "twee slugs worden twee links" \
    'Werkpakket: referentie-casus-i, effect-over-tijd' \
    "Werkpakket: [referentie-casus-i](${url}/referentie-casus-i), [effect-over-tijd](${url}/effect-over-tijd)"

check "een regel die al een link is blijft ongemoeid" \
    "Werkpakket: [referentie-casus-i](${url}/referentie-casus-i)" \
    ''

check "geen met een reden blijft ongemoeid" \
    'Werkpakket: geen — losse typefout in de docs' \
    ''

check "een PR zonder de regel krijgt geen PATCH" \
    'Gewoon een omschrijving.' \
    ''

check "de rest van de body blijft staan" \
    'Eerste regel.

Werkpakket: referentie-casus-i' \
    'Eerste regel.'

# De uitkomst moet door de poort komen. Draait de herschrijving en wordt de
# volgende run daardoor rood, dan is de bot een valstrik.
wp_dir="$tmp/werkpakketten"
mkdir -p "$wp_dir"
printf -- '---\nid: referentie-casus-i\n---\n' >"$wp_dir/referentie-casus-i.md"

jq -n --arg b 'Werkpakket: referentie-casus-i' '{body: $b}' >"$tmp/pr.json"
rm -f "$tmp/patched.json"
PATH="$tmp:$PATH" FIXTURES="$tmp" REPO=o/r PR_NUMBER=42 bash "$script" >/dev/null 2>&1
herschreven=$(jq -r '.body' <"$tmp/patched.json")

cat >"$tmp/gh2" <<'STUB'
#!/usr/bin/env bash
for ((i = 1; i <= $#; i++)); do
    case "${!i}" in
    */files*) exit 0 ;;
    esac
done
jq -n --arg b "$BODY" '{user: {login: "anne"}, body: $b, head: {repo: {full_name: "o/r"}}}'
STUB
chmod +x "$tmp/gh2"
mkdir -p "$tmp/bin2" && cp "$tmp/gh2" "$tmp/bin2/gh"

if PATH="$tmp/bin2:$PATH" BODY="$herschreven" REPO=o/r PR_NUMBER=42 \
    WERKPAKKETTEN_DIR="$wp_dir" CORPUS_DIR="$tmp/leeg" \
    GITHUB_OUTPUT=/dev/null GITHUB_STEP_SUMMARY=/dev/null \
    bash "$gate" >/dev/null 2>&1; then
    echo "ok: de poort keurt goed wat de herschrijving oplevert"
    pass=$((pass + 1))
else
    echo "FAIL: de poort keurt goed wat de herschrijving oplevert"
    echo "    body na herschrijven: $herschreven"
    fail=$((fail + 1))
fi

echo
echo "${pass} geslaagd, ${fail} gefaald"
[ "$fail" -eq 0 ]
