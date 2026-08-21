#!/usr/bin/env bash
# Draait cargo-deny op de versie die CI ook draait.
#
# Welke checks er draaien geef je mee als argument (`advisories`, `bans`,
# `licenses`, `sources`, `all`); zonder argument draait `all`. De aanroepers
# zijn `just audit` (deterministisch: bans, licenses, sources) en
# `just audit-advisories` (tijdsafhankelijk), zie de Justfile.
#
# De aanroep verschilt per versie: 0.19 wil `check --config <pad>`, 0.20 wil
# `--config <pad> check`. Eén aanroep die op beide werkt bestaat niet, dus wie
# cargo-deny vers installeert kreeg `unexpected argument '--config' found` in
# plaats van een audit. Versie en checksum staan in ci.yml en worden hier
# gelezen, zodat er niet twee plekken zijn die uiteen kunnen lopen.
set -euo pipefail

cd "$(dirname "$0")/.."
root=$PWD
workflow=.github/workflows/ci.yml

version=$(sed -n 's/^ *CARGO_DENY_VERSION="\(.*\)"$/\1/p' "$workflow" | head -1)
sha=$(sed -n 's/^ *CARGO_DENY_SHA256="\(.*\)"$/\1/p' "$workflow" | head -1)

if [ -z "$version" ] || [ -z "$sha" ]; then
    echo "FOUT: kon CARGO_DENY_VERSION/CARGO_DENY_SHA256 niet uit ${workflow} lezen" >&2
    exit 1
fi

cache="${XDG_CACHE_HOME:-$HOME/.cache}/regelrecht"
binary="${cache}/cargo-deny-${version}"

versie_van() { "$1" --version 2>/dev/null | awk '{print $2}'; }

# Staat de juiste versie al op PATH, gebruik die. De Security Audit-baan in CI
# installeert hem naar /usr/local/bin; zonder deze tak zou dit script daarnaast
# een tweede exemplaar ophalen.
op_path=$(command -v cargo-deny || true)
if [ -n "$op_path" ] && [ "$(versie_van "$op_path")" = "$version" ]; then
    binary="$op_path"
elif [ ! -x "$binary" ]; then
    if [ "$(uname -s)" != "Linux" ] || [ "$(uname -m)" != "x86_64" ]; then
        echo "FOUT: deze repo draait cargo-deny ${version}, hier staat ${op_path:+$(versie_van "$op_path")}${op_path:-niets}." >&2
        echo "      Er is geen prebuilt binary voor $(uname -s)/$(uname -m); installeer met:" >&2
        echo "      cargo install cargo-deny --locked --version ${version}" >&2
        exit 1
    fi
    echo "cargo-deny ${version} ophalen (eenmalig, naar ${cache})"
    mkdir -p "$cache"
    tarball="cargo-deny-${version}-x86_64-unknown-linux-musl"
    tmp=$(mktemp -d)
    trap 'rm -rf "$tmp"' EXIT
    curl -sSL "https://github.com/EmbarkStudios/cargo-deny/releases/download/${version}/${tarball}.tar.gz" \
        -o "$tmp/cargo-deny.tar.gz"
    echo "${sha}  $tmp/cargo-deny.tar.gz" | sha256sum -c -
    tar -xz --strip-components=1 -C "$tmp" -f "$tmp/cargo-deny.tar.gz" "${tarball}/cargo-deny"
    install -m0755 "$tmp/cargo-deny" "$binary"
fi

checks=("$@")
if [ "${#checks[@]}" -eq 0 ]; then
    checks=(all)
fi

cd packages
exec "$binary" check --config "${root}/deny.toml" "${checks[@]}"
