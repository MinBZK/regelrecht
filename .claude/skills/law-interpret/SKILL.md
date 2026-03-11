---
name: law-interpret
description: >
  Generates machine-readable execution logic for Dutch law YAML files using an
  iterative generate-validate-test loop. First searches for Memorie van Toelichting
  (explanatory memoranda) to extract legislature-intended examples and generates
  Gherkin test scenarios from them. Then creates machine_readable sections, validates
  against the schema/engine, runs BDD tests, and iterates until correct (up to 3
  iterations). Use when user wants to make a law executable or add machine_readable
  sections.
allowed-tools: Read, Edit, Write, Bash, Grep, Glob, WebFetch
---

# Law Interpret — Agentic Generate→Validate→Test Loop

Generates `machine_readable` sections for Dutch law YAML files through an iterative
cycle of MvT research, Gherkin scenario generation, machine_readable creation,
validation, and BDD testing.

**CRITICAL**: All generated YAML MUST pass `just validate <file>`. The schema is the
single source of truth. When in doubt, consult `schema/latest/schema.json` and study
working examples in `regulation/nl/wet/wet_op_de_zorgtoeslag/2025-01-01.yaml`.

## Phase 0: Setup

1. Read the target law YAML file
2. Read the zorgtoeslag example as few-shot reference:
   `regulation/nl/wet/wet_op_de_zorgtoeslag/2025-01-01.yaml`
3. Read the schema reference: `.claude/skills/law-interpret/reference.md`
4. Read the examples: `.claude/skills/law-interpret/examples.md`
5. Read an existing feature file as Gherkin reference:
   `features/bijstand.feature`
6. Count articles; if >20 articles, process in batches of ~15

## Phase 1: Search and Extract Memorie van Toelichting

This phase runs **independently** from the machine_readable generation. Its sole
purpose is to find legislature-intended examples, calculation scenarios, and edge
cases, then turn them into Gherkin acceptance tests.

### Step 1.1: Find MvT Documents

Extract the `bwb_id` (e.g., `BWBR0018451`) from the law YAML's `bwb_id` field.

Search for related parliamentary documents using the overheid.nl SRU API:

```
http://zoekservice.overheid.nl/sru/Search?operation=searchRetrieve&version=1.2&x-connection=officielepublicaties&query=dcterms.references=={BWB_ID}&maximumRecords=20
```

Use WebFetch to retrieve the results. Parse the XML response to find documents of
these types (in `<dcterms:type>`):
- **Memorie van toelichting** (explanatory memorandum)
- **Nota naar aanleiding van het verslag** (response to parliamentary report)
- **Nota van wijziging** (amendment note)
- **Brief van de minister** (ministerial letter with examples)

Also search by law title for additional coverage:
```
http://zoekservice.overheid.nl/sru/Search?operation=searchRetrieve&version=1.2&x-connection=officielepublicaties&query=dcterms.title%20any%20"{LAW_TITLE}"%20AND%20dcterms.type=="Memorie van toelichting"&maximumRecords=10
```

There may be **multiple MvT documents** (original + amendments). Collect all of them.

### Step 1.2: Download and Read MvT Content

For each found document, extract the document identifier from the search results
(e.g., `kst-36XXX-3`) and download the HTML version:

```
https://zoek.officielebekendmakingen.nl/{DOCUMENT_ID}.html
```

Use WebFetch to retrieve the content. If HTML is too large, focus on sections that
contain:
- "voorbeeld" (example)
- "rekenvoorbeeld" (calculation example)
- "casus" (case)
- "scenario"
- "tabel" (table — often contains example calculations)
- "berekening" (calculation)
- "stel dat" (suppose that)
- "in het geval" (in the case of)

### Step 1.3: Extract Test-Relevant Information

From the MvT content, extract:

1. **Rekenvoorbeelden** (calculation examples):
   - Input values used by the legislature
   - Expected output values
   - Step-by-step calculations shown

2. **Concrete scenario's** (concrete scenarios):
   - Described situations with specific parameters
   - Expected outcomes stated by the legislature

3. **Randgevallen** (edge cases):
   - Boundary conditions explicitly discussed
   - Special cases the legislature considered

4. **Bedoelde uitkomsten** (intended outcomes):
   - "De bedoeling is dat..." (the intention is that...)
   - "Dit betekent dat een persoon die..." (this means that a person who...)

For each extracted example, note:
- Which article(s) it relates to
- The input parameters and their values
- The expected output/result
- The source document and page/section reference

### Step 1.4: Generate Gherkin Feature File

Write a `.feature` file to `features/{law_id}.feature` based on the MvT examples.

Follow the existing project conventions (see `features/bijstand.feature` and
`features/zorgtoeslag.feature` for style).

**Structure:**
```gherkin
Feature: {Law title} — scenarios uit Memorie van Toelichting
  Testscenario's afgeleid uit de Memorie van Toelichting en parlementaire
  stukken bij {law_title}.

  # Bron: {MvT document identifier(s)}
  # URL: {MvT document URL(s)}

  Background:
    Given the calculation date is "{effective_date}"

  # === Rekenvoorbeelden uit MvT ===

  Scenario: {Description from MvT}
    # Bron: {document_id}, {section/page reference}
    Given a citizen with the following data:
      | parameter_1 | value_1 |
      | parameter_2 | value_2 |
    When the {law_execution} is executed for {law_id} article {N}
    Then the {output_name} is "{expected_value}" eurocent

  # === Randgevallen ===

  Scenario: {Edge case from MvT}
    # Bron: {document_id}, {section/page reference}
    ...
```

**Guidelines:**
- Each scenario MUST trace back to a specific MvT passage (add `# Bron:` comments)
- Convert monetary amounts in MvT to eurocent
- Use the same Given/When/Then step patterns as existing feature files
- If MvT examples reference external data sources (RVIG, Belastingdienst, etc.),
  use the appropriate Given steps for those sources
- If the MvT doesn't provide enough examples for a specific article, note this in
  a comment but do NOT invent scenarios — only use what the legislature provided
- Group scenarios by: rekenvoorbeelden, randgevallen, afwijzingsscenario's

### Step 1.5: Report MvT Findings

Report to the user before proceeding to machine_readable generation:

```
MvT Research for {LAW_NAME}

  Documents found: {COUNT}
  - {doc_id_1}: {title} ({date})
  - {doc_id_2}: {title} ({date})

  Extracted scenarios: {SCENARIO_COUNT}
  - Rekenvoorbeelden: {N}
  - Randgevallen: {N}
  - Afwijzingsscenario's: {N}

  Feature file: features/{law_id}.feature

  Articles without MvT examples: {list}
  Note: No synthetic scenarios were added for these articles.
```

If NO MvT documents are found, report this clearly and continue to Phase 2 without
a feature file. The later phases will fall back to the JSON-based test approach.

## Phase 2: Generate `machine_readable` Sections

For each article with computable logic, generate the `machine_readable` section.

### Action Format (CRITICAL — four valid patterns)

Actions are the core of the execution logic. Each action MUST have an `output` field.
There are **four valid patterns** for specifying what to compute:

**Pattern 1: `value` — for assignments, comparisons, conditionals, and logical ops**
```yaml
actions:
  - output: heeft_recht
    value:
      operation: AND
      conditions:
        - operation: GREATER_THAN_OR_EQUAL
          subject: $leeftijd
          value: 18
        - operation: EQUALS
          subject: $is_verzekerd
          value: true
```

**Pattern 2: `operation` + `values` — shorthand for arithmetic at action level**
```yaml
actions:
  - output: totaal
    operation: SUBTRACT
    values:
      - $bruto_bedrag
      - $korting
```

**Pattern 3: `value` — for direct literal/variable assignment**
```yaml
actions:
  - output: wet_naam
    value: Wet op de zorgtoeslag
  - output: constante
    value: $SOME_DEFINITION
```

**Pattern 4: `resolve` — for ministeriele regeling lookups**
```yaml
actions:
  - output: standaardpremie
    resolve:
      type: ministeriele_regeling
      output: standaardpremie
      match:
        output: berekeningsjaar
        value: $referencedate.year
```

### Operation Syntax by Category

**Arithmetic** — use `values` array (NOT `subject`/`value`):
```yaml
operation: ADD          # or SUBTRACT, MULTIPLY, DIVIDE, MIN, MAX, CONCAT
values:
  - $operand_1
  - $operand_2
```

**Comparison** — use `subject` + `value`:
```yaml
operation: EQUALS       # or NOT_EQUALS, GREATER_THAN, LESS_THAN,
                        # GREATER_THAN_OR_EQUAL, LESS_THAN_OR_EQUAL
subject: $variable      # MUST be a $variable reference
value: 18               # literal or $variable
```

**Membership** — use `subject` + `value` (array):
```yaml
operation: IN           # or NOT_IN
subject: $status
value: ["ACTIEF", "GEPAUZEERD"]
```

**Null check** — use `subject` only:
```yaml
operation: NOT_NULL
subject: $some_field
```

**Logical** — use `conditions` array:
```yaml
operation: AND          # or OR
conditions:
  - operation: EQUALS
    subject: $a
    value: true
  - operation: EQUALS
    subject: $b
    value: true
```

**Conditional IF** — use `when`/`then`/`else` (NOT `condition`/`then_value`/`else_value`):
```yaml
operation: IF
when:
  operation: EQUALS
  subject: $heeft_partner
  value: true
then: $bedrag_partner
else: $bedrag_alleenstaand
```

**SWITCH** — use `cases` array:
```yaml
operation: SWITCH
cases:
  - when:
      operation: EQUALS
      subject: $categorie
      value: "A"
    then: 100000
  - when:
      operation: EQUALS
      subject: $categorie
      value: "B"
    then: 75000
default: 50000
```

**Date** — use `subject` + `value` + `unit`:
```yaml
operation: SUBTRACT_DATE
subject: $peildatum
value: $geboortedatum
unit: years
```

### Cross-Law References (source)

Input fields reference other laws via `source`. Use `regulation` + `output`, NOT `url`:

```yaml
input:
  - name: toetsingsinkomen
    type: amount
    source:
      regulation: algemene_wet_inkomensafhankelijke_regelingen
      output: toetsingsinkomen
      parameters:
        bsn: $bsn
    type_spec:
      unit: eurocent
```

For **internal references** (same law, different article), omit `regulation`:
```yaml
input:
  - name: vermogen_onder_grens
    type: boolean
    source:
      output: vermogen_onder_grens
```

For **delegated regulations** (e.g., gemeentelijke verordeningen):
```yaml
input:
  - name: verlaging_percentage
    type: number
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

### Field Types

| Context | Valid types |
|---------|------------|
| `parameters` | `string`, `number`, `boolean`, `date` |
| `input` | `string`, `number`, `boolean`, `amount`, `object`, `array`, `date` |
| `output` | `string`, `number`, `boolean`, `amount`, `object`, `array`, `date` |

For monetary values, use `type: amount` with `type_spec: { unit: eurocent }`.

### Other Rules
- Convert monetary amounts to eurocent (€100 = 10000)
- Use `$variable` references for inter-action dependencies
- Skip articles that are purely definitional/procedural (no computable output)
- `subject` in comparisons MUST be a `$variable`, never a nested operation
- Operations can be nested: a `value` in an arithmetic array can itself be an operation
- `endpoint` on `machine_readable` makes an article callable from other regulations

### Available Operations
| Category | Operations |
|----------|------------|
| Arithmetic | `ADD`, `SUBTRACT`, `MULTIPLY`, `DIVIDE`, `MIN`, `MAX`, `CONCAT` |
| Comparison | `EQUALS`, `NOT_EQUALS`, `GREATER_THAN`, `LESS_THAN`, `GREATER_THAN_OR_EQUAL`, `LESS_THAN_OR_EQUAL` |
| Logical | `AND`, `OR` |
| Membership | `IN`, `NOT_IN` |
| Null check | `NOT_NULL` |
| Conditional | `IF`, `SWITCH` |
| Iteration | `FOREACH` |
| Date | `SUBTRACT_DATE` |
| Other | `NOT` |

### Common Legal Text → Operation Mappings
| Legal Text | Operation |
|------------|-----------|
| "heeft bereikt de leeftijd van 18 jaar" | `GREATER_THAN_OR_EQUAL`, subject: $leeftijd, value: 18 |
| "niet meer bedraagt dan X" | `LESS_THAN_OR_EQUAL` |
| "ten minste X" | `GREATER_THAN_OR_EQUAL` |
| "indien ... en ..." | `AND` with `conditions` array |
| "indien ... of ..." | `OR` with `conditions` array |
| "niet ..." | `NOT` |
| "gelijk aan" | `EQUALS` |
| "vermenigvuldigd met" | `MULTIPLY` with `values` array |
| "verminderd met" | `SUBTRACT` with `values` array |
| "vermeerderd met" | `ADD` with `values` array |

## Phase 3: Validate (with repair sub-loop)

Run validation:
```bash
just validate <file_path>
```

- If OK → proceed to Phase 4
- If errors → **Repair** (up to 2 rounds):
  1. Read error output, identify broken articles/fields
  2. Fix with Edit tool
  3. Re-run `just validate`
  4. If still failing after 2 repair rounds: **stop and report the validation errors
     to the user**. Do NOT proceed to Phase 4 with invalid YAML — BDD tests against
     a schema-invalid file will produce misleading failures that look like logic bugs,
     wasting iterations on the wrong problem.

## Phase 4: Run BDD Tests

Run the Gherkin scenarios from Phase 1 against the machine_readable logic:

First, capture the **baseline** BDD state before your changes by running:
```bash
just bdd 2>&1 | tail -20
```
Note any pre-existing failures. Then, after generating machine_readable sections, run:
```bash
just bdd
```

This runs ALL feature files (in `features/`) including the one generated in Phase 1.
The command is equivalent to:
```bash
cd packages/engine && cargo test --test bdd -- --nocapture
```

**Important:** Only investigate failures that are NEW compared to the baseline. Pre-existing
failures from other laws are not your problem — do not attempt to fix them.

### Creating New Step Definitions

If the feature file uses Given/When/Then steps that don't exist yet, you must add
them before running `just bdd`. The BDD harness lives in:

```
packages/engine/tests/bdd/
├── main.rs              # Test runner (finds features/, runs cucumber)
├── world.rs             # RegelrechtWorld state struct
├── steps/
│   ├── mod.rs           # Module exports
│   ├── given.rs         # Setup steps (data input)
│   ├── when.rs          # Action steps (law execution)
│   └── then.rs          # Assertion steps (output checks)
└── helpers/
    ├── regulation_loader.rs  # Loads all YAML from regulation/nl/
    └── value_conversion.rs   # Gherkin string → Value conversion
```

#### Adding a Given Step (data setup)

For simple parameter tables (`| key | value |`), reuse the existing step:
```gherkin
Given a citizen with the following data:
  | leeftijd | 35 |
  | inkomen  | 2000000 |
```

For external data sources (RVIG, Belastingdienst, etc.), reuse existing steps like:
```gherkin
Given the following RVIG "personal_data" data:
  | bsn | geboortedatum | land_verblijf |
  | 999993653 | 1990-01-01 | NEDERLAND |
```

If a new external data source is needed, add a step in `steps/given.rs`:
```rust
#[given(regex = r#"the following NEWSOURCE "(\w+)" data:"#)]
async fn given_newsource_data(world: &mut RegelrechtWorld, field: String, step: &Step) {
    let table = step.table.as_ref().expect("Expected a data table");
    let records = parse_external_data_table(table);
    world.external_data.newsource.insert(field, records);
}
```

And add the corresponding field to `ExternalData` in `world.rs`.

#### Adding a When Step (law execution)

Each law needs a When step that triggers execution. Pattern:
```rust
#[when(regex = r"the {law_name} is executed for {law_id} article (\d+)")]
async fn when_execute_new_law(world: &mut RegelrechtWorld, article: u32) {
    // Register any external data sources needed
    register_if_present(&mut world.service, "RVIG", "bsn", &world.external_data.rvig);
    // ... register other sources

    // Execute the law
    world.execute_law("{law_id}", "{output_name}").await;
}
```

The `register_if_present` helper registers a dict source only if data was provided:
```rust
fn register_if_present(
    service: &mut LawExecutionService,
    source_name: &str,
    key_field: &str,
    data: &HashMap<String, Vec<HashMap<String, Value>>>,
) {
    for (field, records) in data {
        service.register_dict_source(
            &format!("{}_{}", source_name, field),
            key_field,
            records.clone(),
        );
    }
}
```

#### Adding a Then Step (assertions)

For checking output values:
```rust
#[then(regex = r#"the {output_name} is "([^"]+)" eurocent"#)]
async fn then_check_amount(world: &mut RegelrechtWorld, expected: String) {
    assert!(world.is_success(), "Execution failed: {}", world.error_message());
    let output = world.get_output("{output_name}").expect("Output not found");
    let expected_val = parse_eurocent(&expected);
    // Handle Int/Float comparison
    match output {
        Value::Int(v) => assert_eq!(*v, expected_val as i64),
        Value::Float(v) => assert!((v - expected_val as f64).abs() < 0.01),
        other => panic!("Expected number, got {:?}", other),
    }
}
```

For boolean checks:
```rust
#[then("the citizen has the right to {benefit}")]
async fn then_has_right(world: &mut RegelrechtWorld) {
    assert!(world.is_success());
    let output = world.get_output("heeft_recht").expect("Missing heeft_recht");
    assert_eq!(output, &Value::Bool(true));
}
```

#### Key World Methods

- `world.execute_law(law_id, output_name)` — runs the engine, stores result/error
- `world.get_output(name)` — retrieves a named output from the last result
- `world.is_success()` — true if execution succeeded
- `world.error_message()` — error string from last failed execution
- `world.parameters` — `HashMap<String, Value>` for simple inputs
- `world.external_data` — struct with fields for each data source

#### Prefer Reusing Existing Steps

Before creating new steps, check if existing patterns cover your case. Read the
existing step files first:
- `packages/engine/tests/bdd/steps/given.rs`
- `packages/engine/tests/bdd/steps/when.rs`
- `packages/engine/tests/bdd/steps/then.rs`

Many scenarios can be expressed using the existing generic steps. Only add new steps
when the law requires a genuinely different execution pattern or data source.

### If no MvT feature file was generated

Fall back to ad-hoc testing: for each article with `execution.output`, build the
evaluate binary and pipe a JSON payload to it:

```bash
cargo build --manifest-path packages/engine/Cargo.toml --bin evaluate --release
```

**Important:** Do NOT use `echo` to pipe JSON — Dutch law YAML contains quotes,
newlines, and special characters that will break shell escaping. Instead, use the
`Write` tool to create a temp file, then pipe from it:

```bash
cat /tmp/eval_payload.json | ./target/release/evaluate
```

The JSON payload format (written to the temp file):
```json
{
  "law_yaml": "<full YAML content of the law file>",
  "output_name": "heeft_recht",
  "params": {"bsn": "123456789", "peildatum": "2025-01-01"},
  "date": "2025-01-01",
  "extra_laws": []
}
```

### Cross-law Dependencies
- If the law references other laws via `source.regulation`, find those law files
  in `regulation/nl/` and include their YAML content in `extra_laws`:
  ```json
  "extra_laws": [
    {"id": "wet_op_de_zorgtoeslag", "yaml": "<content>"}
  ]
  ```
- Use Glob to find referenced law files

## Phase 5: Iterate (up to 3 total iterations)

- **All BDD scenarios pass** → proceed to Phase 6
- **Failures** → analyze each failure:
  - **Logic bug in machine_readable**: fix the YAML actions/operations
  - **Wrong step definition**: fix the BDD step code
  - **NEVER change the expected values in MvT-derived scenarios** — these are
    the legislature's intended outcomes and serve as ground truth
  - Go back to Phase 3 (validate → test again)
- **After 3 iterations**: stop and report remaining issues

## Phase 6: Report

Report to the user:

```
Interpreted {LAW_NAME}

  MvT sources: {MvT_COUNT} documents found
  - {doc_id}: {title}

  Articles processed: {TOTAL}
  Made executable: {EXECUTABLE_COUNT}
  Validation: {PASSED/FAILED}

  BDD scenarios (from MvT): {MvT_PASS}/{MvT_TOTAL} passing
  Ad-hoc scenarios: {ADHOC_PASS}/{ADHOC_TOTAL} passing

  Iterations needed: {N}

  Remaining issues:
  - {description of any unresolved failures}

  TODOs:
  - {external laws that need to be downloaded/implemented}

  Feature file: features/{law_id}.feature
  The law is now executable via the engine!
```

## Reverse Validation (Hallucination Check)

Before finalizing, verify every element in `machine_readable` traces back to
the original legal text:

| Traceable in text? | Needed for logic? | Action |
|-------------------|-------------------|--------|
| YES | YES | Keep |
| YES | NO | Keep (informational) |
| NO | YES | Report as assumption |
| NO | NO | **Remove** |

Report any assumptions that need user review.

## Key Schema Rules Summary

- `endpoint`: named endpoint making article callable from other regulations (string)
- `competent_authority`: who has binding authority (string ref or `{name, type}`)
- `requires`: dependencies on other articles/laws
- `definitions`: constants (eurocent for money)
- `execution.produces`: legal character (`BESCHIKKING`/`TOETS`/`WAARDEBEPALING`/`BESLUIT_VAN_ALGEMENE_STREKKING`/`INFORMATIEF`) and decision type
- `execution.parameters`: caller inputs (types: `string`/`number`/`boolean`/`date`)
- `execution.input`: data from other sources (types include `amount` with `type_spec`)
- `execution.output`: what article produces (types include `amount` with `type_spec`)
- `execution.actions`: computation logic — `output` required, then `value`/`operation`+`values`/`resolve`
- `legal_basis`: traceability to specific law text (on fields and actions)
- `resolve`: lookup from ministeriele_regeling (with `type`, `output`, `match`)
