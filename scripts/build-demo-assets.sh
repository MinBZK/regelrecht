#!/usr/bin/env bash
# Bundle the assets that the burger-demo (frontend-demo/) needs at runtime:
# - Law YAMLs referenced by corpus-demo/demo-index.yaml
# - Persona profiles (YAML -> JSON for easy JS consumption)
# - The demo-index itself (YAML -> JSON)
#
# Output lands in frontend-demo/public/demo-assets/ and can be fetched by the
# Vue app as static files (no backend required).
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT_DIR="$REPO_ROOT/frontend-demo/public/demo-assets"
INDEX="$REPO_ROOT/corpus-demo/demo-index.yaml"

if [ ! -f "$INDEX" ]; then
    echo "error: $INDEX not found" >&2
    exit 1
fi

mkdir -p "$OUT_DIR/laws" "$OUT_DIR/profiles"

# yq is the only non-core dependency. If unavailable, fall back to a python
# helper (python3 is present in CI and the dev container).
yaml_to_json() {
    local src="$1"
    if command -v yq >/dev/null 2>&1; then
        yq -o=json eval '.' "$src"
    else
        python3 -c 'import sys, json, yaml; json.dump(yaml.safe_load(open(sys.argv[1])), sys.stdout)' "$src"
    fi
}

# 1. Copy every law YAML referenced by the demo-index.
#    The index is YAML so we need yq or python to read it.
law_paths() {
    if command -v yq >/dev/null 2>&1; then
        yq -r '.laws[].path' "$INDEX"
    else
        python3 -c 'import sys, yaml; [print(l["path"]) for l in yaml.safe_load(open(sys.argv[1]))["laws"]]' "$INDEX"
    fi
}

while IFS= read -r rel_path; do
    [ -n "$rel_path" ] || continue
    src="$REPO_ROOT/$rel_path"
    if [ ! -f "$src" ]; then
        echo "error: law file not found: $src" >&2
        exit 1
    fi
    dest="$OUT_DIR/laws/$(basename "$rel_path")"
    cp "$src" "$dest"
    echo "copied law: $rel_path -> demo-assets/laws/$(basename "$rel_path")"
done < <(law_paths)

# 2. Convert every persona YAML to JSON.
for profile in "$REPO_ROOT"/corpus-demo/profiles/*.yaml; do
    [ -f "$profile" ] || continue
    name=$(basename "$profile" .yaml)
    yaml_to_json "$profile" > "$OUT_DIR/profiles/$name.json"
    echo "converted profile: $name.yaml -> demo-assets/profiles/$name.json"
done

# 3. Convert demo-index to JSON.
yaml_to_json "$INDEX" > "$OUT_DIR/demo-index.json"
echo "converted demo-index -> demo-assets/demo-index.json"

echo ""
echo "demo assets bundled in: $OUT_DIR"
