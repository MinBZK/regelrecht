---
title: "Markings"
description: "Constructs the format cannot express yet, why each one is a concrete piece of work rather than a complaint, and how they are handled at runtime."
---

The engine's operation set is small by design: arithmetic, comparison, conditional logic, date operations. Dutch law regularly uses constructs that fall outside this set. When a construct cannot yet be faithfully expressed, the article carries a **marking**.

A marking means "not yet", not "never". It names a gap in the format: a specific operation or schema feature that has not been built. Every marking is a concrete piece of work, recorded at the article that needs it. The position paper makes publishing these gaps part of the proposal itself ([Rules as Executed, section 5.4](/research/rules-as-executed#sec:untranslatables)): a reader sees where interpretation still happens outside the format.

A marking is a flag on an article that is otherwise worked out. It names the one thing that does not fit and leaves everything that does fit standing. A marking that empties an article is a defect, not a translation.

Keep markings strictly apart from [open terms](./inversion-of-control), which say the opposite: the language expresses this fine, the content is filled in elsewhere.

## What earns a marking

A construct is marked when the format cannot express it without approximation. Examples:

- **Table lookups** (bracket tables with many rows) that would require fragile chains of IF cases
- **Calendar logic** ("the next working day") when the engine has no holiday calendar
- **Discretionary assessments** ("naar het oordeel van de minister") that are inherently human

In each case the law is clear about what it means and the formal language cannot express it yet. The gap belongs to the format, and we expect to close it.

Rounding used to be the standard example here. `ROUND`, `CEIL` and `FLOOR` shipped in schema v0.5.5, which is what closing a gap looks like.

## How they are flagged

Each article's `machine_readable` section can include a `markings` array:

```yaml
machine_readable:
  markings:
    - about: "de eerstvolgende werkdag"
      reason: "The format has no calendar of public holidays to count against"
      resolution: operation
      resolved_by: "A WORKING_DAY operation that skips weekends and public holidays"
      target: ["datum_van_betaling"]
      legal_text_excerpt: "De betaling geschiedt op de eerstvolgende werkdag"
      accepted: false
  execution:
    # execution logic for the parts that DO fit
```

Every field but `accepted` is required, and each one does a job:

| Field | What it carries |
|-------|-----------------|
| `about` | The construct that does not fit, in the article's own words |
| `reason` | Why it does not fit, in terms of what the format does have. This is the diagnosis a reviewer needs to tell a well-examined gap from a wish |
| `resolution` | `operation` when an operation has to be built, `model` when the format itself lacks the shape |
| `resolved_by` | The change that would resolve it, named concretely enough to become work |
| `target` | The outputs, inputs or parameters this article cannot produce because of the marking |
| `legal_text_excerpt` | The words from the article's own legal text that the marking hangs on |

`resolved_by` is what keeps `resolution: model` from becoming the bin that everything awkward falls into. The enrichment checks hold a file to `target`: every name listed there must be absent from the article's actions, because computing a value you declared blocked is a contradiction.

Articles with markings still carry execution logic for the parts that are expressible. The annotation records what the format is missing, and says nothing about the law being wrong.

`accepted` records whether a human has reviewed and acknowledged the gap, which controls per-article runtime behavior.

## Runtime behavior

Behavior depends on the `--untranslatable` flag:

| Mode | Behavior | Use case |
|------|----------|----------|
| `error` (default) | Hard error on unaccepted markings | CI, production |
| `propagate` | Execute partial logic, taint outputs with `UNTRANSLATABLE` | Audit, analysis |
| `warn` | Execute partial logic, log warning in trace | Development |
| `ignore` | Execute partial logic silently for `accepted: true` entries; unaccepted entries still error | Human-verified gaps |

The default is fail-fast. Tolerating gaps requires opting in.

In `propagate` mode, `UNTRANSLATABLE` behaves like `NaN` in floating point: any operation involving a tainted input produces a tainted output. The trace shows which outputs are affected and which are trustworthy.

## Driving the roadmap

Markings tell us what to build next. When enough laws need a working-day calendar, we build one. Each `resolved_by` is a vote, weighted by how many articles depend on it, so the corpus sets the order of the work.

## A note on the name

This channel was called `untranslatables` through schema v0.5.x, alongside a separate `norm_gaps`. Schema v0.7.0 replaced both with `markings`, and a file carrying the old field does not validate under v0.7.0; `law-migrate` converts it. The engine still reads both channels, so a law on an older schema version keeps working. [RFC-012](/rfcs/rfc-012) describes the original design under the old name and is kept as written.

## Further reading

- [Law Format](./law-format) - structure of YAML law files
- [Schema Reference](../reference/schema) - the generated field reference for `markings`
- [RFC-012: Untranslatables](/rfcs/rfc-012) - the original specification, under the former name
- [Rules as Executed, section 9.2](/research/rules-as-executed#sec:opset) - the position paper on why the operation set stays small and only grows in public
