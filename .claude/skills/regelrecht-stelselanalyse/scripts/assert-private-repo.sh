#!/usr/bin/env bash
# assert-private-repo.sh — fail-closed guard: exit 0 alleen als de push-target
# van de huidige git-repo aantoonbaar een PRIVATE GitHub-repo is.
#
# Gebruik (preflight vóór elke commit/push in een autonome routine):
#   bash assert-private-repo.sh [remote]      # remote default: origin
#   bash assert-private-repo.sh && git push   # push alleen als de guard slaagt
#
# Fail-closed: elke onzekerheid (geen repo / geen remote / geen gh / niet te bepalen /
# niet-GitHub / INTERNAL / PUBLIC) → non-zero exit. Lokale commits zijn altijd veilig;
# deze guard bewaakt het PUBLICEREN (push), want daar zit de blootstelling.
set -euo pipefail

fail() { echo "GUARD BLOCK: $*" >&2; exit 1; }

# 1. Binnen een git-repo?
git rev-parse --is-inside-work-tree >/dev/null 2>&1 || fail "geen git-repo (cwd=$(pwd))"

# 2. Remote bepalen
remote="${1:-origin}"
url="$(git remote get-url "$remote" 2>/dev/null)" || fail "geen remote '$remote'"

# 3. Host strikt github.com (bewust geen GHE, geen GitLab/Bitbucket). Anders zou de
#    slug-extractie de host blind strippen en de naam alsnog op github.com queriën —
#    een match daar zegt niets over de werkelijke push-target. Fail-closed.
case "$url" in
  git@github.com:*|https://github.com/*|ssh://git@github.com/*) : ;;
  *) fail "remote '$url' is geen github.com — guard alleen geldig voor github.com" ;;
esac

# 4. gh aanwezig + geauthenticeerd? (autoritatieve bron voor zichtbaarheid)
command -v gh >/dev/null 2>&1 || fail "gh niet geïnstalleerd — zichtbaarheid niet te verifiëren"

# 5. owner/repo afleiden uit ssh- of https-URL
slug="$(printf '%s' "$url" \
  | sed -E 's#^git@github\.com:##; s#^https://github\.com/##; s#^ssh://git@github\.com/##; s#\.git$##')"
case "$slug" in
  */*) : ;;
  *) fail "kan GitHub owner/repo niet uit '$url' halen" ;;
esac

# 6. Zichtbaarheid opvragen; alleen PRIVATE laat door (INTERNAL/PUBLIC = block).
#    Korte retry mét pauze tegen transiënte gh-haperingen — blijft fail-closed.
vis=""
for _attempt in 1 2 3; do
  if vis="$(gh repo view "$slug" --json visibility --jq .visibility 2>/dev/null)" && [ -n "$vis" ]; then
    break
  fi
  vis=""
  sleep 1
done
[ -n "$vis" ] || fail "kan zichtbaarheid van '$slug' niet opvragen na 3 pogingen (auth/netwerk?)"

if [ "$vis" = "PRIVATE" ]; then
  echo "GUARD OK: $slug is PRIVATE — push toegestaan"
  exit 0
fi
fail "$slug visibility=$vis (niet PRIVATE) — push geweigerd"
