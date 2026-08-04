#!/usr/bin/env bash
# Proves the gates catch what they claim to catch.
#
# For each gate: it must pass on statements.clean.yaml, and it must fail on
# statements.broken.yaml *for its own reason*. The second half matters - a gate
# that fails for the wrong reason would still look green in a plain exit-code
# test, and the whole point of these gates is that a specific failure mode is
# impossible to pass over.
set -uo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
GATES="$HERE/../scripts/statement_gates.py"
FIX="$HERE/fixture"
CANON="$FIX/canonical.md"
fails=0

expect_pass() {  # gate
    local gate="$1" out rc
    out="$(python3 "$GATES" "$gate" --canonical "$CANON" --ledger "$FIX/statements.clean.yaml" 2>&1)"
    rc=$?
    if [[ $rc -ne 0 ]]; then
        echo "FAIL  $gate hoort te slagen op de schone ledger"
        echo "$out" | sed 's/^/      /'
        fails=$((fails + 1))
    else
        echo "ok    $gate  clean   -> $(grep -i -m1 "$gate" <<<"$out" || echo "$out" | head -1)"
    fi
}

expect_fail() {  # gate, marker
    local gate="$1" marker="$2" out rc
    out="$(python3 "$GATES" "$gate" --canonical "$CANON" --ledger "$FIX/statements.broken.yaml" 2>&1)"
    rc=$?
    if [[ $rc -eq 0 ]]; then
        echo "FAIL  $gate hoort te falen op de kapotte ledger"
        fails=$((fails + 1))
    elif ! grep -qi -- "$marker" <<<"$out"; then
        echo "FAIL  $gate faalt, maar niet op '$marker'"
        echo "$out" | sed 's/^/      /'
        fails=$((fails + 1))
    else
        echo "ok    $gate  broken  -> $(grep -i -m1 -- "$marker" <<<"$out" | sed 's/^ *//')"
    fi
}

echo "== schone ledger: alle gates slagen =="
for g in ledger verbatim coverage anchor signaalnet; do expect_pass "$g"; done

echo
echo "== kapotte ledger: elke gate pakt zijn eigen defect =="
expect_fail verbatim   "niet verbatim"                    # A: geparafraseerd citaat
expect_fail verbatim   "zonder search_terms"              # E: negatieve bevinding niet overdoenbaar
expect_fail coverage   "GAP aan het eind"                 # B: colofon-segment ontbreekt
expect_fail anchor     "AMBIGUOUS"                        # C: anker zonder context
expect_fail signaalnet "Indien het verzoek te laat"       # D: normzin stil overgeslagen
expect_fail ledger     "staat niet in het vocabulaire"    # F: vocabulaire-afwijking

# De ledger-gate draait bij élke aanroep, ook bij een losse gate. Anders zou een
# ledger met een typefout in het vocabulaire drie groene gates opleveren terwijl
# de regel die had moeten vuren nergens naar keek.
echo
echo "== ledger-gate draait ook bij een losse gate-aanroep =="
# Let op: geen pipe naar grep hier - met `pipefail` zou de exit-code van de gate
# (1, want er zijn bevindingen) de uitkomst van de grep overschrijven.
solo_out="$(python3 "$GATES" anchor --canonical "$CANON" \
            --ledger "$FIX/statements.broken.yaml" 2>&1)"
if grep -q "LEDGER" <<<"$solo_out"; then
    echo "ok    ledger-gate meldt zich ook bij 'anchor'"
else
    echo "FAIL  ledger-gate ontbreekt bij een losse gate-aanroep"
    fails=$((fails + 1))
fi

echo
if [[ $fails -eq 0 ]]; then
    echo "ALLE TESTS OK"
    exit 0
fi
echo "$fails TEST(S) GEFAALD"
exit 1
