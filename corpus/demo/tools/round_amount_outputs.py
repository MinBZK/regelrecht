# /// script
# dependencies = ["ruamel.yaml>=0.18"]
# ///
"""Make the rounding of eurocent outputs explicit in the demo laws (RFC-024).

The POC engine rounded every `type: amount` output with `type_spec.precision: 0`
to whole eurocents implicitly. The regelrecht engine keeps the exact decimal:
rounding is never implicit, a law that rounds has to say so. For the outputs
listed in `OUTPUTS` this tool wraps the action value in

    operation: ROUND
    precision: <the output's type_spec.precision, 0 when absent>
    value: <the existing expression>

ROUND is half-up. The POC's own feature expectations pin that down: 126307.84
is asserted as 126308 and 209691.79 as 209692, which FLOOR (truncation) would
not produce.

An earlier version wrapped the rounding in `IF EQUALS(expr, null) THEN null
DEFAULT ROUND(expr)`, to keep the POC's None propagation. Under RFC-036 that
guard is dead: every expression rounded here is arithmetic, and arithmetic on an
absent operand is an error, never a null result. The guard also declared, in
effect, that the output could be absent, which the type checker (schema v0.5.8,
`nullable`) then carries into every law that consumes the amount. The tool now
writes the bare ROUND and unwraps a guard it finds.

Idempotent: an action whose value already is a bare ROUND is left alone. Usage:

    uv run corpus/demo/tools/round_amount_outputs.py corpus/demo/regulation/nl
"""

from __future__ import annotations

import sys
from pathlib import Path

from ruamel.yaml import YAML
from ruamel.yaml.comments import CommentedMap

yaml = YAML()
yaml.preserve_quotes = True
yaml.width = 4096
yaml.explicit_start = True
# The corpus style (see reformat_yaml.py): indented sequences, explicit null.
yaml.indent(mapping=2, sequence=4, offset=2)
yaml.representer.add_representer(
    type(None), lambda r, _d: r.represent_scalar("tag:yaml.org,2002:null", "null")
)

COMMENT = "RFC-024: afronding expliciet (POC rondde impliciet af op de precisie van type_spec)"

# (law $id, output) -> rounding operation. ROUND (half-up) everywhere the POC
# expectations are consistent with it; FLOOR only where the POC value can only
# have come from truncation (ww_duur_maanden: 12.5 asserted as 12).
OUTPUTS = {
    ("algemene_ouderdomswet", "pensioenbedrag"): "ROUND",
    ("pensioenwet", "pensioen_uitkering_maandelijks"): "ROUND",
    ("werkloosheidswet", "ww_uitkering_per_maand"): "ROUND",
    ("werkloosheidswet", "ww_duur_maanden"): "FLOOR",
    ("wet_inkomstenbelasting", "totale_belastingschuld"): "ROUND",
    ("wet_inkomstenbelasting", "totale_heffingskortingen"): "ROUND",
    ("wet_op_het_kindgebonden_budget", "kindgebonden_budget_jaar"): "ROUND",
    ("zorgtoeslagwet", "hoogte_toeslag"): "ROUND",
    ("zvw/werkgeversbijdrage", "zvw_werkgeversbijdrage"): "ROUND",
    ("participatiewet/bijstand/amsterdam", "uitkeringsbedrag"): "ROUND",
    # Not an amount but the same mechanism: 13.0021 verzekerde jaren was 13.00 in
    # the POC (precision 2), which is what makes the AOW opbouw 48/50 exactly.
    ("wet_structuur_uitvoeringsorganisatie_werk_en_inkomen", "verzekerde_jaren"): "ROUND",
}


ROUNDING_OPS = ("ROUND", "CEIL", "FLOOR")


def wrap(value, precision: int, operation: str) -> CommentedMap:
    rounding = CommentedMap()
    rounding["operation"] = operation
    rounding["precision"] = precision
    rounding["value"] = value
    return rounding


def is_plain_rounding(value) -> bool:
    return isinstance(value, dict) and value.get("operation") in ROUNDING_OPS


def unwrap_guarded_rounding(value):
    """The earlier `IF EQUALS(expr, null) THEN null DEFAULT ROUND(expr)` form;
    return the rounded expression, else None."""
    if (
        isinstance(value, dict)
        and value.get("operation") == "IF"
        and isinstance(value.get("default"), dict)
        and value["default"].get("operation") in ROUNDING_OPS
        and len(value.get("cases") or []) == 1
        and value["cases"][0].get("then") is None
    ):
        return value["default"]["value"]
    return None


def round_file(path: Path) -> int:
    original = path.read_text()
    doc = yaml.load(original)
    law_id = doc.get("$id")
    wanted = {output: op for (law, output), op in OUTPUTS.items() if law == law_id}
    if not wanted:
        return 0
    changed = 0
    for article in doc.get("articles") or []:
        execution = (article.get("machine_readable") or {}).get("execution")
        if not execution:
            continue
        precisions = {
            o.get("name"): int((o.get("type_spec") or {}).get("precision", 0) or 0)
            for o in execution.get("output") or []
        }
        for action in execution.get("actions") or []:
            if action.get("output") not in wanted or "value" not in action:
                continue
            value = action["value"]
            if is_plain_rounding(value):
                continue
            inner = unwrap_guarded_rounding(value)
            if inner is not None:
                value = inner
            action["value"] = wrap(value, precisions.get(action.get("output"), 0), wanted[action.get("output")])
            if COMMENT not in original:
                action.yaml_set_comment_before_after_key("value", before=COMMENT, indent=8)
            changed += 1
    if changed:
        with path.open("w") as handle:
            yaml.dump(doc, handle)
    return changed


def main() -> None:
    if len(sys.argv) != 2:
        sys.exit("usage: round_amount_outputs.py corpus/demo/regulation/nl")
    total = 0
    for path in sorted(Path(sys.argv[1]).rglob("*.yaml")):
        n = round_file(path)
        if n:
            print(f"{path}: {n} output(s) wrapped in ROUND")
        total += n
    print(f"total: {total}")


if __name__ == "__main__":
    main()
