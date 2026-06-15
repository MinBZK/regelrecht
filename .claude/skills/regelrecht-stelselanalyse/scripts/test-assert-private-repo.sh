#!/usr/bin/env bash
# test-assert-private-repo.sh — smoke-test voor de fail-closed push-guard.
#
# Stubt `git`, `gh` en `sleep` via een tijdelijke PATH-dir, zodat de guard zonder
# echte repo of netwerk te testen is. Houdt de invariant vast:
#   - github.com PRIVATE                  → exit 0  (push toegestaan)
#   - github.com PUBLIC / INTERNAL        → exit 1  (geblokkeerd)
#   - GitLab / GitHub-Enterprise / overig → exit 1  (host-check, fail-closed)
#   - geen remote / gh onbereikbaar / geen repo → exit 1
#
# Gebruik: bash test-assert-private-repo.sh   (exit 0 = alle cases groen)
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
GUARD="$SCRIPT_DIR/assert-private-repo.sh"
[ -f "$GUARD" ] || { echo "FAIL: guard niet gevonden op $GUARD" >&2; exit 1; }

BIN="$(mktemp -d)"
trap 'rm -rf "$BIN"' EXIT

# --- stubs: lezen scenario uit env (TEST_REMOTE_URL, TEST_VISIBILITY, TEST_IS_REPO) ---
cat >"$BIN/git" <<'STUB'
#!/usr/bin/env bash
case "$1 $2" in
  "rev-parse --is-inside-work-tree")
    [ "${TEST_IS_REPO:-1}" = "1" ] && exit 0 || exit 128 ;;
  "remote get-url")
    [ -n "${TEST_REMOTE_URL:-}" ] && { printf '%s\n' "$TEST_REMOTE_URL"; exit 0; } || exit 2 ;;
esac
exit 0
STUB

cat >"$BIN/gh" <<'STUB'
#!/usr/bin/env bash
if [ "$1" = "repo" ] && [ "$2" = "view" ]; then
  [ -n "${TEST_VISIBILITY:-}" ] && { printf '%s\n' "$TEST_VISIBILITY"; exit 0; } || exit 1
fi
exit 0
STUB

# no-op sleep zodat de retry-loop de test niet vertraagt
printf '#!/usr/bin/env bash\nexit 0\n' >"$BIN/sleep"
chmod +x "$BIN/git" "$BIN/gh" "$BIN/sleep"

PATH="$BIN:$PATH"

pass=0; fail=0
run_case() {
  # $1 naam  $2 verwachte-exit  $3 verwachte-substring (of "")  [env via globals]
  local name="$1" want_code="$2" want_sub="$3" out code
  set +e
  out="$(TEST_REMOTE_URL="${REMOTE_URL:-}" TEST_VISIBILITY="${VIS:-}" TEST_IS_REPO="${IS_REPO:-1}" \
        bash "$GUARD" 2>&1)"
  code=$?
  set -e
  local ok=1
  [ "$code" = "$want_code" ] || ok=0
  if [ -n "$want_sub" ]; then case "$out" in *"$want_sub"*) : ;; *) ok=0 ;; esac; fi
  if [ "$ok" = 1 ]; then
    printf '  ✓ %s\n' "$name"; pass=$((pass+1))
  else
    printf '  ✗ %s — exit=%s (verwacht %s), output: %s\n' "$name" "$code" "$want_code" "$out"; fail=$((fail+1))
  fi
}

echo "Smoke-test push-guard:"
REMOTE_URL="git@github.com:MinBZK/regelrecht-private.git" VIS="PRIVATE"  IS_REPO=1 run_case "github.com SSH private → pass"        0 "GUARD OK"
REMOTE_URL="https://github.com/MinBZK/regelrecht-private.git" VIS="PRIVATE" IS_REPO=1 run_case "github.com HTTPS private → pass"  0 "GUARD OK"
REMOTE_URL="ssh://git@github.com/MinBZK/x.git" VIS="PRIVATE"  IS_REPO=1 run_case "github.com ssh:// private → pass"        0 "GUARD OK"
REMOTE_URL="git@github.com:MinBZK/regelrecht.git" VIS="PUBLIC" IS_REPO=1 run_case "github.com public → block"             1 "niet PRIVATE"
REMOTE_URL="git@github.com:MinBZK/regelrecht.git" VIS="INTERNAL" IS_REPO=1 run_case "github.com internal → block"         1 "niet PRIVATE"
REMOTE_URL="git@gitlab.com:MinBZK/regelrecht.git" VIS="PRIVATE" IS_REPO=1 run_case "gitlab.com → block (host-check)"      1 "geen github.com"
REMOTE_URL="git@github.example.com:org/repo.git"  VIS="PRIVATE" IS_REPO=1 run_case "GitHub-Enterprise → block (host-check)" 1 "geen github.com"
REMOTE_URL="git@bitbucket.org:org/repo.git"       VIS="PRIVATE" IS_REPO=1 run_case "bitbucket → block (host-check)"       1 "geen github.com"
REMOTE_URL=""                                     VIS="PRIVATE" IS_REPO=1 run_case "geen remote → block"                  1 "geen remote"
REMOTE_URL="git@github.com:MinBZK/x.git"          VIS=""        IS_REPO=1 run_case "gh onbereikbaar → block"              1 "na 3 pogingen"
REMOTE_URL="git@github.com:MinBZK/x.git"          VIS="PRIVATE" IS_REPO=0 run_case "geen git-repo → block"                1 "geen git-repo"

echo "----"
printf '%d geslaagd, %d gefaald\n' "$pass" "$fail"
[ "$fail" = 0 ]
