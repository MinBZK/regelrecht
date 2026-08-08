#!/usr/bin/env python3
"""Tile canonical.md into segments (fase 2) - a draft ledger, by construction complete.

The tiling is mechanical on purpose. If a human types the segments by hand, the
100%-coverage rule becomes a chore and the chore becomes a shortcut. Here the
segments are cut from the text itself, so `concat(segments) == canonical` holds
before anyone has made a single judgement call.

What the script does NOT do is decide anything: every segment comes out as
`disposition: normative` with no statements. Setting dispositions (with a reason),
writing statements and doing the verankering is the reading work, and that is
where the judgement belongs.

Headings are detected as numbered lines ("4", "4.3", "4.3.1 Titel"); the heading
path is derived from the numbering depth. Consecutive headings with no body
between them are merged into one segment, which collapses a table of contents
into a single block instead of forty empty ones.

Usage:
  python3 tile.py canonical.md > statements.draft.yaml
  python3 tile.py canonical.md --heading-regex '^(\\d+(?:\\.\\d+)*)\\s+(\\S.*)$'
  python3 tile.py canonical.md --no-merge-empty
"""
import argparse
import re
import sys

try:
    import yaml
except ImportError:
    sys.stderr.write("tile.py requires pyyaml (pip install pyyaml)\n")
    sys.exit(2)

DEFAULT_HEADING = r"^(\d+(?:\.\d+)*)\s+(\S.*)$"


# A numbered heading needs a title. A table's header row ("2019 2020 2021 …")
# matches the same pattern and, left alone, becomes a section heading that cuts
# the table in half - coverage stays at 100% and the structure is quietly wrong.
# Two cheap rejections catch it: a tab means table cells (that is what the HTML
# reader emits), and a "title" without a single three-letter word is not a title.
WORD = re.compile(r"[A-Za-zÀ-ÿ]{3}")


def looks_like_heading(number: str, title: str) -> bool:
    if "\t" in title:
        return False
    return bool(WORD.search(title))


def find_headings(lines, pattern):
    """Return [(line_index, number, title)] for every line that looks like a heading."""
    out = []
    for i, line in enumerate(lines):
        m = pattern.match(line.strip())
        if m and looks_like_heading(m.group(1), m.group(2).strip()):
            out.append((i, m.group(1), m.group(2).strip()))
    return out


def build_segments(text, pattern, merge_empty=True):
    lines = text.split("\n")
    heads = find_headings(lines, pattern)

    # Cut points: start of the text, then every heading line.
    cuts = [0] + [i for i, _, _ in heads]
    blocks = []
    for n, start in enumerate(cuts):
        end = cuts[n + 1] if n + 1 < len(cuts) else len(lines)
        head = next((h for h in heads if h[0] == start), None)
        body = "\n".join(lines[start:end]).strip("\n")
        if body:
            blocks.append({"head": head, "text": body})

    if merge_empty:
        merged = []
        for b in blocks:
            body_lines = [ln for ln in b["text"].split("\n")[1:] if ln.strip()]
            heading_only = b["head"] is not None and not body_lines
            if heading_only and merged:
                merged[-1]["text"] += "\n" + b["text"]
                merged[-1]["merged"] = merged[-1].get("merged", 0) + 1
            else:
                merged.append(b)
        blocks = merged

    # Heading path: keep a stack keyed on numbering depth.
    segments, stack = [], []
    for n, b in enumerate(blocks, start=1):
        seg = {"id": f"s{n:03d}"}
        if b["head"]:
            _, number, title = b["head"]
            depth = number.count(".")
            stack = stack[:depth]
            stack.append(f"{number} {title}")
            seg["number"] = number
            seg["path"] = list(stack)
        else:
            seg["path"] = list(stack)
        seg["disposition"] = "normative"
        seg["text"] = b["text"]
        if b.get("merged"):
            seg["_note"] = (f"{b['merged']} kop(pen) zonder eigen tekst hierin samengevoegd "
                            "- controleer of dit een inhoudsopgave is")
        segments.append(seg)
    return segments


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("canonical")
    ap.add_argument("--heading-regex", default=DEFAULT_HEADING)
    ap.add_argument("--no-merge-empty", action="store_true")
    ap.add_argument("--doc-id", default="CHANGE_ME")
    args = ap.parse_args()

    with open(args.canonical, encoding="utf-8") as fh:
        text = fh.read()

    segments = build_segments(text, re.compile(args.heading_regex),
                              merge_empty=not args.no_merge_empty)

    # Degrading silently is the failure mode that matters here. A document
    # without numbered headings produces one segment of 40.000 characters, the
    # coverage gate reports a cheerful 100%, and nothing looks wrong until
    # someone notices the whole document is a single tile. Say so, in the file
    # as well as on stderr - a stderr line disappears into a `>` redirect.
    warning = None
    if segments and len(text) > 4000:
        biggest = max(len(s["text"]) for s in segments)
        if len(segments) == 1:
            warning = (f"1 segment voor {len(text)} tekens: dit document heeft waarschijnlijk "
                       f"geen genummerde koppen. Geef --heading-regex mee.")
        elif biggest > len(text) * 0.6:
            warning = (f"één segment beslaat {biggest * 100 // len(text)}% van de tekst: de "
                       f"koppenherkenning pakt waarschijnlijk niet de echte structuur.")

    ledger = {
        "document": {
            "id": args.doc_id,
            "title": "CHANGE_ME",
            "source_url": "CHANGE_ME",
            "source_sha256": "zie manifest.yaml",
            "retrieved_at": "zie manifest.yaml",
            "status": "CHANGE_ME: beleidsregel | toelichting | werkinstructie | handboek | faq",
            "canonical": args.canonical.split("/")[-1],
        },
        "segments": segments,
        "statements": [],
    }
    if warning:
        ledger["document"]["_waarschuwing_betegeling"] = warning

    print("---")
    print("# Concept-ledger uit tile.py. Nog te doen per segment: disposition zetten")
    print("# (met reason als hij niet normative is) en de statements schrijven.")
    if warning:
        print(f"# WAARSCHUWING: {warning}")
    print(yaml.safe_dump(ledger, allow_unicode=True, sort_keys=False, width=100), end="")

    covered = sum(len(s["text"]) for s in segments)
    sys.stderr.write(f"segmenten={len(segments)} tekens_in_segmenten={covered} "
                     f"tekens_in_canonical={len(text)}\n")
    if warning:
        sys.stderr.write(f"WAARSCHUWING: {warning}\n")
        return 3
    return 0


if __name__ == "__main__":
    sys.exit(main())
