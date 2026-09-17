#!/usr/bin/env bash
# Dekt elk pad dat groen of rood beslist in script/require-werkpakket.sh, met
# een `gh`-stub op PATH en een map met werkpakketten op schijf.
#
# Zonder deze test is een poort die altijd doorlaat niet te onderscheiden van
# een die werkt, en dat is hier extra makkelijk over het hoofd te zien: de
# poort is groen in het geval dat het vaakst voorkomt.
set -uo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
gate="$here/require-werkpakket.sh"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

pass=0
fail=0

# De stub bedient de twee endpoints die de poort aanroept. Staat er `FAIL` in
# de fixture, dan faalt die aanroep, zodat een leesfout op één endpoint los te
# testen is van de rest.
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
*/files*) file="$FIXTURES/files.json" ;;
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

# De werkpakketten zoals ze op de base-branch staan.
wp_dir="$tmp/werkpakketten"
mkdir -p "$wp_dir"
for slug in referentie-casus-i referentie-casus-ii effect-over-tijd \
    controle-en-herstel specificaties-i-documentatie-op-orde; do
    printf -- '---\nid: %s\n---\n' "$slug" >"$wp_dir/$slug.md"
done

# Een corpus zoals het op schijf staat: corpus/regulation/<land>/<soort>/<wet>/
# met per wet een of meer versiebestanden. De wet met een `url` levert een link
# op, de wet met alleen een `bwb_id` een opgebouwde, en de derde geen van beide
# — dat zijn de drie paden in de linkopbouw.
corpus_dir="$tmp/corpus"
mkdir -p "$corpus_dir/nl/wet/wet_op_de_zorgtoeslag" \
    "$corpus_dir/nl/wet/algemene_wet_bestuursrecht" \
    "$corpus_dir/nl/wet/wet_zonder_bron"
printf -- '---\n$id: wet_op_de_zorgtoeslag\nbwb_id: BWBR0018451\nurl: https://wetten.overheid.nl/BWBR0018451/2025-01-01\n' \
    >"$corpus_dir/nl/wet/wet_op_de_zorgtoeslag/2025-01-01.yaml"
printf -- '---\n$id: algemene_wet_bestuursrecht\nbwb_id: BWBR0005537\n' \
    >"$corpus_dir/nl/wet/algemene_wet_bestuursrecht/2024-01-01.yaml"
printf -- '---\n$id: wet_zonder_bron\n' \
    >"$corpus_dir/nl/wet/wet_zonder_bron/2024-01-01.yaml"
# Mappen die géén wet zijn, maar in het echte corpus wel bestaan: de soortmap
# (`wet`), een gemeente, en de scenario's naast een wet. Op "de map bestaat"
# zouden die alle drie als wet doorgaan.
mkdir -p "$corpus_dir/nl/wet/wet_op_de_zorgtoeslag/scenarios" \
    "$corpus_dir/nl/gemeentelijke_verordening/amsterdam/apv_erfgrens"
printf -- '---\n$id: apv_erfgrens\nbwb_id: CVDR0001\n' \
    >"$corpus_dir/nl/gemeentelijke_verordening/amsterdam/apv_erfgrens/2024-01-01.yaml"
: >"$corpus_dir/nl/wet/wet_op_de_zorgtoeslag/scenarios/een.feature"

pr_json() { # $1 = auteur, $2 = body, $3 = head repo (leeg = zelfde repo)
    jq -n --arg u "$1" --arg b "$2" --arg r "${3:-o/r}" \
        '{user: {login: $u}, body: $b, head: {repo: {full_name: $r}}}'
}

geen_bestanden='[]'
# Een PR die zelf een werkpakket toevoegt. Het pad is relatief aan de repo, net
# als wat de API teruggeeft — en dus níét gelijk aan WERKPAKKETTEN_DIR, dat in
# deze test een absoluut tijdelijk pad is. Dat verschil is de reden dat de poort
# op de mapnaam matcht en niet op het hele pad.
voegt_toe='[{"filename": "docs/src/content/roadmap/werkpakketten/nieuw-werkpakket.md"},
             {"filename": "docs/src/lib/roadmap.ts"}]'

# $1 = naam, $2 = verwachte exitcode, $3 = pr.json, $4 = files.json,
# $5 = optioneel patroon in de uitvoer, $6 = optioneel patroon in $GITHUB_OUTPUT
check() {
    local name="$1" want="$2" needle="${5:-}" out_needle="${6:-}"
    printf '%s' "$3" >"$tmp/pr.json"
    printf '%s' "$4" >"$tmp/files.json"
    : >"$tmp/outputs"

    local out status
    : >"$tmp/summary"
    out="$(PATH="$tmp:$PATH" FIXTURES="$tmp" \
        REPO=o/r PR_NUMBER=42 WERKPAKKETTEN_DIR="$wp_dir" CORPUS_DIR="$corpus_dir" \
        GITHUB_OUTPUT="$tmp/outputs" GITHUB_STEP_SUMMARY="$tmp/summary" \
        bash "$gate" 2>&1)"
    status=$?
    # De wet-links komen in de samenvatting, niet in de melding, dus die telt
    # mee als uitvoer waar een test een patroon in kan zoeken.
    out="${out}$(cat "$tmp/summary")"

    if [ "$status" -ne "$want" ]; then
        echo "FAIL: $name — exit $status, verwacht $want"
        sed 's/^/    /' <<<"$out"
        fail=$((fail + 1))
        return
    fi
    # `--` ervoor: een patroon dat met een streepje begint (een markdown-lijst)
    # zou grep anders als optie lezen.
    if [ -n "$needle" ] && ! grep -qF -- "$needle" <<<"$out"; then
        echo "FAIL: $name — '$needle' niet in de uitvoer"
        sed 's/^/    /' <<<"$out"
        fail=$((fail + 1))
        return
    fi
    if [ -n "$out_needle" ] && ! grep -qxF -- "$out_needle" "$tmp/outputs"; then
        echo "FAIL: $name — '$out_needle' niet in \$GITHUB_OUTPUT"
        sed 's/^/    /' "$tmp/outputs"
        fail=$((fail + 1))
        return
    fi
    echo "ok: $name"
    pass=$((pass + 1))
}

# --- de gewone gevallen ---

check "een genoemd werkpakket laat de PR door" 0 \
    "$(pr_json anne 'Wat dit doet.

Werkpakket: referentie-casus-i')" "$geen_bestanden" \
    'draagt bij aan' 'werkpakketten=referentie-casus-i'

check "het werkpakket komt als link naar de roadmap in de samenvatting" 0 \
    "$(pr_json anne 'Werkpakket: referentie-casus-i')" "$geen_bestanden" \
    '[referentie-casus-i](https://regelrecht.rijks.app/roadmap/werkpakket/referentie-casus-i)'

check "twee werkpakketten mogen" 0 \
    "$(pr_json anne 'Werkpakket: referentie-casus-i, effect-over-tijd')" "$geen_bestanden" \
    'draagt bij aan' 'werkpakketten=referentie-casus-i,effect-over-tijd'

check "spaties en hoofdletters in de regel geven niets" 0 \
    "$(pr_json anne '  Werkpakket :  Referentie-Casus-I  ')" "$geen_bestanden" \
    'draagt bij aan' 'werkpakketten=referentie-casus-i'

check "backticks om de slug geven niets" 0 \
    "$(pr_json anne 'Werkpakket: `controle-en-herstel`')" "$geen_bestanden" \
    'draagt bij aan' 'werkpakketten=controle-en-herstel'

check "de laatste regel telt als er meerdere staan" 0 \
    "$(pr_json anne 'Werkpakket: referentie-casus-i

herzien:

Werkpakket: effect-over-tijd')" "$geen_bestanden" \
    'draagt bij aan' 'werkpakketten=effect-over-tijd'

check "CRLF uit de GitHub-body breekt de regel niet" 0 \
    "$(printf '{"user":{"login":"anne"},"body":"Werkpakket: referentie-casus-i\\r\\n","head":{"repo":{"full_name":"o/r"}}}')" \
    "$geen_bestanden" 'draagt bij aan' 'werkpakketten=referentie-casus-i'

# --- geen werkpakket ---

check "geen met reden mag" 0 \
    "$(pr_json anne 'Werkpakket: geen — losse typefout in de docs')" "$geen_bestanden" \
    'hoort bij geen werkpakket' 'reden=losse typefout in de docs'

check "geen met een kort streepje mag ook" 0 \
    "$(pr_json anne 'Werkpakket: geen - losse typefout')" "$geen_bestanden" \
    'hoort bij geen werkpakket'

check "geen met een dubbele punt mag ook" 0 \
    "$(pr_json anne 'Werkpakket: geen: losse typefout')" "$geen_bestanden" \
    'hoort bij geen werkpakket'

check "geen zonder reden blokkeert" 1 \
    "$(pr_json anne 'Werkpakket: geen')" "$geen_bestanden" \
    'zonder reden'

check "geen met alleen een streepje blokkeert" 1 \
    "$(pr_json anne 'Werkpakket: geen —   ')" "$geen_bestanden" \
    'zonder reden'

# `geen` moet een woord op zichzelf zijn. Zonder die eis leest de poort een
# echte slug die met "geen-" begint als ontheffing, en toetst hem nooit.
check "een slug die met geen- begint is geen ontheffing" 1 \
    "$(pr_json anne 'Werkpakket: geen-werkpakket-bestaat')" "$geen_bestanden" \
    'niet bestaat'

# --- wat er mis kan gaan ---

check "een PR zonder de regel blokkeert" 1 \
    "$(pr_json anne 'Deze PR doet van alles maar zegt niet waarvoor.')" "$geen_bestanden" \
    'noemt geen werkpakket'

check "een lege body blokkeert" 1 \
    "$(pr_json anne '')" "$geen_bestanden" \
    'noemt geen werkpakket'

check "een onbekende slug blokkeert" 1 \
    "$(pr_json anne 'Werkpakket: referentie-casus-iii')" "$geen_bestanden" \
    'noemt een werkpakket dat niet bestaat'

check "een onbekende slug krijgt een suggestie mee" 1 \
    "$(pr_json anne 'Werkpakket: referentie-casus-iii')" "$geen_bestanden" \
    'Bedoelde je'

check "een lege waarde achter de regel blokkeert" 1 \
    "$(pr_json anne 'Werkpakket:')" "$geen_bestanden" \
    'noemt geen werkpakket'

check "een regel met alleen een komma blokkeert" 1 \
    "$(pr_json anne 'Werkpakket: ,')" "$geen_bestanden" \
    'noemt geen werkpakket'

check "een slug die alleen in de PR-head bestaat telt mee" 0 \
    "$(pr_json anne 'Werkpakket: nieuw-werkpakket')" "$voegt_toe" \
    'draagt bij aan' 'werkpakketten=nieuw-werkpakket'

# --- de optionele Wet-regel ---

check "een wet uit het corpus komt er als link bij" 0 \
    "$(pr_json anne 'Werkpakket: referentie-casus-i
Wet: wet_op_de_zorgtoeslag')" "$geen_bestanden" \
    '[wet_op_de_zorgtoeslag](https://wetten.overheid.nl/BWBR0018451/2025-01-01)' \
    'wetten=wet_op_de_zorgtoeslag'

check "een wet zonder url krijgt een link uit het bwb-nummer" 0 \
    "$(pr_json anne 'Werkpakket: referentie-casus-i
Wet: algemene_wet_bestuursrecht')" "$geen_bestanden" \
    '[algemene_wet_bestuursrecht](https://wetten.overheid.nl/BWBR0005537)'

check "een wet zonder url en zonder bwb-nummer komt zonder link" 0 \
    "$(pr_json anne 'Werkpakket: referentie-casus-i
Wet: wet_zonder_bron')" "$geen_bestanden" \
    '- wet_zonder_bron'

check "twee wetten mogen" 0 \
    "$(pr_json anne 'Werkpakket: referentie-casus-i
Wet: wet_op_de_zorgtoeslag, algemene_wet_bestuursrecht')" "$geen_bestanden" \
    'draagt bij aan' 'wetten=wet_op_de_zorgtoeslag,algemene_wet_bestuursrecht'

check "een onbekende wet blokkeert" 1 \
    "$(pr_json anne 'Werkpakket: referentie-casus-i
Wet: wet_op_de_zonnebloem')" "$geen_bestanden" \
    'niet in het corpus staat'

# `find -name` neemt een glob. Zonder vormtoets zou `*` de eerste de beste wet
# matchen en een link met het label `*` opleveren: de poort zou dan iets
# bevestigen wat niet waar is.
check "een glob als wet blokkeert in plaats van de eerste wet te pakken" 1 \
    "$(pr_json anne 'Werkpakket: referentie-casus-i
Wet: *')" "$geen_bestanden" \
    'niet in het corpus staat'

check "een wet met een pad erin blokkeert" 1 \
    "$(pr_json anne 'Werkpakket: referentie-casus-i
Wet: ../../etc')" "$geen_bestanden" \
    'niet in het corpus staat'

# Een wet is een map mét versiebestanden. Deze drie mappen bestaan wel maar zijn
# geen wet; op "de map bestaat" zouden ze groen geven en als geraakte wet in de
# samenvatting belanden.
check "de soortmap is geen wet" 1 \
    "$(pr_json anne 'Werkpakket: referentie-casus-i
Wet: wet')" "$geen_bestanden" \
    'niet in het corpus staat'

check "een scenariomap naast een wet is geen wet" 1 \
    "$(pr_json anne 'Werkpakket: referentie-casus-i
Wet: scenarios')" "$geen_bestanden" \
    'niet in het corpus staat'

check "een gemeentemap is geen wet" 1 \
    "$(pr_json anne 'Werkpakket: referentie-casus-i
Wet: amsterdam')" "$geen_bestanden" \
    'niet in het corpus staat'

check "een gemeentelijke verordening is wel een wet" 0 \
    "$(pr_json anne 'Werkpakket: referentie-casus-i
Wet: apv_erfgrens')" "$geen_bestanden" \
    '[apv_erfgrens](https://wetten.overheid.nl/CVDR0001)'

check "geen Wet-regel is in orde, hij is optioneel" 0 \
    "$(pr_json anne 'Werkpakket: referentie-casus-i')" "$geen_bestanden" \
    'draagt bij aan'

check "de Wet-regel wordt niet met de Werkpakket-regel verward" 0 \
    "$(pr_json anne 'Werkpakket: referentie-casus-i')" "$geen_bestanden" \
    'draagt bij aan' 'werkpakketten=referentie-casus-i'

# --- uitzonderingen ---

check "dependabot valt buiten de eis" 0 \
    "$(pr_json 'dependabot[bot]' 'Bumps serde from 1.0.1 to 1.0.2.')" "$geen_bestanden" \
    'komt van Dependabot'

check "een fork-PR valt buiten de eis" 0 \
    "$(pr_json iemand 'Geen idee wat een werkpakket is.' 'fork/r')" "$geen_bestanden" \
    'komt uit een fork'

# --- leesfouten zijn geen uitspraak, en blokkeren ---

check "een onleesbare PR blokkeert" 1 \
    'FAIL' "$geen_bestanden" \
    'niet op te halen'

check "een onleesbare bestandslijst blokkeert" 1 \
    "$(pr_json anne 'Werkpakket: referentie-casus-i')" 'FAIL' \
    'niet op te halen'

# De map zelf weg: dan valt er niets te toetsen en is groen geven een uitspraak
# over iets wat de poort niet gezien heeft.
out="$(PATH="$tmp:$PATH" FIXTURES="$tmp" REPO=o/r PR_NUMBER=42 \
    WERKPAKKETTEN_DIR="$tmp/bestaat-niet" GITHUB_OUTPUT=/dev/null \
    bash "$gate" 2>&1)"
if [ $? -eq 1 ] && grep -qF 'is er niet' <<<"$out"; then
    echo "ok: een ontbrekende werkpakketten-map blokkeert"
    pass=$((pass + 1))
else
    echo "FAIL: een ontbrekende werkpakketten-map blokkeert"
    sed 's/^/    /' <<<"$out"
    fail=$((fail + 1))
fi

echo
echo "${pass} geslaagd, ${fail} gefaald"
[ "$fail" -eq 0 ]
