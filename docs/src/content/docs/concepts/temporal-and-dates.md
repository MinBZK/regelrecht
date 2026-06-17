---
title: "Temporal Validity and Dates"
description: "How the engine selects the law version in force on a date, how it reports references to expired laws, and how dates are compared and subtracted."
---

A calculation always happens *as of* a date. The rules and amounts in force on 15 June 2024 are not the ones in force on 15 June 2025, and a faithful answer uses the version that applied on the day in question. Two mechanisms make this work: version selection by date, and date operations inside the rules.

## Which version is in force

A law version carries `valid_from`, and optionally `valid_to`: the first and last calendar days on which it is in force, both inclusive. The engine selects the version where `valid_from <= reference_date <= valid_to`, where the reference date is the calculation date supplied by the caller.

`valid_to` is what lets a law expire without a successor. A version with `valid_to: 2024-12-31` resolves on its last day:

```gherkin
Given the calculation date is "2024-12-31"
When the law "test_einddatum" is executed for outputs "normbedrag"
Then the execution succeeds
And the output "normbedrag" is "500"
```

and the day after, it is gone. Selection does **not** fall through to an older version once the in-force one has ended; an expired law is expired, not replaced by its predecessor. A reference to a law that has ended fails with the data fact rather than a vague "no rule found":

```gherkin
Given the calculation date is "2025-06-01"
When the law "test_einddatum" is executed for outputs "normbedrag"
Then the execution fails with
  "No version of law 'test_einddatum' in force on 2025-06-01; last in force until 2024-12-31"
```

The same honesty holds across a cross-law reference: a law that reads an ended law reports which law ended and when, instead of silently computing on no-longer-valid rules. The selection outcome is one of in force, not yet in force, or ended on a date (`SelectionReason` in `packages/engine/src/resolver.rs`), and both `valid_from` and `valid_to` are recorded in the [Execution Receipt](./execution-provenance) so the choice is reproducible. The scenarios above come from `features/einddatum.feature`.

## Comparing and subtracting dates

Deadlines and durations need arithmetic on dates, not just on numbers. Two routes cover it.

**Comparison operators dispatch on operand type.** `GREATER_THAN`, `LESS_THAN`, `LESS_THAN_OR_EQUAL` and the rest compare numbers when both operands are numeric, and compare chronologically when both are ISO 8601 dates. So `$indieningsdatum <= $peildatum` works directly, with no detour through `AGE`. `EQUALS` gains a date fallback for the mixed case (a date string against the `{iso, year, month, day}` object form of `referencedate`).

**`DATE_DIFF` measures the span between two dates** with an explicit unit:

```yaml
operation: DATE_DIFF
from: $indieningsdatum
to: $referencedate
in: days        # or: months, years
```

It is signed: positive when `to` is on or after `from`, negative otherwise. For a request filed on 2025-01-01 against a peildatum of 2025-07-01, the span is `181` days; flip the two dates and it is `-181`. Months and years count whole calendar units, reusing the same arithmetic as `AGE` (BW art. 1:2), so end-of-month and leap-year cases stay consistent: 31 January to 28 February is one whole month, because January has no 31st counterpart in February.

```gherkin
Given the calculation date is "2025-02-28"
And a query with the following data:
  | indieningsdatum | 2025-01-31 |
When the law "test_date_operations" is executed for outputs "doorlooptijd_maanden"
Then the output "doorlooptijd_maanden" is "1"
```

Dates must be in canonical `YYYY-MM-DD` form, zero-padded; `2025-1-1` is rejected rather than guessed. These scenarios come from `features/date_operations.feature`, and the related operations `AGE`, `DATE_ADD`, `DATE`, and `DAY_OF_WEEK` are listed in the [Law Format](./law-format) operation table.

## Further reading

- [Cross-Law References](./cross-law-references) - how an expired reference surfaces in a chain
- [Execution Provenance](./execution-provenance) - validity windows in the receipt
- [RFC-019: Law End Dates](/rfcs/rfc-019) and [RFC-021: Date Comparison](/rfcs/rfc-021) - full specifications
