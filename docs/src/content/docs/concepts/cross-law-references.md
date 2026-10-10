---
title: "Cross-Law References"
description: "How a law declares what it needs from other laws, and how the engine resolves those references automatically."
---

Dutch laws reference each other constantly. The Healthcare Allowance Act (*Zorgtoeslagwet*) needs your income, which is defined by the Awir. It needs your insurance status, which comes from the Zorgverzekeringswet. It needs to know whether you have an allowance partner, which the Awir determines.

Rather than duplicating these definitions, each law declares what it needs from other laws. The engine follows these references automatically.

## How it works

An article declares its inputs. When an input has a `source` block pointing to another law, the engine loads that law, executes it with the specified parameters, and feeds the result back.

```yaml
# Zorgtoeslagwet, article 2 - needs income from the Awir
input:
  - name: toetsingsinkomen
    type: amount
    source:
      regulation: algemene_wet_inkomensafhankelijke_regelingen
      output: toetsingsinkomen
      parameters:
        bsn: $bsn
```

The engine loads `algemene_wet_inkomensafhankelijke_regelingen`, executes it for the given BSN, gets the `toetsingsinkomen` output, and uses that value in the healthcare allowance calculation.

## Chains of references

References can chain. The Zorgtoeslagwet references the Awir, which might reference the Wet inkomstenbelasting, which references the BRP. The engine resolves the full chain, loading and executing each law as needed.

```mermaid
flowchart LR
    ZT[Zorgtoeslagwet] -->|toetsingsinkomen| Awir
    ZT -->|heeft_toeslagpartner| Awir
    ZT -->|is_verzekerde| ZVW[Zorgverzekeringswet]
    Awir -->|inkomensgegeven| AWR[Algemene wet inzake rijksbelastingen]
    AWR -->|verzamelinkomen| WIB[Wet inkomstenbelasting]
    WIB -->|persoonsgegevens| BRP[BRP]
```

Results are cached: if two laws both need the same value from the BRP, it is computed once.

## A real example

The Zorgtoeslagwet article 2 declares these cross-law inputs:

```yaml
input:
  - name: is_verzekerde
    type: boolean
    source:
      regulation: zorgverzekeringswet
      output: is_verzekerd
      parameters:
        bsn: $bsn

  - name: heeft_toeslagpartner
    type: boolean
    source:
      regulation: algemene_wet_inkomensafhankelijke_regelingen
      output: heeft_toeslagpartner
      parameters:
        bsn: $bsn

  - name: toetsingsinkomen
    type: amount
    source:
      regulation: algemene_wet_inkomensafhankelijke_regelingen
      output: toetsingsinkomen
      parameters:
        bsn: $bsn
```

Each `source` block says: load this other law, pass it these parameters, and give me the named output.

## Same-law references

Articles within the same law can also reference each other. When `source` has an `output` but no `regulation`, the engine looks within the current law:

```yaml
# Zorgtoeslagwet, article 2 referencing article 4 (same law)
input:
  - name: standaardpremie
    type: amount
    source:
      output: standaardpremie
```

## Which article an output name means

A reference names an output, not an article, so the engine has to find the article that produces it. Some articles produce a name without being what a reference to it means, and the engine passes over them:

- A hook ([RFC-007](/rfcs/rfc-007)) delivers its outputs by firing on a decision. Awir 8 (the toetsingsinkomen) and Awir 16 (the estimated toetsingsinkomen, a hook on the voorschot) both produce `toetsingsinkomen`; a reference to it means article 8. A hook counts only when no ordinary article produces the name, which is how Awb 6:8 reads the bezwaartermijn that the hook Awb 6:7 sets.
- An override or an implementation within the same law replaces or fills that output of another article. The reference means that other article, and the override or implementation applies to it.

When two ordinary articles of one version produce the same name, a reference to it is refused with both article numbers, rather than resolved by the order of the articles in the file. Loading such a law is not refused: the Wlz has articles that each name their own `bevoegd_gezag`, and nothing refers to that name from outside.

## Circular reference detection

The engine detects circular references (law A needs law B which needs law A) and raises an error. A `MAX_CROSS_LAW_DEPTH` limit of 20 prevents runaway chains.

One loop is not an error: a rule that reads the value it departs from. An override reading the output it replaces, or an implementation reading the law whose open term it fills, gets that value without itself. See [Reading the value an override departs from](./hooks-and-reactive-execution#reading-the-value-an-override-departs-from).

## Further reading

- [Law Format](./law-format) - full structure of a rulework
- [Inversion of Control](./inversion-of-control) - a different pattern for cross-law values: delegation
- [Temporal Validity and Dates](./temporal-and-dates) - what happens when a reference points at a law that has ended
- [Traceability](./traceability) - a real cross-law chain shown in an execution trace
- [RFC-007: Cross-Law Execution](/rfcs/rfc-007) - the full design specification
- [Rules as Executed, section 5.1](/research/rules-as-executed#sec:depgraphs) - the position paper's dependency graph behind a single zorgtoeslag decision
