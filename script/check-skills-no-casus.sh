#!/usr/bin/env bash
# Leak-guard voor de dossier-agnostische skills.
#
# De skills onder .claude/skills/ beschrijven de regelrecht-METHODE en moeten
# dossier-agnostisch blijven. Deze repo is publiek; casus-specifieke inhoud mag
# hier nooit in belanden: de naam van een specifieke overheidsorganisatie
# (gemeente, provincie, waterschap, ministerie, uitvoeringsorganisatie, …), een
# persoonsidentificerend nummer met echte waarde (BSN, KvK-nummer, A-nummer,
# zaaknummer, …), een private repo-naam, of een trace-fragment met echte data.
#
# Twee lagen (hybride), zodat de guard geen gevoelige inhoud zélf hoeft te
# publiceren:
#   1. PUBLIEK (dit bestand): domein-loze structuur-vormen die elk dossier vangen
#      zonder er één te noemen. Draait ALTIJD, ook zonder aanvulbestand of secret
#      — daarom kan een groene uitkomst nooit een lege regelset verbergen
#      (fail-closed).
#   2. LOKAAL / CI-SECRET (optioneel): casus-/sector-specifieke patronen in een
#      git-ignored aanvulbestand; in CI via een secret naar datzelfde pad
#      geschreven. Die patronen staan NOOIT in deze publieke repo.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SKILLS="$ROOT/.claude/skills"
[ -d "$SKILLS" ] || exit 0

# Laag 1 — publieke, domein-loze structuur-vormen (geen concrete naam/suffix).
# Eén extended-regex per regel. Bewust géén sector-woord of dossiernaam.
#   - private corpus-repo-naamvorm: vaste project-prefix + willekeurig suffix
#     (vangt elk huidig én toekomstig corpus-dossier zonder er één te noemen)
PUBLIC_PATTERNS=(
  'regelrecht-corpus-[A-Z][A-Za-z0-9-]*'
)

# Laag 2 — optioneel git-ignored aanvulbestand (één extended-regex per regel,
# '#'-commentaar en lege regels toegestaan). In CI via een secret naar dit pad
# geschreven. Ontbreekt het → alleen laag 1 draait; veilig, want laag 1 is nooit
# leeg. De concrete casus-/sector-patronen horen HIER, niet in dit publieke script.
SUPPLEMENT="${SKILLS_CASUS_DENYLIST_FILE:-$ROOT/script/.skills-casus-denylist.local}"

PATTERNS_FILE="$(mktemp)"
HITS_FILE="$(mktemp)"
trap 'rm -f "$PATTERNS_FILE" "$HITS_FILE"' EXIT

printf '%s\n' "${PUBLIC_PATTERNS[@]}" > "$PATTERNS_FILE"
if [ -f "$SUPPLEMENT" ]; then
    grep -vE '^[[:space:]]*(#|$)' "$SUPPLEMENT" >> "$PATTERNS_FILE" || true
fi

if grep -rEnIf "$PATTERNS_FILE" "$SKILLS" > "$HITS_FILE" 2>/dev/null; then
    echo "LEAK-GUARD: casus-specifieke inhoud gevonden in dossier-agnostische skills:" >&2
    sed "s#${ROOT}/##" "$HITS_FILE" >&2
    echo "" >&2
    echo "Skills moeten dossier-agnostisch blijven (deze repo is publiek)." >&2
    echo "Verwijder de verwijzing of generaliseer 'm. Een bewust casus-/sector-" >&2
    echo "patroon hoort in het git-ignored aanvulbestand, niet in dit publieke script." >&2
    exit 1
fi
exit 0
