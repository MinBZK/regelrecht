# /// script
# dependencies = ["ruamel.yaml>=0.18"]
# ///
"""Declare nullability in the demo laws (RFC-036, `nullable` in schema v0.5.8).

Under RFC-036 whether a value may be absent (`null`) is a property of the
type: a parameter, input or output carries `nullable: true` when "there is
none" is a legitimate value of it, and the engine rejects a `null` at every
boundary where the declaration says none can occur. This tool derives the
flag for the 80 demo laws from what the laws, the bindings and the scenarios
already say, and writes it into the YAML. It changes nothing else about a law's
logic; where a law genuinely lacks a guard the type checker (`just validate`)
says so, and that is a hand fix with a legal source, not a tool's decision.

A field is nullable when at least one of these holds (the criterion is printed
per field in `--report` mode and recorded in CONVERSION_NOTES.md):

  binding    the input's `absent:` in bindings.yaml is `null`: the register is
             authoritative for non-existence and materialises an explicit null
  selector   the input is a table lookup whose `select_on` refers to a nullable
             parameter or input (the partner's income without a partner): the
             lookup is on nobody, so the value is null whatever `absent` says
  null-test  the law compares the field with a literal null (`EQUALS ... null`,
             also under `NOT`): a test that can only be true for a nullable field
  scenario   a scenario passes the parameter as `null` (`parameter "x" is
             "null"`, or a `null` cell in a `the following parameters:` table);
             a non-nullable parameter passed null is an error at the top level
  literal    the output's value can be a literal `null` (directly, or as a
             `then:`/`default:` of the `IF` that is the output's value)
  no-default the output's value is (or ends in) an `IF` without `default`,
             which yields null when no case matches
  empty-min  the output's value is a `FOREACH` combined with `MIN`/`MAX`, null
             over an empty collection
  through    the output's value passes a nullable variable straight through
             (`value: $partner_bsn`, or a property `$adres.postcode` of a
             nullable record: a field of no record is no record)
  cross-law  the input takes a cross-law output that is nullable
  skip       the input is a cross-law call that passes a nullable variable for a
             required parameter of the target: the target is skipped and the
             input is null (RFC-007/RFC-036 skip rule)
  argument   the (optional) parameter receives a nullable variable from a
             calling law; a non-nullable optional parameter passed null is an
             error at the call

The rules feed each other (a nullable output makes the consuming input
nullable, which may make an output of that law nullable), so the tool iterates
to a fixpoint over the whole corpus. A law referenced by `$id` is looked up in
every version present; an output nullable in any version counts.

Every law that carries at least one `nullable: true` moves its `$schema` to
v0.5.8, the version that defines the attribute. Idempotent: a field that is
already declared is left alone, and the report says what would change. A
declaration no criterion derives any more (after a guard was added at the
source) is reported as STALE and left for a reviewer; the tool never removes.

Usage:
    uv run corpus/demo/tools/declare_nullable.py corpus/demo --report
    uv run corpus/demo/tools/declare_nullable.py corpus/demo
"""

from __future__ import annotations

import re
import sys
from collections import defaultdict
from dataclasses import dataclass, field
from pathlib import Path

from ruamel.yaml import YAML
from ruamel.yaml.comments import CommentedMap

SCHEMA_V058 = (
    "https://raw.githubusercontent.com/MinBZK/regelrecht/refs/tags/schema-v0.5.8/schema/v0.5.8/schema.json"
)

yaml = YAML()
yaml.preserve_quotes = True
yaml.width = 4096
yaml.explicit_start = True
yaml.indent(mapping=2, sequence=4, offset=2)
# An explicit `null` instead of an empty value, as reformat_yaml.py writes it.
yaml.representer.add_representer(
    type(None), lambda r, _d: r.represent_scalar("tag:yaml.org,2002:null", "null")
)


# --------------------------------------------------------------------------- model


@dataclass
class Field:
    kind: str  # parameter | input | output
    name: str
    type: str
    required: bool
    node: CommentedMap
    source: dict | None = None  # cross-law source, for inputs


@dataclass
class Article:
    law: "Law"
    number: str
    fields: dict[str, list[Field]]  # an input and an output may share a name (wet_brp geboortedatum)
    definitions: set[str]
    actions: list
    # Keys are ("var", name) for a parameter or input and ("out", name) for an
    # output: wet_brp declares `geboortedatum` as both, and only the input can
    # come from a register while only the output can be a computed null.
    nullable: set[tuple[str, str]] = field(default_factory=set)
    reasons: dict[tuple[str, str], set[str]] = field(default_factory=lambda: defaultdict(set))

    def key_for_ref(self, name: str) -> tuple[str, str] | None:
        """What `$name` denotes in an action: the parameter or input, else the output."""
        kinds = {f.kind for f in self.fields.get(name, ())}
        if kinds & {"parameter", "input"}:
            return ("var", name)
        if "output" in kinds:
            return ("out", name)
        return None

    def ref_nullable(self, name: str) -> bool:
        return self.key_for_ref(name) in self.nullable

    def mark(self, key: tuple[str, str] | None, reason: str) -> bool:
        if key is None:
            return False
        new = key not in self.nullable
        self.nullable.add(key)
        self.reasons[key].add(reason)
        return new

    def fields_of(self, key: tuple[str, str]):
        kinds = {"parameter", "input"} if key[0] == "var" else {"output"}
        return [f for f in self.fields.get(key[1], ()) if f.kind in kinds]

    def of_kind(self, kind: str):
        for fs in self.fields.values():
            for f in fs:
                if f.kind == kind:
                    yield f


@dataclass
class Law:
    path: Path
    id: str
    doc: CommentedMap
    articles: list[Article]

    def field_in_any_article(self, kind: str, name: str):
        for a in self.articles:
            for f in a.fields.get(name, ()):
                if f.kind == kind:
                    yield a, f


def load_laws(root: Path) -> dict[str, list[Law]]:
    by_id: dict[str, list[Law]] = defaultdict(list)
    for path in sorted(root.rglob("*.yaml")):
        doc = yaml.load(path.read_text())
        articles = []
        law = Law(path=path, id=doc["$id"], doc=doc, articles=articles)
        for art in doc.get("articles") or []:
            mr = art.get("machine_readable")
            if not mr or "execution" not in mr:
                continue
            ex = mr["execution"]
            fields: dict[str, list[Field]] = defaultdict(list)
            for kind, key in (("parameter", "parameters"), ("input", "input"), ("output", "output")):
                for f in ex.get(key) or []:
                    fields[f["name"]].append(Field(
                        kind=kind,
                        name=f["name"],
                        type=f.get("type", "string"),
                        required=bool(f.get("required", True)),
                        node=f,
                        source=f.get("source") if kind == "input" else None,
                    ))
            articles.append(
                Article(
                    law=law,
                    number=str(art.get("number")),
                    fields=dict(fields),
                    definitions=set((mr.get("definitions") or {}).keys()),
                    actions=list(ex.get("actions") or []),
                )
            )
            for fs in fields.values():
                for f in fs:
                    if f.node.get("nullable"):
                        articles[-1].mark(("out" if f.kind == "output" else "var", f.name), "declared")
        by_id[law.id].append(law)
    return by_id


# ------------------------------------------------------------------- expression walk


def ref(value) -> str | None:
    """`$x` -> `x`; `$x.y` -> `x`; anything else -> None."""
    if isinstance(value, str) and value.startswith("$"):
        return value[1:].split(".")[0]
    return None


def is_property_path(value) -> bool:
    return isinstance(value, str) and value.startswith("$") and "." in value


def walk(node):
    """Every mapping in an expression tree, depth first."""
    if isinstance(node, dict):
        yield node
        for v in node.values():
            yield from walk(v)
    elif isinstance(node, list):
        for v in node:
            yield from walk(v)


def null_tested_names(actions) -> set[str]:
    """Variables compared with a literal null by EQUALS/NOT_EQUALS (bare `$x`)."""
    names = set()
    for op in walk(actions):
        if op.get("operation") not in ("EQUALS", "NOT_EQUALS"):
            continue
        subject, value = op.get("subject"), op.get("value")
        if "value" not in op or "subject" not in op:
            continue
        for a, b in ((subject, value), (value, subject)):
            if a is None and isinstance(b, str) and b.startswith("$") and not is_property_path(b):
                names.add(b[1:])
    return names


def null_test(expr):
    """`EQUALS $x null` -> ("null", x); `NOT(EQUALS $x null)` -> ("nonnull", x); else None."""
    if not isinstance(expr, dict):
        return None
    op = expr.get("operation")
    if op == "NOT":
        inner = null_test(expr.get("value"))
        if inner:
            return ("nonnull" if inner[0] == "null" else "null", inner[1])
        return None
    if op in ("EQUALS", "NOT_EQUALS") and "subject" in expr and "value" in expr:
        for a, b in ((expr["subject"], expr["value"]), (expr["value"], expr["subject"])):
            if a is None and isinstance(b, str) and b.startswith("$") and not is_property_path(b):
                return ("null" if op == "EQUALS" else "nonnull", b[1:])
    return None


def established_non_null(when) -> set[str]:
    """Variables a satisfied `when` proves present: a bare `NOT(EQUALS $x null)`, or
    every such conjunct of an `AND` (RFC-036 flow rule a/d)."""
    t = null_test(when)
    if t and t[0] == "nonnull":
        return {t[1]}
    if isinstance(when, dict) and when.get("operation") == "AND":
        out = set()
        for c in when.get("conditions") or []:
            t = null_test(c)
            if t and t[0] == "nonnull":
                out.add(t[1])
        return out
    return set()


def established_null(when) -> set[str]:
    """Variables a satisfied `when` proves absent, so later cases see them present:
    a bare `EQUALS $x null`, or every such disjunct of an `OR` (flow rule b)."""
    t = null_test(when)
    if t and t[0] == "null":
        return {t[1]}
    if isinstance(when, dict) and when.get("operation") == "OR":
        out = set()
        for c in when.get("conditions") or []:
            t = null_test(c)
            if t and t[0] == "null":
                out.add(t[1])
        return out
    return set()


def may_be_null(expr, article: Article, non_null: frozenset = frozenset()) -> str | None:
    """The criterion under which the value of `expr` can be null, else None.
    `non_null` holds the variables the enclosing IF cases have established present."""
    if expr is None:
        return "literal"
    if isinstance(expr, str) and expr.startswith("$"):
        head = ref(expr)
        if head == "referencedate":
            return None
        if article.ref_nullable(head) and head not in non_null:
            return "through"
        return None
    if not isinstance(expr, dict):
        return None
    op = expr.get("operation")
    if op == "IF":
        known = set(non_null)
        for case in expr.get("cases") or []:
            when = case.get("when")
            r = may_be_null(case.get("then"), article, frozenset(known | established_non_null(when)))
            if r:
                return r
            known |= established_null(when)
        if "default" not in expr:
            return "no-default"
        return may_be_null(expr.get("default"), article, frozenset(known))
    if op == "FOREACH" and expr.get("combine") in ("MIN", "MAX"):
        return "empty-min"
    # Arithmetic, comparison, logic, dates, rounding: an absent operand is an
    # error there, never a null result (RFC-036).
    return None


# --------------------------------------------------------------------- scenarios

PARAM_NULL = re.compile(r'^\s*(?:Given|And|When|Then|But)\s+parameter "([^"]+)" is "null"\s*$')
EVALUATE = re.compile(r'of "([^"]+)"\s*$')
PARAMS_TABLE = re.compile(r"^\s*(?:Given|And|When)\s+the following parameters:\s*$")
ROW = re.compile(r"^\s*\|(.*)\|\s*$")


def null_parameters_in_scenarios(root: Path) -> dict[str, set[str]]:
    """Per law id, the parameters a scenario passes as null."""
    out: dict[str, set[str]] = defaultdict(set)
    for feature in sorted(root.rglob("*.feature")):
        lines = feature.read_text().splitlines()
        laws = set()
        for line in lines:
            if "I evaluate" in line:
                m = EVALUATE.search(line)
                if m:
                    laws.add(m.group(1))
        nulls = set()
        in_table = False
        for line in lines:
            if PARAMS_TABLE.match(line):
                in_table = True
                continue
            if in_table:
                m = ROW.match(line)
                if not m:
                    in_table = False
                else:
                    cells = [c.strip() for c in m.group(1).split("|")]
                    if len(cells) >= 2 and cells[1] == "null" and cells[0] not in ("name",):
                        nulls.add(cells[0])
                    continue
            m = PARAM_NULL.match(line)
            if m:
                nulls.add(m.group(1))
        for law in laws:
            out[law] |= nulls
    return out


# ---------------------------------------------------------------------- analysis


def analyse(demo: Path):
    laws_root = demo / "regulation" / "nl"
    by_id = load_laws(laws_root)
    bindings = yaml.load((demo / "bindings.yaml").read_text()) or {}
    scenario_nulls = null_parameters_in_scenarios(laws_root)
    all_articles = [a for laws in by_id.values() for law in laws for a in law.articles]

    # Seeds that do not depend on other fields.
    for article in all_articles:
        law_bindings = bindings.get(article.law.id) or {}
        for f in article.of_kind("input"):
            b = law_bindings.get(f.name)
            if b and b.get("kind") == "table" and "absent" in b and b["absent"] is None:
                article.mark(("var", f.name), "binding")
        for name in null_tested_names(article.actions):
            if name in article.definitions:
                print(f"NOTE {article.law.id} art. {article.number}: definition '{name}' compared with null")
            article.mark(article.key_for_ref(name), "null-test")
        for name in scenario_nulls.get(article.law.id, ()):
            if any(f.kind == "parameter" for f in article.fields.get(name, ())):
                article.mark(("var", name), "scenario")

    def output_nullable(law_id: str, output: str) -> bool:
        return any(("out", output) in a.nullable for law in by_id.get(law_id, []) for a in law.articles)

    changed = True
    while changed:
        changed = False
        for article in all_articles:
            law_bindings = bindings.get(article.law.id) or {}
            # Outputs from their action value.
            for action in article.actions:
                out = action.get("output")
                if out not in article.fields or "value" not in action:
                    continue
                reason = may_be_null(action.get("value"), article)
                if reason and article.mark(("out", out), reason):
                    changed = True
            for f in article.of_kind("input"):
                name = f.name
                # A table lookup keyed on a nullable variable.
                b = law_bindings.get(name)
                if b and b.get("kind") == "table" and f.type != "array":
                    for crit in b.get("select_on") or []:
                        head = ref(crit.get("value"))
                        if head and article.ref_nullable(head) and article.mark(("var", name), "selector"):
                            changed = True
                # Cross-law inputs.
                src = f.source or {}
                target_id, target_out = src.get("regulation"), src.get("output")
                if not target_id or target_id not in by_id:
                    continue
                if output_nullable(target_id, target_out) and article.mark(("var", name), "cross-law"):
                    changed = True
                for pname, arg in (src.get("parameters") or {}).items():
                    arg_reason = may_be_null(arg, article)
                    if not arg_reason:
                        continue
                    for target_law in by_id[target_id]:
                        for t_article, t_field in target_law.field_in_any_article("parameter", pname):
                            if t_field.required:
                                if article.mark(("var", name), "skip"):
                                    changed = True
                            elif t_article.mark(("var", pname), "argument"):
                                changed = True
    return by_id, bindings


# ------------------------------------------------------------------------ apply


def insert_nullable(node: CommentedMap) -> None:
    keys = list(node.keys())
    pos = keys.index("type") + 1 if "type" in keys else 1
    node.insert(pos, "nullable", True)


def main() -> None:
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    report = "--report" in sys.argv
    demo = Path(args[0]) if args else Path("corpus/demo")
    by_id, _bindings = analyse(demo)

    per_reason: dict[str, int] = defaultdict(int)
    per_kind: dict[str, int] = defaultdict(int)
    new_fields = 0
    laws_changed = 0
    for laws in by_id.values():
        for law in laws:
            touched = False
            for article in law.articles:
                for key in sorted(article.nullable):
                    reasons = sorted(r for r in article.reasons[key] if r != "declared")
                    for f in article.fields_of(key):
                        if f.node.get("nullable"):
                            continue
                        new_fields += 1
                        per_kind[f.kind] += 1
                        for r in reasons:
                            per_reason[r] += 1
                        print(f"{law.path.relative_to(demo)} art. {article.number} {f.kind} {f.name}: {', '.join(reasons)}")
                        if not report:
                            insert_nullable(f.node)
                            touched = True
            uses_nullable = any(article.nullable for article in law.articles)
            if uses_nullable and law.doc.get("$schema") != SCHEMA_V058:
                if not report:
                    law.doc["$schema"] = SCHEMA_V058
                    touched = True
                print(f"{law.path.relative_to(demo)}: $schema -> v0.5.8")
            if touched:
                laws_changed += 1
                with law.path.open("w") as fh:
                    yaml.dump(law.doc, fh)
    # A declaration the analysis does not derive any more (a guard was added at
    # the source, a binding changed): the flag is stale and a reviewer decides.
    stale = 0
    for laws in by_id.values():
        for law in laws:
            for article in law.articles:
                for key in sorted(article.nullable):
                    if article.reasons[key] == {"declared"}:
                        for f in article.fields_of(key):
                            stale += 1
                            print(f"STALE {law.path.relative_to(demo)} art. {article.number} {f.kind} {f.name}: declared nullable, no criterion derives it")
    print()
    print(f"{'would declare' if report else 'declared'} {new_fields} fields nullable in {laws_changed} laws" + (f"; {stale} stale declaration(s)" if stale else ""))
    print("by kind:   " + ", ".join(f"{k} {v}" for k, v in sorted(per_kind.items())))
    print("by reason: " + ", ".join(f"{k} {v}" for k, v in sorted(per_reason.items())))


if __name__ == "__main__":
    main()
