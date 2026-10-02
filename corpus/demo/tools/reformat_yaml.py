# /// script
# dependencies = ["ruamel.yaml>=0.18"]
# ///
"""Reformat YAML files in corpus/demo to the repository's yamllint rules:
explicit document start, two-space indentation with indented sequences,
single quotes only where needed, and lines at most 125 characters (long
scalars become folded block scalars). Content and comments are preserved; only
the presentation changes.

Usage:
    uv run corpus/demo/tools/reformat_yaml.py corpus/demo
"""

from __future__ import annotations

import sys
from pathlib import Path

from ruamel.yaml import YAML
from ruamel.yaml.comments import CommentedMap, CommentedSeq
from ruamel.yaml.scalarstring import FoldedScalarString, LiteralScalarString, PlainScalarString

MAX_INLINE = 60  # a scalar longer than this becomes a block scalar (folded at 100)


FOLD_AT = 90


def folded(v: str) -> FoldedScalarString:
    """A folded block scalar with explicit fold points at spaces, so lines stay
    under FOLD_AT without relying on the emitter's global width (which would
    also wrap plain scalars and flow collections, leaving trailing spaces)."""
    fs = FoldedScalarString(v)
    positions = []
    col = 0
    last_space = None
    for i, ch in enumerate(v):
        if ch == "\n":
            col = 0
            last_space = None
            continue
        col += 1
        if ch == " ":
            last_space = i
        if col > FOLD_AT and last_space is not None:
            positions.append(last_space)
            col = i - last_space
            last_space = None
    fs.fold_pos = positions
    return fs


def block_scalar(v: str):
    """Multi-line text: literal when every line is short, folded otherwise."""
    if all(len(line) <= FOLD_AT for line in v.split("\n")):
        return LiteralScalarString(v)
    return folded(v)


def fold_long_strings(node, key_width=0):
    if isinstance(node, CommentedMap):
        if node.fa.flow_style() and len(str(dict(node))) > 60:
            node.fa.set_block_style()
        for k, v in node.items():
            if isinstance(v, str):
                if "\n" in v:
                    # Literal blocks keep their long lines, which yamllint's
                    # line-length rule still counts; a folded block re-wraps.
                    node[k] = block_scalar(v)
                elif len(v) > MAX_INLINE:
                    # Also single long tokens (URLs): a plain scalar would wrap
                    # and leave `key: ` with a trailing space.
                    node[k] = folded(v)
            else:
                fold_long_strings(v)
    elif isinstance(node, CommentedSeq):
        # A long flow sequence (`[a, b, c]`) would wrap mid-line with a
        # trailing space; write it as a block sequence instead.
        if node.fa.flow_style() and len(", ".join(str(x) for x in node)) > 60:
            node.fa.set_block_style()
        for i, v in enumerate(node):
            if isinstance(v, str):
                if "\n" in v:
                    node[i] = block_scalar(v)
                elif len(v) > MAX_INLINE:
                    node[i] = folded(v)
            else:
                fold_long_strings(v)


def strip_comments(node):
    """Drop inline comments. Used for profiles.yaml, whose POC comments sit at
    indentations yamllint rejects and carry no information the demo uses."""
    if isinstance(node, (CommentedMap, CommentedSeq)):
        node.ca.items.clear()
        node.ca.comment = None
        if hasattr(node.ca, "end"):
            node.ca.end = None
        for v in node.values() if isinstance(node, CommentedMap) else node:
            strip_comments(v)


def reformat(path: Path) -> None:
    yaml = YAML()
    yaml.preserve_quotes = False
    yaml.explicit_start = True
    yaml.width = 4096  # never wrap on its own; folding is explicit (see folded())
    yaml.indent(mapping=2, sequence=4, offset=2)
    # An explicit `null` instead of an empty cell, which would leave a
    # trailing space after `- `.
    yaml.representer.add_representer(
        type(None), lambda r, _d: r.represent_scalar("tag:yaml.org,2002:null", "null")
    )
    text = path.read_text()
    docs = list(yaml.load_all(text))
    if len(docs) != 1:
        return
    doc = docs[0]
    if path.name == "profiles.yaml":
        strip_comments(doc)
    fold_long_strings(doc)
    with path.open("w") as fh:
        yaml.dump(doc, fh)


def main() -> None:
    root = Path(sys.argv[1])
    files = sorted(root.rglob("*.yaml"))
    for f in files:
        reformat(f)
    print(f"reformatted {len(files)} files")


if __name__ == "__main__":
    main()
