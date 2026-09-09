#!/usr/bin/env bash
# Bouw een mini-corpus voor een lokale BDD-run.
#
# De engine weigert boven de 100 geladen wetsversies. Een volledig corpus haalt
# die grens ruim, dus draai je tegen een selectie: per wet alleen de nieuwste
# versie <= peildatum, plus status.yaml en de hele map scenarios/.
#
# Gebruik:
#   mini-corpus.sh <corpus-root> <peildatum> <uit-map> <wet-pad>...
#
#   <corpus-root>  map met de nl/-laag (bv. regelrecht-corpus/regulation)
#   <peildatum>    YYYY-MM-DD
#   <uit-map>      wordt weggegooid en opnieuw aangemaakt
#   <wet-pad>      pad onder nl/, bv. wet/<naam> of amvb/<naam>
#
# Voorbeeld:
#   mini-corpus.sh <corpus-repo>/regulation 2026-07-01 /tmp/mini \
#     wet/<wet-a> wet/<wet-b> amvb/<besluit>
#
# Neem naast de wetten van je eigen scenario's ook elke wet op die daaruit via
# source.regulation wordt aangeroepen, en elke regeling die via implements een
# open term ervan invult. Laat je die weg, dan valt de keten om op "Law not
# found" of stil terug op een default.
#
# Zo'n selectie bedient bucket A (BDD_BUCKET=corpus). Bucket B draait tegen de
# synthetische test_*-wetten uit het eigen testcorpus van de repo en hoort niet
# in deze map thuis; draai die bucket apart.

set -euo pipefail

if [ "$#" -lt 4 ]; then
    sed -n '2,27p' "$0" >&2
    exit 2
fi

CORPUS_ROOT=$1
PEILDATUM=$2
OUT=$3
shift 3

SRC="$CORPUS_ROOT"
[ -d "$SRC/nl" ] && SRC="$SRC/nl"

rm -rf "$OUT"
mkdir -p "$OUT/nl"

for law in "$@"; do
    if [ ! -d "$SRC/$law" ]; then
        echo "onbekende wet: $SRC/$law" >&2
        exit 1
    fi
    # nieuwste versie <= peildatum; versiebestanden heten YYYY-MM-DD.yaml
    version=$(find "$SRC/$law" -maxdepth 1 -name '????-??-??.yaml' -printf '%f\n' \
        | sort | awk -v d="$PEILDATUM.yaml" '$0 <= d' | tail -1)
    if [ -z "$version" ]; then
        echo "geen versie <= $PEILDATUM voor $law" >&2
        exit 1
    fi
    mkdir -p "$OUT/nl/$law"
    cp "$SRC/$law/$version" "$OUT/nl/$law/"
    [ -f "$SRC/$law/status.yaml" ] && cp "$SRC/$law/status.yaml" "$OUT/nl/$law/"
    [ -d "$SRC/$law/scenarios" ] && cp -r "$SRC/$law/scenarios" "$OUT/nl/$law/"
    echo "  $law -> $version"
done

echo
echo "mini-corpus: $OUT"
echo "  $(find "$OUT" -name '*.yaml' | wc -l) yaml, $(find "$OUT" -name '*.feature' | wc -l) features"
