---
title: "Validation Methodology"
description: "Short overview of RegelRecht's execution-first validation method and the generate/validate/reverse-check loop."
---

RegelRecht uses an **execution-first** approach to validate machine-readable law interpretations. This page is a short overview; for the research background and the full argument, see [RegelRecht Validation: From Analysis-First to Execution-First](./validation-methodology).

## Execution-First

Traditional approaches analyze law text extensively before writing any code. RegelRecht inverts this:

```mermaid
flowchart LR
    A[Law Text] --> B[Machine-Readable YAML]
    B --> C[Execute with Test Cases]
    C --> D{Results Correct?}
    D -->|No| B
    D -->|Yes| E[Validate Against MvT]
    E --> F{Faithful to Intent?}
    F -->|No| B
    F -->|Yes| G[Published]
```

### Reasons

Execution surfaces errors immediately, without waiting for a lengthy analysis to finish. Test cases from the Memorie van Toelichting (MvT) provide ground truth, so each cycle improves the interpretation on the basis of actual results. After generation, every element is checked against the source text to catch hallucinated logic.

## The Loop

### 1. Generate

Create `machine_readable` sections for law articles, defining inputs, outputs, and operations.

### 2. Validate & Test

- Schema validation ensures structural correctness
- BDD scenarios (derived from MvT examples) verify behavioral correctness
- The engine executes the law and compares outputs to expected values

### 3. Reverse Validate

Every element in the machine-readable interpretation is traced back to the original legal text. Any logic that cannot be grounded in the law is flagged as potentially hallucinated.

## Memorie van Toelichting (MvT)

The MvT is the explanatory memorandum that accompanies Dutch legislation. It contains:

- The legislature's intent and reasoning
- Concrete examples of how the law should be applied
- Edge cases the legislature considered

These examples are the primary test cases for machine-readable interpretations.
