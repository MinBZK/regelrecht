# Law Generate - Technical Reference

Based on schema v0.5.0 (`schema/v0.5.0/schema.json`). Validate with `just validate`.

## Complete Machine-Readable Section Structure

```yaml
machine_readable:
  endpoint: string              # Named endpoint, callable from other regulations
  competent_authority:          # Who has binding authority
    name: "Belastingdienst"
    type: "INSTANCE"            # INSTANCE (default) or CATEGORY
  # OR as internal reference:
  # competent_authority: "#bevoegd_gezag"

  requires:                     # Dependencies (optional)
    - law: "zorgverzekeringswet"
      values: ["is_verzekerd"]
    - article: "11"             # Same-law article reference

  definitions:                  # Constants (optional, arbitrary keys)
    CONSTANT_NAME:
      value: 211200             # Any literal value
      description: "Description"
    # Or simple key-value:
    simple_key: "simple value"

  open_terms:                   # IoC declarations (optional)
    - id: standaardpremie
      type: amount
      required: true
      delegated_to: minister
      delegation_type: MINISTERIELE_REGELING
      legal_basis: artikel 4 Wet op de zorgtoeslag

  implements:                   # IoC fulfillment (optional)
    - law: zorgtoeslagwet
      article: '4'
      open_term: standaardpremie
      gelet_op: Gelet op artikel 4 van de Wet op de zorgtoeslag

  hooks:                        # Reactive execution (optional, RFC-007)
    - hook_point: pre_actions   # pre_actions | post_actions
      applies_to:
        legal_character: BESCHIKKING  # required
        decision_type: TOEKENNING     # optional
        stage: BESLUIT                # optional (default: BESLUIT)

  overrides:                    # Lex specialis declarations (optional, RFC-007)
    - law: algemene_wet_bestuursrecht
      article: '6:7'
      output: bezwaartermijn_weken

  execution:
    produces:                   # Legal character (optional)
      legal_character: BESCHIKKING  # BESCHIKKING | TOETS | WAARDEBEPALING |
                                    # BESLUIT_VAN_ALGEMENE_STREKKING | INFORMATIEF
      decision_type: TOEKENNING     # TOEKENNING | AFWIJZING | GOEDKEURING |
                                    # GEEN_BESLUIT | ALGEMEEN_VERBINDEND_VOORSCHRIFT |
                                    # BELEIDSREGEL | VOORBEREIDINGSBESLUIT |
                                    # ANDERE_HANDELING | AANSLAG
      procedure_id: beschikking     # optional: selects specific AWB procedure variant

    parameters:                 # Caller-provided inputs
      - name: "bsn"
        type: "string"          # string | number | boolean | date
        required: true
        description: "Burgerservicenummer"

    input:                      # Data from external sources
      - name: "toetsingsinkomen"
        type: "amount"          # string | number | boolean | amount | object | array | date
        source:
          regulation: "algemene_wet_inkomensafhankelijke_regelingen"
          output: "toetsingsinkomen"
          parameters:
            bsn: "$bsn"
        type_spec:
          unit: "eurocent"      # eurocent | years | months | weeks | days

    output:                     # What this article produces
      - name: "hoogte_zorgtoeslag"
        type: "amount"
        type_spec:
          unit: "eurocent"
        description: "Hoogte van de zorgtoeslag"

    actions:                    # Computation logic
      - output: "result_name"   # Required: which output to set
        value: <operationValue> # Value assignment (literal, $variable, or operation)
        legal_basis:            # Optional: traceability
          law: "Wet op de zorgtoeslag"
          article: "2"
```

## Procedures (top-level)

Procedures define AWB lifecycle stages for administrative decisions. Declared at the
**top level** of the YAML file (same level as `articles`), typically in the AWB itself.

```yaml
procedure:
  - id: beschikking
    default: true               # Default procedure for this legal_character
    applies_to:
      legal_character: BESCHIKKING
    stages:
      - name: AANVRAAG
        description: Belanghebbende dient aanvraag in (AWB 4:1)
        requires:
          - name: aanvraag_datum
            type: date
      - name: BEHANDELING
        description: Bestuursorgaan onderzoekt de aanvraag (AWB 3:2)
        requires:
          - name: beslistermijn_start
            type: date
      - name: BESLUIT
        description: Bestuursorgaan neemt besluit (AWB 1:3)
        requires:
          - name: besluit_datum
            type: date
      - name: BEKENDMAKING
        description: Besluit wordt bekendgemaakt (AWB 3:41)
        requires:
          - name: bekendmaking_datum
            type: date
      - name: BEZWAAR
        description: Bezwaarperiode (AWB 6:4 e.v.)
```

## Operation Types (all 21)

### Arithmetic Operations — use `values` array
```yaml
operation: ADD              # ADD | SUBTRACT | MULTIPLY | DIVIDE | MIN | MAX
values:
  - $operand_1              # Each item is an operationValue
  - $operand_2              # (literal, $variable, or nested operation)
```

### Logical Operations — use `conditions` array
```yaml
operation: AND              # AND | OR
conditions:
  - operation: EQUALS
    subject: $a
    value: true
  - operation: GREATER_THAN
    subject: $b
    value: 0
```

### NOT — negation, use `value`
```yaml
operation: NOT
value:                      # operationValue (literal, $var, or operation)
  operation: EQUALS
  subject: $is_verzekerd
  value: true
```

Can also negate compound conditions or simple variables:
```yaml
# Negate a compound: "tenzij zowel A als B" → NOT(A AND B)
operation: NOT
value:
  operation: AND
  conditions:
    - operation: EQUALS
      subject: $a
      value: true
    - operation: EQUALS
      subject: $b
      value: true

# Negate a variable directly: NOT($flag)
operation: NOT
value: $heeft_relatieve_weigeringsgrond
```

### Comparison Operations — use `subject` + `value`
```yaml
operation: EQUALS           # EQUALS | GREATER_THAN | LESS_THAN
                            # GREATER_THAN_OR_EQUAL | LESS_THAN_OR_EQUAL
subject: $variable          # MUST be a $variable reference
value: 18                   # operationValue (literal, $var, or operation)
```

### Conditional IF — use `cases` array + `default`
```yaml
operation: IF
cases:
  - when:                   # Condition (operationValue evaluating to boolean)
      operation: EQUALS
      subject: $has_partner
      value: true
    then: $partner_amount   # Value when condition is true (operationValue)
  - when:
      operation: EQUALS
      subject: $categorie
      value: "B"
    then: 75000
default: $single_amount     # Value if no case matches (operationValue, optional)
```

Cases are evaluated in order; the first matching case wins.

### IN — membership test, use `subject` + `value` or `values`
```yaml
# With inline list:
operation: IN
subject: $status
values: ["ACTIEF", "GEPAUZEERD"]

# With single reference (e.g., a LIST output):
operation: IN
subject: $status
value: $allowed_statuses
```

### LIST — construct an array
```yaml
operation: LIST
items:
  - $item_1
  - $item_2
  - "literal_value"
```

### AGE — calculate age in complete years
```yaml
operation: AGE
date_of_birth: $geboortedatum     # Date (operationValue)
reference_date: $peildatum         # Date (operationValue)
```

### DATE_ADD — add duration to a date
```yaml
operation: DATE_ADD
date: $bekendmaking_datum          # Base date (operationValue)
years: 1                           # optional (operationValue)
months: 3                          # optional (operationValue)
weeks: $bezwaartermijn_weken       # optional (operationValue)
days: 1                            # optional (operationValue)
```

Applied coarsest-to-finest: years → months → weeks → days.
Month/year additions use the Dutch legal "corresponding numbered day" rule:
the day is clamped to the last day of the target month (e.g., Jan 31 + 1 month = Feb 28).

### DATE — construct a date from components
```yaml
operation: DATE
year: $jaar                        # Year (operationValue)
month: 1                           # Month 1-12 (operationValue)
day: 1                             # Day 1-31 (operationValue)
```

### DAY_OF_WEEK — get weekday number
```yaml
operation: DAY_OF_WEEK
date: $datum                       # Date (operationValue)
# Returns: 0=Monday, 1=Tuesday, ..., 6=Sunday
```

## Variable References

Pattern: `$name` or `$name.property` (dot notation for nested access)

```yaml
# Parameter reference
subject: $bsn

# Input reference
subject: $toetsingsinkomen

# Definition/constant reference
value: $STANDAARDPREMIE

# Previous action output reference
subject: $intermediate_result
```

## Source Formats (for input fields)

### External Law Reference
```yaml
source:
  regulation: "regeling_standaardpremie"   # Law/regulation $id
  output: "standaardpremie"                # Output field to retrieve
  parameters:                              # Parameters to pass (optional)
    bsn: $bsn
```

### Internal Reference (same law)
```yaml
source:
  output: "vermogen_onder_grens"           # Output from another article
  # No regulation field = same law
```

### Open Terms (IoC — Inversion of Control)

When a higher law delegates a value to a lower regulation (e.g., "bij ministeriële
regeling" or "bij gemeentelijke verordening"), use the `open_terms` + `implements` pattern:

**Higher law** declares an open term:
```yaml
machine_readable:
  open_terms:
    - id: standaardpremie
      type: amount
      required: true
      delegated_to: minister
      delegation_type: MINISTERIELE_REGELING
      legal_basis: artikel 4 Wet op de zorgtoeslag
  execution:
    output:
      - name: standaardpremie
        type: amount
        type_spec:
          unit: eurocent
    actions:
      - output: standaardpremie
        value: $standaardpremie   # Engine resolves via implements_index
```

**Lower regulation** registers as implementing:
```yaml
machine_readable:
  implements:
    - law: zorgtoeslagwet
      article: '4'
      open_term: standaardpremie
      gelet_op: Gelet op artikel 4 van de Wet op de zorgtoeslag
  execution:
    output:
      - name: standaardpremie
        type: amount
        type_spec:
          unit: eurocent
    actions:
      - output: standaardpremie
        value: 211200
```

The engine automatically resolves `$standaardpremie` by finding the regulation
that `implements` the open term, using lex superior / lex posterior priority rules.

## Hooks — Reactive Execution

Hooks allow articles to fire automatically when matching lifecycle events occur.
Used by the AWB for cross-cutting requirements (motivation, appeal deadlines).

```yaml
machine_readable:
  hooks:
    - hook_point: pre_actions      # Fires BEFORE the target article's actions
      applies_to:
        legal_character: BESCHIKKING   # Required: match articles producing this
        decision_type: TOEKENNING      # Optional: narrow to decision type
        stage: BESLUIT                 # Optional: lifecycle stage (default: BESLUIT)
    - hook_point: post_actions     # Fires AFTER the target article's actions
      applies_to:
        legal_character: BESCHIKKING
        stage: BEKENDMAKING
  execution:
    # Normal execution section — output, actions, etc.
```

Valid `hook_point` values: `pre_actions`, `post_actions`
Valid `legal_character` values: `BESCHIKKING`, `TOETS`, `WAARDEBEPALING`,
`BESLUIT_VAN_ALGEMENE_STREKKING`, `INFORMATIEF`

## Overrides — Lex Specialis

When a specific law needs to replace an output from a more general law:

```yaml
machine_readable:
  overrides:
    - law: algemene_wet_bestuursrecht   # $id of the law being overridden
      article: '6:7'                     # Article number being overridden
      output: bezwaartermijn_weken       # Specific output being replaced
  execution:
    output:
      - name: bezwaartermijn_weken
        type: number
    actions:
      - output: bezwaartermijn_weken
        value: 4
```

The engine uses the override output instead of the original when the overriding
law is in scope (lex specialis principle).

## Regulatory Layers

```yaml
regulatory_layer: WET  # One of:
# GRONDWET | WET | AMVB | KONINKLIJK_BESLUIT | MINISTERIELE_REGELING |
# BELEIDSREGEL | EU_VERORDENING | EU_RICHTLIJN | VERDRAG |
# UITVOERINGSBELEID | GEMEENTELIJKE_VERORDENING | PROVINCIALE_VERORDENING
```

## Eurocent Conversion Table

| Written Amount | Eurocent Value | Note |
|----------------|----------------|------|
| €1 | 100 | |
| €10 | 1000 | |
| €100 | 10000 | |
| €795,47 | 79547 | comma = decimal separator |
| €2.112 | 211200 | dot = thousands separator (two thousand one hundred twelve) |
| €79.547 | 7954700 | dot = thousands separator (seventy-nine thousand) |
| €154.859 | 15485900 | dot = thousands separator |
| €1.000.000 | 100000000 | dots = thousands separators (one million) |

**Dutch number format:** In Dutch, `.` is the thousands separator and `,` is the decimal separator.
This is the opposite of English. So `€1.234,56` means one thousand two hundred thirty-four euro and fifty-six cents.

**Rules:**
1. Remove currency symbol (€)
2. Remove thousands separators (.) — these are the dots between digit groups (e.g., `1.000.000`)
3. Replace decimal comma (,) with decimal point (.) — this is the comma before cents (e.g., `795,47` → `795.47`)
4. Parse as decimal number (euros) — e.g., `795.47`
5. Multiply by 100 and round to integer — e.g., `795.47 × 100 = 79547`

**Examples applying the rules:**
- `€2.112` → remove `€` → `2.112` → remove thousands `.` → `2112` → no decimal comma → parse `2112.0` → × 100 = `211200`
- `€795,47` → remove `€` → `795,47` → no thousands sep → `795,47` → replace `,` with `.` → parse `795.47` → × 100 = `79547`
- `€1.234,56` → remove `€` → `1.234,56` → remove thousands `.` → `1234,56` → replace `,` with `.` → parse `1234.56` → × 100 = `123456`

## Common Legal Phrases → Operations

| Dutch Legal Phrase | Operation Pattern |
|-------------------|------------------|
| "heeft bereikt de leeftijd van X jaar" | `AGE` + `GREATER_THAN_OR_EQUAL`, value: X |
| "ten minste X" | `GREATER_THAN_OR_EQUAL`, value: X |
| "niet meer dan X" | `LESS_THAN_OR_EQUAL`, value: X |
| "minder dan X" | `LESS_THAN`, value: X |
| "meer dan X" | `GREATER_THAN`, value: X |
| "gelijk aan X" | `EQUALS`, value: X |
| "vermenigvuldigd met" | `MULTIPLY`, values: [...] |
| "gedeeld door" | `DIVIDE`, values: [...] |
| "vermeerderd met" | `ADD`, values: [...] |
| "verminderd met" | `SUBTRACT`, values: [...] |
| "indien ... en ..." | `AND`, conditions: [...] |
| "indien ... of ..." | `OR`, conditions: [...] |
| "tenzij" / "niet" | `NOT`, value: ... |
| "ingevolge" | Cross-law reference via source.regulation |
| "bedoeld in artikel X" | Internal reference via source.output |
| "binnen X weken na" | `DATE_ADD`, date: ..., weeks: X |
| "in afwijking van artikel X" | `overrides` declaration |
| "bij ministeriële regeling" | `open_terms` + `implements` IoC pattern |

## Data Type Mapping

### Common Parameters
| Legal Concept | Parameter Name | Type |
|--------------|---------------|------|
| Citizen | bsn | string |
| Date | peildatum | date |
| Year | jaar | number |
| Municipality | gemeente_code | string |

### Common Input Fields
| Legal Concept | Input Name | Type | Source |
|--------------|-----------|------|--------|
| Age | leeftijd | number | wet_basisregistratie_personen |
| Insured status | is_verzekerd | boolean | zorgverzekeringswet |
| Partner status | heeft_toeslagpartner | boolean | algemene_wet_inkomensafhankelijke_regelingen |
| Test income | toetsingsinkomen | amount | algemene_wet_inkomensafhankelijke_regelingen |
| Assets | vermogen | amount | belastingdienst |

### Common Outputs
| Legal Concept | Output Name | Type | type_spec |
|--------------|------------|------|-----------|
| Eligibility | heeft_recht | boolean | — |
| Amount | hoogte_toeslag | amount | unit: eurocent |
| Below threshold | onder_grens | boolean | — |

## Debugging Tips

1. **Run `just validate <file>`** — catches schema violations with exact paths
2. **Check action patterns**: `value:` for assignments/operations, `operation:`+`values:` for arithmetic only
3. **IF uses cases/default** — NOT when/then/else or condition/then_value/else_value
4. **Arithmetic uses values array** — NOT subject/value
5. **Logical uses conditions array** — NOT values
6. **Comparison uses subject (must be $var)** — and value
7. **NOT uses value** — NOT conditions or subject
8. **Source uses regulation/output** — NOT url
9. **Monetary fields**: type `amount` with `type_spec: { unit: eurocent }`
10. **AGE uses date_of_birth/reference_date** — NOT subject/value/unit
11. **DATE_ADD uses date + optional years/months/weeks/days** — NOT subject/value
12. **$referencedate is NOT a built-in** — must be declared as a parameter

## External Resources

- **Schema**: `schema/v0.5.0/schema.json` (v0.5.0)
- **Working examples**:
  - `corpus/regulation/nl/wet/wet_op_de_zorgtoeslag/2025-01-01.yaml` — basic patterns
  - `corpus/regulation/nl/wet/algemene_wet_bestuursrecht/2026-01-01.yaml` — hooks, procedures
  - `corpus/regulation/nl/wet/vreemdelingenwet_2000/2026-01-01.yaml` — overrides
  - `corpus/regulation/nl/wet/wet_open_overheid/2025-02-12.yaml` — AGE, complex IF
- **Engine source**: `packages/engine/src/`
- **Validation binary**: `packages/engine/src/bin/validate.rs`
