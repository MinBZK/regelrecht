#!/usr/bin/env python3
"""Every `examples` value must be valid against the subschema it sits on.

The reference page renders these as copyable YAML, so an invalid example is not
a cosmetic slip: a law author copies the official documentation and gets a file
that `just validate` rejects. Two shipped that way before this check existed, a
`gemeente_code` of "0363" against `^GM[0-9]{4}$` and an `open_terms` entry keyed
`name` where the schema requires `id`.

Both hid in the same blind spot. `examples` sits on the array node while
`required` and `additionalProperties` live on `items`, so reading the example
next to its own description shows nothing wrong; you have to walk into `items`
to see it.

Deliberately stdlib-only, like script/cross-law-integriteit.py: it runs in CI
and as a pre-commit hook, and a JSON-Schema library is a dependency neither of
those wants. It therefore checks the keywords that catch real authoring
mistakes rather than implementing draft-07:

    type, enum, const, pattern, required, additionalProperties, items,
    properties, minimum, maximum

Runs over every schema/vX.Y.Z/, not only the latest: a released version is
immutable, so an example that is wrong there stays wrong.

Usage: python3 script/check-schema-examples.py
"""

import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SCHEMA_DIR = ROOT / "schema"

TYPES = {
    "string": str,
    "number": (int, float),
    "integer": int,
    "boolean": bool,
    "object": dict,
    "array": list,
    "null": type(None),
}


def type_ok(value, expected) -> bool:
    names = expected if isinstance(expected, list) else [expected]
    for name in names:
        py = TYPES.get(name)
        if py is None:
            return True  # unknown type name: not ours to judge
        # bool is an int in Python; JSON Schema keeps them apart.
        if name in ("number", "integer") and isinstance(value, bool):
            continue
        if name == "boolean" and not isinstance(value, bool):
            continue
        if isinstance(value, py):
            return True
    return False


def resolve(node, defs):
    """Follow a local $ref one hop, so `items: {$ref: ...}` is checkable."""
    ref = node.get("$ref") if isinstance(node, dict) else None
    if not ref:
        return node
    m = re.match(r"^#/definitions/(.+)$", ref)
    if not m:
        return node
    return defs.get(m.group(1), node)


def check(value, schema, defs, path, problems, depth=0):
    # The budget counts schema hops ($ref, oneOf, allOf, items, properties),
    # not data nesting, and an operation costs roughly five or six of them. At
    # 12 the walk stopped after about two levels of nested operations and waved
    # the rest through, which is the one way a guard like this does real harm.
    # 200 is far past anything the schema can express and still bounds a cycle.
    if depth > 200 or not isinstance(schema, dict):
        return
    schema = resolve(schema, defs)

    # A oneOf/anyOf passes when any branch does; report nothing when one holds.
    # This treats `oneOf` as `anyOf`: a value matching two branches is strictly
    # invalid and passes here. Deliberate. The alternative is false positives on
    # the schema's own unions, which overlap by construction (a bare number is
    # both `operationValue`'s literal branch and, for a reader, several others),
    # and this guard exists to catch authoring mistakes, not to be a validator.
    for key in ("oneOf", "anyOf"):
        branches = schema.get(key)
        if isinstance(branches, list) and branches:
            for branch in branches:
                trial = []
                check(value, branch, defs, path, trial, depth + 1)
                if not trial:
                    return
            problems.append(f"{path}: matches no {key} branch")
            return

    for branch in schema.get("allOf", []) or []:
        check(value, branch, defs, path, problems, depth + 1)

    # `not` matters here for one reason: operationValue's literal-string branch
    # is `string` + `not: {pattern: "^\\$"}`, so without it a malformed
    # variable reference falls through as a plain string and passes.
    if isinstance(schema.get("not"), dict):
        inner = []
        check(value, schema["not"], defs, path, inner, depth + 1)
        if not inner:
            problems.append(f"{path}: {value!r} matches a `not` subschema")

    if "type" in schema and not type_ok(value, schema["type"]):
        problems.append(f"{path}: expected type {schema['type']}, got {type(value).__name__}")
        return

    if "const" in schema and value != schema["const"]:
        problems.append(f"{path}: expected const {schema['const']!r}")
    if "enum" in schema and value not in schema["enum"]:
        problems.append(f"{path}: {value!r} not in enum {schema['enum']}")

    if isinstance(value, str) and "pattern" in schema:
        if not re.search(schema["pattern"], value):
            problems.append(f"{path}: {value!r} does not match {schema['pattern']!r}")

    if isinstance(value, (int, float)) and not isinstance(value, bool):
        if "minimum" in schema and value < schema["minimum"]:
            problems.append(f"{path}: {value} below minimum {schema['minimum']}")
        if "maximum" in schema and value > schema["maximum"]:
            problems.append(f"{path}: {value} above maximum {schema['maximum']}")

    if isinstance(value, dict):
        for name in schema.get("required", []) or []:
            if name not in value:
                problems.append(f"{path}: missing required property {name!r}")
        props = schema.get("properties") or {}
        if schema.get("additionalProperties") is False:
            for name in value:
                if name not in props:
                    problems.append(f"{path}: property {name!r} is not allowed")
        for name, child in value.items():
            if name in props:
                check(child, props[name], defs, f"{path}.{name}", problems, depth + 1)

    if isinstance(value, list):
        items = schema.get("items")
        if isinstance(items, dict):
            for i, child in enumerate(value):
                check(child, items, defs, f"{path}[{i}]", problems, depth + 1)


def walk(node, defs, pointer, out):
    """Yield (pointer, node) for every node carrying an `examples` array."""
    if isinstance(node, list):
        for i, child in enumerate(node):
            walk(child, defs, f"{pointer}/{i}", out)
        return
    if not isinstance(node, dict):
        return
    if isinstance(node.get("examples"), list):
        out.append((pointer, node))
    for key, child in node.items():
        # Never descend into an examples array: its contents are data, and a
        # law snippet that happens to carry an `examples` key would otherwise
        # be mistaken for a schema node.
        if key == "examples":
            continue
        walk(child, defs, f"{pointer}/{key}", out)


def main() -> int:
    if not SCHEMA_DIR.is_dir():
        print("check-schema-examples: schema/ not present, skipping")
        return 0

    versions = sorted(d for d in SCHEMA_DIR.iterdir() if re.match(r"^v\d+\.\d+\.\d+$", d.name))
    problems, checked = [], 0

    for version in versions:
        f = version / "schema.json"
        if not f.is_file():
            continue
        schema = json.loads(f.read_text())
        defs = schema.get("definitions", {})

        carriers = []
        walk(schema, defs, "#", carriers)
        for pointer, node in carriers:
            subschema = {k: v for k, v in node.items() if k != "examples"}
            for i, example in enumerate(node["examples"]):
                checked += 1
                found = []
                check(example, subschema, defs, "example", found)
                for p in found:
                    problems.append(f"{version.name} {pointer}/examples[{i}]: {p}")

    if problems:
        print("check-schema-examples FAILED: an example is invalid against its own subschema:")
        for p in problems:
            print("  - " + p)
        print(
            "\nThese render as copyable YAML on /reference/schema, so a law author who "
            "copies one gets a file that fails `just validate`."
        )
        return 1

    print(f"check-schema-examples passed: {checked} example(s) across {len(versions)} schema version(s).")
    return 0


if __name__ == "__main__":
    sys.exit(main())
