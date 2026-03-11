---
name: law-reverse-validate
description: >
  Performs a hallucination check on machine_readable sections by verifying every
  element traces back to the original legal text. Use after /law-generate to
  catch invented logic that isn't grounded in the law.
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

3. Classify each element:

| Traceable in text? | Needed for logic? | Action |
|-------------------|-------------------|--------|
| YES | YES | Keep |
| YES | NO | Keep (informational) |
| NO | YES | Report as assumption |
| NO | NO | **Remove** |

4. For elements classified as "Remove": delete them from the YAML using Edit
5. For elements classified as "Report as assumption": collect them for the report

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
