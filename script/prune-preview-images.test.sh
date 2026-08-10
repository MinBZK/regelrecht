#!/usr/bin/env bash
# Dekt elk pad dat bepaalt of er een image-versie verdwijnt, met stubs voor
# `curl` en `gh` op PATH. De stub schrijft elke DELETE weg, zodat de test niet
# op de melding afgaat maar op wat er werkelijk zou zijn verwijderd.
#
# Het geval dat er echt toe doet staat bovenaan: een versie waarvan alle tags de
# `sha-`-vorm hebben en die op dat moment in ZAD draait. Dat is de vorm waarin
# productie draait, en de vorige versie van dit script zou hem hebben verwijderd.
set -uo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
script="$here/prune-preview-images.sh"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

pass=0
fail=0

old="$(date -u -d '30 days ago' +%Y-%m-%dT%H:%M:%SZ)"
recent="$(date -u -d '1 day ago' +%Y-%m-%dT%H:%M:%SZ)"

# $1 = images die ZAD draait (spatiegescheiden), $2 = curl-exitcode.
stub_curl() {
    local images="$1" rc="${2:-0}" body="" i
    for i in $images; do
        [ -n "$body" ] && body+=","
        # Een waarde met een `/` erin is een volledige image-URL; anders is het
        # alleen de tag en plakken we er een gewone URL omheen.
        case "$i" in
            */*) body+="{\"image\":\"$i\"}" ;;
            *) body+="{\"image\":\"ghcr.io/o/p:$i\"}" ;;
        esac
    done
    cat >"$tmp/curl" <<STUB
#!/usr/bin/env bash
[ $rc -ne 0 ] && exit $rc
cat <<'JSON'
{"deployments":[{"name":"regelrecht","components":[$body]}]}
JSON
STUB
    chmod +x "$tmp/curl"
}

# $1 = versies als "id|updated|tag,tag" (spatiegescheiden), $2 = exitcode voor
# de lijst-aanroep. Het scheidingsteken is `|` en geen `:`, want de tijdstempel
# bevat zelf dubbele punten.
stub_gh() {
    local versions="$1" rc="${2:-0}" body="" v id updated tags taglist t
    for v in $versions; do
        IFS='|' read -r id updated tags <<<"$v"
        taglist=""
        if [ -n "$tags" ]; then
            for t in ${tags//,/ }; do
                [ -n "$taglist" ] && taglist+=","
                taglist+="\"$t\""
            done
        fi
        [ -n "$body" ] && body+=","
        body+="{\"id\":$id,\"updated_at\":\"$updated\",\"metadata\":{\"container\":{\"tags\":[$taglist]}}}"
    done
    cat >"$tmp/gh" <<STUB
#!/usr/bin/env bash
if [ "\$1" = "api" ] && [ "\$2" = "--method" ]; then
    echo "\$4" >>"$tmp/deleted"
    exit 0
fi
[ $rc -ne 0 ] && exit $rc
cat <<'JSON'
[$body]
JSON
STUB
    chmod +x "$tmp/gh"
}

check() { # naam, verwachte exit, in-gebruik, versies, verwachte-deletes, curl-rc, gh-rc
    local name="$1" want="$2" used="$3" versions="$4" want_del="$5"
    local crc="${6:-0}" grc="${7:-0}"
    : >"$tmp/deleted"
    stub_curl "$used" "$crc"
    stub_gh "$versions" "$grc"

    local out status actual
    out="$(
        PATH="$tmp:$PATH" ORG=o PACKAGES=p ZAD_API_KEY=k \
            ZAD_API_BASE=http://x ZAD_PROJECT=proj bash "$script" 2>&1
    )"
    status=$?
    actual="$(tr '\n' ' ' <"$tmp/deleted" | sed 's/ *$//')"

    if [ "$status" -ne "$want" ]; then
        echo "FAIL: $name — exit $status, verwacht $want"
        echo "$out" | sed 's/^/    /'
        fail=$((fail + 1))
        return
    fi
    if [ "$actual" != "$want_del" ]; then
        echo "FAIL: $name — verwijderd '[$actual]', verwacht '[$want_del]'"
        echo "$out" | sed 's/^/    /'
        fail=$((fail + 1))
        return
    fi
    echo "ok: $name"
    pass=$((pass + 1))
}

# De kern: dit is de vorm waarin productie draait.
check "een sha-only image die in ZAD draait blijft staan" 0 \
    "sha-abc" "1|$old|sha-abc" ""

check "een sha-only image dat nergens draait en oud is gaat weg" 0 \
    "sha-abc" "2|$old|sha-def" "/orgs/o/packages/container/p/versions/2"

check "een sha-only image dat te jong is blijft staan" 0 \
    "sha-abc" "3|$recent|sha-def" ""

check "latest beschermt de versie" 0 \
    "sha-abc" "4|$old|sha-def,latest" ""

check "een pr-tag beschermt de versie" 0 \
    "sha-abc" "5|$old|pr-123" ""

# Single-arch builds: een versie zonder tags is de buildcache in GHCR, geen
# onderdeel van een multi-arch manifest.
check "een versie zonder tags blijft staan" 0 \
    "sha-abc" "6|$old|" ""

check "meerdere sha-tags waarvan er één draait beschermt de versie" 0 \
    "sha-abc" "7|$old|sha-def,sha-abc" ""

# Zonder te weten wat er draait mag er niets weg; dat was de fout van de
# vorige versie, die de 403 in /dev/null gooide en gewoon doorging.
check "een onbereikbare ZAD-API verwijdert niets" 1 \
    "sha-abc" "8|$old|sha-def" "" 7

check "een lege lijst draaiende images verwijdert niets" 1 \
    "" "9|$old|sha-def" ""

check "een mislukte versie-opvraag verwijdert niets" 1 \
    "sha-abc" "10|$old|sha-def" "" 0 1

# De tag zit in het laatste padsegment. Op de hele URL zou de poort als tag
# gelezen worden en beschermt de lijst niets meer.
check "een registry met een poort verandert niets aan de bescherming" 0 \
    "registry.internal:5000/o/p:sha-abc" "11|$old|sha-abc" ""

# Bij een digest is de draaiende tag niet af te leiden; doorgaan zou de
# bescherming stil uitschakelen.
check "een digest-gepinde image stopt het script" 1 \
    "ghcr.io/o/p@sha256:deadbeef" "12|$old|sha-abc" ""

check "een onleesbare datum slaat de versie over" 0 \
    "sha-abc" "13|null|sha-def" ""

echo
echo "geslaagd: $pass, gefaald: $fail"
[ "$fail" -eq 0 ]
