# /// script
# dependencies = ["ruamel.yaml>=0.18"]
# ///
"""Rewrite the POC's implicit list mapping in the demo laws.

The POC engine read `$lijst.veld` on a list as "the `veld` of every element", a
map. The regelrecht engine reads a property path on an array as an error
("expected object, got array"); mapping is spelled FOREACH (RFC-016), which
returns the body values as an array when it has no `combine`. This tool turns
every action value of the form `$X.veld`, where `X` is an array (a declared
array input/output/parameter, or an intermediate output produced by a FOREACH
without combine, a LIST, or an alias of one), into

    operation: FOREACH
    collection: $X
    as: item
    body: $item.veld

Property paths in other positions (comparison subjects, SWITCH cases) are only
reported: the POC semantics there ("is the list IN ..."?) have no single
mechanical equivalent.

The second rule concerns EXISTS. The POC engine read `EXISTS $lijst` as "the
list is not empty" (an empty list is falsy in Python); the migration rewrote
every EXISTS to `NOT (EQUALS $x null)`, which is true for an empty list. For an
array `X` that form becomes

    operation: GREATER_THAN
    subject: {operation: FOREACH, collection: $X, body: 1, combine: ADD}
    value: 0

which is false for both null and the empty list, as EXISTS was. Scalars keep
the null test.

Idempotent. Usage:

    uv run corpus/demo/tools/rewrite_array_property_access.py corpus/demo/regulation/nl
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

from ruamel.yaml import YAML
from ruamel.yaml.comments import CommentedMap, CommentedSeq

yaml = YAML()
yaml.preserve_quotes = True
yaml.width = 4096
yaml.indent(mapping=2, sequence=2, offset=0)

PATH_RE = re.compile(r"^\$([a-z_][a-z0-9_]*)\.([a-z_][a-z0-9_.]*)$")
COMMENT = "POC: impliciete map over een lijst ($%s.%s); expliciet als FOREACH (RFC-016)"


def array_names(execution: CommentedMap) -> set[str]:
    names: set[str] = set()
    for key in ("input", "output", "parameters"):
        for item in execution.get(key) or []:
            if item.get("type") == "array":
                names.add(item["name"])
    grew = True
    while grew:
        grew = False
        for action in execution.get("actions") or []:
            out = action.get("output")
            if not out or out in names:
                continue
            value = action.get("value")
            op = value.get("operation") if isinstance(value, dict) else None
            is_array = (
                (op == "FOREACH" and not value.get("combine"))
                or op == "LIST"
                or (isinstance(value, str) and value.startswith("$") and value[1:] in names)
            )
            if is_array:
                names.add(out)
                grew = True
    return names


def foreach_map(base: str, field: str) -> CommentedMap:
    node = CommentedMap()
    node["operation"] = "FOREACH"
    node["collection"] = f"${base}"
    node["as"] = "item"
    node["body"] = f"$item.{field}"
    return node


EXISTS_COMMENT = "POC: EXISTS op lijst %s (uitvoer %s) betekende 'niet leeg'; expliciet als telling (RFC-016)"
# Comment lines an earlier version of this tool wrote (without the output name, or at column 0).
OLD_EXISTS_COMMENT = re.compile(r"^\s*# POC: EXISTS op (een )?lijst (?!.*\(uitvoer ).*$")


def count_of(base: str) -> CommentedMap:
    node = CommentedMap()
    node["operation"] = "FOREACH"
    node["collection"] = f"${base}"
    node["body"] = 1
    node["combine"] = "ADD"
    return node


def not_empty(base: str) -> CommentedMap:
    node = CommentedMap()
    node["operation"] = "GREATER_THAN"
    node["subject"] = count_of(base)
    node["value"] = 0
    return node


def is_exists_on_array(node, arrays: set[str]) -> str | None:
    """`NOT (EQUALS $X null)` with X an array -> X, else None."""
    if not isinstance(node, CommentedMap) or node.get("operation") != "NOT":
        return None
    inner = node.get("value")
    if not isinstance(inner, CommentedMap) or inner.get("operation") != "EQUALS":
        return None
    if inner.get("value") is not None or "value" not in inner:
        return None
    subject = inner.get("subject")
    if isinstance(subject, str) and subject.startswith("$") and subject[1:] in arrays:
        return subject[1:]
    return None


def is_count_gt_zero(node) -> str | None:
    """The already-rewritten form (see `not_empty`) -> X, else None."""
    if not isinstance(node, CommentedMap) or node.get("operation") != "GREATER_THAN" or node.get("value") != 0:
        return None
    subject = node.get("subject")
    if not isinstance(subject, CommentedMap) or subject.get("operation") != "FOREACH":
        return None
    if subject.get("body") != 1 or subject.get("combine") != "ADD":
        return None
    collection = subject.get("collection")
    return collection[1:] if isinstance(collection, str) and collection.startswith("$") else None


def rewrite_exists(node, arrays: set[str], rewritten: list[str], already: list[str]) -> None:
    """Replace every EXISTS-on-array form inside `node`, in place; record what
    was rewritten now and what an earlier run had rewritten already."""
    if isinstance(node, CommentedSeq):
        items = list(enumerate(node))
    elif isinstance(node, CommentedMap):
        items = list(node.items())
    else:
        return
    for key, child in items:
        base = is_exists_on_array(child, arrays)
        if base:
            node[key] = not_empty(base)
            rewritten.append(base)
            continue
        done = is_count_gt_zero(child)
        if done:
            already.append(done)
            continue
        rewrite_exists(child, arrays, rewritten, already)


def add_comment(node: CommentedMap, key: str, text: str, indent: int, original: str) -> bool:
    """Attach `text` as a comment before `key` unless the file already carries it."""
    if text in original:
        return False
    node.yaml_set_comment_before_after_key(key, before=text, indent=indent)
    return True


def report_other_positions(node, arrays: set[str], path: str, out: list[str]) -> None:
    if isinstance(node, str):
        m = PATH_RE.match(node)
        if m and m.group(1) in arrays:
            out.append(f"{path}: {node}")
    elif isinstance(node, CommentedSeq):
        for i, item in enumerate(node):
            report_other_positions(item, arrays, f"{path}[{i}]", out)
    elif isinstance(node, CommentedMap):
        for k, v in node.items():
            report_other_positions(v, arrays, f"{path}.{k}", out)


def rewrite_file(path: Path) -> tuple[int, list[str]]:
    original = path.read_text()
    doc = yaml.load(original)
    rewritten = 0
    leftovers: list[str] = []
    for article in doc.get("articles") or []:
        execution = (article.get("machine_readable") or {}).get("execution")
        if not execution:
            continue
        arrays = array_names(execution)
        for action in execution.get("actions") or []:
            value = action.get("value")
            # Rule 1: `$X.veld` as the whole action value -> FOREACH map.
            if isinstance(value, str):
                m = PATH_RE.match(value)
                if m and m.group(1) in arrays:
                    base, field = m.group(1), m.group(2)
                    action["value"] = foreach_map(base, field)
                    add_comment(action, "value", COMMENT % (base, field), 8, original)
                    rewritten += 1
                    value = action["value"]
            # Rule 2: EXISTS on an array, at the top or nested anywhere in the value.
            exists_now: list[str] = []
            exists_before: list[str] = []
            base = is_exists_on_array(value, arrays)
            if base:
                action["value"] = not_empty(base)
                exists_now.append(base)
            elif is_count_gt_zero(value):
                exists_before.append(is_count_gt_zero(value))
            else:
                rewrite_exists(value, arrays, exists_now, exists_before)
            rewritten += len(exists_now)
            bases = sorted(set(exists_now + exists_before))
            if bases:
                text = EXISTS_COMMENT % (", ".join(f"${b}" for b in bases), action.get("output"))
                if add_comment(action, "value", text, 8, original):
                    rewritten += 1
            report_other_positions(action.get("value"), arrays, f"article {article.get('number')} action {action.get('output')}", leftovers)
        req_now: list[str] = []
        req_before: list[str] = []
        rewrite_exists(execution.get("requirements"), arrays, req_now, req_before)
        rewritten += len(req_now)
        req_bases = sorted(set(req_now + req_before))
        if req_bases:
            text = EXISTS_COMMENT % (", ".join(f"${b}" for b in req_bases), "requirements")
            if add_comment(execution, "requirements", text, 6, original):
                rewritten += 1
        report_other_positions(execution.get("requirements"), arrays, f"article {article.get('number')} requirements", leftovers)
    if rewritten:
        with path.open("w") as handle:
            yaml.dump(doc, handle)
    # Drop the comment lines an earlier version of this tool wrote.
    text = path.read_text()
    kept = [line for line in text.split("\n") if not OLD_EXISTS_COMMENT.match(line)]
    if len(kept) != len(text.split("\n")):
        path.write_text("\n".join(kept))
        rewritten += 1
    return rewritten, leftovers


def main() -> None:
    if len(sys.argv) != 2:
        sys.exit("usage: rewrite_array_property_access.py corpus/demo/regulation/nl")
    root = Path(sys.argv[1])
    total = 0
    for path in sorted(root.rglob("*.yaml")):
        rewritten, leftovers = rewrite_file(path)
        total += rewritten
        if rewritten:
            print(f"{path}: {rewritten} rewritten")
        for line in leftovers:
            print(f"{path}: NOT rewritten (needs a per-law decision): {line}")
    print(f"total rewritten: {total}")


if __name__ == "__main__":
    main()
