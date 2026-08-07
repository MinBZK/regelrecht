#!/usr/bin/env bash
# De PR-opruiming en de nachtelijke opruiming lopen over dezelfde
# GHCR-namespace en horen dus dezelfde packagelijst te gebruiken. Uiteenlopen
# is stil: een image van een package dat alleen de ene lijst kent, blijft
# liggen zonder dat iets faalt.
set -euo pipefail

cd "$(dirname "$0")/.."

files=(.github/workflows/deploy.yml .github/workflows/scheduled-cleanup.yml)
reference=""
reference_file=""
status=0

for f in "${files[@]}"; do
    packages=$(grep -oE 'for PACKAGE in [^;]+' "$f" |
        sed 's/^for PACKAGE in //' |
        tr ' ' '\n' | grep -v '^$' | sort -u)
    if [ -z "$packages" ]; then
        echo "FOUT: $f heeft geen 'for PACKAGE in'-lus" >&2
        status=1
        continue
    fi
    if [ -z "$reference" ]; then
        reference="$packages"
        reference_file="$f"
    elif [ "$packages" != "$reference" ]; then
        echo "FOUT: de packagelijst in $f wijkt af van die in $reference_file:" >&2
        diff <(echo "$reference") <(echo "$packages") >&2 || true
        status=1
    fi
done

if [ "$status" -eq 0 ]; then
    echo "GHCR-packagelijst is gelijk in ${#files[@]} workflows"
fi
exit "$status"
