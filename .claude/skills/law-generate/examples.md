# Law Generate - Usage Examples

All examples below conform to schema v0.5.0 and pass `just validate`.

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
          cases:
            - when:
                operation: EQUALS
                subject: $heeft_partner
                value: true
              then: $VERMOGENSGRENS_GEHUWD
          default: $VERMOGENSGRENS_ALLEENSTAAND
      - output: vermogen_onder_grens
        value:
          operation: LESS_THAN_OR_EQUAL
          subject: $vermogen
          value: $toepasselijke_grens
```

**Key points:**
- Internal reference: `source: { output: "vermogen_onder_grens" }` (no `regulation`)
- `IF` uses `cases`/`default` (NOT `when`/`then`/`else`)
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
                - operation: IF
                  cases:
                    - when:
                        operation: EQUALS
                        subject: $heeft_toeslagpartner
                        value: true
                      then:
                        operation: MULTIPLY
                        values:
                          - 2
                          - $standaardpremie
                  default: $standaardpremie
                - operation: MULTIPLY
                  values:
                    - operation: IF
                      cases:
                        - when:
                            operation: EQUALS
                            subject: $heeft_toeslagpartner
                            value: true
                          then: $percentage_drempelinkomen_partner
                      default: $percentage_drempelinkomen_alleenstaande
                    - $toetsingsinkomen
```

**Key points:**
- Action uses `value:` wrapper with nested `operation: MAX` + `values: [...]`
- Operations nest deeply: MAX → SUBTRACT → MULTIPLY → IF
- Each nested operation is a full operation object
- No `subject`/`value` on arithmetic — only `values` array
- IF uses `cases`/`default` everywhere

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

## Example 7: IF with Multiple Cases (replaces SWITCH)

```yaml
actions:
  - output: normbedrag
    value:
      operation: IF
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

**Key points:**
- Use `IF` with `cases`/`default` for multi-branch conditionals (SWITCH does not exist in v0.5.0)
- Cases are evaluated in order; the first matching case wins
- Each case has `when` (condition) and `then` (result value)
- `default` is the fallback if no case matches

---

## Example 8: AGE Operation

From Wet open overheid — calculating how old a document is:

```yaml
actions:
  - output: informatie_leeftijd_jaren
    value:
      operation: AGE
      date_of_birth: $informatie_datum
      reference_date: $peildatum
  - output: verzwaarde_motiveringsplicht
    value:
      operation: GREATER_THAN_OR_EQUAL
      subject: $informatie_leeftijd_jaren
      value: 5
```

**Key points:**
- AGE calculates complete years between two dates
- Works for any "age" calculation, not just people (document age, policy age, etc.)
- Uses `date_of_birth` and `reference_date` (not subject/value)
- `$peildatum` must be declared as a parameter — it is NOT a built-in

---

## Example 9: DATE_ADD Operation (deadline calculations)

From AWB article 6:8 — calculating appeal deadlines:

```yaml
machine_readable:
  hooks:
    - hook_point: post_actions
      applies_to:
        legal_character: BESCHIKKING
        stage: BEKENDMAKING
  execution:
    parameters:
      - name: bekendmaking_datum
        type: date
        required: true
        description: Datum waarop het besluit is bekendgemaakt
    input:
      - name: bezwaartermijn_weken
        type: number
        source:
          regulation: algemene_wet_bestuursrecht
          output: bezwaartermijn_weken
    output:
      - name: bezwaartermijn_startdatum
        type: date
      - name: bezwaartermijn_einddatum
        type: date
    actions:
      # "met ingang van de dag na die waarop het besluit is bekendgemaakt"
      - output: bezwaartermijn_startdatum
        value:
          operation: DATE_ADD
          date: $bekendmaking_datum
          days: 1
      # einddatum = bekendmaking + termijn in weken
      - output: bezwaartermijn_einddatum
        value:
          operation: DATE_ADD
          date: $bekendmaking_datum
          weeks: $bezwaartermijn_weken
```

**Key points:**
- DATE_ADD supports `years`, `months`, `weeks`, `days` (all optional)
- Multiple duration components can be combined in one operation
- Values can be literals or $variable references
- Combined with hooks, this fires automatically for all BESCHIKKING decisions

---

## Example 10: Hooks Pattern (AWB cross-cutting concern)

From AWB article 3:46 — motivation requirement:

```yaml
machine_readable:
  hooks:
    - hook_point: pre_actions
      applies_to:
        legal_character: BESCHIKKING
        stage: BESLUIT
  execution:
    output:
      - name: motivering_vereist
        type: boolean
    actions:
      - output: motivering_vereist
        value: true
```

**Key points:**
- `pre_actions` fires before the target article's actions execute
- `post_actions` fires after the target article's actions
- `applies_to.legal_character` is required
- `applies_to.stage` is optional (defaults to BESLUIT)
- This creates a cross-cutting obligation that applies to ALL beschikkingen

---

## Example 11: Overrides Pattern (lex specialis)

From Vreemdelingenwet 2000 article 69 — overriding the standard AWB appeal deadline:

```yaml
machine_readable:
  overrides:
    - law: algemene_wet_bestuursrecht
      article: '6:7'
      output: bezwaartermijn_weken
  execution:
    output:
      - name: bezwaartermijn_weken
        type: number
    actions:
      - output: bezwaartermijn_weken
        value: 4
```

**Legal Text:**
```
In afwijking van artikel 6:7 van de Algemene wet bestuursrecht bedraagt
de termijn voor het indienen van een bezwaar- of beroepschrift vier weken.
```

**Key points:**
- `overrides` declares which law, article, and output are being replaced
- The output name must match what is declared in overrides
- The engine substitutes this output wherever the overridden article's output is used
- Pattern: "In afwijking van artikel X van wet Y" signals an override

---

## Example 12: NOT Operation (negation patterns)

```yaml
# Simple negation of a variable
actions:
  - output: openbaarmaking_toegestaan
    value:
      operation: AND
      conditions:
        - operation: NOT
          value: $heeft_absolute_weigeringsgrond
        - operation: NOT
          value: $heeft_relatieve_weigeringsgrond

# Negation of a comparison
actions:
  - output: niet_verzekerd
    value:
      operation: NOT
      value:
        operation: EQUALS
        subject: $is_verzekerd
        value: true

# Negation of a compound condition
actions:
  - output: geen_uitzondering
    value:
      operation: NOT
      value:
        operation: AND
        conditions:
          - $is_milieu_informatie
          - $betreft_emissies
```

**Key points:**
- NOT uses `value:` (not `subject:` or `conditions:`)
- Can negate a `$variable` directly, a comparison, or a compound condition
- Use NOT to express "tenzij", "niet", "geen" in legal text
- Replaces the removed NOT_EQUALS, NOT_IN, NOT_NULL operations:
  - NOT_EQUALS → `NOT` + `EQUALS`
  - NOT_IN → `NOT` + `IN`
  - NOT_NULL → use `NOT` + `EQUALS` with null, or restructure as a positive check

---

## Example 13: MvT Passage to Gherkin Scenario

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

### Mistake 1: Wrong IF syntax (v0.4.0 style)
**Wrong (v0.4.0 — removed):**
```yaml
operation: IF
when:
  operation: EQUALS
  subject: $x
  value: true
then: 100
else: 0
```

**Correct (v0.5.0):**
```yaml
operation: IF
cases:
  - when:
      operation: EQUALS
      subject: $x
      value: true
    then: 100
default: 0
```

### Mistake 2: Using SWITCH (removed in v0.5.0)
**Wrong:**
```yaml
operation: SWITCH
cases:
  - when: ...
    then: ...
default: 0
```

**Correct (use IF instead):**
```yaml
operation: IF
cases:
  - when: ...
    then: ...
default: 0
```

### Mistake 3: Using url instead of regulation for source
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

### Mistake 4: Using subject/value for arithmetic
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

### Mistake 5: Using values for logical operations
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

### Mistake 6: Using SUBTRACT_DATE for age (removed in v0.5.0)
**Wrong:**
```yaml
operation: SUBTRACT_DATE
subject: $peildatum
value: $geboortedatum
unit: years
```

**Correct:**
```yaml
operation: AGE
date_of_birth: $geboortedatum
reference_date: $peildatum
```

### Mistake 7: Using CONCAT (removed in v0.5.0)
**Wrong:**
```yaml
operation: CONCAT
values:
  - "Beschikking "
  - $wet_naam
```

**Correct (use ADD for string concatenation):**
```yaml
operation: ADD
values:
  - "Beschikking "
  - $wet_naam
```

### Mistake 8: Using NOT_EQUALS, NOT_IN, NOT_NULL (removed in v0.5.0)
**Wrong:**
```yaml
operation: NOT_EQUALS
subject: $status
value: "ACTIEF"
```

**Correct (use NOT + positive operation):**
```yaml
operation: NOT
value:
  operation: EQUALS
  subject: $status
  value: "ACTIEF"
```

### Mistake 9: Wrong monetary type
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

### Mistake 10: Treating $referencedate as a built-in
**Wrong (not declaring it):**
```yaml
execution:
  actions:
    - output: leeftijd
      value:
        operation: AGE
        date_of_birth: $geboortedatum
        reference_date: $referencedate  # ERROR: undeclared variable
```

**Correct (declare as parameter):**
```yaml
execution:
  parameters:
    - name: referencedate
      type: date
      required: true
      description: Peildatum voor berekening
  actions:
    - output: leeftijd
      value:
        operation: AGE
        date_of_birth: $geboortedatum
        reference_date: $referencedate
```

### Mistake 11: Missing $ prefix on variable
**Wrong:**
```yaml
subject: toetsingsinkomen
```

**Correct:**
```yaml
subject: $toetsingsinkomen
```
