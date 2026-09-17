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

# Alleen de kale vorm. Een regel die al een `[` draagt is al eens omgezet, of
# met de hand als link geschreven; daar blijft deze stap vanaf, anders zou een
# tweede run er `[[slug](url)](url)` van maken.
nieuw=$(printf '%s' "$body" | awk -v url="$ROADMAP_URL" '
BEGIN { FS = "" }
{
    regel = $0
    # \r bewaren: GitHub levert de body met CRLF en die hoort er weer in.
    cr = ""
    if (regel ~ /\r$/) { cr = "\r"; sub(/\r$/, "", regel) }

    if (regel ~ /^[[:space:]]*[Ww]erkpakket[[:space:]]*:/ && regel !~ /\[/) {
        kop = regel
        sub(/:.*/, ":", kop)
        waarde = regel
        sub(/^[[:space:]]*[Ww]erkpakket[[:space:]]*:[[:space:]]*/, "", waarde)

        # `geen — reden` blijft staan: dat is geen slug en heeft geen pagina.
        if (tolower(waarde) ~ /^geen([[:space:]]|$)/) {
            print regel cr
            next
        }

        n = split(waarde, delen, ",")
        uit = ""
        for (i = 1; i <= n; i++) {
            slug = delen[i]
            gsub(/^[[:space:]]+|[[:space:]]+$/, "", slug)
            gsub(/`/, "", slug)
            if (slug == "") continue
            if (uit != "") uit = uit ", "
            uit = uit "[" slug "](" url "/" slug ")"
        }
        if (uit == "") { print regel cr; next }
        print kop " " uit cr
        next
    }
    print regel cr
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
