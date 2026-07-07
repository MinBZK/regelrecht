#!/usr/bin/env python3
"""Migrate soft-wrapped law text from literal (|-) to folded (>-) block scalars.

The harvester used to wrap article/preamble text at 115 chars with real
newlines inside literal block scalars, so the cosmetic wrapping became real
``\\n`` characters in the loaded data. The harvester now emits folded (``>-``)
blocks for pure prose (see packages/harvester/src/yaml/writer.rs); this script
performs the matching one-off migration of existing corpus files.

Per text field (``preamble.text`` and ``articles[*].text``) the wrap newlines
are joined back to spaces and the scalar is re-emitted in folded style, with
the original wrap points as fold positions — so the diff is exactly
``|-`` -> ``>-`` plus a doubled blank line per paragraph break.

A field is left untouched (literal) when folding would not be provably safe:

- it contains markdown reference definitions (``[refN]: url`` lines are
  line-oriented; folding would join them),
- any line starts with whitespace (more-indented lines change folded-scalar
  semantics),
- it contains runs of 2+ blank lines, or starts/ends with a blank line,
- any single line break is not provably wrap-forced at width 115 (then it is
  a deliberate hard break, e.g. a markdown list).

Before writing anything the script verifies its own output: the dumped YAML
is reloaded and every migrated field must equal the computed unwrapped text,
every skipped field must be unchanged, the documents must be deep-equal
otherwise, and the transform must be idempotent. Any mismatch aborts without
writing.

Usage:
    python script/migrate_text_to_folded.py [paths...]        # default: corpus/regulation
    python script/migrate_text_to_folded.py --dry-run          # show diffs, write nothing
"""

import argparse
import difflib
import io
import re
import sys
from pathlib import Path

try:
    from ruamel.yaml import YAML
    from ruamel.yaml.scalarstring import FoldedScalarString, LiteralScalarString
except ImportError:
    print(
        "ERROR: ruamel.yaml is required for round-trip YAML formatting.\n"
        "Install with: pip install ruamel.yaml",
        file=sys.stderr,
    )
    sys.exit(1)

# Must match TEXT_WRAP_WIDTH in packages/harvester/src/config.rs.
WRAP_WIDTH = 115

REF_DEFINITION_RE = re.compile(r"^\[ref\d+\]: ")


def make_yaml():
    yaml = YAML()
    yaml.preserve_quotes = True
    yaml.explicit_start = True
    yaml.width = 4096  # never re-flow scalars we did not touch
    yaml.indent(mapping=2, sequence=4, offset=2)
    return yaml


def unwrap_field(value: str):
    """Return (unwrapped, fold_positions) if the field is provably safe to
    fold, else None.

    Safe means: every single line break is a cosmetic wrap at WRAP_WIDTH and
    every blank line is exactly one paragraph break. ``fold_positions`` are
    the character offsets (into the unwrapped string) of the original wrap
    points, so the folded emission reproduces the original line layout.
    """
    if "\n" not in value:
        return None

    lines = value.split("\n")
    if lines[0].strip() == "" or lines[-1].strip() == "":
        return None
    for line in lines:
        if REF_DEFINITION_RE.match(line):
            return None
        if line.startswith((" ", "\t")) or line != line.rstrip():
            return None
    # Runs of 2+ blank lines cannot round-trip within yamllint's
    # empty-lines: max 2 (a \n\n\n needs three blank lines when folded).
    if "\n\n\n" in value:
        return None

    paragraphs = value.split("\n\n")
    unwrapped_paragraphs = []
    fold_positions = []
    offset = 0
    for p, paragraph in enumerate(paragraphs):
        if p > 0:
            offset += 2  # the \n\n paragraph separator
        plines = paragraph.split("\n")
        for a, b in zip(plines, plines[1:]):
            # The break must have been wrap-forced: line a plus the first
            # word of line b would not have fit within WRAP_WIDTH. If it
            # would have fit, the newline was a deliberate hard break.
            first_word = b.split(" ", 1)[0]
            if len(a) + 1 + len(first_word) <= WRAP_WIDTH:
                return None
        for i, pline in enumerate(plines):
            if i > 0:
                fold_positions.append(offset - 1)  # position of the joining space
            offset += len(pline) + 1  # +1 for the space/newline that follows
        offset -= 1  # last line of the paragraph has no separator yet
        unwrapped_paragraphs.append(" ".join(plines))
    return "\n\n".join(unwrapped_paragraphs), fold_positions


def iter_text_fields(data):
    """Yield (container, key, path) for every text field in scope."""
    preamble = data.get("preamble")
    if isinstance(preamble, dict) and isinstance(preamble.get("text"), str):
        yield preamble, "text", "preamble.text"
    for i, article in enumerate(data.get("articles") or []):
        if isinstance(article, dict) and isinstance(article.get("text"), str):
            yield article, "text", f"articles[{i}].text"


def transform(data):
    """Fold eligible text fields in-place. Returns {path: unwrapped}.

    Only literal (``|-``) blocks are candidates — plain scalars have no
    newlines and existing folded blocks are already in the target style.
    """
    migrated = {}
    for container, key, path in iter_text_fields(data):
        if not isinstance(container[key], LiteralScalarString):
            continue
        result = unwrap_field(str(container[key]))
        if result is None:
            continue
        unwrapped, fold_positions = result
        scalar = FoldedScalarString(unwrapped)
        scalar.fold_pos = fold_positions
        container[key] = scalar
        migrated[path] = unwrapped
    return migrated


def dump(yaml, data) -> str:
    buf = io.StringIO()
    yaml.dump(data, buf)
    return buf.getvalue()


def verify(original_text: str, new_text: str, migrated: dict, path: Path):
    """Fail loudly (exit 1) unless the migration round-trips exactly."""
    yaml = make_yaml()
    original = yaml.load(original_text)
    reloaded = yaml.load(new_text)

    fields_old = {p: str(c[k]) for c, k, p in iter_text_fields(original)}
    fields_new = {p: str(c[k]) for c, k, p in iter_text_fields(reloaded)}
    if set(fields_old) != set(fields_new):
        fail(path, "text field set changed", fields_old.keys(), fields_new.keys())
    for field_path, old_value in fields_old.items():
        expected = migrated.get(field_path, old_value)
        actual = fields_new[field_path]
        if actual != expected:
            diff = "\n".join(
                difflib.unified_diff(
                    expected.split("\n"), actual.split("\n"), "expected", "actual", lineterm=""
                )
            )
            fail(path, f"round-trip mismatch in {field_path}", diff)

    # Everything except the migrated text fields must be untouched.
    for container, key, field_path in iter_text_fields(original):
        if field_path in migrated:
            container[key] = migrated[field_path]
    plain = YAML(typ="safe")
    if plain.load(dump(make_yaml(), original)) != plain.load(new_text):
        fail(path, "document content drifted outside text fields")

    # Idempotency: transforming the migrated document again must reproduce
    # the same output text.
    transform(reloaded)
    if dump(make_yaml(), reloaded) != new_text:
        fail(path, "transform is not idempotent on its own output")


def fail(path: Path, message: str, *details):
    print(f"ERROR: {path}: {message}", file=sys.stderr)
    for detail in details:
        print(detail, file=sys.stderr)
    sys.exit(1)


def migrate_file(path: Path, dry_run: bool) -> bool:
    original_text = path.read_text(encoding="utf-8")
    yaml = make_yaml()
    data = yaml.load(original_text)
    if not isinstance(data, dict):
        return False

    migrated = transform(data)
    if not migrated:
        return False

    new_text = dump(yaml, data)
    verify(original_text, new_text, migrated, path)

    if dry_run:
        sys.stdout.writelines(
            difflib.unified_diff(
                original_text.splitlines(keepends=True),
                new_text.splitlines(keepends=True),
                str(path),
                str(path),
            )
        )
    else:
        path.write_text(new_text, encoding="utf-8")
    print(f"{'would migrate' if dry_run else 'migrated'}: {path} ({len(migrated)} fields)")
    return True


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("paths", nargs="*", default=["corpus/regulation"], type=Path)
    parser.add_argument("--dry-run", action="store_true", help="show diffs, write nothing")
    args = parser.parse_args()

    files = []
    for p in args.paths:
        p = Path(p)
        if p.is_dir():
            files.extend(sorted(p.rglob("*.yaml")))
        else:
            files.append(p)

    changed = sum(migrate_file(f, args.dry_run) for f in files)
    print(f"{changed}/{len(files)} files {'would be ' if args.dry_run else ''}migrated")


if __name__ == "__main__":
    main()
