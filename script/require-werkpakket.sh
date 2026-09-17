#!/usr/bin/env bash
# Merge-poort: elke pull request noemt het werkpakket waaraan hij bijdraagt.
#
# De roadmap op /roadmap zegt wat er te doen is; de pull requests zeggen wat er
# gedaan wordt. Die twee stonden los van elkaar. Het werd met de hand overbrugd
# ("werkpakket werkwijze wetsontwikkeling uitgewerkt", #1409): dezelfde
# verwijzing, elke keer anders geschreven en door niets te controleren. Deze
# poort maakt er één vorm van die een mens leest en een script kan oogsten.
#
# De vorm is een trailer, onderaan de body:
#
#     Werkpakket: referentie-casus-i
#     Werkpakket: referentie-casus-i, effect-over-tijd
#     Werkpakket: geen — losse typefout in de docs
#
# Trailer-vormig met opzet, en niet "ergens een slug in de tekst". Een trailer
# staat op zijn eigen regel, is met één grep te vinden, overleeft het kopiëren
# van een PR-body naar een merge-commit, en laat zich later uit `git log`
# oogsten zonder dat er iets aan deze poort hoeft te veranderen. Dat laatste is
# het doel achter het doel: commits en PR's per werkpakket kunnen optellen.
#
# `geen` moet een reden dragen. Zonder die eis is het een leeg vakje dat je
# zonder nadenken invult, en dan meet de poort of iemand een regel kan plakken
# in plaats van of hij de vraag heeft gesteld. De reden is het hele punt.
#
# Wat de poort als feit behandelt, haalt zij zelf op. Uit de omgeving komen
# alleen de coördinaten (repository, PR-nummer); de body, de auteur en of dit
# een fork is komen van de API. Anders zou `IS_DEPENDABOT: true` in een
# env-blok van de workflow — geschreven door de auteur van de PR die hier
# beoordeeld wordt — genoeg zijn om eronderuit te komen.
#
# Waarom dit wél blokkeert en `docs/scripts/check-roadmap-rfcs.mjs` niet: dat
# script eist dat iemand een wérkpakket bijwerkt, en zou dus af werk tegenhouden
# tot de roadmap is bijgetrokken — redactionele achterstand, die je oploopt
# zonder het te merken. Deze poort vraagt één regel in de body die je toch aan
# het schrijven bent, en accepteert een onderbouwd `geen`. Er is niets om op
# achter te lopen.
#
# Wat dit niet dekt: deze job staat in het workflowbestand dat de PR meebrengt.
# Wie de job vervangt door `run: true` is groen zonder dat dit script draait.
# Sluiten kan alleen met een regel buiten de pull request om (een ruleset of
# CODEOWNERS over .github/workflows/**), net als bij de review-poort.
set -uo pipefail

: "${REPO:?REPO is verplicht}"
: "${PR_NUMBER:?PR_NUMBER is verplicht}"

# De map met werkpakketten, als checkout van de base-branch. De poort toetst
# tegen wat er op main staat plus wat deze PR toevoegt; zie hieronder.
WERKPAKKETTEN_DIR="${WERKPAKKETTEN_DIR:-docs/src/content/roadmap/werkpakketten}"

GITHUB_OUTPUT="${GITHUB_OUTPUT:-/dev/null}"
GITHUB_STEP_SUMMARY="${GITHUB_STEP_SUMMARY:-/dev/null}"

readonly TITEL='Werkpakket genoemd'

summary() { printf '%s\n' "$1" >>"$GITHUB_STEP_SUMMARY"; }
output() { printf '%s=%s\n' "$1" "$2" >>"$GITHUB_OUTPUT"; }

green() {
    echo "::notice title=${TITEL}::$1"
    summary "### Werkpakket: in orde"
    summary ""
    summary "$1"
    exit 0
}

blocked() {
    echo "::error title=${TITEL}::$1"
    summary "### Werkpakket: niet genoemd"
    summary ""
    summary "$1"
    exit 1
}

# Een leesfout is geen uitspraak over de PR. Hij blokkeert wel: doorlaten zou
# groen geven over iets wat de poort niet gezien heeft.
unreadable() {
    echo "::error title=${TITEL}::$1"
    summary "### Werkpakket: niet vast te stellen"
    summary ""
    summary "$1"
    exit 1
}

gh_stderr=$(mktemp "${TMPDIR:-/tmp}/werkpakket-gate-stderr.XXXXXX")
trap 'rm -f "$gh_stderr"' EXIT

if ! pr=$(gh api "repos/${REPO}/pulls/${PR_NUMBER}" 2>"$gh_stderr"); then
    unreadable "Pull request ${PR_NUMBER} in ${REPO} is niet op te halen, dus of er een werkpakket genoemd wordt valt niet vast te stellen. Draai deze job opnieuw. Foutmelding: $(tr '\n' ' ' <"$gh_stderr")"
fi

pr_field() { jq -r "${1} // \"\"" <<<"$pr"; }

body=$(pr_field '.body')
auteur=$(pr_field '.user.login')

# Dependabot opent werk dat bij geen werkpakket hoort en langs een eigen route
# loopt (claude-dependabot.yml). Een kwart van de PR's van de laatste honderd
# kwam daarvandaan; die allemaal een `geen — dependency-bump` laten schrijven
# maakt de regel tot ruis.
if [ "$auteur" = 'dependabot[bot]' ]; then
    green "Deze PR komt van Dependabot. Een versie-bump hoort bij geen werkpakket, dus de eis geldt hier niet."
fi

# Een fork-PR kan de roadmap niet kennen zoals wij hem kennen, en de
# bijdragerichtlijn zegt dat externe PR's doorgaans niet gemerged worden maar
# een issue opleveren. Hem laten stranden op een vormeis helpt niemand.
if [ "$(pr_field '.head.repo.full_name')" != "$REPO" ]; then
    green "Deze PR komt uit een fork. De werkpakket-eis geldt voor het werk van het team zelf."
fi

# De geldige slugs: wat op de base-branch staat plus wat deze PR toevoegt.
#
# De base-checkout is het anker — hij kan niet door de PR herschreven worden.
# Maar een PR die een nieuw werkpakket toevoegt én eraan werkt moet zichzelf
# kunnen noemen, dus komen de bestandsnamen uit de PR erbij. Dat is geen gat:
# een slug verzinnen om hier langs te komen betekent een werkpakket aan de
# roadmap toevoegen, en dat staat zichtbaar in de diff.
if [ ! -d "$WERKPAKKETTEN_DIR" ]; then
    unreadable "De map met werkpakketten (${WERKPAKKETTEN_DIR}) is er niet, dus welke slugs geldig zijn valt niet vast te stellen."
fi

geldig=$(find "$WERKPAKKETTEN_DIR" -maxdepth 1 -name '*.md' -exec basename {} .md \; | sort -u)

if ! bestanden=$(gh api "repos/${REPO}/pulls/${PR_NUMBER}/files" --paginate --jq '.[].filename' 2>"$gh_stderr"); then
    unreadable "De bestandslijst van pull request ${PR_NUMBER} is niet op te halen, dus een werkpakket dat deze PR zelf toevoegt zou ten onrechte als onbekend gelden. Draai deze job opnieuw. Foutmelding: $(tr '\n' ' ' <"$gh_stderr")"
fi

# Op de mapnaam en niet op het volledige pad: de API geeft paden relatief aan
# de repo terug, terwijl WERKPAKKETTEN_DIR ook een absoluut pad kan zijn (in de
# tests is het dat). De mapnaam is wat beide gemeen hebben.
map=$(basename "$WERKPAKKETTEN_DIR")
toegevoegd=$(grep -E "(^|/)${map}/[^/]+\.md$" <<<"$bestanden" |
    sed -E 's#.*/##; s#\.md$##' | sort -u)
geldig=$(printf '%s\n%s\n' "$geldig" "$toegevoegd" | grep -v '^$' | sort -u)

if [ -z "$geldig" ]; then
    unreadable "Er zijn geen werkpakketten gevonden in ${WERKPAKKETTEN_DIR}, dus de genoemde slug valt nergens aan te toetsen."
fi

# De trailer, op zijn eigen regel. De laatste telt als er meerdere staan: een
# body wordt van boven naar beneden bijgewerkt, dus onderaan staat de nieuwste.
# \r eraf, want GitHub levert de body met CRLF aan.
regel=$(printf '%s' "$body" | tr -d '\r' | grep -iE '^[[:space:]]*Werkpakket[[:space:]]*:' | tail -1)

if [ -z "$regel" ]; then
    blocked "Deze pull request noemt geen werkpakket. Zet onderaan de omschrijving een regel \`Werkpakket: <slug>\` met het werkpakket waaraan hij bijdraagt, of \`Werkpakket: geen — <reden>\` als het werk bij geen enkel werkpakket hoort. De slugs staan in ${WERKPAKKETTEN_DIR}/ en op /roadmap."
fi

waarde=$(sed -E 's/^[[:space:]]*[Ww]erkpakket[[:space:]]*:[[:space:]]*//' <<<"$regel")

# `geen` met een reden erachter. Het scheidingsteken mag een kort of lang
# streepje zijn of een dubbele punt: wie de regel met de hand typt moet niet op
# een teken struikelen dat zijn toetsenbord niet makkelijk geeft.
if grep -qiE '^geen([[:space:]]*$|[[:space:]]*[-—:])' <<<"$waarde"; then
    reden=$(sed -E 's/^[Gg]een[[:space:]]*[-—:]?[[:space:]]*//' <<<"$waarde")
    if [ -z "${reden//[[:space:]]/}" ]; then
        blocked "Deze pull request zegt \`Werkpakket: geen\` zonder reden. Schrijf op waarom dit werk bij geen enkel werkpakket hoort, bijvoorbeeld \`Werkpakket: geen — losse typefout in de docs\`. De reden is waar het om gaat: zonder is het een vakje dat zichzelf invult."
    fi
    output 'werkpakketten' ''
    output 'reden' "$reden"
    green "Deze pull request hoort bij geen werkpakket, en zegt waarom: ${reden}"
fi

# Kommagescheiden; één PR mag aan meer dan één werkpakket bijdragen.
# `genoemd=()` vooraf, want een lege waarde laat `read -ra` de array ongemoeid
# en onder `set -u` is een lege array uitvouwen dan een fout in plaats van een
# lege lijst.
genoemd=()
IFS=',' read -ra genoemd <<<"$waarde"

onbekend=()
gevonden=()
for ruw in ${genoemd[@]+"${genoemd[@]}"}; do
    slug=$(printf '%s' "$ruw" | tr -d '[:space:]' | tr '[:upper:]' '[:lower:]')
    slug=${slug//\`/}
    [ -n "$slug" ] || continue
    if grep -qxF "$slug" <<<"$geldig"; then
        gevonden+=("$slug")
    else
        onbekend+=("$slug")
    fi
done

if [ ${#onbekend[@]} -gt 0 ]; then
    # Een suggestie erbij: een poort die alleen "nee" zegt laat de ander zoeken.
    # De dichtstbijzijnde op gedeelde woorddelen, niet op letterafstand: slugs
    # zijn samengesteld uit woorden en `referentie-casus-ii` hoort bij
    # `referentie-casus-i` te landen.
    suggesties=''
    for slug in "${onbekend[@]}"; do
        kop=${slug%%-*}
        dichtbij=$(grep -F "$kop" <<<"$geldig" | head -3 | tr '\n' '@' | sed 's/@$//; s/@/, /g')
        [ -n "$dichtbij" ] && suggesties="${suggesties} Bedoelde je bij \`${slug}\`: ${dichtbij}?"
    done
    lijst=$(printf '`%s`, ' "${onbekend[@]}" | sed 's/, $//')
    blocked "Deze pull request noemt een werkpakket dat niet bestaat: ${lijst}.${suggesties} Kijk in ${WERKPAKKETTEN_DIR}/ of op /roadmap welke slugs er zijn."
fi

if [ ${#gevonden[@]} -eq 0 ]; then
    blocked "De regel \`${regel}\` noemt geen werkpakket. Zet er een slug achter, of \`geen — <reden>\`."
fi

output 'werkpakketten' "$(printf '%s,' "${gevonden[@]}" | sed 's/,$//')"
output 'reden' ''
green "Deze pull request draagt bij aan: $(printf '`%s`, ' "${gevonden[@]}" | sed 's/, $//')."
