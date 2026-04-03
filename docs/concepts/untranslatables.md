# Untranslatables

The engine's operation set is deliberately small: arithmetic, comparison, conditional logic, date operations. Dutch law regularly uses constructs that fall outside this set. When a legal construct cannot be faithfully expressed with available operations, it is an **untranslatable**.

The term comes from translation theory. The law-generate process *is* translation — from legal Dutch to machine-readable YAML — and some things do not cross that boundary.

## What makes something untranslatable

A construct is untranslatable when the engine cannot express it without approximation. Examples:

- **Rounding rules** ("afgerond op hele euro's") when no ROUND operation exists
- **Table lookups** (bracket tables with many rows) that would require fragile chains of IF cases
- **Calendar logic** ("the next working day") when the engine has no holiday calendar
- **Discretionary assessments** ("naar het oordeel van de minister") that are inherently human

The key distinction: the law is clear about what it means, but the engine's formal language cannot yet express it.

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

The `accepted` field indicates whether a human has reviewed and acknowledged the gap. This controls per-article runtime behavior.

## Runtime behavior

When the engine encounters articles with untranslatables, behavior depends on the `--untranslatable` flag:

| Mode | Behavior | Use case |
|------|----------|----------|
| `error` (default) | Hard error on unaccepted untranslatables | CI, production |
| `propagate` | Execute partial logic, taint outputs with `UNTRANSLATABLE` | Audit, analysis |
| `warn` | Execute partial logic, log warning in trace | Development |
| `ignore` | Execute partial logic silently (only for `accepted: true`) | Human-verified gaps |

The default is fail-fast. Tolerating gaps requires explicit opt-in.

In `propagate` mode, `UNTRANSLATABLE` behaves like `NaN` in floating point: any operation involving an untranslatable input produces an untranslatable output. The trace shows exactly which outputs are tainted and which are trustworthy.

## Driving the engine roadmap

Untranslatables tell us which operations to add next, based on real legal texts rather than speculation. When enough laws need rounding, we add ROUND. When enough laws need table lookups, we add TABLE. The corpus itself drives the engine's evolution.

## Further reading

- [Law Format](./law-format) - structure of YAML law files
- [RFC-012: Untranslatables](/rfcs/rfc-012) - full specification
