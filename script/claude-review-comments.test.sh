#!/usr/bin/env bash
# Tests voor script/claude-review-comments.sh.
#
# `gh` wordt vervangen door een stub die drie lijst-endpoints kent (reviews,
# inline comments, issue comments) en elke aanroep die iets verandert wegschrijft
# in plaats van hem uit te voeren. Zo is te bewijzen wat er wordt vastgelegd en
# wat er wordt opgeruimd, zonder GitHub.
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SCRIPT="${HERE}/claude-review-comments.sh"

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

mkdir -p "${WORK}/bin"
cat >"${WORK}/bin/gh" <<'STUB'
#!/usr/bin/env bash
# Alles wat geen GET is verandert iets; dat wordt opgetekend, niet uitgevoerd.
if [ "${2:-}" = "-X" ]; then
  echo "$3 $4" >>"$STUB_MUTATIES"
  [ -e "$STUB_DELETE_FAALT" ] && exit 1
  exit 0
fi

url="$2"
case "$url" in
*/pulls/*/reviews*) fixture="$STUB_REVIEWS" ;;
*/pulls/*/comments*) fixture="$STUB_INLINE" ;;
*/issues/*/comments*) fixture="$STUB_STICKY" ;;
*)
  echo "gh: onverwachte URL: $url" >&2
  exit 1
  ;;
esac

body=$(cat "$fixture")
if [ "$body" = "ERROR" ]; then
  echo "gh: simulated API failure" >&2
  exit 1
fi
# `--slurp` bundelt de pagina's tot één array van pagina's; de stub doet één
# pagina, maar wel in dezelfde vorm.
if printf '%s\n' "$@" | grep -qx -- --slurp; then
  printf '%s\n' "$body" | jq -s '.'
else
  printf '%s\n' "$body"
fi
STUB
chmod +x "${WORK}/bin/gh"
export PATH="${WORK}/bin:${PATH}"

export STUB_REVIEWS="${WORK}/reviews"
export STUB_INLINE="${WORK}/inline"
export STUB_STICKY="${WORK}/sticky"
export STUB_MUTATIES="${WORK}/mutaties"
export STUB_DELETE_FAALT="${WORK}/delete-faalt"

export REPO='example-org/example-repo'
export PR=1
export SNAPSHOT_FILE="${WORK}/previous-review.json"
export GITHUB_OUTPUT="${WORK}/output"
export GITHUB_STEP_SUMMARY="${WORK}/summary.md"

# Auteur en ernstmarkering komen uit de poort, niet uit een eigen kopie. Dit
# script heeft ze allebei nog een keer staan — als jq-filter en als
# herschrijfregel — en die moeten dezelfde blijven, anders herkent de een niet
# meer wat de ander leest. De tests hieronder draaien op deze waarden en vallen
# dus om zodra de twee uit elkaar lopen.
GATE="${HERE}/await-claude-review.sh"
BOT=$(sed -n "s/^readonly REVIEW_AUTHOR='\(.*\)'\$/\1/p" "$GATE")
CRITICAL=$(sed -n "s/^readonly CRITICAL_MARKER='\(.*\)'\$/\1/p" "$GATE")
TAG=$(sed -n "s/^readonly REVIEW_TAG='\(.*\)'\$/\1/p" "$SCRIPT")
if [ -z "$BOT" ] || [ -z "$CRITICAL" ] || [ -z "$TAG" ]; then
  echo "FAIL de auteur, de ernstmarkering of de review-markering is niet uit de scripts te lezen"
  exit 1
fi

passed=0
failed=0

ok() {
  echo "ok   $1"
  passed=$((passed + 1))
}
nok() {
  echo "FAIL $1"
  failed=$((failed + 1))
}

# De momentopname van vóór de review: één samenvattende comment, één inline
# comment en één ingediende review, plus tekst van een mens die er niet in hoort.
sticky_toen() {
  cat >"$STUB_STICKY" <<JSON
[{"id":30,"updated_at":"T1","user":{"login":"${BOT}"},"body":"## Correctheid\n\n${CRITICAL} — de samenvatting.\n\n${TAG}"},
 {"id":31,"updated_at":"T1","user":{"login":"iemand"},"body":"een mens"},
 {"id":32,"updated_at":"T1","user":{"login":"${BOT}"},"body":"een antwoord op @claude in de draad"}]
JSON
}
inline_toen() {
  cat >"$STUB_INLINE" <<JSON
[{"id":20,"updated_at":"T1","path":"script/x.sh","line":4,"user":{"login":"${BOT}"},"body":"🟠 **Significant** — hier.\n\n${TAG}"}]
JSON
}
reviews_toen() {
  cat >"$STUB_REVIEWS" <<JSON
[{"id":10,"state":"COMMENTED","submitted_at":"T1","user":{"login":"${BOT}"},"body":"${CRITICAL} — alleen in de body van de review.\n\n${TAG}"},
 {"id":11,"state":"CHANGES_REQUESTED","submitted_at":"T1","user":{"login":"${BOT}"},"body":"${TAG}"},
 {"id":12,"state":"APPROVED","submitted_at":"T1","user":{"login":"iemand"},"body":"prima"}]
JSON
}

reset() {
  sticky_toen
  inline_toen
  reviews_toen
  rm -f "$STUB_DELETE_FAALT" "$SNAPSHOT_FILE"
  : >"$STUB_MUTATIES"
  : >"$GITHUB_OUTPUT"
  : >"$GITHUB_STEP_SUMMARY"
}

context() { sed -n '2,$p' "$GITHUB_OUTPUT" | sed '$d'; }

echo "== de momentopname =="
reset
if "$SCRIPT" snapshot; then
  ok "de momentopname draait"
else
  nok "de momentopname draait niet"
fi

if [ "$(jq -r '[.sticky[].id, .inline[].id, .reviews[].id] | sort | join(",")' "$SNAPSHOT_FILE")" = "10,11,20,30" ]; then
  ok "alleen wat ${BOT} schreef gaat de momentopname in"
else
  nok "de momentopname bevat de verkeerde ids: $(jq -c '[.sticky[].id, .inline[].id, .reviews[].id]' "$SNAPSHOT_FILE")"
fi

if grep -qF 'de samenvatting' "$GITHUB_OUTPUT" &&
  grep -qF 'alleen in de body van de review' "$GITHUB_OUTPUT" &&
  grep -qF 'script/x.sh' "$GITHUB_OUTPUT"; then
  ok "de samenvatting, de review-body en de inline-bevinding gaan als context mee"
else
  nok "de context mist een van de drie plekken waar een bevinding kan staan"
fi

if grep -qF 'een mens' "$GITHUB_OUTPUT"; then
  nok "tekst van iemand anders dan ${BOT} komt in de context terecht"
else
  ok "tekst van een mens blijft buiten de context"
fi

# `claude.yml` laat dezelfde bot in dezelfde draad antwoorden op `@claude`. Dat
# is gesprekstekst, geen review: hij hoort niet als bevinding de volgende prompt
# in en hij hoort niet opgeruimd te worden.
if grep -qF 'antwoord op @claude' "$GITHUB_OUTPUT"; then
  nok "een @claude-antwoord komt als bevinding van de vorige review in de prompt"
else
  ok "een @claude-antwoord van dezelfde bot blijft buiten de context"
fi
if jq -e '[.sticky[].id] | index(32)' "$SNAPSHOT_FILE" >/dev/null; then
  nok "een @claude-antwoord staat in de momentopname en wordt straks weggegooid"
else
  ok "een @claude-antwoord blijft buiten de momentopname"
fi

# De poort leest `🔴 **Critical**` machinaal. Gaat die tekst letterlijk de
# volgende prompt in, dan zet een review die hem aanhaalt om te melden dat de
# bevinding is opgelost de poort alsnog rood.
if grep -qF -- "$CRITICAL" "$GITHUB_OUTPUT"; then
  nok "de ernstmarkering gaat letterlijk de volgende prompt in"
else
  ok "de ernstmarkering is in de context onschadelijk gemaakt"
fi
if grep -qF '[Critical]' "$GITHUB_OUTPUT" && grep -qF '[Significant]' "$GITHUB_OUTPUT"; then
  ok "de ernst blijft in de context wel leesbaar"
else
  nok "de ernst is uit de context verdwenen in plaats van omgeschreven"
fi

if grep -qF 'earlier version of this' "$GITHUB_OUTPUT" &&
  grep -qF 'Drop a finding that the current diff fixes' "$GITHUB_OUTPUT"; then
  ok "de context zegt erbij dat elke bevinding opnieuw getoetst wordt"
else
  nok "de context geeft de bevindingen kaal mee, zonder de opdracht ze te toetsen"
fi

# Niets van de vorige keer: dan hoort er geen contextblok te zijn, en al helemaal
# geen kop die om bevindingen vraagt die er niet zijn.
reset
echo '[]' >"$STUB_STICKY"
echo '[]' >"$STUB_INLINE"
echo '[]' >"$STUB_REVIEWS"
"$SCRIPT" snapshot
if [ -z "$(context)" ]; then
  ok "geen vorige review: geen context in de prompt"
else
  nok "zonder vorige review komt er toch tekst in de prompt: $(context)"
fi

reset
echo ERROR >"$STUB_STICKY"
if "$SCRIPT" snapshot 2>/dev/null; then
  nok "een onophaalbare lijst levert stilzwijgend een lege momentopname op"
else
  ok "een onophaalbare lijst laat de stap vallen in plaats van niets vast te leggen"
fi

echo "== het opruimen =="
# De review draait: de sticky wordt hergebruikt (nieuwe updated_at), er komt een
# nieuwe inline comment bij, en de inline comment van de vorige keer blijft staan.
na_de_review() {
  cat >"$STUB_STICKY" <<JSON
[{"id":30,"updated_at":"T2","user":{"login":"${BOT}"},"body":"nieuw"},
 {"id":31,"updated_at":"T1","user":{"login":"iemand"},"body":"een mens"}]
JSON
  cat >"$STUB_INLINE" <<JSON
[{"id":20,"updated_at":"T1","user":{"login":"${BOT}"},"body":"oud"},
 {"id":21,"updated_at":"T2","user":{"login":"${BOT}"},"body":"nieuw"}]
JSON
}

reset
"$SCRIPT" snapshot
na_de_review
: >"$STUB_MUTATIES"
"$SCRIPT" clean-up

mutaties=$(sort "$STUB_MUTATIES")
verwacht=$(printf '%s\n' \
  "DELETE repos/${REPO}/pulls/comments/20" \
  "PUT repos/${REPO}/pulls/${PR}/reviews/11/dismissals" | sort)
if [ "$mutaties" = "$verwacht" ]; then
  ok "opgeruimd wordt alleen wat onaangeraakt bleef; de hergebruikte comment blijft staan"
else
  nok "het opruimen deed iets anders dan verwacht:"
  printf '     | %s\n' $mutaties
fi

if grep -qF 'reviews/10/dismissals' "$STUB_MUTATIES"; then
  nok "een COMMENTED-review wordt gedismist, en dat geeft 422"
else
  ok "alleen een CHANGES_REQUESTED-review wordt gedismist"
fi

reset
"$SCRIPT" snapshot
na_de_review
: >"$STUB_MUTATIES"
touch "$STUB_DELETE_FAALT"
"$SCRIPT" clean-up
if grep -qF 'niet verwijderd' "$GITHUB_STEP_SUMMARY"; then
  ok "een mislukte verwijdering blijft niet onzichtbaar"
else
  nok "een mislukte verwijdering verdwijnt zonder melding"
fi

# Een API-hik tijdens het opruimen mag de stap niet laten vallen: de job komt dan
# op `failure` en de poort meldt dat er geen bruikbare review is, terwijl die er
# wel degelijk staat.
reset
"$SCRIPT" snapshot
na_de_review
echo ERROR >"$STUB_INLINE"
: >"$STUB_MUTATIES"
if "$SCRIPT" clean-up; then
  ok "een onophaalbare lijst laat het opruimen niet vallen"
else
  nok "een onophaalbare lijst laat de opruimstap falen, en daarmee de hele review-job"
fi
if grep -qF 'misgegaan' "$GITHUB_STEP_SUMMARY"; then
  ok "en meldt dat er iets is blijven staan"
else
  nok "en meldt niet dat er iets is blijven staan"
fi
if grep -qF 'pulls/comments/20' "$STUB_MUTATIES"; then
  nok "er wordt verwijderd op grond van een lijst die niet te lezen was"
else
  ok "wat niet te lezen was, wordt niet op goed geluk verwijderd"
fi

# Een onleesbare momentopname mag geen lege lus opleveren: dan lijkt "niets
# opgeruimd" op "alles opgeruimd".
reset
"$SCRIPT" snapshot
na_de_review
echo 'geen json' >"$SNAPSHOT_FILE"
: >"$STUB_MUTATIES"
"$SCRIPT" clean-up
if grep -qF 'misgegaan' "$GITHUB_STEP_SUMMARY"; then
  ok "een onleesbare momentopname wordt gemeld in plaats van als leeg gelezen"
else
  nok "een onleesbare momentopname levert stil nul op te ruimen comments op"
fi

reset
if "$SCRIPT" clean-up 2>/dev/null; then
  nok "opruimen zonder momentopname doet maar wat"
else
  ok "opruimen zonder momentopname stopt"
fi

reset
if "$SCRIPT" iets-anders 2>/dev/null; then
  nok "een onbekend subcommando doet iets"
else
  ok "een onbekend subcommando stopt"
fi

# De workflow roept dit script aan; de namen van de subcommando's moeten kloppen.
WORKFLOW="$(cd "${HERE}/.." && pwd)/.github/workflows/claude-code-review.yml"
if grep -qF 'script/claude-review-comments.sh snapshot' "$WORKFLOW" &&
  grep -qF 'script/claude-review-comments.sh clean-up' "$WORKFLOW"; then
  ok "de workflow roept beide subcommando's aan onder de naam die het script kent"
else
  nok "de workflow roept dit script niet (meer) aan zoals het heet"
fi

# De markering waaraan een review-comment te herkennen is, staat in de prompt en
# in dit script. Lopen die uit elkaar, dan herkent het script niets meer en
# verdwijnt de context stil.
if grep -qF -- "$TAG" "$WORKFLOW"; then
  ok "de prompt schrijft dezelfde markering voor als het script zoekt"
else
  nok "de prompt in ${WORKFLOW} kent de markering ${TAG} niet"
fi

echo
echo "${passed} geslaagd, ${failed} gefaald"
[ "$failed" -eq 0 ]
