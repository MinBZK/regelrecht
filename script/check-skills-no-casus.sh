#!/usr/bin/env bash
# Leak-guard voor de dossier-agnostische methode-teksten.
#
# De skills onder .claude/skills/ en de site-inhoud onder docs/src/content/
# beschrijven de regelrecht-METHODE en moeten dossier-agnostisch blijven. Deze
# repo is publiek; casus-specifieke inhoud mag hier nooit in belanden: de naam
# van een specifieke overheidsorganisatie (gemeente, provincie, waterschap,
# ministerie, uitvoeringsorganisatie, …), een persoonsidentificerend nummer met
# echte waarde (BSN, KvK-nummer, A-nummer, zaaknummer, …), een private
# repo-naam, of een trace-fragment met echte data.
#
# Sinds 2026-09-08 ook: sporen van de máchine waarop geschreven is. Een absoluut
# pad lekt een gebruikersnaam en een mappenstructuur ook wanneer de inhoud
# verder volledig dossier-agnostisch is, en het valt bij het lezen niet op omdat
# het op een gewone verwijzing lijkt.
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

# Bewaakte paden. Bewust NIET de hele repo: de laag-2-patronen zijn sector- en
# casuswoorden, en die komen in het publieke corpus volkomen legitiem voor — een
# wet óver waterschappen is publiek recht. Repo-breed scannen zou dus vals alarm
# geven op wettekst, en een guard die vals alarm geeft wordt uitgezet. Bewaakt
# wordt alleen wat wij zelf als methode-tekst schrijven.
GUARDED_RELPATHS=(
  ".claude/skills"
  "docs/src/content"
)

GUARDED=()
for rel in "${GUARDED_RELPATHS[@]}"; do
    [ -d "$ROOT/$rel" ] && GUARDED+=("$ROOT/$rel")
done
[ ${#GUARDED[@]} -gt 0 ] || exit 0

# Laag 1 — publieke, domein-loze structuur-vormen (geen concrete naam/suffix).
# Eén extended-regex per regel. Bewust géén sector-woord of dossiernaam.
#   - private corpus-repo-naamvorm: vaste project-prefix + willekeurig suffix
#     (vangt elk huidig én toekomstig corpus-dossier zonder er één te noemen)
#   - absolute home-paden, Windows-paden en file-URL's: lekken de gebruikersnaam
#     en de mappenstructuur van de schrijver
#   - agent-scratchpads en sessielinks: lekken waar en wanneer er gewerkt is
#
# NB bewust NIET in deze lijst: `~/.claude/`. Dat komt legitiem voor — skills
# instrueren de lezer een hook in zijn éigen `~/.claude/` te installeren. En
# bewust géén kale gebruikersnaam: het woord alleen geeft vals alarm op
# wettekst. Een persoonsnaam hoort aan zijn padvorm te hangen, in laag 2.
PUBLIC_PATTERNS=(
  'regelrecht-corpus-[A-Z][A-Za-z0-9-]*'
  '/(Users|home)/[A-Za-z0-9._-]+/'
  '[A-Za-z]:\\Users\\'
  'file:///'
  '/(private/)?tmp/claude-[0-9]+'
  'claude\.ai/code/session_'
)

# Laag 2 — optioneel git-ignored aanvulbestand (één extended-regex per regel,
# '#'-commentaar en lege regels toegestaan). In CI via een secret naar dit pad
# geschreven. Ontbreekt het → alleen laag 1 draait; veilig, want laag 1 is nooit
# leeg. De concrete casus-/sector-patronen horen HIER, niet in dit publieke
# script. Denk daarbij ook aan verwijzingen die er onschuldig uitzien omdat ze
# relatief zijn: een pad naar een map die alleen in een private repo bestaat
# heeft geen `/Users/` ervoor en is met een domein-loos patroon niet te vangen.
SUPPLEMENT="${SKILLS_CASUS_DENYLIST_FILE:-$ROOT/script/.skills-casus-denylist.local}"

PATTERNS_FILE="$(mktemp)"
HITS_FILE="$(mktemp)"
trap 'rm -f "$PATTERNS_FILE" "$HITS_FILE"' EXIT

printf '%s\n' "${PUBLIC_PATTERNS[@]}" > "$PATTERNS_FILE"
if [ -f "$SUPPLEMENT" ]; then
    grep -vE '^[[:space:]]*(#|$)' "$SUPPLEMENT" >> "$PATTERNS_FILE" || true
fi

if grep -rEnIf "$PATTERNS_FILE" "${GUARDED[@]}" > "$HITS_FILE" 2>/dev/null; then
    echo "LEAK-GUARD: casus- of machine-specifieke inhoud gevonden in dossier-agnostische methode-teksten:" >&2
    sed "s#${ROOT}/##" "$HITS_FILE" >&2
    echo "" >&2
    echo "Skills en site-inhoud moeten dossier-agnostisch blijven (deze repo is publiek)." >&2
    echo "Verwijder de verwijzing of generaliseer 'm — maak een pad relatief, of baseer" >&2
    echo "het voorbeeld op het publieke corpus. Een bewust casus-/sector-patroon hoort" >&2
    echo "in het git-ignored aanvulbestand, niet in dit publieke script." >&2
    exit 1
fi
exit 0
