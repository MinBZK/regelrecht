#!/usr/bin/env bash
# Tests voor script/check-skill-references.sh.
#
# Elke test bouwt een minimale repo in een tijdelijke map (SKILL_REF_ROOT) en
# toetst één beslissing: groen of rood, en waarom. Zonder de poort zelf slagen
# de rood-gevallen niet.
set -uo pipefail

GUARD="$(cd "$(dirname "$0")" && pwd)/check-skill-references.sh"
failures=0

fixture() {
    local root="$1" latest="${2:-v0.5.6}"
    mkdir -p "$root/schema/$latest" "$root/.claude/skills/demo" "$root/script"
    ln -sfn "$latest" "$root/schema/latest"
    : > "$root/script/.skill-path-exceptions"
}

expect() {
    local want="$1" name="$2" root="$3"
    local out
    out="$(SKILL_REF_ROOT="$root" bash "$GUARD" 2>&1)"
    local got=$?
    if [ "$got" != "$want" ]; then
        echo "FAIL: $name (exit $got, verwacht $want)"
        echo "$out" | sed 's/^/      /'
        failures=$((failures + 1))
    else
        echo "ok: $name"
    fi
}

# --- 1. Een pad dat bestaat is groen ---------------------------------------
root="$(mktemp -d)"
fixture "$root"
mkdir -p "$root/packages/engine/src"
: > "$root/packages/engine/src/service.rs"
echo 'Lees `packages/engine/src/service.rs` eerst.' > "$root/.claude/skills/demo/SKILL.md"
expect 0 "bestaand pad" "$root"
rm -rf "$root"

# --- 2. Een dood pad is rood ------------------------------------------------
root="$(mktemp -d)"
fixture "$root"
echo 'Lees `packages/engine/tests/bdd/steps/given.rs` eerst.' > "$root/.claude/skills/demo/SKILL.md"
expect 1 "dood pad" "$root"
rm -rf "$root"

# --- 3. Een dood pad met reden in de uitzonderingenlijst is groen -----------
root="$(mktemp -d)"
fixture "$root"
echo 'Schrijf hem naar `corpus/regulation/nl/wet/x/.enrichment-result.yaml`.' > "$root/.claude/skills/demo/SKILL.md"
echo 'corpus/regulation/nl/wet/x/.enrichment-result.yaml  # pipeline-uitvoer' > "$root/script/.skill-path-exceptions"
expect 0 "uitzondering met reden" "$root"
rm -rf "$root"

# --- 4. Een uitzondering voor een pad dat inmiddels bestaat is rood ---------
root="$(mktemp -d)"
fixture "$root"
mkdir -p "$root/docs"
: > "$root/docs/bestaat.md"
echo 'Zie `docs/bestaat.md`.' > "$root/.claude/skills/demo/SKILL.md"
echo 'docs/bestaat.md' > "$root/script/.skill-path-exceptions"
expect 1 "verouderde uitzondering (pad bestaat)" "$root"
rm -rf "$root"

# --- 5. Een uitzondering die geen skill meer noemt is rood -----------------
root="$(mktemp -d)"
fixture "$root"
echo 'Niets bijzonders.' > "$root/.claude/skills/demo/SKILL.md"
echo 'docs/niemand-noemt-mij.md' > "$root/script/.skill-path-exceptions"
expect 1 "verouderde uitzondering (ongenoemd)" "$root"
rm -rf "$root"

# --- 6. Sjabloon- en glob-vormen tellen niet als pad ------------------------
root="$(mktemp -d)"
fixture "$root"
cat > "$root/.claude/skills/demo/SKILL.md" <<'MD'
Zet hem in `schema/vX.Y.Z/schema.json`, of ergens onder `corpus/regulation/nl/...`.
Alles onder `packages/**/tests` telt mee.
MD
expect 0 "sjablonen en globs" "$root"
rm -rf "$root"

# --- 6b. Een dood pad in een skill-script telt ook mee ---------------------
root="$(mktemp -d)"
fixture "$root"
echo 'Niets bijzonders.' > "$root/.claude/skills/demo/SKILL.md"
printf '# schrijf naar `packages/engine/weg.rs`\n' > "$root/.claude/skills/demo/tool.py"
expect 1 "dood pad in een .py-skillscript" "$root"
rm -rf "$root"

# --- 6c. Een uitzondering met tab-inspringing telt ook ---------------------
root="$(mktemp -d)"
fixture "$root"
echo 'Schrijf hem naar `corpus/regulation/nl/wet/x/.enrichment-result.yaml`.' > "$root/.claude/skills/demo/SKILL.md"
printf '\tcorpus/regulation/nl/wet/x/.enrichment-result.yaml\t# uitvoer\n' > "$root/script/.skill-path-exceptions"
expect 0 "uitzondering met tabs" "$root"
rm -rf "$root"

# --- 6d. Zonder symlink valt de poort terug op de hoogste versiemap --------
root="$(mktemp -d)"
fixture "$root"
rm -f "$root/schema/latest"
mkdir -p "$root/schema/v0.5.4"
echo 'Gebruik https://raw.githubusercontent.com/MinBZK/regelrecht/refs/tags/schema-v0.5.6/schema/v0.5.6/schema.json' \
    > "$root/.claude/skills/demo/SKILL.md"
expect 0 "terugval op de hoogste versiemap" "$root"
rm -rf "$root"

# --- 7. Een muteerbare $schema-URL is rood ---------------------------------
root="$(mktemp -d)"
fixture "$root"
echo 'Gebruik https://raw.githubusercontent.com/MinBZK/regelrecht/refs/heads/main/schema/v0.5.6/schema.json' \
    > "$root/.claude/skills/demo/SKILL.md"
expect 1 "refs/heads/main-URL" "$root"
rm -rf "$root"

# --- 8. Een tag-URL op een oudere schemaversie is rood ---------------------
root="$(mktemp -d)"
fixture "$root"
echo 'Gebruik https://raw.githubusercontent.com/MinBZK/regelrecht/refs/tags/schema-v0.5.0/schema/v0.5.0/schema.json' \
    > "$root/.claude/skills/demo/SKILL.md"
expect 1 "tag-URL op oude versie" "$root"
rm -rf "$root"

# --- 9. De geldende tag-URL is groen ---------------------------------------
root="$(mktemp -d)"
fixture "$root"
echo 'Gebruik https://raw.githubusercontent.com/MinBZK/regelrecht/refs/tags/schema-v0.5.6/schema/v0.5.6/schema.json' \
    > "$root/.claude/skills/demo/SKILL.md"
expect 0 "geldende tag-URL" "$root"
rm -rf "$root"

# --- 10. De poort volgt schema/latest mee ----------------------------------
root="$(mktemp -d)"
fixture "$root" v0.6.0
echo 'Gebruik https://raw.githubusercontent.com/MinBZK/regelrecht/refs/tags/schema-v0.5.6/schema/v0.5.6/schema.json' \
    > "$root/.claude/skills/demo/SKILL.md"
expect 1 "URL loopt achter op schema/latest" "$root"
rm -rf "$root"

if [ "$failures" -gt 0 ]; then
    echo "$failures test(s) gefaald"
    exit 1
fi
echo "alle tests geslaagd"
