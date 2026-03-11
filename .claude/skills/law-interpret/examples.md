# Law Machine-Readable Interpreter - Usage Examples

## Example 1: Interpret a Complete Law File

**User Request:**
```
Interpret the Wet op de Zorgtoeslag
```

**Skill Actions:**

1. **Find the law file:**
   ```
   regulation/nl/wet/wet_op_de_zorgtoeslag/2025-01-01.yaml
   ```

2. **Read and analyze:** Find 23 articles

3. **Process each article:**
   - Article 1: Definitions only → Skip
   - Article 2: Eligibility check → Add machine_readable with output `bereken_zorgtoeslag`
   - Article 3: Asset limit → Add machine_readable with output `vermogen_onder_grens`
   - ... continue for all articles

4. **Report:**
   ```
   ✓ Interpreted Wet op de Zorgtoeslag

     Articles processed: 23
     Made executable: 15
     Public outputs: 2

     Public outputs available:
     - bereken_zorgtoeslag
     - vermogen_onder_grens

     TODOs remaining:
     - Download and interpret: Zorgverzekeringswet (ZVW)
     - Download and interpret: Wet BRP (Basisregistratie Personen)
     - Download and interpret: AWIR (toetsingsinkomen)
     - Download and interpret: Belastingdienst (vermogen)

     The law is now executable via the engine!
     Use: service.evaluate_law_output("zorgtoeslagwet", "bereken_zorgtoeslag", {"BSN": "..."})
   ```

---

## Example 2: Before and After - Simple Constant

**Before (text only):**
```yaml
articles:
  - number: "1"
    text: |
      De standaardpremie bedraagt € 2.112 per jaar.
    url: "https://wetten.overheid.nl/BWBR0020307/2025-01-01#Artikel1"
```

**After (with machine_readable):**
```yaml
articles:
  - number: "1"
    text: |
      De standaardpremie bedraagt € 2.112 per jaar.
    url: "https://wetten.overheid.nl/BWBR0020307/2025-01-01#Artikel1"
    machine_readable:
      definitions:
        STANDAARDPREMIE:
          value: 211200  # €2.112 converted to eurocent
          description: "Standaardpremie zorgverzekering per jaar"

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

**Key Changes:**
- Monetary amount converted: €2.112 → 211200 eurocent
- Public output created: `standaardpremie`
- Simple constant definition
- Direct assignment action

---

## Example 3: Before and After - Eligibility Check

**Before (text only):**
```yaml
articles:
  - number: "2"
    text: |
      Een persoon heeft recht op zorgtoeslag indien hij:

      a. de leeftijd van 18 jaar heeft bereikt;
      b. verzekerd is ingevolge de [Zorgverzekeringswet](https://wetten.overheid.nl/BWBR0018450);
      c. in Nederland woont;
      d. rechtmatig in Nederland verblijft.
    url: "https://wetten.overheid.nl/BWBR0018451/2025-01-01#Artikel2"
```

**After (with machine_readable):**
```yaml
articles:
  - number: "2"
    text: |
      Een persoon heeft recht op zorgtoeslag indien hij:

      a. de leeftijd van 18 jaar heeft bereikt;
      b. verzekerd is ingevolge de [Zorgverzekeringswet](https://wetten.overheid.nl/BWBR0018450);
      c. in Nederland woont;
      d. rechtmatig in Nederland verblijft.
    url: "https://wetten.overheid.nl/BWBR0018451/2025-01-01#Artikel2"
    machine_readable:
      execution:
        parameters:
          - name: "BSN"
            type: "string"
            required: true
            description: "Burgerservicenummer van de persoon"

        input:
          - name: "leeftijd"
            type: "integer"
            source:
              # TODO: Implement Wet BRP
              # Should reference: regulation/nl/wet/wet_brp#leeftijd
              url: "TODO_brp"
              parameters:
                BSN: "$BSN"

          - name: "is_verzekerd"
            type: "boolean"
            source:
              # TODO: Implement Zorgverzekeringswet
              # Should reference: regulation/nl/wet/zorgverzekeringswet#is_verzekerd
              url: "TODO_zvw"
              parameters:
                BSN: "$BSN"

          - name: "woont_in_nederland"
            type: "boolean"
            source:
              # TODO: Implement Wet BRP
              url: "TODO_brp"
              parameters:
                BSN: "$BSN"

          - name: "rechtmatig_verblijf"
            type: "boolean"
            source:
              # TODO: Implement Wet BRP
              url: "TODO_brp"
              parameters:
                BSN: "$BSN"

        output:
          - name: "heeft_recht_op_zorgtoeslag"
            type: "boolean"
            description: "Geeft aan of de persoon recht heeft op zorgtoeslag"

        actions:
          - output: "heeft_recht_op_zorgtoeslag"
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
              - operation: "EQUALS"
                subject: "$rechtmatig_verblijf"
                value: true
```

**Key Changes:**
- Text preserved exactly (including markdown links)
- Public output: `bereken_zorgtoeslag`
- BSN parameter identified
- Four external data sources with TODO comments
- AND operation combining all conditions
- Age comparison: "18 jaar" → value: 18

---

## Example 4: Before and After - Internal Reference

**Before (text only, two articles):**

**Article 2:**
```yaml
- number: "2"
  text: |
    Een persoon heeft recht indien het vermogen niet meer bedraagt
    dan de in [artikel 3](#Artikel3) genoemde grens.
  url: "https://wetten.overheid.nl/BWBR0018451/2025-01-01#Artikel2"
```

**Article 3:**
```yaml
- number: "3"
  text: |
    De grens bedraagt:

    a. € 154.859 voor een alleenstaande;
    b. € 186.875 voor gehuwden of samenwonenden.
  url: "https://wetten.overheid.nl/BWBR0018451/2025-01-01#Artikel3"
```

**After (with machine_readable):**

**Article 2:**
```yaml
- number: "2"
  text: |
    Een persoon heeft recht indien het vermogen niet meer bedraagt
    dan de in [artikel 3](#Artikel3) genoemde grens.
  url: "https://wetten.overheid.nl/BWBR0018451/2025-01-01#Artikel2"
  machine_readable:
    public: true
    endpoint: "bepaal_recht"

    execution:
      parameters:
        - name: "BSN"
          type: "string"
          required: true

      input:
        - name: "vermogen_onder_grens"
          type: "boolean"
          source:
            url: "#vermogen_onder_grens"  # Internal reference to article 3!
            parameters:
              BSN: "$BSN"

      output:
        - name: "heeft_recht"
          type: "boolean"
          description: "Heeft recht op zorgtoeslag (vermogenstoets)"

      actions:
        - output: "heeft_recht"
          operation: "EQUALS"
          subject: "$vermogen_onder_grens"
          value: true
```

**Article 3:**
```yaml
- number: "3"
  text: |
    De grens bedraagt:

    a. € 154.859 voor een alleenstaande;
    b. € 186.875 voor gehuwden of samenwonenden.
  url: "https://wetten.overheid.nl/BWBR0018451/2025-01-01#Artikel3"
  machine_readable:
    public: true
    endpoint: "vermogen_onder_grens"

    definitions:
      VERMOGENSGRENS_ALLEENSTAAND:
        value: 15485900  # €154.859 in eurocent
        description: "Vermogensgrens voor alleenstaande personen"

      VERMOGENSGRENS_GEHUWD:
        value: 18687500  # €186.875 in eurocent
        description: "Vermogensgrens voor gehuwden/samenwonenden"

    execution:
      parameters:
        - name: "BSN"
          type: "string"
          required: true

      input:
        - name: "vermogen"
          type: "number"
          source:
            # TODO: Implement Belastingdienst vermogen
            # Should reference: regulation/nl/wet/belastingwet#vermogen
            url: "TODO_belastingdienst"
            parameters:
              BSN: "$BSN"

        - name: "heeft_partner"
          type: "boolean"
          source:
            # TODO: Implement AWIR
            url: "TODO_awir"
            parameters:
              BSN: "$BSN"

      output:
        - name: "vermogen_onder_grens"
          type: "boolean"
          description: "Is vermogen onder de toepasselijke grens?"

      actions:
        - output: "toepasselijke_grens"
          operation: "IF_THEN_ELSE"
          condition:
            operation: "EQUALS"
            subject: "$heeft_partner"
            value: true
          then_value: "$VERMOGENSGRENS_GEHUWD"
          else_value: "$VERMOGENSGRENS_ALLEENSTAAND"

        - output: "vermogen_onder_grens"
          operation: "LESS_THAN_OR_EQUAL"
          subject: "$vermogen"
          value: "$toepasselijke_grens"
```

**Key Changes:**
- Article 2 references article 3 via `#vermogen_onder_grens`
- Article 3 provides the `vermogen_onder_grens` output
- Two monetary amounts converted to eurocent
- IF_THEN_ELSE to select correct limit based on partner status
- Intermediate output `toepasselijke_grens` used in final comparison

---

## Example 5: Before and After - Complex Calculation

**Before (text only):**
```yaml
- number: "5"
  text: |
    De zorgtoeslag bedraagt de standaardpremie, vermenigvuldigd met
    het normpremiepercentage van 6,68%, verminderd met de inkomensafhankelijke
    bijdrage.

    De inkomensafhankelijke bijdrage wordt berekend door het toetsingsinkomen
    te vermenigvuldigen met 2,005%.
  url: "https://wetten.overheid.nl/BWBR0018451/2025-01-01#Artikel5"
```

**After (with machine_readable):**
```yaml
- number: "5"
  text: |
    De zorgtoeslag bedraagt de standaardpremie, vermenigvuldigd met
    het normpremiepercentage van 6,68%, verminderd met de inkomensafhankelijke
    bijdrage.

    De inkomensafhankelijke bijdrage wordt berekend door het toetsingsinkomen
    te vermenigvuldigen met 2,005%.
  url: "https://wetten.overheid.nl/BWBR0018451/2025-01-01#Artikel5"
  machine_readable:
    public: true
    endpoint: "bereken_zorgtoeslag_bedrag"

    definitions:
      NORMPREMIEPERCENTAGE:
        value: 0.0668  # 6,68%
        description: "Normpremiepercentage voor zorgtoeslag"

      INKOMENSPERCENTAGE:
        value: 0.02005  # 2,005%
        description: "Percentage voor inkomensafhankelijke bijdrage"

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
            # TODO: Implement AWIR toetsingsinkomen
            url: "TODO_awir"
            parameters:
              BSN: "$BSN"

      output:
        - name: "zorgtoeslag_bedrag"
          type: "number"
          description: "Het bedrag van de zorgtoeslag in eurocenten"

        - name: "hoogte_zorgtoeslag"
          type: "number"
          description: "Alias voor zorgtoeslag_bedrag"

      actions:
        # Step 1: Calculate maximum allowance (standaardpremie * percentage)
        - output: "maximale_toeslag"
          operation: "MULTIPLY"
          subject: "$standaardpremie"
          value: "$NORMPREMIEPERCENTAGE"

        # Step 2: Calculate income-dependent contribution
        - output: "inkomensafhankelijke_bijdrage"
          operation: "MULTIPLY"
          subject: "$toetsingsinkomen"
          value: "$INKOMENSPERCENTAGE"

        # Step 3: Subtract contribution from maximum
        - output: "zorgtoeslag_voorlopig"
          operation: "SUBTRACT"
          subject: "$maximale_toeslag"
          value: "$inkomensafhankelijke_bijdrage"

        # Step 4: Ensure non-negative (max of 0 and result)
        - output: "zorgtoeslag_bedrag"
          operation: "IF_THEN_ELSE"
          condition:
            operation: "GREATER_THAN"
            subject: "$zorgtoeslag_voorlopig"
            value: 0
          then_value: "$zorgtoeslag_voorlopig"
          else_value: 0

        # Step 5: Create alias output
        - output: "hoogte_zorgtoeslag"
          operation: "EQUALS"
          subject: "$zorgtoeslag_bedrag"
          value: "$zorgtoeslag_bedrag"
```

**Key Changes:**
- Percentages extracted as constants (6,68% → 0.0668)
- Complex formula broken into 5 sequential steps
- Each step has intermediate output
- External reference to regeling_standaardpremie (exists in repo!)
- TODO for toetsingsinkomen
- Non-negativity check (no negative allowances)
- Alias output for compatibility

---

## Example 6: Handling Ambiguity

**User Request:**
```
Interpret artikel 7 which says: "Het bedrag wordt jaarlijks aangepast"
```

**Skill Response:**

This article is ambiguous - it references an adjustment but doesn't specify:
- Which amount is adjusted?
- How is it adjusted? (inflation index? fixed percentage?)
- When exactly? (start of year?)

**Generated machine_readable with TODO:**
```yaml
machine_readable:
  public: false  # Not directly callable until clarified
  endpoint: "jaarlijkse_aanpassing"

  execution:
    # TODO: CLARIFICATION NEEDED
    # - Which bedrag is being adjusted?
    # - What is the adjustment formula?
    # - Is there a reference to an index (CPI, wage index)?
    # - Suggest consulting implementation decree (AMvB)

    parameters:
      - name: "jaar"
        type: "integer"
        required: true

    output:
      - name: "aangepast_bedrag"
        type: "number"
        description: "TODO: Needs clarification"

    actions:
      - output: "aangepast_bedrag"
        # TODO: Implement adjustment logic once clarified
        operation: "EQUALS"
        subject: 0
        value: 0
```

**Report to user:**
```
⚠ Article 7 interpretation incomplete

  Article 7 references "jaarlijkse aanpassing" but lacks specific details.
  Added machine_readable section with TODOs.

  Manual review needed:
  - Identify which amount is adjusted
  - Find adjustment formula (likely in AMvB or ministeriele regeling)
  - Consider adding reference to adjustment index

  Marked as public: false until clarified.
```

---

## Example 7: Integration Workflow

**Full workflow from download to execution:**

```
Step 1: Download the law
User: "Download the Wet op de Zorgtoeslag"
[dutch-law-downloader skill]
Result: regulation/nl/wet/wet_op_de_zorgtoeslag/2025-01-01.yaml (text only)

Step 2: Interpret the law
User: "Interpret the Wet op de Zorgtoeslag"
[law-machine-readable-interpreter skill]
Result: Same file updated with machine_readable sections

Step 3: Test with cucumber-rs
User: "Run the BDD tests"
Result: All tests pass using `just bdd`

Step 4: Identify missing dependencies
Skill reports:
  TODOs:
  - Zorgverzekeringswet (ZVW)
  - Wet BRP
  - AWIR

Step 5: Download missing laws
User: "Download Zorgverzekeringswet"
[dutch-law-downloader skill]
Result: regulation/nl/wet/zorgverzekeringswet/2025-01-01.yaml

Step 6: Interpret missing laws
User: "Interpret Zorgverzekeringswet"
[law-machine-readable-interpreter skill]
Result: Updated with machine_readable sections

Step 7: Execute!
User: "Execute zorgtoeslag for BSN 999993653"
Engine: Calculates result using all interconnected laws
```

---

## Tips for Working with Interpreted Laws

1. **Review TODOs first**: List all missing dependencies before execution
2. **Download in order**: Start with base laws, then dependent laws
3. **Test incrementally**: Run `just bdd` after each interpretation
4. **Check cross-references**: Verify `#field` references match actual outputs
5. **Validate eurocent**: Double-check monetary conversions
6. **Add test scenarios**: Create Gherkin feature files for edge cases
7. **Document assumptions**: Add comments for ambiguous interpretations
8. **Iterate**: Re-run interpretation if legal text changes

---

## Common Mistakes and Fixes

### Mistake 1: Forgetting Eurocent Conversion
**Wrong:**
```yaml
value: 795.47  # Still in euros!
```

**Correct:**
```yaml
value: 79547  # Converted to eurocent
```

### Mistake 2: Wrong Reference Format
**Wrong:**
```yaml
url: "zorgverzekeringswet#is_verzekerd"  # Missing path
```

**Correct:**
```yaml
url: "regulation/nl/wet/zorgverzekeringswet#is_verzekerd"
```

### Mistake 3: Mismatched Types
**Wrong:**
```yaml
output:
  - name: "heeft_recht"
    type: "boolean"

actions:
  - output: "heeft_recht"
    operation: "ADD"  # Returns number, not boolean!
    subject: "$a"
    value: "$b"
```

**Correct:**
```yaml
output:
  - name: "heeft_recht"
    type: "boolean"

actions:
  - output: "heeft_recht"
    operation: "EQUALS"  # Returns boolean
    subject: "$condition"
    value: true
```

### Mistake 4: Missing Variable Prefix
**Wrong:**
```yaml
subject: "toetsingsinkomen"  # Should have $ prefix
```

**Correct:**
```yaml
subject: "$toetsingsinkomen"  # Variable reference
```
