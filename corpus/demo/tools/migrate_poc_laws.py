# /// script
# dependencies = ["ruamel.yaml>=0.18"]
# ///
"""Migrate the poc-machine-law corpus (schema v0.5.1 dialect) to the regelrecht
schema (v0.5.7) as used by this repository.

The POC dialect differed from the canonical schema in three ways:

1. Cross-law inputs carried a `service` (the organisation running the referenced
   law). The law id already identifies the referenced law, so the key is dropped.
2. External data was bound *inside* the YAML (`source.table`, `field`, `fields`,
   `select_on`, `source_type`). The canonical schema says `source: {}` - the data
   is resolved outside the YAML. The binding moves to `bindings.yaml`, a sidecar
   the demo application uses to materialise persona data per law input.
3. A handful of operations existed only in the POC's engine fork (EXISTS, IS_NULL,
   NOT_IN, NOT_NULL, NOT_EQUALS, CONCAT, LENGTH, SUBTRACT_DATE, STATIC, GET,
   COMBINE_DATETIME). They are rewritten to canonical operations; the three that
   have no mechanical rewrite (STATIC, GET, COMBINE_DATETIME) are handled per law.

Usage:
    uv run corpus/demo/tools/migrate_poc_laws.py <poc-checkout> corpus/demo
    uv run corpus/demo/tools/reformat_yaml.py corpus/demo   # yamllint layout
"""

from __future__ import annotations

import io
import sys
from pathlib import Path

from ruamel.yaml import YAML
from ruamel.yaml.comments import CommentedMap, CommentedSeq

SCHEMA_URL = (
    "https://raw.githubusercontent.com/MinBZK/regelrecht/refs/tags/schema-v0.5.7/schema/v0.5.7/schema.json"
)

BINDING_KEYS = ("table", "field", "fields", "select_on", "source_type", "as_array")

yaml = YAML()
yaml.preserve_quotes = True
yaml.width = 4096
yaml.indent(mapping=2, sequence=2, offset=0)

stats: dict[str, int] = {}


def bump(key: str) -> None:
    stats[key] = stats.get(key, 0) + 1


def cm(**kwargs) -> CommentedMap:
    m = CommentedMap()
    for k, v in kwargs.items():
        m[k] = v
    return m


def equals_null(subject) -> CommentedMap:
    return cm(operation="EQUALS", subject=subject, value=None)


def not_op(value) -> CommentedMap:
    return cm(operation="NOT", value=value)


def rewrite_operation(node: CommentedMap) -> CommentedMap | None:
    """Return a replacement node for a POC-only operation, or None to keep it."""
    op = node.get("operation")
    if op == "EXISTS":
        bump("EXISTS")
        return not_op(equals_null(node["subject"]))
    if op == "IS_NULL":
        bump("IS_NULL")
        return equals_null(node["subject"])
    if op == "NOT_NULL":
        bump("NOT_NULL")
        return not_op(equals_null(node["subject"]))
    if op == "NOT_IN":
        bump("NOT_IN")
        inner = CommentedMap(node)
        inner["operation"] = "IN"
        return not_op(inner)
    if op == "NOT_EQUALS":
        bump("NOT_EQUALS")
        inner = CommentedMap(node)
        inner["operation"] = "EQUALS"
        return not_op(inner)
    if op == "CONCAT":
        bump("CONCAT")
        node["operation"] = "ADD"
        return None
    if op == "LENGTH":
        bump("LENGTH")
        return cm(operation="FOREACH", collection=node["subject"], body=1, combine="ADD")
    if op == "SUBTRACT_DATE":
        bump("SUBTRACT_DATE")
        values = node["values"]
        unit = node.get("unit", "days")
        if unit == "hours":
            raise ValueError("SUBTRACT_DATE in hours needs a per-law rewrite")
        replacement = cm(operation="DATE_DIFF")
        replacement["from"] = values[1]
        replacement["to"] = values[0]
        replacement["in"] = unit
        for key in ("legal_basis",):
            if key in node:
                replacement[key] = node[key]
        return replacement
    return None


COMPARISON_OPS = {"EQUALS", "NOT_EQUALS", "IN", "NOT_IN", "GREATER_THAN", "LESS_THAN",
                  "GREATER_THAN_OR_EQUAL", "LESS_THAN_OR_EQUAL"}


def list_literal(items) -> CommentedMap:
    """An array literal is not an `operationValue`; the schema spells it LIST."""
    bump("array literal -> LIST")
    return cm(operation="LIST", items=items)


def normalise_value_slots(node: CommentedMap) -> None:
    """Rewrite value positions that the POC filled with raw arrays or a bare
    `{value: null}` object into canonical operation values."""
    is_action = "output" in node and "operation" not in node
    for key in ("then", "default"):
        if key in node and isinstance(node[key], CommentedSeq):
            node[key] = list_literal(node[key])
    if "default" in node and isinstance(node["default"], CommentedMap):
        d = node["default"]
        if list(d.keys()) == ["value"] and d["value"] is None:
            node["default"] = None
            bump("default {value: null} -> null")
    if "value" in node and isinstance(node["value"], CommentedSeq):
        if is_action or node.get("operation") in COMPARISON_OPS:
            node["value"] = list_literal(node["value"])


def walk(node):
    """Depth-first rewrite of every operation node in place."""
    if isinstance(node, CommentedMap):
        normalise_value_slots(node)
        for key in list(node.keys()):
            child = node[key]
            if isinstance(child, CommentedMap) and "operation" in child:
                replacement = rewrite_operation(child)
                if replacement is not None:
                    node[key] = replacement
                    child = replacement
            walk(child)
    elif isinstance(node, CommentedSeq):
        for i, child in enumerate(list(node)):
            if isinstance(child, CommentedMap) and "operation" in child:
                replacement = rewrite_operation(child)
                if replacement is not None:
                    node[i] = replacement
                    child = replacement
            walk(child)


def migrate_inputs(law: CommentedMap, bindings: dict) -> None:
    law_id = law["$id"]
    law_service = law.get("service")
    for article in law.get("articles", []):
        execution = (article.get("machine_readable") or {}).get("execution") or {}
        for inp in execution.get("input", []) or []:
            source = inp.get("source")
            if source is None:
                inp["source"] = CommentedMap()
                inp["source"].fa.set_flow_style()
                bump("input without source -> source: {}")
                continue
            if "regulation" in source:
                if "service" in source:
                    del source["service"]
                    bump("crosslaw service dropped")
                continue
            if any(k in source for k in BINDING_KEYS):
                binding = {}
                for k in BINDING_KEYS:
                    if k in source:
                        binding[k] = plain(source[k])
                source_type = binding.pop("source_type", None)
                if source_type in ("claim", "cases", "events", "laws", "reference_data"):
                    binding["kind"] = source_type
                    binding["service"] = law_service
                else:
                    binding["kind"] = "table"
                    binding["service"] = source_type or law_service
                    absent = absent_heuristic(inp.get("type"))
                    if absent is not MISSING:
                        binding["absent"] = absent
                law_bindings = bindings.setdefault(law_id, {})
                existing = law_bindings.get(inp["name"])
                if existing is not None and existing != binding:
                    print(f"  ! conflicting binding for {law_id}.{inp['name']}: {existing} vs {binding}")
                law_bindings[inp["name"]] = binding
                inp["source"] = CommentedMap()
                inp["source"].fa.set_flow_style()
                bump("table binding moved to sidecar")
            # An empty `source: {}` stays as is.


MISSING = object()


def absent_heuristic(input_type: str | None):
    """The first-pass `absent:` for a table binding (RFC-036): what a missing
    register row means for the input. This reproduces the type heuristic the
    materialiser applied before RFC-036 (an amount or number counted as 0, any
    other scalar as null) so that the choice is written down where it can be
    reviewed; the reviewed deviations live in bindings.yaml with a comment and
    are not regenerated. An array input is the list of matching rows and has no
    `absent`."""
    if input_type == "array":
        return MISSING
    if input_type in ("amount", "number"):
        return 0
    return None


def strip_temporal_reference(law: CommentedMap) -> None:
    """`temporal.reference` matches two branches of a schema `oneOf` (an enum and
    the generic variable reference) and is therefore rejected; the engine does not
    read it. Keep the rest of the temporal block."""
    for article in law.get("articles", []):
        execution = (article.get("machine_readable") or {}).get("execution") or {}
        for field in list(execution.get("input", []) or []) + list(execution.get("output", []) or []):
            temporal = field.get("temporal")
            if isinstance(temporal, CommentedMap) and "reference" in temporal:
                del temporal["reference"]
                bump("temporal reference dropped")
                if len(temporal) == 0:
                    del field["temporal"]


def plain(value):
    """Convert ruamel containers to plain Python for the sidecar."""
    if isinstance(value, CommentedMap):
        return {k: plain(v) for k, v in value.items()}
    if isinstance(value, CommentedSeq):
        return [plain(v) for v in value]
    return value


# --- per-law rewrites for operations without a mechanical equivalent -------


def rewrite_static_assignments(law: CommentedMap) -> None:
    """AWB art. 1:1: repeated `STATIC {condition, value}` assignments to one output
    become a single IF with one case per assignment (first match wins). A plain
    assignment to the same output that precedes them becomes the IF's default."""
    for article in law.get("articles", []):
        execution = (article.get("machine_readable") or {}).get("execution") or {}
        actions = execution.get("actions")
        if not actions:
            continue
        holders: dict[str, CommentedMap] = {}
        plain_defaults: dict[str, CommentedMap] = {}
        final = CommentedSeq()
        for action in actions:
            output = action.get("output")
            value = action.get("value")
            is_static = isinstance(value, CommentedMap) and value.get("operation") == "STATIC"
            if not is_static:
                final.append(action)
                if not (isinstance(value, CommentedMap) and "operation" in value):
                    plain_defaults[output] = action
                continue
            bump("STATIC")
            holder = holders.get(output)
            if holder is None:
                holder = cm(output=output, value=cm(operation="IF", cases=CommentedSeq()))
                if "legal_basis" in action:
                    holder["legal_basis"] = action["legal_basis"]
                preceding = plain_defaults.pop(output, None)
                if preceding is not None:
                    holder["value"]["default"] = preceding["value"]
                    idx = next(i for i, a in enumerate(final) if a is preceding)
                    final[idx] = holder
                else:
                    final.append(holder)
                holders[output] = holder
            holder["value"]["cases"].append(cm(when=value["condition"], then=value["value"]))
        execution["actions"] = final


def rewrite_get_lookup(law: CommentedMap) -> None:
    """Participatiewet art. 22a: `GET subject values` over the definition
    `kostendelersnorm_factoren` becomes explicit IF cases per household size."""
    for article in law.get("articles", []):
        mr = article.get("machine_readable") or {}
        definitions = mr.get("definitions") or {}
        execution = mr.get("execution") or {}
        for action in execution.get("actions", []) or []:
            value = action.get("value")
            if not (isinstance(value, CommentedMap) and value.get("operation") == "IF"):
                continue
            for case in value.get("cases", []):
                then = case.get("then")
                if isinstance(then, CommentedMap) and then.get("operation") == "GET":
                    bump("GET")
                    table_ref = then["values"]
                    table = definitions[table_ref.lstrip("$")]
                    subject = then["subject"]
                    new_cases = CommentedSeq()
                    for key, factor in table.items():
                        new_cases.append(
                            cm(when=cm(operation="EQUALS", subject=subject, value=int(key)), then=factor)
                        )
                    value["cases"] = new_cases
                    break


def rewrite_apv_geluid(law: CommentedMap) -> None:
    """APV Rotterdam 4:2: the 48-hour application term was computed on a combined
    date+time. DATE_DIFF has no hour granularity, so the term is expressed in
    whole days (48 h = 2 days) between the reference date and the activity date.
    The start time no longer plays a part; recorded under `untranslatables`."""
    for article in law.get("articles", []):
        mr = article.get("machine_readable") or {}
        definitions = mr.get("definitions") or {}
        if "min_aanvraagtermijn_uren" in definitions:
            hours = definitions.pop("min_aanvraagtermijn_uren")
            definitions["min_aanvraagtermijn_dagen"] = hours // 24
        execution = mr.get("execution") or {}

        def fix(node):
            if isinstance(node, CommentedMap):
                if node.get("operation") == "SUBTRACT_DATE" and node.get("unit") == "hours":
                    bump("COMBINE_DATETIME")
                    combined = node["values"][0]
                    node.clear()
                    node["operation"] = "DATE_DIFF"
                    node["from"] = "$referencedate"
                    node["to"] = combined["date"]
                    node["in"] = "days"
                    return
                for k, v in list(node.items()):
                    if v == "$min_aanvraagtermijn_uren":
                        node[k] = "$min_aanvraagtermijn_dagen"
                    fix(v)
            elif isinstance(node, CommentedSeq):
                for v in node:
                    fix(v)

        fix(execution)
        untranslatables = mr.setdefault("untranslatables", CommentedSeq())
        untranslatables.append(
            cm(
                construct="aanvraagtermijn in uren, gemeten vanaf de starttijd van de activiteit",
                reason=(
                    "De engine rekent met datums, niet met tijdstippen. De termijn van 48 uur is "
                    "uitgedrukt als twee kalenderdagen tussen de peildatum en de activiteitsdatum; "
                    "de starttijd van de activiteit weegt niet mee."
                ),
                accepted=True,
            )
        )


def rewrite_aow_units(law: CommentedMap) -> None:
    """AOW art. 9: the partner-toeslag reduction was written as
    `(inkomensgrens - partner_inkomen) * 0.02`, an amount in eurocent where the
    surrounding expression expects a fraction. RFC-023 unit inference rejects
    the resulting eurocent * eurocent product. Dividing by 50 eurocent yields
    the same number as multiplying by 0.02 and is dimensionally a ratio."""
    for article in law.get("articles", []):
        mr = article.get("machine_readable") or {}
        definitions = mr.get("definitions") or {}
        if "kortingspercentage" not in definitions:
            continue
        definitions.pop("kortingspercentage")
        definitions["kortingsdeler"] = cm(value=50, type="amount", type_spec=cm(unit="eurocent"))
        bump("AOW kortingspercentage -> kortingsdeler")

        def fix(node):
            if isinstance(node, CommentedMap):
                if (
                    node.get("operation") == "MULTIPLY"
                    and isinstance(node.get("values"), CommentedSeq)
                    and len(node["values"]) == 2
                    and node["values"][1] == "$kortingspercentage"
                ):
                    node["operation"] = "DIVIDE"
                    node["values"][1] = "$kortingsdeler"
                for v in node.values():
                    fix(v)
            elif isinstance(node, CommentedSeq):
                for v in node:
                    fix(v)

        fix(mr.get("execution"))


PER_LAW = {
    "algemene_ouderdomswet/SVB-2024-01-01.yaml": rewrite_aow_units,
    "algemene_wet_bestuursrecht/artikel_1_1_bestuursorgaan/AWB-1994-01-01.yaml": rewrite_static_assignments,
    "participatiewet/bijstand/SZW-2023-01-01.yaml": rewrite_get_lookup,
    "algemene_plaatselijke_verordening/ontheffingspas_geluid/gemeenten/GEMEENTE_ROTTERDAM-2024-01-01.yaml": (
        rewrite_apv_geluid
    ),
}


BINDINGS_HEADER = """# Data bindings for the demo corpus.
#
# Every `source: {}` input of a demo law is resolved outside the YAML. This file
# says where the demo application finds the value: which organisation (`service`),
# which register table, which column(s), and how to select the row (`select_on`,
# `$name` refers to a parameter or an already-bound input of the same law).
# `kind: claim` marks values that only exist once the citizen supplies them;
# `kind: cases` reads from the demo's own case store.
#
# `absent:` says what a missing register row means for the input (RFC-036), and
# it is written on every scalar `kind: table` binding so the choice is visible:
#   absent: unknown  the register cannot say "none"; the value is omitted from
#                    the record and the engine treats the input as unknown
#                    (it names the input as a missing fact). Also the default
#                    when the key is missing.
#   absent: null     the register is authoritative for non-existence (no
#                    partner, no permit, no detention): an explicit null, which
#                    a law tests with `EQUALS ... null`.
#   absent: 0        an income or count register where "no row" means "no such
#                    income" (Belastingdienst boxes, insured years, mail
#                    returns): an explicit 0 the law can calculate with.
# A matched row that lacks the `field:` column follows `absent` as well: the
# register has a row for this person but no such fact in it. An explicit null
# in the row is kept as the absence the data states. Array inputs are the list
# of matching rows (empty when there are none) and carry no `absent`. `fields:`
# objects follow `absent` for the row as a whole; a listed column the row lacks
# is null inside the object (the record exists, the field is empty; a property
# cannot be unknown, and a missing property is an author error). A `select_on`
# value that is itself null (a partner lookup without a partner) makes the
# input null whatever `absent` says: there is nobody to look up, so the value
# is absent, not unknown. A `select_on` value nobody has (a form field not yet
# answered) makes the input unknown whatever `absent` says. `kind: claim` and
# the `events`/`laws`/`reference_data` kinds have no register in the demo and
# are always omitted (unknown until the citizen or a source supplies them).
#
# The first pass of `absent` came from the type heuristic the materialiser used
# before RFC-036 (amount/number -> 0, else null); a value with a comment was
# reviewed against what the register is and deviates from that heuristic.
#
# Generated by corpus/demo/tools/migrate_poc_laws.py from the POC laws; edit the
# law YAML and this file together.
"""


def migrate_file(src: Path, rel: str, dest: Path, bindings: dict) -> None:
    law = yaml.load(src.read_text())
    law["$schema"] = SCHEMA_URL
    if rel in PER_LAW:
        PER_LAW[rel](law)
    migrate_inputs(law, bindings)
    strip_temporal_reference(law)
    walk(law)
    dest.parent.mkdir(parents=True, exist_ok=True)
    with dest.open("w") as fh:
        yaml.dump(law, fh)


def main() -> None:
    poc = Path(sys.argv[1])
    out = Path(sys.argv[2])
    laws_dir = poc / "laws"
    bindings: dict = {}
    files = sorted(laws_dir.rglob("*.yaml"))
    for src in files:
        rel = str(src.relative_to(laws_dir))
        dest = out / "regulation" / "nl" / rel
        migrate_file(src, rel, dest, bindings)
    # Companion markdown files travel along.
    for src in sorted(laws_dir.rglob("*.md")):
        rel = src.relative_to(laws_dir)
        dest = out / "regulation" / "nl" / rel
        dest.parent.mkdir(parents=True, exist_ok=True)
        dest.write_text(src.read_text())

    sidecar = out / "bindings.yaml"
    plain_yaml = YAML()
    plain_yaml.default_flow_style = False
    plain_yaml.width = 4096
    header = BINDINGS_HEADER
    buf = io.StringIO()
    plain_yaml.dump(dict(sorted(bindings.items())), buf)
    # ruamel writes None as an empty value; `absent` says `null` in as many words.
    body = "\n".join(f"{line} null" if line.rstrip().endswith("absent:") else line for line in buf.getvalue().split("\n"))
    with sidecar.open("w") as fh:
        fh.write("---\n")
        fh.write(header)
        fh.write(body)

    print(f"migrated {len(files)} laws → {out}")
    for k, v in sorted(stats.items()):
        print(f"  {v:5} {k}")


if __name__ == "__main__":
    main()
