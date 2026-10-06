#!/usr/bin/env bash
# Dekt elk pad dat groen of rood beslist in script/check-skills-no-casus.sh.
#
# Waarom deze test er is: een leak-guard die niets vangt is niet te onderscheiden
# van een die werkt — beide zijn groen. Alleen een negatieve test bewijst dat de
# patronen werkelijk dekken wat ze beloven. En omdat laag 2 (het git-ignored
# aanvulbestand) op de meeste machines ontbreekt, is een groene lokale run
# zonder deze test helemaal geen uitspraak.
#
# Elke casus draait tegen een eigen nep-boom in $TMPDIR, zodat de test nooit
# leunt op de echte repo-inhoud én nooit een aanvulbestand van de ontwikkelaar
# meepakt (ROOT is de nep-boom, dus het default-pad wijst daar naartoe).
set -uo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
gate="$here/check-skills-no-casus.sh"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

pass=0
fail=0

mk_root() { # $1 = naam → echoot het pad van een verse nep-boom
    local r="$tmp/$1"
    mkdir -p "$r/script" "$r/.claude/skills" "$r/docs/src/content" "$r/corpus"
    cp "$gate" "$r/script/check-skills-no-casus.sh"
    printf 'Een gewone methode-tekst zonder sporen.\n' > "$r/.claude/skills/SKILL.md"
    printf 'Een gewone conceptpagina.\n' > "$r/docs/src/content/page.md"
    echo "$r"
}

check() { # $1 = naam, $2 = verwachte exitcode, $3 = root, $4 = optioneel aanvulbestand
    local naam="$1" want="$2" root="$3" supp="${4:-}"
    local out st
    if [ -n "$supp" ]; then
        out="$(SKILLS_CASUS_DENYLIST_FILE="$supp" bash "$root/script/check-skills-no-casus.sh" 2>&1)"
    else
        out="$(bash "$root/script/check-skills-no-casus.sh" 2>&1)"
    fi
    st=$?
    if [ "$st" -eq "$want" ]; then
        pass=$((pass + 1))
        printf '  ok   %s\n' "$naam"
    else
        fail=$((fail + 1))
        printf '  FOUT %s — exit %s, verwacht %s\n' "$naam" "$st" "$want"
        [ -n "$out" ] && printf '       %s\n' "$out"
    fi
}

echo "leak-guard:"

# 1. Schone boom laat door.
r="$(mk_root schoon)"
check "schone boom → groen" 0 "$r"

# 2-4. Machine-sporen in een bewaakt pad worden gevangen.
r="$(mk_root homepad)"
printf 'zie /Users/iemand/projecten/x voor het voorbeeld\n' >> "$r/.claude/skills/SKILL.md"
check "absoluut home-pad in een skill → rood" 1 "$r"

r="$(mk_root winpad)"
printf 'zie C:\\Users\\iemand\\x\n' >> "$r/docs/src/content/page.md"
check "windows-pad in site-inhoud → rood" 1 "$r"

r="$(mk_root sessie)"
printf 'zie https://claude.ai/code/session_ABC123\n' >> "$r/docs/src/content/page.md"
check "sessielink → rood" 1 "$r"

r="$(mk_root scratch)"
printf 'geschreven in /private/tmp/claude-000/werkmap\n' >> "$r/.claude/skills/SKILL.md"
check "agent-scratchpad → rood" 1 "$r"

# 5. De private corpus-repo-vorm (het oorspronkelijke patroon) blijft werken.
r="$(mk_root corpusnaam)"
printf 'kloon regelrecht-corpus-Voorbeeld ernaast\n' >> "$r/.claude/skills/SKILL.md"
check "private corpus-repo-naam → rood" 1 "$r"

# 6. Site-inhoud is nieuw in de scope; zonder deze regel gleed een RFC er langs.
r="$(mk_root rfc)"
mkdir -p "$r/docs/src/content/rfcs"
printf 'pad: /home/iemand/repo\n' > "$r/docs/src/content/rfcs/rfc-999.md"
check "machine-pad in een RFC → rood" 1 "$r"

# 7. Buiten de bewaakte paden blijft het stil — anders geeft de guard vals alarm
#    op publieke wettekst en zet iemand hem uit.
r="$(mk_root buiten_scope)"
printf 'pad: /Users/iemand/x\n' > "$r/corpus/wet.yaml"
printf 'pad: /Users/iemand/x\n' > "$r/README.md"
check "zelfde spoor buiten de bewaakte paden → groen" 0 "$r"

# 8. `~/.claude/` is legitiem: skills instrueren de lezer een hook in zijn eigen
#    map te installeren. Vangen we dat, dan breekt bestaande inhoud.
r="$(mk_root tilde)"
printf 'installeer de hook in ~/.claude/hooks/\n' >> "$r/.claude/skills/SKILL.md"
check "~/.claude/ in een skill → groen" 0 "$r"

# 9. Laag 2 doet mee als het aanvulbestand er is — en alleen dan.
r="$(mk_root laag2)"
printf 'de Voorbeeldschap-regeling\n' >> "$r/.claude/skills/SKILL.md"
check "casus-woord zonder aanvulbestand → groen (alleen laag 1)" 0 "$r"
printf '# commentaar wordt genegeerd\n\nVoorbeeldschap\n' > "$tmp/denylist"
check "casus-woord mét aanvulbestand → rood" 1 "$r" "$tmp/denylist"

# 10. Geen bewaakte paden aanwezig (deelcheckout) → geen uitspraak, geen fout.
r="$tmp/leeg"
mkdir -p "$r/script"
cp "$gate" "$r/script/check-skills-no-casus.sh"
check "geen bewaakte paden → groen" 0 "$r"

echo ""
echo "$pass geslaagd, $fail mislukt"
[ "$fail" -eq 0 ]
