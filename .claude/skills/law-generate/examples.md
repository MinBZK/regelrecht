# Law Generate - Usage Examples

All examples below conform to schema v0.4.0 and pass `just validate`.

## Example 1: Simple Constant (direct value assignment)

**Legal Text:**
```
De standaardpremie bedraagt € 2.112 per jaar.
```

**machine_readable:**
```yaml
machine_readable:
  execution:
    output:
      - name: standaardpremie
        type: amount
        type_spec:
          unit: eurocent
    actions:
      - output: standaardpremie
        value: 211200  # €2.112 in eurocent
```

---

## Example 2: Eligibility Check (AND with comparisons)

**Legal Text:**
```
Een persoon heeft recht op zorgtoeslag indien hij:
a. de leeftijd van 18 jaar heeft bereikt;
b. verzekerd is ingevolge de Zorgverzekeringswet;
c. in Nederland woont.
```

**machine_readable:**
```yaml
machine_readable:
  execution:
    produces:
      legal_character: BESCHIKKING
      decision_type: TOEKENNING
    parameters:
      - name: bsn
        type: string
        required: true
        description: Burgerservicenummer
    input:
      - name: leeftijd
        type: number
        source:
          regulation: wet_basisregistratie_personen
          output: leeftijd
          parameters:
            bsn: $bsn
      - name: is_verzekerd
        type: boolean
        source:
          regulation: zorgverzekeringswet
          output: is_verzekerd
          parameters:
            bsn: $bsn
      - name: woont_in_nederland
        type: boolean
        source:
          regulation: wet_basisregistratie_personen
          output: woont_in_nederland
          parameters:
            bsn: $bsn
    output:
      - name: heeft_recht_op_zorgtoeslag
        type: boolean
        description: Geeft aan of de persoon recht heeft op zorgtoeslag
    actions:
      - output: heeft_recht_op_zorgtoeslag
        value:
          operation: AND
          conditions:
            - operation: GREATER_THAN_OR_EQUAL
              subject: $leeftijd
              value: 18
            - operation: EQUALS
              subject: $is_verzekerd
              value: true
            - operation: EQUALS
              subject: $woont_in_nederland
              value: true
```

**Key points:**
- `source` uses `regulation` + `output` (NOT `url`)
- `AND` uses `conditions` array
- Comparisons use `subject` (must be `$variable`) + `value`
- Action uses `value:` pattern (not top-level `operation:`)

---

## Example 3: Internal Reference Between Articles

**Article 2** references article 3's output:
```yaml
# Article 2
machine_readable:
  execution:
    parameters:
      - name: bsn
        type: string
        required: true
    input:
      - name: vermogen_onder_grens
        type: boolean
        source:
          output: vermogen_onder_grens  # Internal reference (no regulation field)
    output:
      - name: heeft_recht
        type: boolean
    actions:
      - output: heeft_recht
        value:
          operation: EQUALS
          subject: $vermogen_onder_grens
          value: true
```

**Article 3** provides the referenced output:
```yaml
# Article 3
machine_readable:
  definitions:
    VERMOGENSGRENS_ALLEENSTAAND:
      value: 15485900  # €154.859 in eurocent
    VERMOGENSGRENS_GEHUWD:
      value: 18687500  # €186.875 in eurocent
  execution:
    parameters:
      - name: bsn
        type: string
        required: true
    input:
      - name: vermogen
        type: amount
        source:
          description: "Vermogen van de belanghebbende (extern)"
        type_spec:
          unit: eurocent
      - name: heeft_partner
        type: boolean
        source:
          regulation: algemene_wet_inkomensafhankelijke_regelingen
          output: heeft_toeslagpartner
          parameters:
            bsn: $bsn
    output:
      - name: vermogen_onder_grens
        type: boolean
        description: Is vermogen onder de toepasselijke grens?
    actions:
      - output: toepasselijke_grens
        value:
          operation: IF
          when:
            operation: EQUALS
            subject: $heeft_partner
            value: true
          then: $VERMOGENSGRENS_GEHUWD
          else: $VERMOGENSGRENS_ALLEENSTAAND
      - output: vermogen_onder_grens
        value:
          operation: LESS_THAN_OR_EQUAL
          subject: $vermogen
          value: $toepasselijke_grens
```

**Key points:**
- Internal reference: `source: { output: "vermogen_onder_grens" }` (no `regulation`)
- `IF` uses `when`/`then`/`else` (NOT `condition`/`then_value`/`else_value`)
- Intermediate output `toepasselijke_grens` is referenced by later action

---

## Example 4: Complex Nested Calculation

From the actual zorgtoeslag law (simplified):

```yaml
machine_readable:
  definitions:
    percentage_drempelinkomen_alleenstaande:
      value: 0.01896
    percentage_drempelinkomen_partner:
      value: 0.04273
  execution:
    parameters:
      - name: bsn
        type: string
        required: true
    input:
      - name: standaardpremie
        type: amount
        source:
          regulation: regeling_standaardpremie
          output: standaardpremie
        type_spec:
          unit: eurocent
      - name: toetsingsinkomen
        type: amount
        source:
          regulation: algemene_wet_inkomensafhankelijke_regelingen
          output: toetsingsinkomen
          parameters:
            bsn: $bsn
        type_spec:
          unit: eurocent
      - name: heeft_toeslagpartner
        type: boolean
        source:
          regulation: algemene_wet_inkomensafhankelijke_regelingen
          output: heeft_toeslagpartner
          parameters:
            bsn: $bsn
    output:
      - name: hoogte_zorgtoeslag
        type: amount
        type_spec:
          unit: eurocent
    actions:
      - output: hoogte_zorgtoeslag
        value:
          operation: MAX
          values:
            - 0
            - operation: SUBTRACT
              values:
                - $standaardpremie
                - operation: MULTIPLY
                  values:
                    - operation: IF
                      when:
                        operation: EQUALS
                        subject: $heeft_toeslagpartner
                        value: true
                      then: $percentage_drempelinkomen_partner
                      else: $percentage_drempelinkomen_alleenstaande
                    - $toetsingsinkomen
```

**Key points:**
- Action uses `value:` wrapper with nested `operation: MAX` + `values: [...]`
- Operations nest deeply: MAX → SUBTRACT → MULTIPLY → IF
- Each nested operation is a full operation object
- No `subject`/`value` on arithmetic — only `values` array

---

## Example 5: Open Terms — Higher Law (IoC declaration)

When a law delegates a value to a lower regulation ("bij ministeriële regeling"),
the higher law declares an `open_term`:

```yaml
# wet_op_de_zorgtoeslag article 4
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
        value: $standaardpremie   # Resolved by engine via implements_index
```

**Key points:**
- `open_terms` declares what the higher law expects from lower regulations
- `$standaardpremie` references the open term as a variable
- The engine resolves it by finding the regulation that `implements` this term
- `delegation_type` constrains which regulatory layer may fill it
- `required: true` means execution fails if no implementing regulation is found

---

## Example 6: Open Terms — Lower Regulation (IoC implementation)

The lower regulation registers as implementing the open term:

```yaml
# regeling_standaardpremie article 1
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
      - name: berekeningsjaar
        type: number
    actions:
      - output: standaardpremie
        value: 211200
      - output: berekeningsjaar
        value: 2025
```

**Key points:**
- `implements` links back to the higher law, article, and open_term id
- `gelet_op` provides legal traceability ("Considering article X of law Y")
- The output name must match the open term `id` so the engine can resolve it
- Priority is resolved via lex superior (regulatory layer) and lex posterior (date)

---

## Example 7: SWITCH Operation

```yaml
actions:
  - output: normbedrag
    value:
      operation: SWITCH
      cases:
        - when:
            operation: AND
            conditions:
              - operation: GREATER_THAN_OR_EQUAL
                subject: $leeftijd
                value: 21
              - operation: EQUALS
                subject: $is_alleenstaande
                value: true
          then: 109171
        - when:
            operation: AND
            conditions:
              - operation: GREATER_THAN_OR_EQUAL
                subject: $leeftijd
                value: 21
              - operation: EQUALS
                subject: $is_alleenstaande
                value: false
          then: 155958
      default: 0
```

---

## Example 8: MvT Passage to Gherkin Scenario

Shows how to convert a Memorie van Toelichting passage into a BDD scenario.

**MvT passage (from kst-30912-3, Wet op de zorgtoeslag):**
```
Rekenvoorbeeld 1: Alleenstaande met een inkomen van €20.000

De standaardpremie bedraagt €2.112. Het percentage van het drempelinkomen
voor een alleenstaande bedraagt 1,896%. Het percentage normpremie
toetsingsinkomen bedraagt 13,7%.

Normpremie = 1,896% × €20.000 = €379,20
Zorgtoeslag = €2.112 - €379,20 = €1.732,80
```

**Generated Gherkin scenario:**
```gherkin
Feature: Zorgtoeslag — scenarios uit Memorie van Toelichting
  Testscenario's afgeleid uit de Memorie van Toelichting bij de
  Wet op de zorgtoeslag (kst-30912-3).

  # Bron: kst-30912-3
  # URL: https://zoek.officielebekendmakingen.nl/kst-30912-3.html

  Background:
    Given the calculation date is "2025-01-01"

  # === Rekenvoorbeelden uit MvT ===

  Scenario: Alleenstaande met inkomen van 20.000 euro
    # Bron: kst-30912-3, Rekenvoorbeeld 1
    Given the following RVIG "personal_data" data:
      | bsn       | geboortedatum | land_verblijf |
      | 999993653 | 1990-01-01    | NEDERLAND     |
    And the following RVZ "insurance" data:
      | bsn       | is_verzekerd |
      | 999993653 | true         |
    And the following BELASTINGDIENST "box1" data:
      | bsn       | belastbaar_inkomen |
      | 999993653 | 2000000            |
    When the zorgtoeslag is executed for wet_op_de_zorgtoeslag article 2
    Then the hoogte_zorgtoeslag is "173280" eurocent
```

**Key points:**
- Each scenario traces back to a specific MvT passage with `# Bron:` comment
- Monetary inputs are in eurocent (€20.000 = 2000000)
- Expected outputs are ALWAYS in eurocent (€1.732,80 = 173280) — never use euro with decimals
- When/Then steps use concrete law names (not placeholders like `{law_name}`)
- The scenario uses existing Given/When/Then steps, not new ones
- Do NOT invent scenarios — only use what the legislature provided

---

## Common Mistakes and Fixes

### Mistake 1: Wrong IF syntax
**Wrong (old/invalid):**
```yaml
operation: IF_THEN_ELSE
condition:
  operation: EQUALS
  subject: $x
  value: true
then_value: 100
else_value: 0
```

**Correct:**
```yaml
operation: IF
when:
  operation: EQUALS
  subject: $x
  value: true
then: 100
else: 0
```

### Mistake 2: Using url instead of regulation for source
**Wrong:**
```yaml
source:
  url: "regulation/nl/wet/zorgverzekeringswet#is_verzekerd"
```

**Correct:**
```yaml
source:
  regulation: zorgverzekeringswet
  output: is_verzekerd
  parameters:
    bsn: $bsn
```

### Mistake 3: Using subject/value for arithmetic
**Wrong:**
```yaml
operation: SUBTRACT
subject: $bruto
value: $korting
```

**Correct:**
```yaml
operation: SUBTRACT
values:
  - $bruto
  - $korting
```

### Mistake 4: Using values for logical operations
**Wrong:**
```yaml
operation: AND
values:
  - operation: EQUALS
    subject: $a
    value: true
```

**Correct:**
```yaml
operation: AND
conditions:
  - operation: EQUALS
    subject: $a
    value: true
```

### Mistake 5: Using public/endpoint at wrong level
**Wrong:**
```yaml
machine_readable:
  public: true
  endpoint: "bepaal_recht"
```

**Correct (endpoint is valid, public is NOT a schema field):**
```yaml
machine_readable:
  endpoint: "bepaal_recht"
```

### Mistake 6: Wrong monetary type
**Wrong:**
```yaml
output:
  - name: bedrag
    type: number  # Should be amount for monetary values
```

**Correct:**
```yaml
output:
  - name: bedrag
    type: amount
    type_spec:
      unit: eurocent
```

### Mistake 7: Missing $ prefix on variable
**Wrong:**
```yaml
subject: toetsingsinkomen
```

**Correct:**
```yaml
subject: $toetsingsinkomen
```
