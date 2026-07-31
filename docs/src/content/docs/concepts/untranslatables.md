---
title: "Untranslatables"
description: "Legal constructs the engine cannot express yet, why each one is a feature request against the engine, and how they are handled at runtime."
---

The engine's operation set is small by design: arithmetic, comparison, conditional logic, date operations. Dutch law regularly uses constructs that fall outside this set. When a legal construct cannot yet be faithfully expressed with available operations, it is an **untranslatable**.

"Untranslatable" means "not yet", not "never". It names a gap in the engine: a specific operation or schema feature we have not built yet. Every untranslatable is a concrete feature request against the engine, recorded at the article that needs it.

The term comes from translation theory. The law-generate process *is* translation, from legal Dutch to machine-readable YAML, and some things do not cross that boundary yet.

## What makes something untranslatable

A construct is untranslatable when the engine cannot yet express it without approximation. Examples:

- **Rounding rules** ("afgerond op hele euro's") when no ROUND operation exists
- **Table lookups** (bracket tables with many rows) that would require fragile chains of IF cases
- **Calendar logic** ("the next working day") when the engine has no holiday calendar
- **Discretionary assessments** ("naar het oordeel van de minister") that are inherently human

In each case the law is clear about what it means and the engine's formal language cannot express it yet. The gap is the engine's, and we expect to close it.

## How they are flagged

Each article's `machine_readable` section can include an `untranslatables` array:

```yaml
machine_readable:
  untranslatables:
    - construct: "afronden op hele euro's"
      reason: "Rounding is not available as an engine operation"
      suggestion: "Add ROUND/CEIL/FLOOR operation to engine"
      legal_text_excerpt: "Het bedrag wordt naar boven afgerond op hele euro's"
      accepted: false
  execution:
    # execution logic for the parts that ARE translatable
```

Articles with untranslatables can still have partial execution logic for the parts that are expressible. The annotation records what is missing from the engine, and says nothing about the law being wrong.

The `suggestion` field names the engine operation or schema feature that would close the gap (for example `Add ROUND/CEIL/FLOOR operation to engine`). That field makes the entry actionable: it points at what to build next.

The `accepted` field indicates whether a human has reviewed and acknowledged the gap. This controls per-article runtime behavior.

## Runtime behavior

When the engine encounters articles with untranslatables, behavior depends on the `--untranslatable` flag:

| Mode | Behavior | Use case |
|------|----------|----------|
| `error` (default) | Hard error on unaccepted untranslatables | CI, production |
| `propagate` | Execute partial logic, taint outputs with `UNTRANSLATABLE` | Audit, analysis |
| `warn` | Execute partial logic, log warning in trace | Development |
| `ignore` | Execute partial logic silently for `accepted: true` entries; unaccepted entries still error | Human-verified gaps |

The default is fail-fast. Tolerating gaps requires opting in.

In `propagate` mode, `UNTRANSLATABLE` behaves like `NaN` in floating point: any operation involving an untranslatable input produces an untranslatable output. The trace shows which outputs are tainted and which are trustworthy.

## Driving the engine roadmap

Untranslatables tell us which operations to add next. When enough laws need rounding, we add ROUND. When enough laws need table lookups, we add TABLE. Each `suggestion` is a vote, weighted by how many articles depend on it, so the corpus sets the order of work on the engine.

## Further reading

- [Law Format](./law-format) - structure of YAML law files
- [RFC-012: Untranslatables](/rfcs/rfc-012) - full specification
