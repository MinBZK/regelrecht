---
title: "Collections"
description: "How a law iterates over a group whose size is not known in advance, so that the legal test stays in the law instead of in the data source."
---

Dutch law reasons about groups whose size nobody knows in advance: the medebewoners in a household, the children a benefit is calculated for, the registrations in a register. A law that cannot iterate over such a group has to be handed a pre-counted number instead, and whoever produces that number has to decide who counted before the law gets a say.

`FOREACH` keeps that decision in the law.

## Iterating

```yaml
operation: FOREACH
collection: $medebewoners      # an array
as: medebewoner                # names the element, defaults to "item"
filter:                        # optional: skip elements where this is false
  operation: LESS_THAN
  subject: $medebewoner.leeftijd
  value: 23
body: $medebewoner.toetsingsinkomen   # evaluated once per surviving element
combine: ADD                   # optional: reduce to a single value
```

Read it as one clause: *the combined income of the medebewoners under 23*. Legal text puts it that way too, which is why selecting, transforming and totalling are one operation here rather than three. The shape comes from AWIR article 7 lid 5, which exempts part of the income of a medebewoner who is a first-degree relative under 23; that article is in the corpus and not yet modeled.

`collection` is any expression that evaluates to an array. A single value iterates once; `null` is an empty collection.

## The element binding

`as` names the current element, and that name exists only inside `filter` and `body`. It shadows an outer variable of the same name. `FOREACH` is the only operation that introduces a name rather than reading one.

When the elements are objects, both forms work:

```yaml
body: $medebewoner.bijdrage    # dot notation, preferred
body: $bijdrage                # the object's fields are also injected as locals
```

Prefer the dot notation. A flattened field can collide with something in an outer scope, and the prefixed form says which collection the value came from.

## Nesting

Each `FOREACH` gets its own scope, and a child scope starts empty. That has one consequence worth knowing before you write a nested one: the inner `collection` is evaluated in the *outer* scope, so it can see the outer binding, and the inner `body` cannot.

```yaml
operation: FOREACH
collection: $huishoudens
as: huishouden
body:
  operation: FOREACH
  collection: $huishouden.leden   # sees $huishouden
  as: lid
  body: $lid.bijdrage             # sees $lid, not $huishouden
  combine: ADD
combine: ADD
```

Routing a value through `collection` is the only way into an inner iteration. A child that inherited its parent's locals would let one iteration leak into the next, which is why it does not.

## Combining

| `combine` | Result | Empty collection |
|---|---|---|
| `ADD` | Sum; concatenates strings and flattens arrays | `0` |
| `OR` | True if any result is truthy | `false` |
| `AND` | True if every result is truthy | `true` |
| `MIN` | Lowest value | `null` |
| `MAX` | Highest value | `null` |
| omitted | The results as an array | `[]` |

These five are the aggregations legal text performs: *het totaal van*, *indien ten minste een van*, *indien aan alle*, *het laagste of hoogste van*. `SUBTRACT`, `MULTIPLY` and `DIVIDE` are not offered, because subtracting a list leaves open what it is subtracted from. A law that needs one can omit `combine` and apply the arithmetic to the resulting array.

Two of the empty cases deserve attention. `MIN` and `MAX` return `null` rather than a number, because there is no lowest value of nothing; the law has to handle that. `AND` returns `true` by vacuous truth, so a law that must not read "no items" as "all conditions met" has to check for emptiness itself.

## Counting

Article 22a Participatiewet computes the kostendelersnorm from *A*, which the text defines as "het aantal kostendelende medebewoners plus de belanghebbende en zijn echtgenoot van 21 jaar of ouder indien hij gehuwd is". Counting is `body: 1` with `combine: ADD`:

```yaml
- output: aantal_kostendelende_medebewoners
  value:
    operation: FOREACH
    collection: $medebewoners
    as: medebewoner
    body: 1
    combine: ADD
```

No filter: this article counts every kostendelende medebewoner. Both limits of 21 in that sentence govern the belanghebbende and the echtgenoot, and they appear as ordinary conditions elsewhere in the article. That reading is easy to get wrong, which is the argument for keeping it here, where a jurist can check it against the text.

Before the corpus could iterate, the same article was modeled with a boolean input named `heeft_kostendelende_medebewoners`. A number became a yes-or-no, and whoever supplied it had to decide who qualified.

Scenarios drive it the same way, with one row per element:

```gherkin
Given parameter "medebewoners" is the collection:
  | leeftijd |
  | 25       |
  | 19       |
  | 34       |
When I evaluate "aantal_kostendelende_medebewoners" of "participatiewet"
Then output "aantal_kostendelende_medebewoners" equals 3
```

The worked article is in `corpus/regulation/nl/wet/participatiewet/2022-03-15.yaml`, with its scenarios next to it.

## When the answer is not known

An error in `filter` or `body` aborts the whole operation. Partial results are never returned: a sum over some of the children is not a legal determination.

An [untranslatable](./untranslatables) taints the whole result, wherever it appears. Dropping the untranslatable elements and combining the rest would produce a number that looks complete and is not.

The same holds for a value the engine cannot determine. A `filter` that evaluates to `null` makes the result `null`, because the engine cannot tell whether that element belongs in the collection. A `body` that evaluates to `null` does too: the element is definitely in the collection and its contribution is unknown, so `ADD`, `MIN` and `MAX` report that rather than a total that is short by an unknown amount. `OR` and `AND` settle on a definitive `true` or `false` where one exists, and are `null` otherwise.

An element whose filter is definitively `false` is simply skipped, and an empty collection returns the identities in the table above. Nothing is missing in either case.

## Bounds and units

A collection longer than 1000 elements is an error, and nesting is bounded by the operation-depth limit of 100. Those two multiply: a FOREACH inside a FOREACH, each over a thousand elements, is a million evaluations with both limits satisfied. Collections come from finite data sources; there are no generators.

Under [quantities](/rfcs/rfc-023) the result carries the unit of `body`: summing eurocents yields eurocents, and `MIN` and `MAX` preserve it the same way. `AND` and `OR` produce a boolean, and without `combine` the result is an array, so neither carries a unit.

Every iteration appears in the [execution trace](./execution-provenance) with its index and the value the body produced, or the element itself where the filter skipped it, under a node reporting how many elements were evaluated and how many were skipped. A total that shows only itself is not an explanation of how it was reached.

## Further reading

- [Law Format](./law-format) lists the full operation set.
- [RFC-016](/rfcs/rfc-016) records why collection operations look like this, and which alternatives were rejected.
- [Untranslatables](./untranslatables) covers what happens when a construct cannot be modeled at all.
