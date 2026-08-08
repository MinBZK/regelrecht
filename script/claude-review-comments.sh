#!/usr/bin/env bash
# `snapshot` vóór de review, `clean-up` erna. Ruimde je vóór de review op, dan
# nam een run die daarna klapte de vorige bevinding mee, en was opnieuw pushen
# genoeg om langs de poort te komen.
#
# clean-up vergelijkt `updated_at`: `use_sticky_comment` hergebruikt de comment
# uit de momentopname, dus op id alleen wissen zou de zojuist geschreven review
# weggooien.
set -uo pipefail

: "${REPO:?REPO is verplicht}"
: "${PR:?PR is verplicht}"

SNAPSHOT_FILE="${SNAPSHOT_FILE:-${RUNNER_TEMP:-/tmp}/previous-review.json}"
GITHUB_OUTPUT="${GITHUB_OUTPUT:-/dev/null}"
GITHUB_STEP_SUMMARY="${GITHUB_STEP_SUMMARY:-/dev/null}"

readonly REVIEW_AUTHOR='claude[bot]'
# Op auteur alleen selecteren is niet genoeg: `claude.yml` antwoordt als dezelfde
# bot op `@claude`, en dat is geen bevinding. De prompt schrijft deze regel voor.
readonly REVIEW_TAG='<!-- claude-review -->'

fail() {
  echo "::error title=Claude review comments::$1" >&2
  exit 1
}

# `--slurp` geeft één array per pagina; vandaar `.[][]` bij de lezers hieronder.
list() { gh api "repos/${REPO}/${1}?per_page=100" --paginate --slurp; }

snapshot() {
  local reviews inline sticky context delimiter
  local mine
  mine=$(printf 'select(.user.login == "%s") | select((.body // "") | contains("%s"))' \
    "$REVIEW_AUTHOR" "$REVIEW_TAG")

  # De body van een review draagt bevindingen die nergens anders staan.
  reviews=$(list "pulls/${PR}/reviews" |
    jq "[.[][] | ${mine} | {id, state, submitted_at, body}]") ||
    fail "De reviews van pull request ${PR} zijn niet op te halen of niet te lezen. Zonder die lijst legt de momentopname niet vast wat de vorige review achterliet, en dan gaat die bevinding bij het opruimen verloren."
  inline=$(list "pulls/${PR}/comments" |
    jq "[.[][] | ${mine} | {id, updated_at, path, line: (.line // .original_line), body}]") ||
    fail "De inline comments van pull request ${PR} zijn niet op te halen of niet te lezen. Zie hierboven."
  sticky=$(list "issues/${PR}/comments" |
    jq "[.[][] | ${mine} | {id, updated_at, body}]") ||
    fail "De samenvattende comments van pull request ${PR} zijn niet op te halen of niet te lezen. Zie hierboven."

  jq -n --argjson r "$reviews" --argjson i "$inline" --argjson s "$sticky" \
    '{reviews: $r, inline: $i, sticky: $s}' >"$SNAPSHOT_FILE" ||
    fail "De momentopname is niet naar ${SNAPSHOT_FILE} te schrijven."

  # De poort leest `🔴 **Critical**` machinaal, dus een review die een oude
  # bevinding aanhaalt zou hem daarmee opnieuw rood zetten.
  context=$(jq -r '
    def leesbaar:
      gsub("🔴 \\*\\*Critical\\*\\*"; "[Critical]")
      | gsub("🟠 \\*\\*Significant\\*\\*"; "[Significant]")
      | gsub("🟡 \\*\\*Minor\\*\\*"; "[Minor]");
    [ (.sticky[] | "### Previous summary review\n\n" + (.body | leesbaar)),
      (.reviews[] | select((.body // "") != "") | "### Previous review body\n\n" + (.body | leesbaar)),
      (.inline[] | "### Previous inline finding on `\(.path)` line \(.line // "unknown")\n\n" + (.body | leesbaar))
    ] | join("\n\n")' "$SNAPSHOT_FILE") ||
    fail "De teksten uit de momentopname zijn niet te lezen."

  if [ -n "$context" ]; then
    context=$(
      cat <<PREAMBLE

## Findings from the previous review of this pull request

These findings were written by an earlier review of an **earlier version of this
diff**, and that version is not the one in front of you. They are context, not
conclusions.

Re-check every one of them against the current diff before you write anything:

- Repeat a finding only if it still holds against the code as it stands now.
- Drop a finding that the current diff fixes. Do not carry it over because it
  was reported before.
- Do not let a finding go just because it was reported before either. One that
  still stands belongs in this review too.

Severity markers in the text below are written as \`[Critical]\`, \`[Significant]\`
and \`[Minor]\`. Use the markers from the output format above for your own
findings, and only for findings you are reporting yourself.

$context
PREAMBLE
    )
  fi

  # Komt de scheider in de tekst voor, dan schrijft de rest zich als eigen
  # sleutels het stapbestand in.
  delimiter="PREVIOUS_REVIEW_${RANDOM}${RANDOM}${RANDOM}"
  if grep -qF "$delimiter" <<<"$context"; then
    fail "De scheider ${delimiter} komt voor in de tekst van de vorige review."
  fi
  {
    printf 'context<<%s\n' "$delimiter"
    printf '%s\n' "$context"
    printf '%s\n' "$delimiter"
  } >>"$GITHUB_OUTPUT"
}

# Een mislukte verwijdering laat oude tekst staan. Dat is de goede kant om naar
# te falen, maar het moet wel zichtbaar zijn.
mislukt=0
try() {
  gh api "$@" >/dev/null 2>&1 || mislukt=$((mislukt + 1))
}

clean_up() {
  local current key id ids
  [ -r "$SNAPSHOT_FILE" ] ||
    fail "De momentopname ${SNAPSHOT_FILE} ontbreekt, dus er valt niet vast te stellen wat van de vorige review was."

  # Eerst binnenhalen, dan pas doorlopen: in `while read < <(jq ...)` valt een
  # gevallen jq weg als een lege lus.
  if ids=$(jq -r '.reviews[] | select(.state == "CHANGES_REQUESTED") | .id' "$SNAPSHOT_FILE" 2>/dev/null); then
    while read -r id; do
      [ -n "$id" ] || continue
      try -X PUT "repos/${REPO}/pulls/${PR}/reviews/${id}/dismissals" \
        -f message="Superseded by new review" -f event="DISMISS"
    done <<<"$ids"
  else
    mislukt=$((mislukt + 1))
  fi

  for key in inline sticky; do
    if [ "$key" = inline ]; then
      current=$(list "pulls/${PR}/comments")
    else
      current=$(list "issues/${PR}/comments")
    fi
    # Een gevallen stap zou de job op `failure` zetten, en dan meldt de poort
    # dat er geen bruikbare review is terwijl die er wel is.
    if ! current=$(jq "[.[][] | select(.user.login == \"${REVIEW_AUTHOR}\") | {id, updated_at}]" <<<"${current:-}" 2>/dev/null) ||
      [ -z "$current" ]; then
      mislukt=$((mislukt + 1))
      continue
    fi

    if ! ids=$(jq -r --argjson now "$current" --arg key "$key" '
      ($now | map({key: (.id | tostring), value: .updated_at}) | from_entries) as $huidig
      | .[$key][]
      | select($huidig[.id | tostring] == .updated_at)
      | .id' "$SNAPSHOT_FILE" 2>/dev/null); then
      mislukt=$((mislukt + 1))
      continue
    fi

    while read -r id; do
      [ -n "$id" ] || continue
      if [ "$key" = inline ]; then
        try -X DELETE "repos/${REPO}/pulls/comments/${id}"
      else
        try -X DELETE "repos/${REPO}/issues/comments/${id}"
      fi
    done <<<"$ids"
  done

  if [ "$mislukt" -gt 0 ]; then
    echo "::warning title=Claude review comments::${mislukt} keer misgegaan bij het opruimen; comments van de vorige review zijn blijven staan."
    {
      echo "### Opruimen van de vorige review"
      echo
      echo "${mislukt} keer misgegaan: er is niet verwijderd wat er weg had gemoeten. Die comments van \`${REVIEW_AUTHOR}\` blijven op de pull request staan. De poort kijkt alleen naar wat déze run schreef, dus de uitslag verandert er niet door."
    } >>"$GITHUB_STEP_SUMMARY"
  fi
}

case "${1:-}" in
snapshot) snapshot ;;
clean-up) clean_up ;;
*) fail "Gebruik: $0 {snapshot|clean-up}" ;;
esac
