#!/usr/bin/env bash
# De tijdsafhankelijke helft van de security-audit: kwetsbaarheden in
# afhankelijkheden, uit de RustSec-database en uit de npm-advisory-database.
#
# Deze controle staat bewust buiten de PR-poort. Een nieuwe advisory verandert
# niets aan de pull request, maar maakt hem wel rood: op 7 augustus 2026 legde
# dompurify overdag elke openstaande PR stil en nanoid 's avonds de hele repo,
# beide keren zonder dat iemand iets had gewijzigd. Wat deterministisch is
# (`bans`, `licenses`, `sources`, license-checker) blijft in `just audit` en
# blijft dus blokkeren; dit deel hangt aan de buitenwereld en draait periodiek.
#
# De uitkomst gaat naar twee bestanden in $ADVISORY_OUT, want een uitslag die
# alleen in een runlog staat wordt door niemand opgepakt:
#   advisories.ids  één regel per bevinding: "<tool> <id>", de vingerafdruk
#                   waarop script/report-advisories.sh dubbele meldingen
#                   herkent.
#   advisories.md   het lichaam van de melding.
#
# Exitcode 0 = schoon, 1 = bevindingen.
set -uo pipefail

cd "$(dirname "$0")/.."

out="${ADVISORY_OUT:-$(mktemp -d)}"
mkdir -p "$out"
ids="$out/advisories.ids"
body="$out/advisories.md"
: >"$ids"

cargo_log="$out/cargo-deny.log"
npm_log="$out/npm-audit.log"

echo "== cargo-deny advisories =="
script/cargo-deny.sh advisories 2>&1 | tee "$cargo_log"
cargo_status=${PIPESTATUS[0]}

echo
echo "== npm audit =="
script/npm-audit-all.sh 2>&1 | tee "$npm_log"
npm_status=${PIPESTATUS[0]}

# Een tool die omvalt zonder een advisory-id te noemen (een yanked crate, een
# onbereikbare database) is óók een bevinding: stil doorlaten is precies wat
# deze controle moet uitsluiten. Zo'n geval krijgt het vaste id `onbekend`, en
# niet de foutmelding zelf, zodat een storing die zich dagelijks herhaalt op
# hetzelfde issue landt in plaats van elke nacht een nieuw issue te openen.
verzamel() { # tool, logbestand, exitcode, patroon
    local tool="$1" log="$2" status="$3" patroon="$4" gevonden
    gevonden=$(grep -oE "$patroon" "$log" | sort -u)
    if [ -n "$gevonden" ]; then
        while IFS= read -r id; do echo "$tool $id"; done <<<"$gevonden" >>"$ids"
    elif [ "$status" -ne 0 ]; then
        echo "$tool onbekend" >>"$ids"
    fi
}

verzamel cargo-deny "$cargo_log" "$cargo_status" 'RUSTSEC-[0-9]{4}-[0-9]{4}'
verzamel npm "$npm_log" "$npm_status" 'GHSA-[0-9a-z]{4}-[0-9a-z]{4}-[0-9a-z]{4}'

sort -u -o "$ids" "$ids"

{
    if [ ! -s "$ids" ]; then
        echo "Geen openstaande advisories in de afhankelijkheden."
    else
        echo "De periodieke advisory-controle vond kwetsbaarheden in de"
        echo "afhankelijkheden. Ze blokkeren geen pull request; dit issue is de"
        echo "enige plek waar ze staan."
        echo
        echo "| Bron | Advisory |"
        echo "| --- | --- |"
        while read -r tool id; do
            if [ "$id" = "onbekend" ]; then
                echo "| \`$tool\` | viel om zonder advisory-id, zie het log hieronder |"
            else
                echo "| \`$tool\` | \`$id\` |"
            fi
        done <"$ids"
        echo
        echo "<details><summary>cargo-deny advisories</summary>"
        echo
        echo '```'
        # De kop, niet de staart: cargo-deny zet de foutblokken vooraan en laat
        # daar een inclusion-graph op volgen die het scherm vult.
        head -80 "$cargo_log"
        echo '```'
        echo
        echo "</details>"
        echo
        echo "<details><summary>npm audit</summary>"
        echo
        echo '```'
        head -80 "$npm_log"
        echo '```'
        echo
        echo "</details>"
        echo
        echo "Een advisory in een Rust-crate gaat weg met een bump in"
        echo "\`packages/Cargo.lock\`, of anders met een onderbouwde \`ignore\` in"
        echo "\`deny.toml\`. Voor npm doet \`npm audit fix\` in de map van de"
        echo "betreffende lockfile het meeste; wat overblijft is een handmatige"
        echo "bump. Reproduceren kan met \`just audit-advisories\`."
    fi
} >"$body"

echo
if [ -s "$ids" ]; then
    echo "Advisories gevonden: $(wc -l <"$ids")"
    cat "$ids"
    exit 1
fi

echo "Geen advisories."
exit 0
