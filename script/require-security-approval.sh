#!/usr/bin/env bash
# Merge-poort: een security-update van Dependabot komt pas door als een engineer
# hem heeft goedgekeurd.
#
# De eis geldt uitsluitend voor security-PR's van Dependabot. Dat is niet met
# branch protection of een ruleset te maken: `required_approving_review_count`
# hangt aan een branch, niet aan een auteur of een label, en zou dus elke PR
# treffen. Voor een repo waarin de auteur van een PR zijn eigen wijziging niet
# mag goedkeuren betekent dat: alles op slot. Daarom is de eis een *check*, met
# de voorwaarde erin, precies zoals `Claude review completed` dat doet.
#
# Wat de poort als feit behandelt, haalt zij zelf op. Uit de omgeving komen
# alleen de coördinaten (repository, PR-nummer); auteur, tekst, reviews en
# alerts komen van de API.
#
# Wanneer is een Dependabot-PR een security-update? Er is geen veld in de API
# dat dat zegt, dus de poort leest drie onafhankelijke signalen:
#
#   1. een open Dependabot-alert voor precies het pakket dat deze PR bumpt;
#   2. Dependabots eigen regel "This update includes a security fix." in de
#      body;
#   3. een GHSA- of CVE-nummer in Dependabots eigen tekst — dat is de body tot
#      aan het eerste `<details>`-blok, want daarna staan release notes en
#      changelogs die net zo goed over de beveiliging van een *ander* pakket
#      kunnen gaan.
#
# Eén signaal is genoeg. Ze staan er los van elkaar omdat het eerste een
# token-scope nodig heeft die een door Dependabot gestarte run niet altijd
# heeft, en het tweede en derde afhangen van een formulering die GitHub kan
# wijzigen. Een vals positief kost een goedkeuring die niet nodig was; een vals
# negatief laat een security-patch ongezien door. De poort leunt dus naar het
# eerste.
#
# Wat dit niet dekt: deze job staat in het workflowbestand dat de PR meebrengt.
# Dependabot raakt `.github/workflows/` alleen aan in het `github-actions`-
# ecosysteem, en zulke PR's zijn geen security-updates van npm of cargo, maar
# de bypass bestaat op papier — net als bij de review-poort. Sluiten kan alleen
# met een regel buiten de pull request om.
set -uo pipefail

: "${REPO:?REPO is verplicht}"
: "${PR_NUMBER:?PR_NUMBER is verplicht}"

# Uit staat de poort alleen daar waar hij niets te blokkeren heeft: de
# dependabot-review gebruikt hem om te wéten of dit een security-PR is, en de
# meldstap om te weten of er iets te melden valt.
ENFORCE="${ENFORCE:-true}"
GITHUB_OUTPUT="${GITHUB_OUTPUT:-/dev/null}"
GITHUB_STEP_SUMMARY="${GITHUB_STEP_SUMMARY:-/dev/null}"

readonly BOT='dependabot[bot]'

summary() { printf '%s\n' "$1" >>"$GITHUB_STEP_SUMMARY"; }
output() { printf '%s=%s\n' "$1" "$2" >>"$GITHUB_OUTPUT"; }

# Foutuitvoer van `gh` apart houden: een waarschuwing op een verder geslaagde
# aanroep zou de payload onparseerbaar maken, en dat kwam dan naar buiten als
# een uitspraak over de PR in plaats van als leesprobleem.
gh_stderr=$(mktemp "${TMPDIR:-/tmp}/security-gate-stderr.XXXXXX")
trap 'rm -f "$gh_stderr"' EXIT

green() {
    echo "::notice title=Security update approved::$1"
    summary "### Security-goedkeuring: niet nodig of aanwezig"
    summary ""
    summary "$1"
    exit 0
}

blocked() {
    if [ "$ENFORCE" != "true" ]; then
        echo "::notice title=Security update approved::$1 (deze stap handhaaft niet)"
        summary "### Security-goedkeuring: nog niet gegeven"
        summary ""
        summary "$1"
        exit 0
    fi
    echo "::error title=Security update approved::$1"
    summary "### Security-goedkeuring: geblokkeerd"
    summary ""
    summary "$1"
    exit 1
}

# Een leesfout is geen uitspraak over de PR. Hij blokkeert wel, want doorlaten
# zou betekenen dat de poort groen geeft over iets wat ze niet gezien heeft.
unreadable() {
    echo "::error title=Security update approved::$1"
    summary "### Security-goedkeuring: niet vast te stellen"
    summary ""
    summary "$1"
    exit 1
}

if ! pr=$(gh api "repos/${REPO}/pulls/${PR_NUMBER}" 2>"$gh_stderr") ||
    ! head_sha=$(jq -er '.head.sha' <<<"$pr" 2>/dev/null); then
    unreadable "Pull request ${PR_NUMBER} in ${REPO} is niet op te halen, dus of dit een security-update is valt niet vast te stellen. Draai deze job opnieuw. Foutmelding: $(tr '\n' ' ' <"$gh_stderr")"
fi

pr_field() { jq -r "${1} // \"\"" <<<"$pr"; }

title=$(pr_field '.title')
body=$(pr_field '.body')

output 'head_sha' "$head_sha"
output 'title' "$title"

if [ "$(pr_field '.user.login')" != "$BOT" ]; then
    output 'is_security' 'false'
    output 'approved' 'false'
    output 'advisories' ''
    green "Deze PR komt niet van ${BOT}. De goedkeuringseis geldt alleen voor de security-updates die Dependabot zelf opent; een PR van een mens loopt langs de gewone review."
fi

# "bump serde from 1.0.1 to 1.0.2" → serde. Leestekens waarmee Dependabot de
# naam soms omgeeft gaan eraf, zodat de vergelijking met de alert klopt.
dependency=$(sed -nE 's/.*[Bb]umps? (\S+) from \S+ to \S+.*/\1/p' <<<"$title" | head -1)
dependency=${dependency//[\`\[\]\*]/}
output 'dependency' "$dependency"

# Alleen Dependabots eigen tekst; alles vanaf het eerste `<details>` is
# geciteerde changelog.
own_text=${body%%<details>*}

# Uit de aanhef en niet uit de hele body, om dezelfde reden als bij signaal 3:
# achter het eerste `<details>` staan geciteerde changelogs die een advisory van
# een heel ander pakket kunnen noemen. Die zou hier in de blokkeermelding komen
# als de advisory van déze update, terwijl de goedkeurder er juist naar moet
# kijken.
advisories=$(grep -oiE 'GHSA-[0-9a-z]{4}-[0-9a-z]{4}-[0-9a-z]{4}|CVE-[0-9]{4}-[0-9]+' <<<"$own_text" |
    tr '[:lower:]' '[:upper:]' | sort -u | paste -sd, -)
output 'advisories' "$advisories"

is_security=false
signal=''

# Het sterkste signaal, want het komt van de API en niet uit tekst die van
# formulering kan veranderen. Mislukt de aanroep — een door Dependabot gestarte
# run krijgt een token met minder rechten — dan is dat geen "geen alert": de
# poort zegt het en valt terug op de twee tekstsignalen.
if alerts=$(gh api "repos/${REPO}/dependabot/alerts?state=open&per_page=100" --paginate \
    --jq '.[].security_vulnerability.package.name' 2>"$gh_stderr"); then
    if [ -n "$dependency" ] && grep -qixF "$dependency" <<<"$alerts"; then
        is_security=true
        signal="er staat een open Dependabot-alert voor \`${dependency}\`"
    fi
else
    echo "::warning title=Security update approved::De Dependabot-alerts zijn niet op te halen ($(tr '\n' ' ' <"$gh_stderr")). De poort beoordeelt deze PR op de tekst van Dependabot alleen."
fi

if [ "$is_security" = false ] && grep -qiF 'This update includes a security fix' <<<"$own_text"; then
    is_security=true
    signal='Dependabot noemt deze bump zelf een security fix'
fi

if [ "$is_security" = false ] &&
    grep -qiE 'GHSA-[0-9a-z]{4}-[0-9a-z]{4}-[0-9a-z]{4}|CVE-[0-9]{4}-[0-9]+' <<<"$own_text"; then
    is_security=true
    signal="Dependabot noemt een advisory (${advisories}) in de aanhef van deze PR"
fi

output 'is_security' "$is_security"

if [ "$is_security" = false ]; then
    output 'approved' 'false'
    green "Dit is een gewone versie-bump van Dependabot, geen security-update: geen open alert voor \`${dependency:-het gebumpte pakket}\` en geen advisory in de aanhef. Die PR's vallen onder de cooldown van vijf dagen en mogen zonder goedkeuring mergen."
fi

# Een goedkeuring telt alleen voor de commit waarop ze is gegeven. Branch
# protection dismist stale reviews pas bij `required_approving_review_count > 0`
# en dat staat hier op 0, dus die binding moet hiervandaan komen: na een rebase
# of een nieuwe push is er niets meer goedgekeurd.
if ! reviews=$(gh api "repos/${REPO}/pulls/${PR_NUMBER}/reviews" --paginate 2>"$gh_stderr"); then
    unreadable "De reviews van pull request ${PR_NUMBER} zijn niet op te halen, dus of deze security-update is goedgekeurd valt niet vast te stellen. Draai deze job opnieuw. Foutmelding: $(tr '\n' ' ' <"$gh_stderr")"
fi

# Een goedkeuring van een bot is geen menselijke blik, en een goedkeuring van
# een willekeurige buitenstaander evenmin: iedereen met een account kan een
# review achterlaten, dus alleen wie schrijfrechten heeft telt.
#
# Eerst de laatste review per persoon, dan pas filteren op APPROVED. De API
# geeft elke ingediende review als eigen object terug en herschrijft een
# eerdere niet: keurt iemand goed en vraagt hij daarna op dezelfde commit
# wijzigingen aan, dan staat die APPROVED er nog steeds. Filteren-dan-laatste
# pakt hem dus alsnog op, en een ingetrokken goedkeuring zou de poort openen.
approver=$(jq -r --arg sha "$head_sha" '
    [ .[]
      | select(.commit_id == $sha)
      | select(.user.login | endswith("[bot]") | not)
      | select(.author_association == "OWNER"
               or .author_association == "MEMBER"
               or .author_association == "COLLABORATOR")
      # COMMENTED laat het oordeel ongemoeid; alleen wat een standpunt is telt
      # mee bij het bepalen van iemands laatste woord.
      | select(.state == "APPROVED" or .state == "CHANGES_REQUESTED" or .state == "DISMISSED")
    ]
    | group_by(.user.login) | map(max_by(.id))
    | map(select(.state == "APPROVED"))
    | last | .user.login // ""' <<<"$reviews")

if [ -n "$approver" ]; then
    output 'approved' 'true'
    output 'approver' "$approver"
    green "Security-update (${signal}), goedgekeurd door @${approver} op commit \`${head_sha:0:7}\`.${advisories:+ Advisories: ${advisories}.}"
fi

output 'approved' 'false'
blocked "Dit is een security-update (${signal})${advisories:+, advisories: ${advisories}}. Zulke PR's vallen buiten de cooldown van vijf dagen: de nieuwe versie kan uren oud zijn en niemand anders heeft ernaar gekeken. Een engineer met schrijfrechten moet deze PR goedkeuren op commit \`${head_sha:0:7}\`; kijk daarbij naar de advisory, de diff van het lockfile en de publicatiedatum van de nieuwe versie. Een push naar de branch verlaat de goedkeuring."
