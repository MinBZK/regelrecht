#!/usr/bin/env bash
# Meldt de uitkomst van script/audit-advisories.sh als issue, en houdt die
# melding actueel.
#
# Een advisory die geen pull request rood maakt, wordt door niemand opgepakt
# zolang hij alleen in een runlog staat. De melding moet dus een eigen plek in
# de repo hebben, en die plek moet vanzelf verdwijnen zodra de advisory weg is —
# anders is een openstaand issue over een halfjaar niets meer waard.
#
# Drie regels bepalen dat gedrag:
#
#   Schoon        elk openstaand advisory-issue gaat dicht, met de reden erbij.
#                 De controle draait ook op main na een bump, dus een fix sluit
#                 het issue binnen minuten in plaats van bij de volgende nacht.
#   Zelfde set    er gebeurt niets. De vingerafdruk (sha256 over de gevonden
#                 advisory-ids) staat in het issuelichaam, dus een dagelijkse
#                 run herhaalt zichzelf niet.
#   Andere set    een nieuw issue, en het vorige gaat dicht met een verwijzing.
#                 Zo is de titel altijd de stand van vandaag en staat er nooit
#                 meer dan één open.
#
# Blijft dezelfde set langer dan NAG_DAYS staan, dan komt er één herinnering per
# NAG_DAYS met de leeftijd erin. Dat is de bovengrens: geen dagelijkse ruis,
# maar ook geen issue dat maanden onbesproken wegzakt.
set -uo pipefail

: "${REPO:?REPO is verplicht}"

out="${ADVISORY_OUT:?ADVISORY_OUT is verplicht}"
ids="$out/advisories.ids"
body="$out/advisories.md"
label="${LABEL:-security-advisory}"
nag_days="${NAG_DAYS:-7}"
run_url="${RUN_URL:-}"
now_epoch=$(date -u -d "${NOW:-now}" +%s)

for f in "$ids" "$body"; do
    if [ ! -f "$f" ]; then
        echo "FOUT: $f bestaat niet; draai eerst script/audit-advisories.sh" >&2
        exit 1
    fi
done

fout() {
    echo "::error title=Advisory-melding::$1"
    exit 1
}

vandaag=$(date -u -d "@$now_epoch" +%Y-%m-%d)

dagen_sinds() { # ISO-8601 tijdstip -> hele dagen tot nu
    local t
    t=$(date -u -d "$1" +%s 2>/dev/null) || { echo 0; return; }
    echo $(((now_epoch - t) / 86400))
}

# Nummer, aanmaakdatum en vingerafdruk van elk openstaand advisory-issue. Een
# mislukte aanroep is niet hetzelfde als "er staat er geen": dat verschil
# bepaalt of er zo meteen een tweede issue over dezelfde advisory bijkomt.
if ! open_issues=$(gh issue list --repo "$REPO" --label "$label" --state open \
    --limit 50 --json number,createdAt,body \
    --jq '.[] | [.number, .createdAt, (((.body // "") | capture("<!-- advisories: (?<fp>[0-9a-f]+) -->") | .fp) // "geen")] | @tsv' 2>&1); then
    fout "kon de openstaande issues met label ${label} niet opvragen: ${open_issues}"
fi

sluit() { # nummer, reden
    gh issue close "$1" --repo "$REPO" --comment "$2" ||
        fout "kon issue #$1 niet sluiten"
}

if [ ! -s "$ids" ]; then
    if [ -z "$open_issues" ]; then
        echo "Schoon, en er staat geen advisory-issue open."
        exit 0
    fi
    while IFS=$'\t' read -r nummer _ _; do
        [ -n "$nummer" ] || continue
        echo "Schoon: issue #${nummer} gaat dicht."
        sluit "$nummer" "De advisory-controle van ${vandaag} is schoon: geen van de gemelde advisories staat nog in de afhankelijkheden.${run_url:+ Zie ${run_url}.}"
    done <<<"$open_issues"
    exit 0
fi

fingerprint=$(sha256sum "$ids" | cut -c1-16)
aantal=$(wc -l <"$ids")

bestaand=""
bestaand_datum=""
while IFS=$'\t' read -r nummer datum fp; do
    [ -n "$nummer" ] || continue
    if [ "$fp" = "$fingerprint" ]; then
        bestaand="$nummer"
        bestaand_datum="$datum"
    fi
done <<<"$open_issues"

if [ -n "$bestaand" ]; then
    # De herinnering hangt aan de laatste herinnering, niet aan de leeftijd van
    # het issue: anders zou hij vanaf dag NAG_DAYS elke dag opnieuw komen.
    if ! laatste=$(gh issue view "$bestaand" --repo "$REPO" --json comments \
        --jq '[.comments[] | select((.body // "") | contains("<!-- advisories-nag -->")) | .createdAt] | max // ""' 2>&1); then
        fout "kon de reacties op issue #${bestaand} niet opvragen: ${laatste}"
    fi

    sinds=$(dagen_sinds "${laatste:-$bestaand_datum}")
    leeftijd=$(dagen_sinds "$bestaand_datum")

    if [ "$sinds" -lt "$nag_days" ]; then
        echo "Zelfde advisories als in issue #${bestaand} (${leeftijd} dagen oud); geen nieuwe melding."
        exit 0
    fi

    gh issue comment "$bestaand" --repo "$REPO" --body \
        "Deze advisories staan nu ${leeftijd} dagen open en zijn er vandaag (${vandaag}) nog steeds. Ze blokkeren geen enkele pull request, dus er is niets dat ze vanzelf onder de aandacht brengt.${run_url:+ De run van vandaag: ${run_url}.}

<!-- advisories-nag -->" ||
        fout "kon geen herinnering op issue #${bestaand} plaatsen"
    echo "Herinnering geplaatst op issue #${bestaand} (${leeftijd} dagen open)."
    exit 0
fi

gh label create "$label" --repo "$REPO" --color B60205 \
    --description "Openstaande advisory uit de periodieke security-controle" 2>/dev/null || true

{
    cat "$body"
    echo
    if [ -n "$run_url" ]; then
        echo "De [run](${run_url}) heeft het volledige log."
        echo
    fi
    echo "Dit issue gaat vanzelf dicht zodra \`just audit-advisories\` schoon is."
    echo
    echo "<!-- advisories: ${fingerprint} -->"
} >"$out/issue-body.md"

titel="Advisories in afhankelijkheden: ${aantal} openstaand (${vandaag})"
if ! nieuw=$(gh issue create --repo "$REPO" --title "$titel" --label "$label" \
    --body-file "$out/issue-body.md" 2>&1); then
    fout "kon het issue niet aanmaken: ${nieuw}"
fi
echo "Aangemaakt: $nieuw"

# Het vorige issue ging over een andere set advisories. Wat daarvan nog geldt,
# staat opnieuw in het nieuwe issue; twee open issues over dezelfde controle
# zouden alleen maar de vraag oproepen welke de stand is.
while IFS=$'\t' read -r nummer _ _; do
    [ -n "$nummer" ] || continue
    [ "$nummer" = "${nieuw##*/}" ] && continue
    sluit "$nummer" "Vervangen door de controle van ${vandaag}: ${nieuw}"
done <<<"$open_issues"
