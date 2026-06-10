---
title: "Untranslatables"
description: "Legal constructs the engine cannot express yet, why each one is a feature request against the engine, and how they are handled at runtime."
---

The engine's operation set is deliberately small: arithmetic, comparison, conditional logic, date operations. Dutch law regularly uses constructs that fall outside this set. When a legal construct cannot yet be faithfully expressed with available operations, it is an **untranslatable**.

"Untranslatable" means "not yet", not "never". It is not a verdict that the law is beyond machines. It is a named gap in the engine: a specific operation or schema feature we have not built yet. Every untranslatable is therefore a concrete feature request against the engine, recorded at the exact article that needs it.

The term comes from translation theory. The law-generate process *is* translation, from legal Dutch to machine-readable YAML, and some things do not cross that boundary yet.

## What makes something untranslatable

A construct is untranslatable when the engine cannot yet express it without approximation. Examples:

- **Rounding rules** ("afgerond op hele euro's") when no ROUND operation exists
- **Table lookups** (bracket tables with many rows) that would require fragile chains of IF cases
- **Calendar logic** ("the next working day") when the engine has no holiday calendar
- **Discretionary assessments** ("naar het oordeel van de minister") that are inherently human

The key distinction: the law is clear about what it means, but the engine's formal language cannot express it yet. The gap is the engine's, not the law's, and we expect to close it.

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

Articles with untranslatables can still have partial execution logic for the parts that are expressible. The untranslatable annotation marks what is missing, not what is wrong.

The `suggestion` field names the engine operation or schema feature that would close the gap (for example `Add ROUND/CEIL/FLOOR operation to engine`). This is what turns an untranslatable from a complaint into a feature request: it points directly at what to build next.

The `accepted` field indicates whether a human has reviewed and acknowledged the gap. This controls per-article runtime behavior.

## Runtime behavior

When the engine encounters articles with untranslatables, behavior depends on the `--untranslatable` flag:

| Mode | Behavior | Use case |
|------|----------|----------|
| `error` (default) | Hard error on unaccepted untranslatables | CI, production |
| `propagate` | Execute partial logic, taint outputs with `UNTRANSLATABLE` | Audit, analysis |
| `warn` | Execute partial logic, log warning in trace | Development |
| `ignore` | Execute partial logic silently for `accepted: true` entries; unaccepted entries still error | Human-verified gaps |

The default is fail-fast. Tolerating gaps requires explicit opt-in.

In `propagate` mode, `UNTRANSLATABLE` behaves like `NaN` in floating point: any operation involving an untranslatable input produces an untranslatable output. The trace shows exactly which outputs are tainted and which are trustworthy.

## Driving the engine roadmap

This is the point of the feature, not a side effect. Untranslatables tell us which operations to add next. When enough laws need rounding, we add ROUND. When enough laws need table lookups, we add TABLE. Each `suggestion` is a vote, weighted by how many articles depend on it, and the corpus drives the engine roadmap. As the engine grows, today's untranslatables become tomorrow's ordinary execution logic.

## Further reading

- [Law Format](./law-format) - structure of YAML law files
- [RFC-012: Untranslatables](/rfcs/rfc-012) - full specification
