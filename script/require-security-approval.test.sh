#!/usr/bin/env bash
# Dekt elk pad dat groen of rood beslist in script/require-security-approval.sh,
# met een `gh`-stub op PATH. Zonder deze test is een poort die altijd doorlaat
# niet te onderscheiden van een die werkt — en dat is precies het geval dat een
# security-patch ongezien naar main brengt.
set -uo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
gate="$here/require-security-approval.sh"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

pass=0
fail=0

# De stub bedient de drie endpoints die de poort aanroept en leest per endpoint
# een fixture. Staat er `FAIL` in de fixture, dan faalt die aanroep, zodat een
# API-fout op één endpoint los te testen is van de rest.
cat >"$tmp/gh" <<'STUB'
#!/usr/bin/env bash
endpoint=""
filter=""
for ((i = 1; i <= $#; i++)); do
    case "${!i}" in
    --jq)
        j=$((i + 1))
        filter="${!j}"
        ;;
    repos/*) endpoint="${!i}" ;;
    esac
done

case "$endpoint" in
*/dependabot/alerts*) file="$FIXTURES/alerts.json" ;;
*/reviews*) file="$FIXTURES/reviews.json" ;;
*) file="$FIXTURES/pr.json" ;;
esac

if [ ! -f "$file" ] || grep -q '^FAIL' "$file"; then
    echo "gh: stub-fout op ${endpoint}" >&2
    exit 1
fi

if [ -n "$filter" ]; then
    jq -r "$filter" <"$file"
else
    cat "$file"
fi
STUB
chmod +x "$tmp/gh"

pr_json() { # $1 = auteur, $2 = titel, $3 = body
    jq -n --arg u "$1" --arg t "$2" --arg b "$3" \
        '{head: {sha: "cafebabe1234567"}, user: {login: $u}, title: $t, body: $b}'
}

# Elke review krijgt een oplopend id, zoals de API ze ook geeft: de poort
# bepaalt daarmee wiens laatste woord telt. Aanroepvolgorde is dus de volgorde
# waarin er gereviewd is.
review_id=0
review() { # $1 = login, $2 = state, $3 = commit, $4 = association
    review_id=$((review_id + 1))
    jq -n --arg l "$1" --arg s "$2" --arg c "$3" --arg a "$4" --argjson i "$review_id" \
        '{id: $i, user: {login: $l}, state: $s, commit_id: $c, author_association: $a}'
}

# $1 = naam, $2 = verwachte exitcode, $3 = pr.json, $4 = alerts.json,
# $5 = reviews.json, $6 = optioneel patroon in de uitvoer,
# $7 = optioneel patroon in $GITHUB_OUTPUT
check() {
    local name="$1" want="$2" needle="${6:-}" out_needle="${7:-}"
    printf '%s' "$3" >"$tmp/pr.json"
    printf '%s' "$4" >"$tmp/alerts.json"
    printf '%s' "$5" >"$tmp/reviews.json"
    : >"$tmp/outputs"

    local out status
    out="$(PATH="$tmp:$PATH" FIXTURES="$tmp" ENFORCE="${ENFORCE:-true}" \
        REPO=o/r PR_NUMBER=42 GITHUB_OUTPUT="$tmp/outputs" \
        bash "$gate" 2>&1)"
    status=$?

    if [ "$status" -ne "$want" ]; then
        echo "FAIL: $name — exit $status, verwacht $want"
        sed 's/^/    /' <<<"$out"
        fail=$((fail + 1))
        return
    fi
    if [ -n "$needle" ] && ! grep -qF "$needle" <<<"$out"; then
        echo "FAIL: $name — '$needle' niet in de uitvoer"
        sed 's/^/    /' <<<"$out"
        fail=$((fail + 1))
        return
    fi
    if [ -n "$out_needle" ] && ! grep -qxF "$out_needle" "$tmp/outputs"; then
        echo "FAIL: $name — '$out_needle' niet in \$GITHUB_OUTPUT"
        sed 's/^/    /' "$tmp/outputs"
        fail=$((fail + 1))
        return
    fi
    echo "ok: $name"
    pass=$((pass + 1))
}

no_alerts='[]'
no_reviews='[]'
alert_serde='[{"security_vulnerability": {"package": {"name": "serde"}}}]'

marker='Bumps serde from 1.0.1 to 1.0.2. **This update includes a security fix.**'
plain='Bumps serde from 1.0.1 to 1.0.2.'
changelog_only="$plain
<details><summary>Changelog</summary>fixes GHSA-aaaa-bbbb-cccc in an unrelated package</details>"
advisory_in_lead="Bumps serde from 1.0.1 to 1.0.2, tegen GHSA-aaaa-bbbb-cccc.
<details><summary>Changelog</summary>niets</details>"

check "PR van een mens valt buiten de eis" 0 \
    "$(pr_json someone 'fix(deps): dompurify naar 3.4.13' "$marker")" \
    "$alert_serde" "$no_reviews" \
    'komt niet van dependabot' 'is_security=false'

check "gewone bump van dependabot mag zonder goedkeuring mergen" 0 \
    "$(pr_json 'dependabot[bot]' 'chore(deps): bump serde from 1.0.1 to 1.0.2' "$plain")" \
    "$no_alerts" "$no_reviews" \
    'gewone versie-bump' 'is_security=false'

check "een advisory in een geciteerde changelog maakt er nog geen security-update van" 0 \
    "$(pr_json 'dependabot[bot]' 'chore(deps): bump serde from 1.0.1 to 1.0.2' "$changelog_only")" \
    "$no_alerts" "$no_reviews" \
    'gewone versie-bump' 'is_security=false'

check "security-update zonder goedkeuring blokkeert" 1 \
    "$(pr_json 'dependabot[bot]' 'chore(deps): bump serde from 1.0.1 to 1.0.2' "$marker")" \
    "$no_alerts" "$no_reviews" \
    'moet deze PR goedkeuren' 'is_security=true'

check "een open alert voor het gebumpte pakket is genoeg signaal" 1 \
    "$(pr_json 'dependabot[bot]' 'chore(deps): bump serde from 1.0.1 to 1.0.2' "$plain")" \
    "$alert_serde" "$no_reviews" \
    'open Dependabot-alert' 'is_security=true'

check "een advisory in de aanhef van dependabot is genoeg signaal" 1 \
    "$(pr_json 'dependabot[bot]' 'chore(deps): bump serde from 1.0.1 to 1.0.2' "$advisory_in_lead")" \
    "$no_alerts" "$no_reviews" \
    'GHSA-AAAA-BBBB-CCCC' 'advisories=GHSA-AAAA-BBBB-CCCC'

check "een alert voor een ander pakket telt niet mee" 0 \
    "$(pr_json 'dependabot[bot]' 'chore(deps): bump tokio from 1.0.1 to 1.0.2' "$plain")" \
    "$alert_serde" "$no_reviews" \
    'gewone versie-bump' 'is_security=false'

check "goedkeuring van een engineer op deze commit laat door" 0 \
    "$(pr_json 'dependabot[bot]' 'chore(deps): bump serde from 1.0.1 to 1.0.2' "$marker")" \
    "$no_alerts" "[$(review eelco APPROVED cafebabe1234567 MEMBER)]" \
    'goedgekeurd door @eelco' 'approved=true'

check "goedkeuring op een oudere commit telt niet" 1 \
    "$(pr_json 'dependabot[bot]' 'chore(deps): bump serde from 1.0.1 to 1.0.2' "$marker")" \
    "$no_alerts" "[$(review eelco APPROVED 0000000deadbeef MEMBER)]" \
    'moet deze PR goedkeuren' 'approved=false'

check "goedkeuring van een bot telt niet" 1 \
    "$(pr_json 'dependabot[bot]' 'chore(deps): bump serde from 1.0.1 to 1.0.2' "$marker")" \
    "$no_alerts" "[$(review 'claude[bot]' APPROVED cafebabe1234567 MEMBER)]" \
    'moet deze PR goedkeuren' 'approved=false'

check "goedkeuring van iemand zonder schrijfrechten telt niet" 1 \
    "$(pr_json 'dependabot[bot]' 'chore(deps): bump serde from 1.0.1 to 1.0.2' "$marker")" \
    "$no_alerts" "[$(review buitenstaander APPROVED cafebabe1234567 NONE)]" \
    'moet deze PR goedkeuren' 'approved=false'

check "een becommentarieerde review is geen goedkeuring" 1 \
    "$(pr_json 'dependabot[bot]' 'chore(deps): bump serde from 1.0.1 to 1.0.2' "$marker")" \
    "$no_alerts" "[$(review eelco COMMENTED cafebabe1234567 MEMBER)]" \
    'moet deze PR goedkeuren' 'approved=false'

check "een ingetrokken goedkeuring telt niet" 1 \
    "$(pr_json 'dependabot[bot]' 'chore(deps): bump serde from 1.0.1 to 1.0.2' "$marker")" \
    "$no_alerts" "[$(review eelco DISMISSED cafebabe1234567 MEMBER)]" \
    'moet deze PR goedkeuren' 'approved=false'

# De API herschrijft een eerdere review niet: wie goedkeurt en zich daarna
# bedenkt, laat dat APPROVED-object gewoon staan. Filteren op APPROVED en dan de
# laatste pakken vindt hem alsnog, en dan opent een ingetrokken goedkeuring de
# poort.
check "wie zich na zijn goedkeuring bedenkt, keurt niet meer goed" 1 \
    "$(pr_json 'dependabot[bot]' 'chore(deps): bump serde from 1.0.1 to 1.0.2' "$marker")" \
    "$no_alerts" \
    "[$(review eelco APPROVED cafebabe1234567 MEMBER),$(review eelco CHANGES_REQUESTED cafebabe1234567 MEMBER)]" \
    'moet deze PR goedkeuren' 'approved=false'

# En andersom: wie eerst wijzigingen vroeg en daarna alsnog goedkeurt, telt wel.
check "wie na wijzigingen alsnog goedkeurt, telt wel" 0 \
    "$(pr_json 'dependabot[bot]' 'chore(deps): bump serde from 1.0.1 to 1.0.2' "$marker")" \
    "$no_alerts" \
    "[$(review eelco CHANGES_REQUESTED cafebabe1234567 MEMBER),$(review eelco APPROVED cafebabe1234567 MEMBER)]" \
    'goedgekeurd door @eelco' 'approved=true'

# Iemand anders die zich bedenkt mag een geldige goedkeuring niet wegnemen.
check "de intrekking van de een raakt de goedkeuring van de ander niet" 0 \
    "$(pr_json 'dependabot[bot]' 'chore(deps): bump serde from 1.0.1 to 1.0.2' "$marker")" \
    "$no_alerts" \
    "[$(review anne APPROVED cafebabe1234567 MEMBER),$(review eelco APPROVED cafebabe1234567 MEMBER),$(review eelco CHANGES_REQUESTED cafebabe1234567 MEMBER)]" \
    'goedgekeurd door @anne' 'approved=true'

check "onbereikbare alerts blokkeren niet, maar worden wel gemeld" 1 \
    "$(pr_json 'dependabot[bot]' 'chore(deps): bump serde from 1.0.1 to 1.0.2' "$marker")" \
    'FAIL' "$no_reviews" \
    'alerts zijn niet op te halen' 'is_security=true'

check "een onleesbare PR blokkeert en noemt geen oorzaak die niet getoetst is" 1 \
    'FAIL' "$no_alerts" "$no_reviews" \
    'niet op te halen'

check "onleesbare reviews blokkeren" 1 \
    "$(pr_json 'dependabot[bot]' 'chore(deps): bump serde from 1.0.1 to 1.0.2' "$marker")" \
    "$no_alerts" 'FAIL' \
    'reviews van pull request 42 zijn niet op te halen'

ENFORCE=false check "zonder handhaving blijft de uitkomst leesbaar en groen" 0 \
    "$(pr_json 'dependabot[bot]' 'chore(deps): bump serde from 1.0.1 to 1.0.2' "$marker")" \
    "$no_alerts" "$no_reviews" \
    'handhaaft niet' 'is_security=true'

echo
echo "${pass} geslaagd, ${fail} gefaald"
[ "$fail" -eq 0 ]
