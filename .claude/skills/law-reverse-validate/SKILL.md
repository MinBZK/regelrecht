---
name: law-reverse-validate
description: >
  Performs a hallucination check on machine_readable sections by verifying every
  element traces back to the original legal text. Use this skill proactively when:
  machine_readable sections have been generated or modified, after /law-generate
  completes, when reviewing corpus YAML files for legal accuracy, or when user
  mentions "validate", "verify", or "hallucination check" for law YAML files.
  Activate automatically after editing machine_readable sections in corpus
  regulation YAML files.
allowed-tools: Read, Edit, Bash, Grep, Glob
user-invocable: true
---

# Law Reverse Validate — Hallucination Check

Verifies that every element in `machine_readable` sections traces back to the
original legal text. This catches invented logic, phantom conditions, and
hallucinated operations that aren't grounded in the law.

## Instructions

1. Read the target law YAML file
2. For each article that has a `machine_readable` section:
   a. Read the article's `text` field carefully
   b. Check every element in the `machine_readable` section:
      - Every `input` field — is it referenced in the legal text?
      - Every `parameter` — is it needed by the legal text?
      - Every `definition` — does the value match the legal text exactly?
      - Every `action` and its `operation` — does the legal text describe this logic?
      - Every comparison value — does the legal text state this threshold/amount?
      - Every `source.regulation` reference — does the legal text reference that law?
      - Every `endpoint` — is there a reason for external callability?
      - Every `hooks` entry — does the legal text describe a rule triggered by lifecycle events (e.g., "na bekendmaking", "bij bezwaar")?
      - Every `overrides` entry — does the legal text explicitly state "in afwijking van artikel X" or similar override language?
      - Every `produces.legal_character` — does the article produce a legal decision (beschikking, toets, etc.)?
      - Every `produces.procedure_id` — is there a specific procedure variant referenced?
      - Every `open_terms` entry — does the legal text delegate to a lower regulation ("bij ministeriële regeling", "bij gemeentelijke verordening")?

3. Classify each element:

| Traceable in text? | Needed for logic? | Action |
|-------------------|-------------------|--------|
| YES | YES | Keep |
| YES | NO | Keep (informational) |
| NO | YES | Report as assumption |
| NO | NO | **Remove** |

4. For elements classified as "Remove": delete them from the YAML using Edit
5. For elements classified as "Report as assumption": collect them for the report
6. **After any removals:** re-run `just validate <file>` to ensure the file still
   passes schema validation. Removing elements can break required field constraints
   or leave dangling `$variable` references. Fix any validation errors before
   completing the report.

## Operation Correctness Check

Verify that no v0.4.0-only operations are used:
- No `when`/`then`/`else` on IF operations (must be `cases`/`default`)
- No SUBTRACT_DATE (must be AGE)
- No CONCAT (must be ADD with string values)
- No NOT_EQUALS, IS_NULL, NOT_NULL, NOT_IN (must use NOT wrapper)
- No FOREACH (removed from schema)

## Report

Report findings to the user:

```
Reverse Validation for {LAW_NAME}

  Articles checked: {COUNT}

  ✅ Fully grounded: {N} articles
  ⚠️  Contains assumptions: {N} articles
  🗑️  Elements removed: {N}

  Assumptions requiring review:
  - Article {N}: {description of assumed element}
  - Article {M}: {description of assumed element}

  Removed elements:
  - Article {N}: {what was removed and why}
```
