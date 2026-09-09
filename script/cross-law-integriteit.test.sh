#!/usr/bin/env bash
# Dekt de toestandskeuze van script/cross-law-integriteit.py.
#
# Waarom juist dit: een wet met meerdere `valid_from`-toestanden deelt één `$id`.
# De loader hield eerder wat glob toevallig als laatste opleverde, dus de controle
# las een willekeurige tekst. Een binding op een term die pas in een nieuwere
# toestand bestaat werd dan als dangling gemeld, terwijl een echte fout in de
# oudere tekst juist onzichtbaar bleef. Beide richtingen worden hier vastgepind.
set -uo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
gate="$here/cross-law-integriteit.py"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

pass=0
fail=0

# $1 = map, $2 = valid_from, $3 = declareert de open term? (ja/nee)
doelwet() {
    mkdir -p "$1"
    cat >"$1/$2.yaml" <<YAML
\$id: doelwet
valid_from: '$2'
articles:
  - number: '1'
    machine_readable:
      execution:
        output:
          - name: uitkomst
YAML
    if [ "$3" = "ja" ]; then
        cat >>"$1/$2.yaml" <<'YAML'
      open_terms:
        - id: open_term_nieuw
          type: amount
YAML
    fi
}

# Een lagere regeling die zich op die open term aanhaakt.
invuller() {
    mkdir -p "$1"
    cat >"$1/2026-01-01.yaml" <<'YAML'
$id: invuller
valid_from: '2026-01-01'
articles:
  - number: '1'
    machine_readable:
      implements:
        - law: doelwet
          article: '1'
          open_term: open_term_nieuw
      execution:
        output:
          - name: invulling
YAML
}

check() { # naam, verwachte exitcode, verwacht patroon in de uitvoer, corpus, [extra args]
    local naam="$1" want_code="$2" want_uit="$3" corpus="$4"
    shift 4
    local uit code
    uit="$(python3 "$gate" "$corpus" "$@" 2>&1)"
    code=$?
    if [ "$code" -eq "$want_code" ] && printf '%s' "$uit" | grep -q "$want_uit"; then
        pass=$((pass + 1))
        printf '  ok   %s\n' "$naam"
    else
        fail=$((fail + 1))
        printf '  FOUT %s — exit %s (verwacht %s), zocht "%s"\n' "$naam" "$code" "$want_code" "$want_uit"
        printf '%s\n' "$uit" | sed 's/^/       /'
    fi
}

echo "cross-law toestandskeuze:"

# 1. Twee toestanden; alleen de nieuwe declareert de open term. Op een peildatum
#    ná die toestand hoort de binding te kloppen — dit is het geval dat vroeger
#    zes valse meldingen opleverde.
c="$tmp/twee"
doelwet "$c/doelwet" 2020-01-01 nee
doelwet "$c/doelwet" 2026-01-01 ja
invuller "$c/invuller"
check "nieuwe toestand geldt → binding klopt" 0 "impl-dangling=0" "$c" --peildatum 2026-06-01

# 2. Dezelfde boom, maar op een peildatum vóór de nieuwe toestand. Dan bestond de
#    open term nog niet en is de melding terecht — de fix mag niet alles groen maken.
check "oude toestand geldt → binding is terecht dangling" 1 "impl-dangling=1" "$c" --peildatum 2021-01-01

# 3. De gekozen toestand hoort zichtbaar te zijn. Een stille keuze is wat de
#    oude loader gevaarlijk maakte.
check "keuze wordt gerapporteerd" 0 "2026-01-01.yaml (uit 2 toestanden)" "$c" --peildatum 2026-06-01

# 4. Alles in de toekomst → de vroegste toestand, zodat een volledig
#    vooruitgedateerd corpus niet ongecontroleerd doorglipt.
check "alles toekomstig → vroegste toestand" 1 "impl-dangling=1" "$c" --peildatum 2010-01-01

# 5. Een wet zonder valid_from blijft laadbaar (geldt altijd).
c2="$tmp/geendatum"
mkdir -p "$c2/doelwet"
cat >"$c2/doelwet/wet.yaml" <<'YAML'
$id: doelwet
articles:
  - number: '1'
    machine_readable:
      open_terms:
        - id: open_term_nieuw
          type: amount
      execution:
        output:
          - name: uitkomst
YAML
invuller "$c2/invuller"
check "wet zonder valid_from blijft laadbaar" 0 "impl-dangling=0" "$c2"

# 6. Zonder --peildatum draait hij op vandaag en blijft hij bruikbaar.
check "zonder --peildatum: vandaag" 0 "impl-dangling=0" "$c"

echo ""
echo "$pass geslaagd, $fail mislukt"
[ "$fail" -eq 0 ]
