#!/usr/bin/env python3
"""Migrate law YAML files from v0.4.0 to v0.5.0 schema patterns.

Transforms:
- IF when/then/else -> IF cases/default
- SWITCH -> IF (rename operation)
- SUBTRACT_DATE -> AGE
- CONCAT -> ADD
- NOT_EQUALS -> NOT + EQUALS
- NOT_NULL -> wrap subject in NOT + EQUALS null check
- IS_NULL -> EQUALS subject null
- NOT_IN -> NOT + IN
- $schema URL update

Usage:
    python migrate_v04_to_v05.py <file.yaml>             # migrate in-place
    python migrate_v04_to_v05.py <file.yaml> --dry-run    # show diff only
    python migrate_v04_to_v05.py <directory> --recursive   # migrate all YAML files
"""

import argparse
import copy
import sys
from pathlib import Path

try:
    from ruamel.yaml import YAML
except ImportError:
    print(
        "ERROR: ruamel.yaml is required for round-trip YAML formatting.\n"
        "Install with: pip install ruamel.yaml",
        file=sys.stderr,
    )
    sys.exit(1)


OLD_SCHEMA_PATTERNS = [
    "schema/v0.3.0/schema.json",
    "schema/v0.3.1/schema.json",
    "schema/v0.3.2/schema.json",
    "schema/v0.4.0/schema.json",
]
NEW_SCHEMA_URL = (
    "https://raw.githubusercontent.com/MinBZK/regelrecht/"
    "refs/heads/main/schema/v0.5.0/schema.json"
)

# Also fix old repo name
OLD_REPO = "MinBZK/regelrecht-mvp"
NEW_REPO = "MinBZK/regelrecht"


def migrate_schema_url(data):
    """Update $schema URL to v0.5.0."""
    schema = data.get("$schema", "")
    if isinstance(schema, str):
        # Fix old repo name
        if OLD_REPO in schema:
            schema = schema.replace(OLD_REPO, NEW_REPO)
        # Fix old schema version
        for old in OLD_SCHEMA_PATTERNS:
            if old in schema:
                schema = schema.replace(old, "schema/v0.5.0/schema.json")
                break
        data["$schema"] = schema


def migrate_operation(op):
    """Recursively migrate an operation dict from v0.4.0 to v0.5.0 patterns."""
    if not isinstance(op, dict):
        return op

    # Recurse into nested operations
    for key in list(op.keys()):
        val = op[key]
        if isinstance(val, dict):
            migrate_operation(val)
        elif isinstance(val, list):
            for i, item in enumerate(val):
                if isinstance(item, dict):
                    migrate_operation(item)

    operation = op.get("operation")
    if not operation:
        return op

    # SWITCH -> IF (just rename)
    if operation == "SWITCH":
        op["operation"] = "IF"

    # IF when/then/else -> IF cases/default
    elif operation == "IF" and "when" in op:
        when = op.pop("when")
        then = op.pop("then", None)
        else_val = op.pop("else", None)

        case = {"when": when}
        if then is not None:
            case["then"] = then
        op["cases"] = [case]
        if else_val is not None:
            op["default"] = else_val

    # SUBTRACT_DATE -> AGE (when unit is "years")
    elif operation == "SUBTRACT_DATE":
        unit = op.get("unit", "days")
        if unit == "years":
            subject = op.pop("subject", None)
            value = op.pop("value", None)
            op.pop("unit", None)
            op["operation"] = "AGE"
            # In SUBTRACT_DATE: subject - value (subject is later date)
            # In AGE: date_of_birth is earlier, reference_date is later
            if value is not None:
                op["date_of_birth"] = value
            if subject is not None:
                op["reference_date"] = subject
        else:
            # For days/months units, SUBTRACT_DATE has no direct replacement.
            # Leave as-is with a comment marker for manual review.
            op["_migration_note"] = (
                f"SUBTRACT_DATE with unit={unit} has no direct v0.5.0 replacement. "
                "Consider using DATE_ADD with negative values or restructuring the logic."
            )

    # CONCAT -> ADD
    elif operation == "CONCAT":
        op["operation"] = "ADD"

    # NOT_EQUALS -> NOT + EQUALS
    elif operation == "NOT_EQUALS":
        subject = op.pop("subject", None)
        value = op.pop("value", None)
        op["operation"] = "NOT"
        inner = {"operation": "EQUALS"}
        if subject is not None:
            inner["subject"] = subject
        if value is not None:
            inner["value"] = value
        op["value"] = inner
        # Remove the old fields that don't belong on NOT
        op.pop("subject", None)

    # IS_NULL -> EQUALS subject null
    elif operation == "IS_NULL":
        subject = op.pop("subject", None)
        op["operation"] = "EQUALS"
        if subject is not None:
            op["subject"] = subject
        op["value"] = None

    # NOT_NULL -> NOT + EQUALS null
    elif operation == "NOT_NULL":
        subject = op.pop("subject", None)
        op["operation"] = "NOT"
        inner = {"operation": "EQUALS"}
        if subject is not None:
            inner["subject"] = subject
        inner["value"] = None
        op["value"] = inner

    # NOT_IN -> NOT + IN
    elif operation == "NOT_IN":
        subject = op.pop("subject", None)
        value = op.pop("value", None)
        values = op.pop("values", None)
        op["operation"] = "NOT"
        inner = {"operation": "IN"}
        if subject is not None:
            inner["subject"] = subject
        if value is not None:
            inner["value"] = value
        if values is not None:
            inner["values"] = values
        op["value"] = inner

    return op


def migrate_actions(actions):
    """Migrate a list of action dicts."""
    if not isinstance(actions, list):
        return
    for action in actions:
        if not isinstance(action, dict):
            continue
        # Migrate the value field if it's an operation
        if "value" in action and isinstance(action["value"], dict):
            migrate_operation(action["value"])
        # Migrate top-level operation on the action (Pattern 2 in old schema)
        if "operation" in action:
            migrate_operation(action)


def migrate_article(article):
    """Migrate a single article's machine_readable section."""
    mr = article.get("machine_readable", {})
    if not mr:
        return

    execution = mr.get("execution", {})
    if not execution:
        return

    actions = execution.get("actions", [])
    migrate_actions(actions)

    # Also migrate open_terms defaults
    for ot in mr.get("open_terms", []):
        if isinstance(ot, dict) and "default" in ot:
            default = ot["default"]
            if isinstance(default, dict) and "actions" in default:
                migrate_actions(default["actions"])


def migrate_file(path, dry_run=False):
    """Migrate a single YAML file. Returns True if changes were made."""
    yaml = YAML()
    yaml.preserve_quotes = True
    yaml.width = 120

    with open(path) as f:
        original = f.read()

    data = yaml.load(original)
    if data is None:
        return False

    # Migrate schema URL
    migrate_schema_url(data)

    # Migrate articles
    for article in data.get("articles", []):
        migrate_article(article)

    # Migrate preamble if it has machine_readable
    preamble = data.get("preamble", {})
    if isinstance(preamble, dict) and "machine_readable" in preamble:
        migrate_article({"machine_readable": preamble["machine_readable"]})

    # Write back
    from io import StringIO

    buf = StringIO()
    yaml.dump(data, buf)
    new_content = buf.getvalue()

    if new_content == original:
        return False

    if dry_run:
        print(f"--- {path}")
        print(f"+++ {path} (migrated)")
        # Simple diff
        old_lines = original.splitlines()
        new_lines = new_content.splitlines()
        for i, (old, new) in enumerate(zip(old_lines, new_lines)):
            if old != new:
                print(f"  Line {i + 1}:")
                print(f"    - {old}")
                print(f"    + {new}")
        if len(new_lines) > len(old_lines):
            for line in new_lines[len(old_lines) :]:
                print(f"    + {line}")
        print()
    else:
        with open(path, "w") as f:
            f.write(new_content)

    return True


def main():
    parser = argparse.ArgumentParser(
        description="Migrate law YAML files from v0.4.0 to v0.5.0 schema"
    )
    parser.add_argument("path", help="YAML file or directory to migrate")
    parser.add_argument(
        "--dry-run", action="store_true", help="Show changes without writing"
    )
    parser.add_argument(
        "--recursive",
        "-r",
        action="store_true",
        help="Recursively process directories",
    )
    args = parser.parse_args()

    path = Path(args.path)

    if path.is_file():
        files = [path]
    elif path.is_dir() and args.recursive:
        files = sorted(path.rglob("*.yaml"))
    elif path.is_dir():
        files = sorted(path.glob("*.yaml"))
    else:
        print(f"ERROR: {path} is not a file or directory", file=sys.stderr)
        sys.exit(1)

    migrated = 0
    skipped = 0

    for f in files:
        if migrate_file(f, dry_run=args.dry_run):
            migrated += 1
            action = "would migrate" if args.dry_run else "migrated"
            print(f"  {action}: {f}", file=sys.stderr)
        else:
            skipped += 1

    print(
        f"\nTotal: {len(files)} files, {migrated} migrated, {skipped} unchanged",
        file=sys.stderr,
    )


if __name__ == "__main__":
    main()
