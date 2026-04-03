# Translation Examples

This page shows how Dutch law is translated into machine-readable YAML, with detailed reasoning about why specific patterns are chosen.

## Example: BW 5:42 and Amsterdam APV — Erfgrensbeplanting

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
# BW 5:42 — naive approach
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
# Amsterdam APV — naive approach
implements:
  - open_term: minimale_afstand_cm

actions:
  - output: minimale_afstand_cm
    value:
      operation: IF
      cases:
        - when: { boom AND centrum }
          then: 100
      default: 50  # ← WRONG: gives 50cm for trees outside centrum
```

The problem: once Amsterdam claims to implement `minimale_afstand_cm`, it must return a value for **every** case. But Amsterdam has nothing to say about trees outside the centrum. If the default is 50, non-centrum trees get 50 cm instead of 200 cm. If we hardcode 200 in the APV, we're putting BW 5:42's value in Amsterdam's regulation — a scope violation.

### The correct translation

The key insight is to read the BW text carefully. It says:

> "twee meter [...] **tenzij** ingevolge een verordening [...] een kleinere afstand is toegelaten"

The "tenzij" (unless) structure tells us exactly how to model this:

1. **The BW sets its own defaults** — these are the rule.
2. **The BW offers an optional delegation** — this is the exception.
3. **The BW decides which to use** — if the delegation produces a value, use it; otherwise, use the default.

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
      # Step 1: The rule — BW's own defaults (lid 2)
      - output: wettelijke_afstand_cm
        value:
          operation: IF
          cases:
            - when:
                operation: EQUALS
                subject: $type_beplanting
                value: boom
              then: 200
          default: 50

      # Step 2: The exception — "tenzij verordening"
      # If a municipality provides a value, use it.
      # If not (null), the rule applies.
      - output: minimale_afstand_cm
        value:
          operation: IF
          cases:
            - when:
                operation: EQUALS
                subject: $gemeentelijke_afstand_cm
                value: null
              then: $wettelijke_afstand_cm
          default: $gemeentelijke_afstand_cm
```

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
        required: true

    output:
      - name: gemeentelijke_afstand_cm
        type: number

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
          # The BW's null-check then uses the statutory 200cm.
```

### Why this works

Each article stays within its own scope:

- **BW 5:42** sets the rule (200/50) and defines the exception mechanism ("tenzij verordening"). Both are in the article text. The null-check is the machine-readable expression of "tenzij" — if no exception exists, the rule applies.

- **Amsterdam APV 2.75** only produces values where it has something to say (centrum bomen: 100, heggen: 50). For cases it doesn't cover (bomen buiten centrum), it returns null — meaning "I have no opinion on this." The BW then applies its own default.

No article hardcodes values from another article. No scope violations. The delegation mechanism faithfully represents the "tenzij" structure of the law.

### The general pattern: "tenzij verordening"

This pattern applies whenever a higher law sets defaults that lower regulations may override:

1. The higher law computes its **own default** as a named output.
2. The higher law declares an **optional open_term** for the override.
3. The higher law uses a **null-check** to choose between the override and the default.
4. The lower regulation **only returns values where it deviates**; null otherwise.

The "tenzij" in the law text is the signal that this pattern applies. The word literally means "unless" — the rule applies *unless* the exception is triggered.

```
Rule: X
Exception: tenzij verordening Y

→ wettelijke_waarde = X
→ gemeentelijke_waarde = open_term (optional, may be null)
→ resultaat = IF gemeentelijke_waarde == null THEN wettelijke_waarde ELSE gemeentelijke_waarde
```
