# Law Machine-Readable Interpreter - Technical Reference

## Schema Reference

### Complete Machine-Readable Section Structure

```yaml
machine_readable:
  definitions:                 # Optional: Constants
    CONSTANT_NAME:
      value: any               # Constant value (number, string, boolean)
      description: string      # Human-readable description

  execution:
    parameters:                # Required inputs from caller
      - name: string           # Parameter name
        type: string           # "string" | "number" | "boolean" | "date"
        required: boolean      # Is this required?
        description: string    # Human-readable description

    input:                     # Data from external sources
      - name: string           # Variable name
        type: string           # Data type
        source:
          url: string          # URI: "regulation/nl/..." or "#field"
          parameters:          # Parameters to pass to source
            key: string        # "$variable" or literal value

    output:                    # What this article produces
      - name: string           # Output field name
        type: string           # Data type
        description: string    # Human-readable description

    actions:                   # Execution logic
      - output: string         # Which output field to set
        operation: string      # Operation type (see below)
        subject: any           # Left operand
        value: any             # Right operand (optional)
        conditions: array      # For AND/OR operations
        condition: object      # For NOT/IF_THEN_ELSE
        then_value: any        # For IF_THEN_ELSE
        else_value: any        # For IF_THEN_ELSE
```

## Operation Types

### Comparison Operations

**EQUALS**
```yaml
operation: "EQUALS"
subject: "$variable"
value: "expected_value"
```
Returns: boolean

**NOT_EQUALS**
```yaml
operation: "NOT_EQUALS"
subject: "$variable"
value: "unexpected_value"
```
Returns: boolean

**GREATER_THAN**
```yaml
operation: "GREATER_THAN"
subject: "$amount"
value: 1000
```
Returns: boolean

**GREATER_THAN_OR_EQUAL**
```yaml
operation: "GREATER_THAN_OR_EQUAL"
subject: "$age"
value: 18
```
Returns: boolean

**LESS_THAN**
```yaml
operation: "LESS_THAN"
subject: "$income"
value: 50000
```
Returns: boolean

**LESS_THAN_OR_EQUAL**
```yaml
operation: "LESS_THAN_OR_EQUAL"
subject: "$vermogen"
value: "$VERMOGENSGRENS"
```
Returns: boolean

### Logical Operations

**AND**
```yaml
operation: "AND"
conditions:
  - operation: "EQUALS"
    subject: "$is_verzekerd"
    value: true
  - operation: "GREATER_THAN_OR_EQUAL"
    subject: "$leeftijd"
    value: 18
  - operation: "EQUALS"
    subject: "$woont_in_nederland"
    value: true
```
Returns: boolean (true if ALL conditions are true)

**OR**
```yaml
operation: "OR"
conditions:
  - operation: "EQUALS"
    subject: "$status"
    value: "A"
  - operation: "EQUALS"
    subject: "$status"
    value: "B"
```
Returns: boolean (true if ANY condition is true)

**NOT**
```yaml
operation: "NOT"
condition:
  operation: "EQUALS"
  subject: "$heeft_partner"
  value: true
```
Returns: boolean (inverts the condition)

### Arithmetic Operations

**ADD**
```yaml
operation: "ADD"
subject: "$bedrag1"
value: "$bedrag2"
```
Returns: number (subject + value)

**SUBTRACT**
```yaml
operation: "SUBTRACT"
subject: "$totaal"
value: "$korting"
```
Returns: number (subject - value)

**MULTIPLY**
```yaml
operation: "MULTIPLY"
subject: "$basis"
value: "$percentage"
```
Returns: number (subject * value)

**DIVIDE**
```yaml
operation: "DIVIDE"
subject: "$totaal"
value: 12
```
Returns: number (subject / value)

### Conditional Operations

**IF_THEN_ELSE**
```yaml
operation: "IF_THEN_ELSE"
condition:
  operation: "GREATER_THAN"
  subject: "$leeftijd"
  value: 65
then_value: "$hoog_tarief"
else_value: "$laag_tarief"
```
Returns: then_value if condition is true, else_value otherwise

## Variable References

### Syntax

**Parameter reference:** `$parameter_name`
```yaml
parameters:
  BSN: "$BSN"  # Pass the BSN parameter value
```

**Input reference:** `$input_name`
```yaml
subject: "$toetsingsinkomen"  # Use the toetsingsinkomen input
```

**Constant reference:** `$CONSTANT_NAME`
```yaml
value: "$VERMOGENSGRENS"  # Use the constant value
```

**Nested operation result:** `$intermediate_output`
```yaml
actions:
  - output: "premie_basis"
    operation: "MULTIPLY"
    subject: "$standaardpremie"
    value: 1.5

  - output: "premie_finaal"
    operation: "SUBTRACT"
    subject: "$premie_basis"  # Reference previous output
    value: "$korting"
```

### Literal Values

**Numbers:**
```yaml
value: 18           # Integer
value: 15485900     # Eurocent amount
```

**Strings:**
```yaml
value: "ACTIEF"
value: "NEDERLAND"
```

**Booleans:**
```yaml
value: true
value: false
```

## URI Formats

### Internal Reference (Same File)

Format: `#output_name`

```yaml
source:
  url: "#vermogen_onder_grens"
  parameters:
    BSN: "$BSN"
```

This calls another article in the same file and extracts the `vermogen_onder_grens` output field.

### External Reference (Different File)

Format: `regulation/nl/{layer}/{law_id}#{output_name}`

```yaml
source:
  url: "regulation/nl/ministeriele_regeling/regeling_standaardpremie#standaardpremie"
```

This calls an article in another law file.

**Layers:**
- `wet`
- `ministeriele_regeling`
- `amvb`
- `koninklijk_besluit`
- `verordening`
- `beleidsregel`

### TODO Placeholder (Missing Law)

```yaml
source:
  # TODO: Implement Zorgverzekeringswet
  # Should reference: regulation/nl/wet/zorgverzekeringswet#is_verzekerd
  url: "TODO_zorgverzekeringswet"
  parameters:
    BSN: "$BSN"
```

## Eurocent Conversion Table

| Written Amount | Eurocent Value |
|----------------|----------------|
| €1 | 100 |
| €10 | 1000 |
| €100 | 10000 |
| €795,47 | 79547 |
| €2.112 | 211200 |
| €79.547 | 7954700 |
| €154.859 | 15485900 |
| €1.000.000 | 100000000 |

**Conversion Rules:**
1. Remove currency symbol (€)
2. Remove thousands separators (. or ,)
3. Replace decimal separator (, or .) with nothing
4. Multiply by 100 (or just treat as eurocent)
5. Result must be integer

**Examples:**
- "€795,47" → Remove € → "795,47" → Remove comma → "79547" → 79547
- "€2.112" → Remove € → "2.112" → Remove dot → "2112" → 211200 (multiply by 100)
- "€154.859" → Remove € → "154.859" → "154859" → 15485900

## Common Legal Phrases → Operations

| Dutch Legal Phrase | English | Operation Pattern |
|-------------------|---------|------------------|
| "heeft bereikt de leeftijd van X jaar" | has reached age X | `GREATER_THAN_OR_EQUAL`, subject: leeftijd, value: X |
| "ten minste X" | at least X | `GREATER_THAN_OR_EQUAL`, value: X |
| "niet meer dan X" | no more than X | `LESS_THAN_OR_EQUAL`, value: X |
| "minder dan X" | less than X | `LESS_THAN`, value: X |
| "meer dan X" | more than X | `GREATER_THAN`, value: X |
| "gelijk aan X" | equal to X | `EQUALS`, value: X |
| "vermenigvuldigd met" | multiplied by | `MULTIPLY` |
| "gedeeld door" | divided by | `DIVIDE` |
| "vermeerderd met" | increased by | `ADD` |
| "verminderd met" | decreased by | `SUBTRACT` |
| "indien ... en ..." | if ... and ... | `AND` operation |
| "indien ... of ..." | if ... or ... | `OR` operation |
| "tenzij" | unless | `NOT` operation |
| "ingevolge" | pursuant to | Cross-law reference |
| "bedoeld in artikel X" | referred to in article X | Internal reference |

## Data Type Mapping

### Common Parameters

| Legal Concept | Parameter Name | Type | Example |
|--------------|---------------|------|---------|
| Citizen | BSN | string | "999993653" |
| Date | peildatum | date | "2025-01-01" |
| Year | jaar | integer | 2025 |
| Amount | bedrag | number | 79547 (eurocent) |
| Age | leeftijd | integer | 18 |
| Income | inkomen | number | 7954700 (eurocent) |
| Assets | vermogen | number | 15485900 (eurocent) |

### Common Inputs

| Legal Concept | Input Name | Type | Source |
|--------------|-----------|------|--------|
| Birth date | geboortedatum | date | BRP |
| Age | leeftijd | integer | Calculated from geboortedatum |
| Insured status | is_verzekerd | boolean | ZVW |
| Residence | woont_in_nederland | boolean | BRP |
| Partner status | heeft_partner | boolean | AWIR |
| Test income | toetsingsinkomen | number | Belastingdienst |
| Assets | vermogen | number | Belastingdienst |

### Common Outputs

| Legal Concept | Output Name | Type | Description |
|--------------|------------|------|-------------|
| Eligibility | heeft_recht | boolean | Has right to benefit |
| Amount | bedrag | number | Benefit amount (eurocent) |
| Percentage | percentage | number | Percentage value |
| Below threshold | onder_grens | boolean | Below limit |
| Age check | is_volwassen | boolean | Is adult (18+) |

## Example Interpretations

### Example 1: Simple Constant

**Legal Text:**
```
De standaardpremie bedraagt € 2.112.
```

**Interpretation:**
```yaml
machine_readable:
  definitions:
    STANDAARDPREMIE:
      value: 211200  # €2.112 in eurocent
      description: "Standaardpremie zorgverzekering"

  execution:
    output:
      - name: "standaardpremie"
        type: "number"
        description: "Standaardpremie in eurocenten"

    actions:
      - output: "standaardpremie"
        operation: "EQUALS"
        subject: "$STANDAARDPREMIE"
        value: "$STANDAARDPREMIE"
```

### Example 2: Age Check with Cross-Reference

**Legal Text:**
```
Een persoon heeft recht indien hij de leeftijd van 18 jaar heeft bereikt.
```

**Interpretation:**
```yaml
machine_readable:
  execution:
    parameters:
      - name: "BSN"
        type: "string"
        required: true
        description: "Burgerservicenummer"

    input:
      - name: "geboortedatum"
        type: "date"
        source:
          # TODO: Implement BRP (Basisregistratie Personen)
          url: "TODO_brp"
          parameters:
            BSN: "$BSN"

    output:
      - name: "heeft_recht"
        type: "boolean"
        description: "Heeft de persoon recht (18+)?"

    actions:
      - output: "leeftijd"
        operation: "CALCULATE_AGE"
        subject: "$geboortedatum"

      - output: "heeft_recht"
        operation: "GREATER_THAN_OR_EQUAL"
        subject: "$leeftijd"
        value: 18
```

### Example 3: Complex Conditions (AND)

**Legal Text:**
```
Een persoon heeft recht op zorgtoeslag indien hij:
a. de leeftijd van 18 jaar heeft bereikt;
b. verzekerd is ingevolge de Zorgverzekeringswet;
c. in Nederland woont.
```

**Interpretation:**
```yaml
machine_readable:
  execution:
    parameters:
      - name: "BSN"
        type: "string"
        required: true

    input:
      - name: "leeftijd"
        type: "integer"
        source:
          url: "TODO_brp"
          parameters:
            BSN: "$BSN"

      - name: "is_verzekerd"
        type: "boolean"
        source:
          # TODO: Implement Zorgverzekeringswet
          url: "TODO_zvw"
          parameters:
            BSN: "$BSN"

      - name: "woont_in_nederland"
        type: "boolean"
        source:
          url: "TODO_brp"
          parameters:
            BSN: "$BSN"

    output:
      - name: "bepaal_recht_op_zorgtoeslag"
        type: "boolean"
        description: "Heeft recht op zorgtoeslag"

    actions:
      - output: "bepaal_recht_op_zorgtoeslag"
        operation: "AND"
        conditions:
          - operation: "GREATER_THAN_OR_EQUAL"
            subject: "$leeftijd"
            value: 18
          - operation: "EQUALS"
            subject: "$is_verzekerd"
            value: true
          - operation: "EQUALS"
            subject: "$woont_in_nederland"
            value: true
```

### Example 4: Calculation with Cross-Reference

**Legal Text:**
```
Het toetsingsinkomen bedraagt niet meer dan de standaardpremie,
vermenigvuldigd met het normpremiepercentage van 6,68%.
```

**Interpretation:**
```yaml
machine_readable:
  definitions:
    NORMPREMIEPERCENTAGE:
      value: 6.68
      description: "Normpremiepercentage voor zorgtoeslag"

  execution:
    parameters:
      - name: "BSN"
        type: "string"
        required: true

    input:
      - name: "standaardpremie"
        type: "number"
        source:
          url: "regulation/nl/ministeriele_regeling/regeling_standaardpremie#standaardpremie"

      - name: "toetsingsinkomen"
        type: "number"
        source:
          # TODO: Implement AWIR
          url: "TODO_awir"
          parameters:
            BSN: "$BSN"

    output:
      - name: "voldoet_aan_inkomenseis"
        type: "boolean"
        description: "Toetsingsinkomen onder grens"

    actions:
      - output: "inkomensgrens"
        operation: "MULTIPLY"
        subject: "$standaardpremie"
        value: "$NORMPREMIEPERCENTAGE"

      - output: "voldoet_aan_inkomenseis"
        operation: "LESS_THAN_OR_EQUAL"
        subject: "$toetsingsinkomen"
        value: "$inkomensgrens"
```

### Example 5: Internal Reference

**Legal Text (Article 2):**
```
Een persoon heeft recht indien het vermogen niet meer bedraagt
dan de in artikel 3 genoemde grens.
```

**Legal Text (Article 3):**
```
De grens bedraagt € 154.859 voor een alleenstaande.
```

**Interpretation (Article 2):**
```yaml
machine_readable:
  execution:
    parameters:
      - name: "BSN"
        type: "string"
        required: true

    input:
      - name: "vermogen_onder_grens"
        type: "boolean"
        source:
          url: "#vermogen_onder_grens"  # Internal reference to article 3
          parameters:
            BSN: "$BSN"

    output:
      - name: "bepaal_recht"
        type: "boolean"

    actions:
      - output: "bepaal_recht"
        operation: "EQUALS"
        subject: "$vermogen_onder_grens"
        value: true
```

**Interpretation (Article 3):**
```yaml
machine_readable:
  definitions:
    VERMOGENSGRENS:
      value: 15485900  # €154.859 in eurocent
      description: "Vermogensgrens voor alleenstaande"

  execution:
    parameters:
      - name: "BSN"
        type: "string"
        required: true

    input:
      - name: "vermogen"
        type: "number"
        source:
          # TODO: Implement Belastingdienst vermogen check
          url: "TODO_belastingdienst"
          parameters:
            BSN: "$BSN"

    output:
      - name: "vermogen_onder_grens"
        type: "boolean"

    actions:
      - output: "vermogen_onder_grens"
        operation: "LESS_THAN_OR_EQUAL"
        subject: "$vermogen"
        value: "$VERMOGENSGRENS"
```

## Debugging Tips

1. **Check variable scope**: Ensure `$variable` references are defined
2. **Verify operation types**: Match operation to expected return type
3. **Test cross-references**: Confirm `#field` references exist in same file
4. **Validate eurocent**: Double-check monetary conversions
5. **Check TODO comments**: List all missing external dependencies
6. **Type consistency**: Boolean operations should return boolean, etc.
7. **Parameter passing**: Ensure parameters match between caller and callee
8. **Nested operations**: Complex operations may need intermediate outputs

## External Resources

- **Schema v0.2.0:** https://raw.githubusercontent.com/MinBZK/poc-machine-law/refs/heads/main/schema/v0.2.0/schema.json
- **Project CLAUDE.md:** See `/Users/anneschuth/regelrecht-mvp/CLAUDE.md`
- **Existing Laws:** Browse `regulation/nl/` for examples
- **Engine Reference:** See `engine/` directory for execution details
