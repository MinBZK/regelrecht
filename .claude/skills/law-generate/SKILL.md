---
name: law-generate
description: >
  Generates machine_readable execution logic for Dutch law YAML files through an
  iterative generate-validate-test loop. Creates machine_readable sections,
  validates against the schema, runs BDD tests, and iterates until correct
  (up to 3 iterations). Use when you already have MvT scenarios and want to
  generate the executable YAML logic.
allowed-tools: Read, Edit, Write, Bash, Grep, Glob
user-invocable: true
---

# Law Generate — Generate→Validate→Test Loop

Generates `machine_readable` sections for Dutch law YAML files through an iterative
cycle of creation, validation, and BDD testing.

**CRITICAL**: All generated YAML MUST pass `just validate <file>`. The schema is the
single source of truth. When in doubt, consult `schema/latest/schema.json` and study
working examples in `regulation/nl/wet/wet_op_de_zorgtoeslag/2025-01-01.yaml`.

## Setup

1. Read the target law YAML file
2. Read the zorgtoeslag example as few-shot reference:
   `regulation/nl/wet/wet_op_de_zorgtoeslag/2025-01-01.yaml`
3. Read the schema reference: `.claude/skills/law-generate/reference.md`
4. Read the examples: `.claude/skills/law-generate/examples.md`
5. Read an existing feature file as Gherkin reference:
   `features/bijstand.feature`
6. Count articles; if >20 articles, process in batches of ~15

## Phase 1: Generate `machine_readable` Sections

For each article with computable logic, generate the `machine_readable` section.

### Action Format (CRITICAL — three valid patterns)

Actions are the core of the execution logic. Each action MUST have an `output` field.
There are **three valid patterns** for specifying what to compute:

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

**Pattern 2: `value` — for direct literal/variable assignment**
```yaml
actions:
  - output: wet_naam
    value: Wet op de zorgtoeslag
  - output: constante
    value: $SOME_DEFINITION
```

**Pattern 3: `resolve` — for ministeriele regeling lookups**
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

### Built-in Variables

The engine provides `$referencedate` as a built-in variable representing the
calculation/reference date. It supports dot notation for property access:
- `$referencedate.year` — the year component (integer)
- `$referencedate` — the full date

This is NOT a parameter — it is automatically available in all executions and does
not need to be declared in `parameters` or `input`.

### When to Skip Articles

Skip articles that have no computable output. Heuristics for non-computable articles:
- **Pure definitions** — "In deze wet wordt verstaan onder..." (definition articles)
- **Procedural** — describes who must do what, deadlines for filing, appeal procedures
- **Delegation** — "Bij of krachtens algemene maatregel van bestuur worden regels gesteld..." (delegates to AMvB without computable logic)
- **Scope/applicability** — "Deze wet is van toepassing op..." (unless it has testable conditions)
- **Transitional provisions** — "overgangsrecht" articles about old-to-new transitions

Articles that SHOULD be made executable:
- Eligibility checks ("heeft recht op ... indien")
- Calculations ("bedraagt", "wordt berekend", "vermenigvuldigd met")
- Thresholds ("niet meer dan", "ten minste")
- Conditional amounts (SWITCH/IF patterns based on categories)

### Other Rules
- Convert monetary amounts to eurocent (€100 = 10000)
- Use `$variable` references for inter-action dependencies
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

## Phase 1.5: Capture BDD Baseline

**Before modifying the law file**, capture the current BDD state so you can distinguish
pre-existing failures from newly introduced ones:
```bash
just bdd 2>&1 | tail -100
```
Note the summary line and any pre-existing failures. This baseline is your reference
for all subsequent Phase 3 runs.

## Phase 2: Validate (with repair sub-loop)

Run validation:
```bash
just validate <file_path>
```

- If OK → proceed to Phase 3
- If errors → **Repair** (up to 2 rounds per iteration):
  1. Read error output, identify broken articles/fields
  2. Fix with Edit tool
  3. Re-run `just validate`
  4. If still failing after 2 repair rounds: **stop and report the validation errors
     to the user**. Do NOT proceed to Phase 3 with invalid YAML — BDD tests against
     a schema-invalid file will produce misleading failures that look like logic bugs,
     wasting iterations on the wrong problem.

## Phase 3: Run BDD Tests

Run the Gherkin scenarios against the machine_readable logic:
```bash
just bdd
```

This runs ALL feature files (in `features/`) including any generated by `/law-mvt-research`.
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

If a new external data source is needed, add a step in `steps/given.rs` following
the existing pattern.

**IMPORTANT: All BDD steps MUST be synchronous `fn`, NOT `async fn`.** The cucumber-rs
harness in this project uses synchronous world execution. Using `async fn` will compile
but cause runtime panics or silent test hangs.

```rust
#[given(regex = r#"the following NEWSOURCE "newsource_field" data:"#)]
fn set_newsource_data(world: &mut RegelrechtWorld, step: &Step) {
    if let Some(table) = &step.table {
        parse_external_data_table(table, &mut world.external_data.newsource_field);
    }
}
```

And add the corresponding field to `ExternalData` in `world.rs`:
```rust
pub struct ExternalData {
    // ... existing fields ...
    pub newsource_field: HashMap<String, HashMap<String, Value>>,
}
```

#### Adding a When Step (law execution)

Each law needs a When step that triggers execution. **Use concrete law names in
the regex, not placeholders.** All steps are synchronous `fn`. Example from the
actual bijstand step:
```rust
#[when(regex = r"^the bijstandsaanvraag is executed for participatiewet article (\d+)$")]
fn execute_bijstand(world: &mut RegelrechtWorld, _article: String) {
    // Register any external data sources if this law uses them
    register_if_present(&mut world.service, "rvig_personal_data", &world.external_data.rvig_personal_data);

    // Execute the law for the desired output
    world.execute_law("participatiewet", "bijstandsnorm");
}
```

The `register_if_present` helper (already defined in `when.rs`) takes 3 arguments:
```rust
fn register_if_present(
    service: &mut regelrecht_engine::LawExecutionService,
    name: &str,
    data: &std::collections::HashMap<String, std::collections::HashMap<String, Value>>,
)
```

#### Adding a Then Step (assertions)

For checking output values — **use the concrete output name in the regex**:
```rust
#[then(regex = r#"^the my_output is "(-?\d+)" eurocent$"#)]
fn assert_my_output(world: &mut RegelrechtWorld, expected: String) {
    assert!(world.is_success(), "Expected success, got error: {:?}", world.error_message());
    let expected_amount: i64 = expected.parse().expect("Invalid eurocent value (must be integer, may be negative)");
    let actual = world.get_output("my_output");
    match actual {
        Some(Value::Int(n)) => assert_eq!(*n, expected_amount),
        Some(Value::Float(f)) => assert_eq!(f.round() as i64, expected_amount),
        _ => panic!("Expected number, got {:?}", actual),
    }
}
```

For boolean checks:
```rust
#[then("the citizen has the right to my_benefit")]
fn assert_has_right(world: &mut RegelrechtWorld) {
    assert!(world.is_success());
    let output = world.get_output("heeft_recht");
    assert!(matches!(output, Some(Value::Bool(true))), "Expected true, got {:?}", output);
}
```

#### Key World Methods

- `world.execute_law(law_id, output_name)` — runs the engine, stores result/error
- `world.get_output(name)` — retrieves a named output from the last result
- `world.is_success()` — true if execution succeeded
- `world.error_message()` — error string from last failed execution (`Option<String>`)
- `world.parameters` — `HashMap<String, Value>` for simple inputs
- `world.external_data` — `ExternalData` struct with fields:
  `rvig_personal`, `rvig_relationship`, `rvz_insurance`, `bd_box1`, `bd_box2`,
  `bd_box3`, `dji_detenties` (each `HashMap<String, HashMap<String, Value>>`)

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

## Phase 4: Iterate (up to 3 total iterations)

- **All BDD scenarios pass** → proceed to Phase 5
- **Failures** → analyze each failure:
  - **Logic bug in machine_readable**: fix the YAML actions/operations
  - **Wrong step definition**: fix the BDD step code
  - **NEVER change the expected values in MvT-derived scenarios** — these are
    the legislature's intended outcomes and serve as ground truth
  - Go back to Phase 2 (validate → test again)
- **After 3 iterations**: stop and report remaining issues. Each iteration includes
    its own Phase 2 validation cycle (up to 2 repair rounds per iteration). For large
    laws (>20 articles), this limit applies per batch — each batch of ~15 articles
    gets its own 3-iteration budget

## Phase 5: Report

Report to the user:

```
Interpreted {LAW_NAME}

  Articles processed: {TOTAL}
  Made executable: {EXECUTABLE_COUNT}
  Validation: {PASSED/FAILED}

  BDD scenarios: {PASS}/{TOTAL} passing
  (from MvT feature file and/or ad-hoc evaluate tests)

  Iterations needed: {N}

  Remaining issues:
  - {description of any unresolved failures}

  TODOs:
  - {external laws that need to be downloaded/implemented}

  The law is now executable via the engine!
```
