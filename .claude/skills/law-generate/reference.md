# Law Generate - Technical Reference

Based on schema v0.3.2 (`schema/latest/schema.json`). Validate with `just validate`.

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

  execution:
    produces:                   # Legal character (optional)
      legal_character: BESCHIKKING  # BESCHIKKING | TOETS | WAARDEBEPALING |
                                    # BESLUIT_VAN_ALGEMENE_STREKKING | INFORMATIEF
      decision_type: TOEKENNING     # TOEKENNING | AFWIJZING | GOEDKEURING |
                                    # GEEN_BESLUIT | ALGEMEEN_VERBINDEND_VOORSCHRIFT |
                                    # BELEIDSREGEL | VOORBEREIDINGSBESLUIT |
                                    # ANDERE_HANDELING | AANSLAG

    parameters:                 # Caller-provided inputs
      - name: "bsn"
        type: "string"          # string | number | boolean | date
        required: true
        description: "Burgerservicenummer"

    input:                      # Data from external sources
      - name: "toetsingsinkomen"
        type: "amount"          # string | number | boolean | amount | object | array | date
        source:
          regulation: "awir"    # External law/regulation ID
          output: "toetsingsinkomen"  # Output field to retrieve
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
        value: <operationValue> # Pattern 1: value assignment
        # OR
        operation: "ADD"        # Pattern 2: top-level arithmetic
        values: [...]
        # OR
        resolve:                # Pattern 3: ministeriele regeling lookup
          type: ministeriele_regeling
          output: standaardpremie
          match:
            output: berekeningsjaar
            value: $referencedate.year
        legal_basis:            # Optional: traceability
          law: "Wet op de zorgtoeslag"
          article: "2"
```

## Operation Types (all 23)

### Arithmetic Operations — use `values` array
```yaml
operation: ADD              # ADD | SUBTRACT | MULTIPLY | DIVIDE | MIN | MAX
values:
  - $operand_1              # Each item is an operationValue
  - $operand_2              # (literal, $variable, or nested operation)
```

### String Concatenation — use `values` array
```yaml
operation: CONCAT
values:
  - "Beschikking inzake "
  - $wet_naam
  - " voor BSN "
  - $bsn
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

### Comparison Operations — use `subject` + `value`
```yaml
operation: EQUALS           # EQUALS | NOT_EQUALS | GREATER_THAN | LESS_THAN
                            # GREATER_THAN_OR_EQUAL | LESS_THAN_OR_EQUAL
                            # IN | NOT_IN
subject: $variable          # MUST be a $variable reference
value: 18                   # operationValue (literal, $var, or operation)
```

### Null Check — `subject` only
```yaml
operation: NOT_NULL
subject: $field
```

### Conditional IF — use `when`/`then`/`else`
```yaml
operation: IF
when:                       # Condition (operationValue that evaluates to boolean)
  operation: EQUALS
  subject: $has_partner
  value: true
then: $partner_amount       # Value when true (operationValue)
else: $single_amount        # Value when false (operationValue, optional)
```

### SWITCH — use `cases` array
```yaml
operation: SWITCH
cases:
  - when:
      operation: EQUALS
      subject: $type
      value: "A"
    then: 100000
  - when:
      operation: EQUALS
      subject: $type
      value: "B"
    then: 75000
default: 50000              # Fallback value
```

### Date Operations — use `subject` + `value` + `unit`
```yaml
operation: SUBTRACT_DATE
subject: $peildatum         # First date (minuend)
value: $geboortedatum       # Second date (subtrahend)
unit: years                 # days | months | years
```

### NOT — negation

Negates a boolean condition. Use `value:` containing the operation to negate.

```yaml
# "tenzij de persoon verzekerd is" → NOT(is_verzekerd == true)
operation: NOT
value:
  operation: EQUALS
  subject: $is_verzekerd
  value: true
```

Can also negate compound conditions:
```yaml
# "tenzij zowel A als B" → NOT(A AND B)
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
```

### FOREACH — iteration over arrays

Iterates over a collection, applying an operation to each item. Uses dot notation
(`$item.field`) to access properties of each element.

```yaml
# Sum all line item amounts: for each item, multiply bedrag × percentage
operation: FOREACH
collection: $items
item_variable: $item
value:
  operation: MULTIPLY
  values:
    - $item.bedrag
    - $item.percentage
```

**Note:** Both `NOT` and `FOREACH` use `additionalProperties: true` in the schema,
which means they accept custom field names beyond `operation`. The examples above
show the established conventions (`value` for NOT, `collection`/`item_variable`/`value`
for FOREACH). Always follow these conventions for consistency.

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

# Dot notation for property access
value: $referencedate.year
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

### Delegated Regulation (e.g., gemeentelijke verordening)
```yaml
source:
  delegation:
    law_id: participatiewet
    article: "8"
    select_on:
      - name: gemeente_code
        value: $gemeente_code
  output: verlaging_percentage
  parameters:
    bsn: $bsn
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
| "heeft bereikt de leeftijd van X jaar" | `GREATER_THAN_OR_EQUAL`, subject: $leeftijd, value: X |
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
| "tenzij" | `NOT` |
| "ingevolge" | Cross-law reference via source.regulation |
| "bedoeld in artikel X" | Internal reference via source.output |

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
| Partner status | heeft_toeslagpartner | boolean | awir |
| Test income | toetsingsinkomen | amount | awir |
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
3. **IF uses when/then/else** — NOT condition/then_value/else_value
4. **Arithmetic uses values array** — NOT subject/value
5. **Logical uses conditions array** — NOT values
6. **Comparison uses subject (must be $var)** — and value
7. **Source uses regulation/output** — NOT url
8. **Monetary fields**: type `amount` with `type_spec: { unit: eurocent }`

## External Resources

- **Schema**: `schema/latest/schema.json` (v0.3.2)
- **Working example**: `regulation/nl/wet/wet_op_de_zorgtoeslag/2025-01-01.yaml`
- **Engine source**: `packages/engine/src/`
- **Validation binary**: `packages/engine/src/bin/validate.rs`
