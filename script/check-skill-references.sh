#!/usr/bin/env bash
# Versheidspoort op de skills onder .claude/skills/.
#
# Een skill is een instructie die een agent uitvoert zonder hem te toetsen. Een
# fout erin vermenigvuldigt zich dus over elke sessie, terwijl niets hem vindt:
# de skills staan buiten `cargo test`, buiten `just validate` en buiten de
# frontend-suite. Deze poort toetst de twee vormen die aantoonbaar zijn
# weggedreven.
#
#   1. Dode paden. Een backtick-pad naar deze repo dat niet bestaat stuurt een
#      agent naar een bestand dat er niet is. Paden die met opzet nog niet
#      bestaan (een uitvoerbestand, of een pad in de corpus-repo) horen in
#      script/.skill-path-exceptions, met de reden erbij. Die lijst werkt twee
#      kanten op: een uitzondering voor een pad dat inmiddels wél bestaat faalt
#      ook, zodat de lijst niet vol raakt met vergeten regels.
#
#   2. Muteerbare of verouderde $schema-URL's. RFC-013 eist een tag-URL; een
#      `refs/heads/main`-URL wijst naar inhoud die kan veranderen. De skill die
#      hem voorschrijft is aantoonbaar de bron van de corpusbestanden die hem
#      vandaag dragen. Een tag-URL moet bovendien de nieuwste schemaversie in
#      deze repo noemen, aan beide kanten van de URL dezelfde.
set -euo pipefail

# Draagbaar gehouden: macOS levert bash 3.2 en BSD grep, dus geen mapfile, geen
# associatieve arrays, geen `grep -P` en geen `find -printf`. De verzamelingen
# staan in tijdelijke bestanden en worden met `grep -Fx` doorzocht.

ROOT="${SKILL_REF_ROOT:-$(cd "$(dirname "$0")/.." && pwd)}"
SKILLS="$ROOT/.claude/skills"
EXCEPTIONS="$ROOT/script/.skill-path-exceptions"
[ -d "$SKILLS" ] || exit 0

status=0
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

# Nieuwste schemaversie in deze repo. Normaal is schema/latest een symlink naar
# die map; in een export zonder symlinks valt hij terug op de hoogste v*-map, zodat
# een checkout-eigenaardigheid niet elke commit blokkeert.
latest="$(basename "$(readlink "$ROOT/schema/latest" 2>/dev/null || echo "")")"
if [ -z "$latest" ]; then
    latest="$(for d in "$ROOT"/schema/v*; do [ -d "$d" ] && basename "$d"; done 2>/dev/null | sort -V | tail -1)"
fi
if [ -z "$latest" ]; then
    echo "SKILL-REFS: geen schemaversie gevonden onder $ROOT/schema" >&2
    exit 1
fi

# --- 1. Dode paden ---------------------------------------------------------

# Alleen backtick-tokens die met een top-level map van deze repo beginnen, geen
# glob- of placeholder-tekens bevatten, en geen regelverwijzing dragen. Een
# `…`- of `vX.Y.Z`-vorm is bedoeld als sjabloon en telt niet als pad. Elk
# tekstbestand telt mee, niet alleen markdown: een skill levert ook scripts die
# een agent uitvoert.
top_level='(packages|corpus|schema|docs|dev|frontend|frontend-lawmaking|bdd|script|conformance|\.claude|\.github)'

{ grep -rhoIE '`[^`]+`' "$SKILLS" 2>/dev/null || true; } |
    tr -d '`' |
    { grep -E "^$top_level/" || true; } |
    { grep -vE '[*?{}<>[:space:]]|\.\.\.|…|X\.Y\.Z' || true; } |
    sed -E 's/:[0-9-]*$//' |
    sort -u > "$work/referenced"

: > "$work/allowed"
if [ -f "$EXCEPTIONS" ]; then
    while IFS= read -r line || [ -n "$line" ]; do
        line="${line%%#*}"
        line="${line#"${line%%[![:space:]]*}"}"
        line="${line%"${line##*[![:space:]]}"}"
        if [ -n "$line" ]; then echo "$line" >> "$work/allowed"; fi
    done < "$EXCEPTIONS"
fi

: > "$work/missing"
while IFS= read -r path; do
    [ -z "$path" ] && continue
    [ -e "$ROOT/$path" ] && continue
    grep -qFx -- "$path" "$work/allowed" && continue
    echo "$path" >> "$work/missing"
done < "$work/referenced"

if [ -s "$work/missing" ]; then
    echo "SKILL-REFS: skills verwijzen naar paden die niet bestaan:" >&2
    sed 's/^/  /' "$work/missing" >&2
    echo "" >&2
    echo "Corrigeer het pad, of zet het met een reden in script/.skill-path-exceptions" >&2
    echo "als het met opzet nog niet bestaat." >&2
    status=1
fi

: > "$work/stale"
while IFS= read -r path; do
    if [ -e "$ROOT/$path" ] || ! grep -qFx -- "$path" "$work/referenced"; then
        echo "$path" >> "$work/stale"
    fi
done < "$work/allowed"

if [ -s "$work/stale" ]; then
    echo "SKILL-REFS: verouderde regels in script/.skill-path-exceptions:" >&2
    sed 's/^/  /' "$work/stale" >&2
    echo "" >&2
    echo "Het pad bestaat inmiddels, of geen enkele skill noemt het nog." >&2
    status=1
fi

# --- 2. Schema-URL's -------------------------------------------------------

expected="https://raw.githubusercontent.com/MinBZK/regelrecht/refs/tags/schema-$latest/schema/$latest/schema.json"
# Een vX.Y.Z-vorm is een sjabloon (zie hierboven) en telt niet mee.
url_re='https://raw\.githubusercontent\.com/MinBZK/regelrecht/[^[:space:]`"'"'"')]*schema\.json'

{ grep -rhoE "$url_re" "$SKILLS" 2>/dev/null || true; } |
    sort -u |
    { grep -vFx -- "$expected" || true; } |
    { grep -vF "X.Y.Z" || true; } > "$work/bad"

if [ -s "$work/bad" ]; then
    echo "SKILL-REFS: skills schrijven een \$schema-URL voor die niet de geldende is:" >&2
    sed 's/^/  /' "$work/bad" >&2
    echo "" >&2
    echo "Verwacht (RFC-013, tag-gebonden en op de nieuwste schemaversie):" >&2
    echo "  $expected" >&2
    status=1
fi

exit $status
