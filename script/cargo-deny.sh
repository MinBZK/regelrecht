#!/usr/bin/env bash
# Draait cargo-deny op de versie die CI ook draait.
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

if [ ! -x "$binary" ]; then
    if [ "$(uname -s)" != "Linux" ] || [ "$(uname -m)" != "x86_64" ]; then
        installed=$(cargo deny --version 2>/dev/null | awk '{print $2}')
        if [ "$installed" != "$version" ]; then
            echo "FOUT: deze repo draait cargo-deny ${version}, hier staat ${installed:-niets}." >&2
            echo "      Er is geen prebuilt binary voor $(uname -s)/$(uname -m); installeer met:" >&2
            echo "      cargo install cargo-deny --locked --version ${version}" >&2
            exit 1
        fi
        binary=$(command -v cargo-deny)
    else
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
fi

cd packages
exec "$binary" check --config "${root}/deny.toml"
