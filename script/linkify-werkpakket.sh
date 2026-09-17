#!/usr/bin/env bash
# Maakt van de kale slug in de `Werkpakket:`-regel een markdown-link naar de
# roadmap, in de body van de pull request zelf.
#
# Waarom in de body en niet alleen in de samenvatting van de check: de
# samenvatting zit achter een klik op de Actions-tab. Wie de PR leest ziet de
# regel, en die hoort te werken zoals elke andere verwijzing in de beschrijving.
#
# Je typt de kale slug; deze stap maakt er een link van. Andersom zou betekenen
# dat iedereen een URL uit zijn hoofd kent, en dat een typefout in het
# domeindeel niemand opvalt.
#
# De regel blijft parseerbaar. De slug staat in de linktekst én in de URL, dus
# `grep 'Werkpakket: \[<slug>\]'` en een oogst uit `git log` werken erna net zo
# goed. De poort zelf leest de linkvorm ook, want hij stript de opmaak voor hij
# toetst — anders zou deze stap de volgende run rood maken.
#
# Deze stap draait alleen als de poort groen was, en is nooit blokkerend: een
# mislukte herschrijving is een schoonheidsfoutje, geen uitspraak over de PR.
# Faalt hij, dan blijft de kale slug staan en dat is nog steeds correct.
set -uo pipefail

: "${REPO:?REPO is verplicht}"
: "${PR_NUMBER:?PR_NUMBER is verplicht}"

ROADMAP_URL="${ROADMAP_URL:-https://regelrecht.rijks.app/roadmap/werkpakket}"

# Nooit falen: deze stap mag de job niet rood maken.
afgebroken() {
    echo "::notice title=Werkpakket-link::$1"
    exit 0
}

gh_stderr=$(mktemp "${TMPDIR:-/tmp}/werkpakket-link-stderr.XXXXXX")
trap 'rm -f "$gh_stderr"' EXIT

if ! body=$(gh api "repos/${REPO}/pulls/${PR_NUMBER}" --jq '.body // ""' 2>"$gh_stderr"); then
    afgebroken "De body van pull request ${PR_NUMBER} is niet op te halen; de slug blijft zoals hij is. Foutmelding: $(tr '\n' ' ' <"$gh_stderr")"
fi

# Alleen de kale vorm, en alleen op de regel die de poort óók leest.
#
# Twee dingen moeten gelijk lopen met script/require-werkpakket.sh, anders
# schrijft deze stap iets de body in dat de poort niet bedoelde:
#
#   1. De poort neemt de láátste `Werkpakket:`-regel buiten een codeblok. Elke
#      regel herschrijven zou een voorbeeld in een codeblok ("zo ziet de regel
#      eruit") omzetten in een link en daarmee documentatie in de omschrijving
#      verminken.
#   2. De poort vergelijkt in kleine letters. Zonder dezelfde vouw zou
#      `Werkpakket: Referentie-Casus-I` groen worden en hier een URL opleveren
#      die 404't, want de pagina bestaat alleen op het kleine pad. De volgende
#      run merkt dat niet, want die vouwt de linktekst weer om. Een verzonnen
#      URL is erger dan geen: een kapotte link wordt geloofd tot iemand hem
#      aanklikt.
#
# De regel wordt daarom eerst gezocht (welke regelnummer), en pas in de tweede
# pas herschreven.
nieuw=$(printf '%s' "$body" | awk -v url="$ROADMAP_URL" '
function is_kaal(r) {
    return (r ~ /^[[:space:]]*[Ww]erkpakket[[:space:]]*:/ && r !~ /\[/)
}
# Een regel telt niet als hij binnen ``` staat of vier spaties is ingesprongen;
# dat is markdown voor "dit is een voorbeeld, geen instructie".
{ regels[NR] = $0 }
{
    r = $0
    sub(/\r$/, "", r)
    if (r ~ /^[[:space:]]*```/) { in_fence = !in_fence; next }
    if (in_fence) next
    if (r ~ /^    /) next
    if (is_kaal(r)) doel = NR
}
END {
    for (i = 1; i <= NR; i++) {
        regel = regels[i]
        # \r bewaren: GitHub levert de body met CRLF en die hoort er weer in.
        cr = ""
        if (regel ~ /\r$/) { cr = "\r"; sub(/\r$/, "", regel) }

        if (i != doel) { print regel cr; continue }

        kop = regel
        sub(/:.*/, ":", kop)
        waarde = regel
        sub(/^[[:space:]]*[Ww]erkpakket[[:space:]]*:[[:space:]]*/, "", waarde)

        # `geen — reden` blijft staan: dat is geen slug en heeft geen pagina.
        # Alleen "geen" als heel woord, net als de poort: een slug die met
        # "geen-" begint is een gewone slug en krijgt wél een link.
        kleine = tolower(waarde)
        if (kleine == "geen" || kleine ~ /^geen[^a-z0-9_-]/) {
            print regel cr
            continue
        }

        n = split(waarde, delen, ",")
        uit = ""
        for (j = 1; j <= n; j++) {
            slug = delen[j]
            gsub(/^[[:space:]]+|[[:space:]]+$/, "", slug)
            gsub(/`/, "", slug)
            slug = tolower(slug)
            if (slug == "") continue
            if (uit != "") uit = uit ", "
            uit = uit "[" slug "](" url "/" slug ")"
        }
        if (uit == "") { print regel cr; continue }
        print kop " " uit cr
    }
}')

if [ "$nieuw" = "$body" ]; then
    afgebroken "De werkpakket-regel is al een link of heeft er geen nodig; er is niets veranderd."
fi

# De body gaat als veld door jq de JSON in, niet via string-plakwerk: hij is
# door de auteur van de PR geschreven en bevat aanhalingstekens en regeleindes.
if ! jq -n --arg body "$nieuw" '{body: $body}' |
    gh api -X PATCH "repos/${REPO}/pulls/${PR_NUMBER}" --input - >/dev/null 2>"$gh_stderr"; then
    afgebroken "De body is niet bij te werken; de kale slug blijft staan. Foutmelding: $(tr '\n' ' ' <"$gh_stderr")"
fi

echo "::notice title=Werkpakket-link::De werkpakket-regel in de omschrijving wijst nu naar de roadmap."
