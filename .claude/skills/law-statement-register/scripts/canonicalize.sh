#!/usr/bin/env bash
# Canonicalize a secondary-text source (PDF) into the evidence set the whole
# register anchors on. Deterministic: same bytes in, same canonical.md out.
#
# Writes three files next to each other in OUTDIR:
#   canonical.md  - pure reading text. NO page markers, NO injected characters:
#                   every anchor in the register must be a literal substring of
#                   this file, including quotes that run across a page break.
#   pages.tsv     - page -> character offset in canonical.md, so a statement can
#                   still be cited as "(p13)" without polluting the text.
#   manifest.yaml - sha256 of the source bytes, sha256 of canonical.md, the
#                   retrieval date and which normalization steps ran.
#
# The source hash is the point: policy PDFs get replaced at the same URL without
# any version marker, so the hash is the only thing that makes "this register was
# read from that document" a checkable claim.
#
# Usage: canonicalize.sh SOURCE OUTDIR [--retrieved YYYY-MM-DD] [--root SELECTOR]
#        SOURCE is a .pdf, or a locally saved .html/.htm
#
# For an HTML source there are no pages, so no pages.tsv is written; --root
# passes a content-block id/class through to html_canonical.py.
set -euo pipefail

if [[ $# -lt 2 ]]; then
    sed -n '2,24p' "$0" | sed 's/^# \{0,1\}//'
    exit 2
fi

SRC="$1"
OUTDIR="$2"
shift 2
RETRIEVED="$(date +%F)"
ROOT=""
while [[ $# -gt 0 ]]; do
    case "$1" in
        --retrieved) RETRIEVED="$2"; shift 2 ;;
        --root) ROOT="$2"; shift 2 ;;
        *) echo "onbekende optie: $1" >&2; exit 2 ;;
    esac
done

[[ -f "$SRC" ]] || { echo "bron niet gevonden: $SRC" >&2; exit 2; }
mkdir -p "$OUTDIR"
HERE="$(cd "$(dirname "$0")" && pwd)"
EXT="$(printf '%s' "${SRC##*.}" | tr 'A-Z' 'a-z')"

if [[ "$EXT" == "html" || "$EXT" == "htm" ]]; then
    # -- HTML branch: no pages, no layout reconstruction, own furniture rule.
    EXTRACTOR="html_canonical.py (stdlib)"
    if [[ -n "$ROOT" ]]; then
        python3 "$HERE/html_canonical.py" "$SRC" --root "$ROOT" > "$OUTDIR/canonical.md"
    else
        python3 "$HERE/html_canonical.py" "$SRC" > "$OUTDIR/canonical.md"
    fi
    rm -f "$OUTDIR/pages.tsv"
    NORMALIZATION="  - unicode-NFC
  - zacht-koppelteken-verwijderd
  - navigatie-/kop-/voetelementen-verwijderd
  - witruimte-genormaliseerd"
else
    EXTRACTOR="pdftotext -layout -enc UTF-8"
    NORMALIZATION="  - unicode-NFC
  - zacht-koppelteken-verwijderd
  - regeleinde-afbreekstreepjes-samengevoegd
  - lopende-kop-/voetteksten-verwijderd
  - witruimte-genormaliseerd"
    command -v pdftotext >/dev/null || { echo "pdftotext (poppler) ontbreekt" >&2; exit 2; }

RAW="$OUTDIR/.raw.txt"
pdftotext -layout -enc UTF-8 "$SRC" "$RAW"

python3 - "$RAW" "$OUTDIR" <<'PY'
import re, sys, unicodedata
from collections import Counter
from pathlib import Path

raw_path, outdir = Path(sys.argv[1]), Path(sys.argv[2])
pages = raw_path.read_text(encoding="utf-8").split("\f")
if pages and not pages[-1].strip():
    pages.pop()

# --- running headers/footers -------------------------------------------------
# A line that shows up in the top-2 or bottom-2 of most pages is furniture, not
# text. Dropping it here (visibly, in one place) beats teaching every later step
# to ignore it.
edge = Counter()
for p in pages:
    lines = [ln.strip() for ln in p.splitlines() if ln.strip()]
    for ln in lines[:2] + lines[-2:]:
        edge[ln] += 1
threshold = max(3, int(len(pages) * 0.6))
furniture = {ln for ln, n in edge.items() if n >= threshold and len(pages) >= 3}
bare_number = re.compile(r"^(pagina\s*)?\d+(\s*(van|/)\s*\d+)?$", re.IGNORECASE)

cleaned, offsets, cursor = [], [], 0
for n, page in enumerate(pages, start=1):
    kept = []
    for ln in page.splitlines():
        s = ln.strip()
        if s in furniture or bare_number.match(s):
            continue
        kept.append(ln.rstrip())
    body = "\n".join(kept).strip("\n")
    offsets.append((n, cursor))
    cursor += len(body) + 1  # the "\n" that joins pages
    cleaned.append(body)

text = "\n".join(cleaned)

# --- normalization, applied once and recorded --------------------------------
steps = []
text = unicodedata.normalize("NFC", text); steps.append("unicode-NFC")
text = text.replace("­", ""); steps.append("zacht-koppelteken-verwijderd")
# Join words broken over a line end ("betalings-\ncapaciteit"), but only when the
# next line continues in lowercase - "wet- en regelgeving" must stay intact.
text, n_hyphen = re.subn(r"(\w)-\n\s*([a-zà-ÿ])", r"\1\2", text)
steps.append(f"regeleinde-afbreekstreepjes-samengevoegd({n_hyphen})")
text = re.sub(r"[ \t]+\n", "\n", text); steps.append("regeleinde-witruimte-gestript")
text = re.sub(r"\n{3,}", "\n\n", text); steps.append("lege-regels-samengevouwen")
text = text.strip() + "\n"

(outdir / "canonical.md").write_text(text, encoding="utf-8")
(outdir / "pages.tsv").write_text(
    "page\toffset\n" + "".join(f"{n}\t{min(o, len(text))}\n" for n, o in offsets),
    encoding="utf-8")
print("\n".join(steps))
print(f"pages={len(pages)} furniture_lines={len(furniture)} chars={len(text)}")
PY

    rm -f "$RAW"
fi

SRC_HASH="$(shasum -a 256 "$SRC" | cut -d' ' -f1)"
CAN_HASH="$(shasum -a 256 "$OUTDIR/canonical.md" | cut -d' ' -f1)"

cat > "$OUTDIR/manifest.yaml" <<EOF
---
source_file: $(basename "$SRC")
source_sha256: '$SRC_HASH'
canonical_sha256: '$CAN_HASH'
retrieved_at: '$RETRIEVED'
extractor: '$EXTRACTOR'
normalization:
$NORMALIZATION
EOF

if [[ -f "$OUTDIR/pages.tsv" ]]; then
    echo "geschreven: $OUTDIR/{canonical.md,pages.tsv,manifest.yaml}"
else
    echo "geschreven: $OUTDIR/{canonical.md,manifest.yaml}  (HTML-bron: geen pagina's)"
fi
echo "source_sha256=$SRC_HASH"
