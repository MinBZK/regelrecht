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

## CRITICAL: Scope Audit

The most important check is **scope**: does the machine_readable section stay within the
boundaries of the legal provision it belongs to?

Each machine_readable must interpret ONLY the text of the article, lid, or provision it is
attached to. It must not pull in logic, conditions, thresholds, or values from other
provisions — even when doing so seems "obvious" or "efficient."

This is the most common failure mode. The engineering instinct is to optimize by combining
logic from multiple articles into one execution block, but this fundamentally breaks the
contract between the law and its machine-readable interpretation. The law may be redundant,
circular, or inefficient. That is intentional — model it as the law writes it.

**Scope violations to flag:**
- Conditions not in this provision's text (e.g., an age check in an article that doesn't
  mention age — even if age is required by another article)
- Hardcoded values from other provisions (e.g., drempelinkomen amounts in an article that
  only references the concept, not the value)
- Logic from other articles reimplemented instead of referenced via `source`
- Eligibility conditions from one article stuffed into an orchestration article
- `legal_character` that doesn't match what this provision does (e.g., BESCHIKKING on a
  norm article that merely sets amounts)

**When cross-article dependencies are needed, the correct mechanism is:**
- `input` with `source.regulation` / `source.output` for values from other provisions
- `open_terms` for delegated values
- `hooks` / `overrides` for reactive cross-law interactions

## Instructions

1. Read the target law YAML file
2. For each article that has a `machine_readable` section:
   a. Read the article's `text` field carefully — this defines the SCOPE
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

| Traceable in THIS provision's text? | Needed for logic? | Action |
|-------------------------------------|-------------------|--------|
| YES | YES | Keep |
| YES | NO | Keep (informational) |
| NO, but in another provision | YES | **Scope violation** — must be refactored to use `source` reference |
| NO | YES | Report as assumption |
| NO | NO | **Remove** |

**Scope violations are the highest priority finding.** They mean logic from one provision
has leaked into another. This is worse than a missing element, because it produces results
that look correct but cannot be traced back to the provision that claims to produce them.

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

## Workaround Detection — Untranslatables (RFC-012)

Check for signs that the translator approximated a construct that should be flagged
as untranslatable:

**Red flags:**
- `IF` with >8 cases — likely an inlined table lookup. Ask: is this logic written in
  the law, or is it an approximation of a table?
- Arithmetic chains that approximate rounding — e.g., `MULTIPLY` then `DIVIDE` by
  powers of 10. If the law says "afgerond", this should be an untranslatable.
- Hardcoded values not mentioned in the article's text — may be pre-computed results
  of a calculation the translator couldn't express

**When you detect a workaround:**
1. Extract the construct to an `untranslatables` entry
2. Simplify the execution logic to handle only the translatable parts
3. Re-run `just validate <file>`
4. Report in the findings

## Domain Knowledge Leak Check

Verify that no domain knowledge has been hardcoded into translations:

- **Holiday dates**: If a `definitions` block contains a hardcoded date matching
  a known holiday (January 1, December 25, April 27, etc.), flag it. Holiday dates
  must be parameters populated from `corpus/context/nl/calendar/`.
- **Institutional facts**: If a `definitions` block contains a specific institution
  name, municipality code, or organizational fact not stated in the article text,
  flag it.
- **Cross-cutting law logic**: If the machine_readable reimplements logic from a
  hook-based law (AWB motiveringsplicht, Termijnenwet deadline extension) instead
  of relying on hooks, flag as a scope violation. Consult `corpus/context/nl/hooks/`
  for the list of hook-based laws.
- **The test**: Can every element in the machine_readable be traced back to the
  article text without external knowledge? If not, domain knowledge has leaked in.

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

  Removed elements:
  - Article {N}: {what was removed and why}

  Possible untranslatable workarounds:
  - Article {N}: {pattern detected} — recommend extracting to untranslatables
```
