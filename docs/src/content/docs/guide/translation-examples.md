---
title: "Translation Examples"
description: "Worked examples of translating Dutch law into machine-readable YAML, with the reasoning behind each choice."
---

The examples below show how Dutch law is translated into machine-readable YAML, and why each pattern is chosen.

## Erfgrensbeplanting in BW 5:42 and the Amsterdam APV

### The law

**Burgerlijk Wetboek Boek 5, artikel 42** sets national rules for planting distance from property boundaries:

> Lid 2: Het is niet geoorloofd [...] bomen [...] te hebben [...] dan op **twee meter** [...] en [...] heesters of heggen [...] dan op **een halve meter** [...] **tenzij ingevolge een verordening of een plaatselijke gewoonte een kleinere afstand is toegelaten.**

The structure:
- **Rule**: trees at least 2 meters, hedges at least 0.5 meters from the boundary.
- **Exception** ("tenzij"): municipalities may allow shorter distances via a local regulation (verordening).

**Amsterdam APV artikel 2.75** uses this delegation:

> Lid 1: In afwijking van artikel 5:42 lid 2 van het Burgerlijk Wetboek bedraagt de afstand voor bomen **in het centrum van Amsterdam (postcodegebied 1011-1018)** een meter.
>
> Lid 2: Voor heesters en heggen geldt **in heel Amsterdam** de afstand van een halve meter als bedoeld in artikel 5:42 lid 2 van het Burgerlijk Wetboek.

Amsterdam says two things:
1. Trees in the centrum: 1 meter (reduced from 2).
2. Hedges everywhere in Amsterdam: 0.5 meters (same as the BW default).

Amsterdam is **silent** about trees outside the centrum. For those, the BW default applies.

### The legal outcome

| Case | Who decides | Distance |
|------|-------------|----------|
| Tree in Amsterdam centrum | Amsterdam APV lid 1 | 100 cm |
| Tree outside Amsterdam centrum | BW 5:42 (APV is silent) | 200 cm |
| Hedge anywhere in Amsterdam | Amsterdam APV lid 2 | 50 cm |
| Tree in municipality without verordening | BW 5:42 default | 200 cm |
| Hedge in municipality without verordening | BW 5:42 default | 50 cm |

### Why a naive translation fails

A first attempt might be to have BW 5:42 declare an `open_term` for the distance and let Amsterdam implement it entirely:

```yaml
# BW 5:42, naive approach
open_terms:
  - id: minimale_afstand_cm
    default:
      actions:
        - output: minimale_afstand_cm
          value:
            operation: IF
            cases:
              - when: { operation: EQUALS, subject: $type_beplanting, value: boom }
                then: 200
            default: 50
```

```yaml
# Amsterdam APV, naive approach
implements:
  - open_term: minimale_afstand_cm

actions:
  - output: minimale_afstand_cm
    value:
      operation: IF
      cases:
        - when: { boom AND centrum }
          then: 100
      default: 50  # WRONG: gives 50cm for trees outside centrum
```

The problem: once Amsterdam claims to implement `minimale_afstand_cm`, it must return a value for **every** case. But Amsterdam has nothing to say about trees outside the centrum. If the default is 50, non-centrum trees get 50 cm instead of 200 cm. If we hardcode 200 in the APV, we're putting BW 5:42's value in Amsterdam's regulation, a scope violation.

### The correct translation

Read the BW text carefully. It says:

> "twee meter [...] **tenzij** ingevolge een verordening [...] een kleinere afstand is toegelaten"

The "tenzij" (unless) structure tells us how to model this:

1. **The BW sets its own defaults**: these are the rule.
2. **The BW offers an optional delegation**: this is the exception.
3. **The BW carries its own rule as the term's default**: if the delegation produces a value, the engine uses it; otherwise it falls back to that default.

This maps directly to the YAML:

```yaml
# BW 5:42
machine_readable:
  open_terms:
    - id: gemeentelijke_afstand_cm
      type: number
      required: false                    # "tenzij" = optional
      delegated_to: gemeenteraad
      delegation_type: GEMEENTELIJKE_VERORDENING
      legal_basis: artikel 5:42 lid 2 Burgerlijk Wetboek
      # The rule of lid 2, carried by the term itself.
      default:
        actions:
          - output: gemeentelijke_afstand_cm
            value:
              operation: IF
              cases:
                - when:
                    operation: EQUALS
                    subject: $type_beplanting
                    value: boom
                  then: 200
              default: 50

  execution:
    parameters:
      - name: gemeente_code
        type: string
        required: true
      - name: type_beplanting
        type: string
        required: true

    output:
      - name: minimale_afstand_cm
        type: number

    actions:
      # The applicable distance is the open term: the municipal distance
      # where a verordening allows one, otherwise the rule of lid 2, which
      # is the term's own default.
      - output: minimale_afstand_cm
        value: $gemeentelijke_afstand_cm
```

The article does not write the fallback itself. The rule of lid 2 lives in the
open term's `default:` block, and the engine falls back to it when the
implementing regulation returns null for a case. That resolution is marked in
the trace as `OpenTermSilent`, which is how you tell "Amsterdam set this
distance" apart from "Amsterdam said nothing, so the BW's own rule applies".
See [RFC-036](/rfcs/rfc-036).

The Amsterdam APV only speaks where it has authority:

```yaml
# Amsterdam APV art. 2.75
machine_readable:
  implements:
    - law: burgerlijk_wetboek_boek_5
      article: '42'
      open_term: gemeentelijke_afstand_cm

  execution:
    parameters:
      - name: type_beplanting
        type: string
        required: true
      - name: postcode
        type: number
        required: false        # only lid 1 uses it; lid 2 covers all of Amsterdam

    output:
      - name: gemeentelijke_afstand_cm
        type: number
        nullable: true         # the APV is silent about trees outside the centrum

    actions:
      - output: gemeentelijke_afstand_cm
        value:
          operation: IF
          cases:
            # Lid 1: bomen in centrum (postcodegebied 1011-1018)
            - when:
                operation: AND
                conditions:
                  - operation: EQUALS
                    subject: $type_beplanting
                    value: boom
                  - operation: GREATER_THAN_OR_EQUAL
                    subject: $postcode
                    value: 1011
                  - operation: LESS_THAN_OR_EQUAL
                    subject: $postcode
                    value: 1018
              then: 100
            # Lid 2: heggen in heel Amsterdam
            - when:
                operation: EQUALS
                subject: $type_beplanting
                value: heg_of_heester
              then: 50
          # No default: returns null for trees outside centrum.
          # The open term's own default then gives the statutory 200cm.
```

### Why this works

Each article stays within its own scope:

- **BW 5:42** sets the rule (200/50) and defines the exception mechanism ("tenzij verordening"). Both are in the article text. The open term's `default:` is the machine-readable expression of "tenzij": if no exception exists, the rule applies.

- **Amsterdam APV 2.75** only produces values where it has something to say (centrum bomen: 100, heggen: 50). For cases it doesn't cover (bomen buiten centrum), it returns null, meaning "I have no opinion on this." The BW then applies its own default.

No article hardcodes values from another article, so no scope is violated. The delegation mechanism represents the "tenzij" structure of the law.

### The general pattern: "tenzij verordening"

This pattern applies whenever a higher law sets defaults that lower regulations may override:

1. The higher law declares an **optional open_term** for the override.
2. That term carries the higher law's **own rule** in its `default:` block.
3. The lower regulation **only returns values where it deviates**; null otherwise.
4. The engine takes the term's default whenever the implementation is silent, and marks it `OpenTermSilent` in the trace.

The "tenzij" in the law text is the signal that this pattern applies. The word means "unless": the rule applies unless the exception is triggered.

```
Rule: X
Exception: tenzij verordening Y

→ gemeentelijke_waarde = open_term (optional), default: X
→ resultaat = $gemeentelijke_waarde
→ the engine takes the default when the verordening is silent (OpenTermSilent)
```
