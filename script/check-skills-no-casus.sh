#!/usr/bin/env bash
# Leak-guard voor de dossier-agnostische skills.
#
# De skills onder .claude/skills/ beschrijven de regelrecht-METHODE en moeten
# dossier-agnostisch blijven. Deze repo is publiek; casus-specifieke inhoud
# (HHNK-namen, BSN's, waterschap-codes, corpus-repo-namen, trace-fragmenten met
# echte waarden) mag hier nooit in belanden. Fail-closed: bij een hit faalt de
# commit. De denylist is bewust uitbreidbaar — voeg nieuwe casus-markers toe
# zodra een nieuw dossier in beeld komt.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SKILLS="$ROOT/.claude/skills"
[ -d "$SKILLS" ] || exit 0

# Casus-markers (uitbreidbaar). Woordgrenzen waar een losse term anders zou matchen.
# NB: BSN's bewust niet generiek gevangen — de fictieve test-BSN's (9999-reeks) zijn juist
# de veilige; een échte BSN-leak vergt een ander, casus-gebonden signaal. Voeg per nieuw
# dossier de concrete identificatoren toe (naam waterschap/gemeente, codes, corpus-repo).
PATTERN='HHNK|Hollands Noorderkwartier|Noorderkwartier|regelrecht-corpus-(HHNK|BES)|WS[0-9]{4}'

if grep -rEnI "$PATTERN" "$SKILLS" > /tmp/skills_casus_leak 2>/dev/null; then
    echo "LEAK-GUARD: casus-specifieke inhoud gevonden in dossier-agnostische skills:" >&2
    sed "s#${ROOT}/##" /tmp/skills_casus_leak >&2
    echo "" >&2
    echo "Skills moeten dossier-agnostisch blijven (deze repo is publiek)." >&2
    echo "Verwijder de casus-verwijzing of generaliseer 'm. Bewust uitzonderen?" >&2
    echo "Pas de denylist in script/check-skills-no-casus.sh aan." >&2
    rm -f /tmp/skills_casus_leak
    exit 1
fi
rm -f /tmp/skills_casus_leak
exit 0
