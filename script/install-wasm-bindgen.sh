#!/usr/bin/env bash
# Install the wasm-bindgen CLI that `just wasm-build` needs, in CI.
#
# Version and checksum come from frontend/Dockerfile instead of being written
# down a second time; that keeps CI and the editor image on exactly the same
# binary. The version has to match the wasm-bindgen crate in
# packages/Cargo.lock, or the bindgen step fails later on a version mismatch.
# Used by every CI job that builds the engine for the browser.
set -euo pipefail

locked=$(grep -A1 '^name = "wasm-bindgen"$' packages/Cargo.lock \
  | sed -n '/^version = /{s/^version = "\(.*\)"$/\1/p;q;}')
version=$(sed -n 's/^ARG WASM_BINDGEN_VERSION=\(.*\)$/\1/p' frontend/Dockerfile)
sha=$(sed -n 's/^ARG WASM_BINDGEN_SHA256_AMD64=\(.*\)$/\1/p' frontend/Dockerfile)
if [ -z "$locked" ]; then
  echo "::error::kon wasm-bindgen-versie niet uit packages/Cargo.lock lezen"
  exit 1
fi
if [ -z "$version" ] || [ -z "$sha" ]; then
  echo "::error::kon WASM_BINDGEN_VERSION/WASM_BINDGEN_SHA256_AMD64 niet uit frontend/Dockerfile lezen"
  exit 1
fi
if [ "$version" != "$locked" ]; then
  echo "::error::wasm-bindgen-cli is in frontend/Dockerfile gepind op $version, packages/Cargo.lock heeft $locked. Werk WASM_BINDGEN_VERSION en beide WASM_BINDGEN_SHA256_* bij naar $locked; de checksums staan naast de release op https://github.com/wasm-bindgen/wasm-bindgen/releases/tag/$locked"
  exit 1
fi
dir="wasm-bindgen-${version}-x86_64-unknown-linux-musl"
curl -fsSLo "/tmp/${dir}.tar.gz" \
  "https://github.com/wasm-bindgen/wasm-bindgen/releases/download/${version}/${dir}.tar.gz"
echo "${sha}  /tmp/${dir}.tar.gz" | sha256sum -c -
tar -xzf "/tmp/${dir}.tar.gz" -C /tmp
install -D -m0755 "/tmp/${dir}/wasm-bindgen" "$HOME/.cargo/bin/wasm-bindgen"
wasm-bindgen --version
