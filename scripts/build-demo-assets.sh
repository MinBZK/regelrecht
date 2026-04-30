#!/usr/bin/env bash
# Bundle the assets that the burger-demo (frontend-demo/) needs at runtime:
# - Law YAMLs referenced by corpus-demo/demo-index.yaml (+ their dependencies)
# - Persona profiles (YAML -> JSON for easy JS consumption)
# - The demo-index itself (YAML -> JSON)
#
# Output lands in frontend-demo/public/demo-assets/ and can be fetched by the
# Vue app as static files (no backend required).
#
# Laws are copied preserving their source-relative path under laws/ so multiple
# files with the same basename (e.g. 2025-01-01.yaml) don't collide.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT_DIR="$REPO_ROOT/frontend-demo/public/demo-assets"
INDEX="$REPO_ROOT/corpus-demo/demo-index.yaml"

if [ ! -f "$INDEX" ]; then
    echo "error: $INDEX not found" >&2
    exit 1
fi

mkdir -p "$OUT_DIR/laws" "$OUT_DIR/profiles"

yaml_to_json() {
    local src="$1"
    if command -v yq >/dev/null 2>&1; then
        yq -o=json eval '.' "$src"
    else
        python3 -c 'import sys, json, yaml; json.dump(yaml.safe_load(open(sys.argv[1])), sys.stdout)' "$src"
    fi
}

# Emit every law path referenced by demo-index (main law + dependencies).
law_paths() {
    if command -v yq >/dev/null 2>&1; then
        yq -r '.laws[] | (.path, (.dependencies // [])[])' "$INDEX"
    else
        python3 -c '
import sys, yaml
idx = yaml.safe_load(open(sys.argv[1]))
for l in idx["laws"]:
    print(l["path"])
    for dep in (l.get("dependencies") or []):
        print(dep)
' "$INDEX"
    fi
}

# Clean previous law output to avoid orphans from a renamed dependency.
rm -rf "$OUT_DIR/laws"
mkdir -p "$OUT_DIR/laws"

while IFS= read -r rel_path; do
    [ -n "$rel_path" ] || continue
    # Reject absolute paths and any '..' segments so a malformed demo-index
    # cannot make us read or write outside REPO_ROOT / OUT_DIR.
    case "$rel_path" in
        /*|*..*)
            echo "error: law path '$rel_path' is absolute or contains '..'" >&2
            exit 1
            ;;
    esac
    src="$REPO_ROOT/$rel_path"
    if [ ! -f "$src" ]; then
        echo "error: law file not found: $src" >&2
        exit 1
    fi
    dest="$OUT_DIR/laws/$rel_path"
    mkdir -p "$(dirname "$dest")"
    cp "$src" "$dest"
    echo "copied law: $rel_path"
done < <(law_paths)

for profile in "$REPO_ROOT"/corpus-demo/profiles/*.yaml; do
    [ -f "$profile" ] || continue
    name=$(basename "$profile" .yaml)
    yaml_to_json "$profile" > "$OUT_DIR/profiles/$name.json"
    echo "converted profile: $name.yaml -> demo-assets/profiles/$name.json"
done

yaml_to_json "$INDEX" > "$OUT_DIR/demo-index.json"
echo "converted demo-index -> demo-assets/demo-index.json"

echo ""
echo "demo assets bundled in: $OUT_DIR"
