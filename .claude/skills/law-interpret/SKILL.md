---
name: law-interpret
description: >
  Orchestrates the full law interpretation pipeline: MvT research, machine_readable
  generation with validation/BDD testing, and reverse validation. Invokes
  /law-mvt-research, /law-generate, and /law-reverse-validate sequentially.
  Use this skill proactively when: user wants to make a law executable, add
  machine_readable sections, or mentions "interpret" in the context of a Dutch
  law YAML file. Activate automatically when user opens a corpus YAML file
  without machine_readable sections and wants to make it executable.
allowed-tools: Read, Bash, Grep, Glob, Skill
user-invocable: true
---

# Law Interpret — Orchestrator

Orchestrates the full pipeline for making a Dutch law YAML file executable.
Invokes three sub-skills sequentially, passing context between them.

## Step 1: Identify Target

1. Determine which law YAML file to interpret (from user input or context)
2. Read the target file to confirm it exists and extract key metadata:
   - `name` (law title)
   - `bwb_id` (BWB identifier for MvT search, top-level field)
   - `valid_from` (effective date)
   - Count of articles
3. Report to user: "Starting interpretation of {law_name} ({N} articles)"

## Step 2: MvT Research

Invoke `/law-mvt-research` on the target law file.

This searches for Memorie van Toelichting documents and generates Gherkin
test scenarios from legislature-intended examples.

**Context passing:** The MvT skill writes its output to `features/{slug}.feature`.
The `/law-generate` skill in Step 3 picks this up automatically when running
`just bdd`, which executes all feature files in `features/`. No manual context
transfer is needed — the file system is the interface.

After completion, note the results:
- How many MvT documents were found
- How many Gherkin scenarios were generated
- Which articles lack MvT examples
- Path to the generated feature file

**If no MvT documents are found:** This is common for many Dutch laws. Proceed
to Step 3 anyway — `/law-generate` will fall back to ad-hoc testing via the
evaluate binary. Not having MvT scenarios reduces confidence but does not block
the pipeline.

## Step 2.5: Schema Version Check

Before generating, verify the target file's `$schema` URL points to v0.5.0. If it points
to an older version (e.g., v0.3.2 or v0.4.0), update it to:
```
https://raw.githubusercontent.com/MinBZK/regelrecht/refs/heads/main/schema/v0.5.0/schema.json
```
This prevents generating v0.5.0 logic against an old schema declaration.

## Step 3: Generate Machine-Readable Logic

Invoke `/law-generate` on the target law file.

This creates `machine_readable` sections, validates against the schema,
runs BDD tests, and iterates until correct (up to 3 iterations).

After completion, note the results:
- How many articles were made executable
- Validation status
- BDD test results
- Any remaining issues

## Step 4: Reverse Validation

Invoke `/law-reverse-validate` on the target law file.

This checks every element in `machine_readable` traces back to the original
legal text, catching hallucinated logic.

**Important:** If reverse validation removes any elements from the YAML, re-run
`just validate <file>` to ensure the file still passes schema validation.
Element removal can break required field constraints or leave dangling references.
If validation fails after removal, re-invoke `/law-generate` to regenerate the
affected sections (this orchestrator does not have Edit access — delegate file
modifications to the sub-skills).

## Step 5: Dependency Check

Before the final report, scan the generated `machine_readable` sections for
`source.regulation` references. For each referenced regulation:

1. Check if it exists in `corpus/regulation/nl/` using Glob
2. If missing, add it to the TODOs list with a note to run `/law-download` for it
3. If present but lacking `machine_readable`, note it needs `/law-interpret`

Additionally:
- If the law declares `hooks`, verify that target laws with matching `produces.legal_character` exist in the corpus
- If the law declares `overrides`, verify the target law/article/output exists
- If `produces` has `procedure_id`, verify the corresponding procedure definition exists

This helps the user understand what additional work is needed for full execution.

## Step 6: Final Report

Combine results from all phases into a single report:

```
Interpreted {LAW_NAME}

  MvT sources: {MvT_COUNT} documents found
  - {doc_id}: {title}

  Articles processed: {TOTAL}
  Made executable: {EXECUTABLE_COUNT}
  Validation: {PASSED/FAILED}

  BDD scenarios: {PASS}/{TOTAL} passing
  (from MvT feature file and/or ad-hoc evaluate tests)

  Iterations needed: {N}

  Reverse validation:
  - Fully grounded: {N} articles
  - Assumptions: {N} (see details above)
  - Elements removed: {N}

  Remaining issues:
  - {description of any unresolved failures}

  Dependencies:
  - {regulation_id}: {status: present/missing/needs machine_readable}
    → Run `/law-download` then `/law-interpret` if missing

  TODOs:
  - {external laws that need to be downloaded/implemented}

  Feature file: features/{slug}.feature
  The law is now executable via the engine!
```
