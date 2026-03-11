---
name: law-interpret
description: >
  Generates machine-readable execution logic for Dutch law YAML files using an
  iterative generate-validate-test loop. Reads legal text, creates machine_readable
  sections, validates against the schema/engine, generates test scenarios, runs them,
  and iterates until correct (up to 3 iterations). Use when user wants to make a law
  executable or add machine_readable sections.
allowed-tools: Read, Edit, Write, Bash, Grep, Glob
---

# Law Interpret — Agentic Generate→Validate→Test Loop

Generates `machine_readable` sections for Dutch law YAML files through an iterative
cycle of generation, validation, test scenario creation, and execution.

## Phase 0: Setup

1. Read the target law YAML file
2. Read the zorgtoeslag example as few-shot reference:
   `regulation/nl/wet/wet_op_de_zorgtoeslag/2025-01-01.yaml`
3. Read the schema reference: `.claude/skills/law-interpret/reference.md`
4. Read the examples: `.claude/skills/law-interpret/examples.md`
5. Count articles; if >20 articles, process in batches of ~15
6. Build the evaluate binary once:
   ```bash
   cargo build --manifest-path packages/engine/Cargo.toml --bin evaluate --release
   ```

## Phase 1: Generate `machine_readable` Sections

For each article with computable logic, generate the `machine_readable` section.

### Rules
- Edit the YAML file in place using the Edit tool
- Follow all schema rules from `reference.md`
- Convert monetary amounts to eurocent (€100 = 10000)
- Use `$variable` references for inter-action dependencies
- Parameter types: `string`, `number`, `boolean`, `date` (NOT `amount` for parameters — `amount` is only for input/output fields)
- Cross-law references use `source.regulation`
- Skip articles that are purely definitional/procedural (no computable output)
- `subject` in comparisons must always be a `$variable`, never nested operations
- `conditions` for AND/OR as array (not `values` for conditionals)
- `values` for arithmetic as array (not `conditions` for arithmetic)

### Available Operations
| Category | Operations |
|----------|------------|
| Arithmetic | `ADD`, `SUBTRACT`, `MULTIPLY`, `DIVIDE`, `MIN`, `MAX` |
| Comparison | `EQUALS`, `NOT_EQUALS`, `GREATER_THAN`, `LESS_THAN`, `GREATER_THAN_OR_EQUAL`, `LESS_THAN_OR_EQUAL` |
| Logical | `AND`, `OR`, `NOT` |
| Membership | `IN`, `NOT_IN` |
| Null check | `NOT_NULL` |
| Conditional | `IF` |
| Iteration | `FOREACH` |
| Date | `SUBTRACT_DATE` |
| String | `CONCAT` |

### Common Legal Text → Operation Mappings
| Legal Text | Operation |
|------------|-----------|
| "heeft bereikt de leeftijd van 18 jaar" | `GREATER_THAN_OR_EQUAL`, subject: $leeftijd, value: 18 |
| "niet meer bedraagt dan X" | `LESS_THAN_OR_EQUAL` |
| "ten minste X" | `GREATER_THAN_OR_EQUAL` |
| "indien ... en ..." | `AND` with values array |
| "indien ... of ..." | `OR` with values array |
| "niet ..." | `NOT` |
| "gelijk aan" | `EQUALS` |

## Phase 2: Validate (with repair sub-loop)

Run validation:
```bash
just validate <file_path>
```

- If OK → proceed to Phase 3
- If errors → **Repair** (up to 2 rounds):
  1. Read error output, identify broken articles/fields
  2. Fix with Edit tool
  3. Re-run `just validate`
  4. If still failing after 2 repair rounds: log errors and continue to Phase 3

## Phase 3: Generate Test Scenarios

For each article with `execution.output`, generate 2–5 test scenarios.

### Scenario Format
Write scenarios as JSON to `/tmp/scenarios_<law_id>.json`:
```json
[
  {
    "name": "Beschrijving van het scenario",
    "output_name": "heeft_recht",
    "params": {
      "bsn": "123456789",
      "peildatum": "2025-01-01"
    },
    "date": "2025-01-01",
    "expected": {
      "heeft_recht": true
    }
  }
]
```

### Scenario Guidelines
- Cover: happy path, edge cases, boundary values
- Amounts in eurocent (integers)
- Dates as YYYY-MM-DD strings
- Include at least one scenario per distinct output
- Test both true/false paths for boolean outputs
- Test boundary values for thresholds (e.g., age exactly 18)

## Phase 4: Execute Scenarios

For each scenario, pipe JSON to the evaluate binary:

```bash
echo '<json>' | ./target/release/evaluate
```

The JSON payload format:
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

### Comparing Results
- Compare actual output values against expected values
- Track pass/fail per scenario

## Phase 5: Iterate (up to 3 total iterations)

- **All scenarios pass** → proceed to Phase 6
- **Failures** → analyze each failure:
  - **Logic bug in machine_readable**: fix the YAML actions/operations
  - **Wrong expected value in scenario**: fix the scenario
  - Go back to Phase 2 (validate → test again)
- **After 3 iterations**: stop and report remaining issues

## Phase 6: Report

Report to the user:

```
Interpreted {LAW_NAME}

  Articles processed: {TOTAL}
  Made executable: {EXECUTABLE_COUNT}
  Validation: {PASSED/FAILED}
  Test scenarios: {PASS_COUNT}/{TOTAL_COUNT} passing

  Iterations needed: {N}

  Remaining issues:
  - {description of any unresolved failures}

  TODOs:
  - {external laws that need to be downloaded/implemented}

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

- `competent_authority`: who has binding authority (`name`, `type: INSTANCE|CATEGORY`)
- `requires`: dependencies on other laws
- `definitions`: constants (eurocent for money)
- `execution.produces`: legal character and decision type
- `execution.parameters`: caller inputs (types: `string`/`number`/`boolean`/`date`)
- `execution.input`: data from other sources (types include `amount`)
- `execution.output`: what article produces (types include `amount`)
- `execution.actions`: computation logic with operations
- `legal_basis`: traceability to specific law text
- `resolve`: lookup from ministeriele_regeling
