---
title: "How RegelRecht Works"
description: "A plain-language walkthrough of the core ideas behind turning legislation into executable files."
---

RegelRecht turns Dutch legislation into structured files that a computer can execute. This page is the map. It follows a law from text to file to execution and names the concepts met on the way, each with a link to the page that covers it in full. Why this matters is on [What is RegelRecht?](/guide/what-is-regelrecht) and, at length, in the position paper [Rules as Executed](/research/rules-as-executed). Which programs do the work is in the [architecture overview](/guide/architecture).

## The approach

RegelRecht encodes each law once, in a structured YAML format that both people and computers can read. A single execution engine runs these law files and produces answers: does this person qualify? How much do they receive? Which rules applied?

The YAML specification *is* the law in executable form. Every article in the file corresponds to an article in the official legal text, with a link back to the original.

A few principles hold throughout:

| Principle | What it means |
|-----------|---------------|
| **Close to the text** | The file follows the structure of the law, article by article. Interpreting the text and executing it are separate steps. |
| **Zero domain knowledge** | The engine has no hardcoded holidays, tax rates or special cases. Everything comes from law YAML. |
| **Identical execution** | Browser, backend, editor: same inputs, same result. |
| **Version control as governance** | Git history captures legislative evolution. Branches are proposals, merges are publication. |
| **Traceability** | Every computed value points back to a specific article and paragraph. |
| **Open by default** | Law, tooling, decisions: all publicly auditable. |

## How laws become YAML

Each Dutch law is stored as a YAML file. The file mirrors the legal structure:

```yaml
$id: wet_op_de_zorgtoeslag
name: Wet op de zorgtoeslag
regulatory_layer: WET
valid_from: '2025-01-01'

articles:
  - number: '2'
    text: |
      1. De verzekerde heeft aanspraak op een zorgtoeslag...
    url: https://wetten.overheid.nl/.../artikel/2
    machine_readable:
      definitions:
        DREMPELINKOMEN:
          value: 2500000   # EUR 25,000 in eurocent
      execution:
        input:
          - name: toetsingsinkomen
            type: amount
            source:
              regulation: algemene_wet_inkomensafhankelijke_regelingen
              output: toetsingsinkomen
              parameters:
                bsn: $bsn
        output:
          - name: hoogte_zorgtoeslag
            type: amount
        actions:
          - output: hoogte_zorgtoeslag
            value:
              operation: MAX
              values:
                - 0
                - operation: SUBTRACT
                  values:
                    - $standaardpremie
                    - $normpremie
```

Files are organized by legal hierarchy (`wet/`, `ministeriele_regeling/`, `gemeentelijke_verordening/`) and versioned by effective date (`2025-01-01.yaml`).

For full format details, see [Law Format](./law-format).

## How the engine executes a law

The engine is a calculator for law. You give it a law YAML file, tell it which output you want, provide some facts about a person (BSN, income, age, etc.), and specify a date.

The engine walks through the relevant articles, resolves all inputs, applies the legal logic (comparisons, arithmetic, conditions), and returns an answer with a full trace of how it got there.

To answer "does person X qualify for healthcare allowance?", the engine:

1. Loads the *Zorgtoeslagwet*
2. Looks at article 2, which needs: insurance status (from the Zorgverzekeringswet), allowance-partner status and income (from the Awir), and the standard premium (from a ministerial regulation)
3. Loads and executes those other laws automatically to get their values
4. Runs the calculation
5. Returns `heeft_recht_op_zorgtoeslag: true` and `hoogte_zorgtoeslag: 1234`

Same inputs always produce the same result. The engine runs as native code on servers and as WebAssembly in browsers, with identical behavior.

Because the engine knows nothing about any particular law, it stays small, but every law must be self-contained or reference other laws for the values it needs.

## Core concepts

These ideas show up throughout the system. Each has a dedicated page with examples from real laws in the corpus.

### Laws that reference each other

Dutch laws reference each other constantly. The healthcare allowance law needs your income (defined by the Awir), your insurance status (from the Zorgverzekeringswet), and your allowance-partner status (also from the Awir). An article declares each of those as an input with a `source` block naming the other law and the output it wants, as `toetsingsinkomen` does in the example file above. The engine follows these chains automatically. See [Cross-Law References](./cross-law-references) for the full picture.

### Delegation from higher to lower law

A *wet* often says "the minister determines the standard premium" or "the municipality sets the sanctions policy." The higher law leaves a blank: it names a value it needs (like `standaardpremie`) but leaves the actual number open (`open_terms`). A lower regulation then says "I provide that value" (`implements`). The engine discovers these connections at load time, matching the real legal hierarchy where a ministerial regulation opens with *"Gelet op artikel 4 van de Wet op de zorgtoeslag."*

This also means that different authorities can each provide their own version of the same value. The Participatiewet delegates sanctions policy to municipalities. Each of the 342 municipalities can write its own ordinance with different percentages. When the engine runs, it uses the `gemeente_code` in the execution scope to pick the right municipality's ordinance. Amsterdam gets Amsterdam's rules, Rotterdam gets Rotterdam's.

See [Inversion of Control](./inversion-of-control).

### Laws that fire automatically

The General Administrative Law Act (Awb) applies to every government decision without being called explicitly. When any law produces a *beschikking*, Awb rules about objection periods and reasoning requirements kick in through hooks. Neither law knows about the other. See [Hooks and Reactive Execution](./hooks-and-reactive-execution).

A *beschikking* is also not an instant computation. It moves through stages over time, from application and review to decision, notification and objection, and which rule applies at which stage is declared in the YAML (`applies_to.stage`), not hardcoded in the engine. See [Administrative procedure stages](./hooks-and-reactive-execution#administrative-procedure-stages).

### Overrides (lex specialis)

Sometimes a specific law overrides a general rule. The Aliens Act (*Vreemdelingenwet*) article 69 says: *"in afwijking van artikel 6:7 Awb bedraagt de termijn vier weken"*, departing from the Awb's standard 6-week objection period. This is modeled with `overrides`: the specific law unilaterally replaces a value from the general law. The Awb does not know it is being overridden. This only applies when the overriding law is part of the execution chain. See [Hooks and Reactive Execution](./hooks-and-reactive-execution#overrides-lex-specialis).

### Markings

The engine's operation set is small by design. When a legal construct cannot yet be faithfully expressed, complex table lookups or discretionary assessments for instance, the article carries a **marking** rather than an approximation. A marking means "not yet", not "never": each one is a named gap in the format and a tracked signal for what to build next. The engine can error, warn, or propagate taint through downstream outputs, depending on the mode. This prevents silent divergence between law text and machine-readable interpretation. The field was called `untranslatables` before schema v0.7.0. See [Markings](./markings).

### Execution provenance

Every execution produces a receipt: a sealed envelope containing the engine version, schema version, all loaded regulations (with content hashes), input parameters, outputs, and trace. This makes every decision reproducible and auditable, as required by the Awb, the AERIUS rulings, and the EU AI Act. For cross-organization decisions, the receipt also captures the provenance of accepted values from other authorities. See [Execution Provenance](./execution-provenance).

### Organizational boundaries and federated corpus

Different government organizations handle different parts of the law chain. The Tax Authority determines income, the Allowances Service determines healthcare allowance, municipalities handle social assistance. An article records which body may issue a binding decision in `competent_authority` (see [Competent Authority](./competent-authority)), and the engine uses that to model these boundaries. Today it runs in simulation mode (compute everything locally); the authoritative mode that exchanges signed results between organizations is the proposed end state, not yet implemented. See [Multi-Org Execution](./multi-org-execution).

On the data side, 342 municipalities, 12 provinces, and 21 water boards all produce their own regulations. The [federated corpus](./federated-corpus) model lets each authority maintain their own law files in their own Git repository while the engine discovers and loads them through a registry.

### Groups of unknown size

A household, the children a benefit is calculated for, the entries in a register: the law often reasons about a group nobody can count in advance. `FOREACH` and the other collection operations let the law iterate over such a group itself, so the legal test stays in the law instead of in the data source. See [Collections](./collections).

## Traceability

Every execution can produce a trace tree showing which articles applied, which inputs were fetched and from where, which operations ran, and what each step produced: the legal reasoning behind a number, in structured form. A trace makes a cross-law chain, an IoC resolution, and the Awb hooks that fired on a *beschikking* all visible in one tree. See [Traceability](./traceability) for how to read one, with a real worked example.

## Temporal versioning

Laws change over time. The standard premium was different in 2024 than in 2025. A calculation for January 2025 must use the rules and values in effect on that date. The engine selects the law version where `valid_from <= reference_date`.

The corpus contains both `regeling_standaardpremie/2024-01-01.yaml` and `regeling_standaardpremie/2025-01-01.yaml`. A calculation with `reference_date: 2024-06-15` automatically uses the 2024 value. See [Temporal Validity and Dates](./temporal-and-dates) for how a law expires with `valid_to`, what happens to a reference into an ended law, and how dates are compared and subtracted inside the rules.

## How an interpretation is checked

A machine-readable article is an interpretation, and an interpretation can be wrong. RegelRecht checks it by running it. A candidate is generated, often with AI assistance, and legal experts challenge it with concrete cases, many of them taken from the Memorie van Toelichting, where the legislature gave its own worked examples. [Execution-First Validation](./methodology) describes that loop; [From Analysis-First to Execution-First](./validation-methodology) gives the research background.

The cases themselves are written as [Scenarios](./scenarios): Gherkin over a fixed vocabulary, executed by the same engine that executes the law, so a legal expert and a programmer sign off on the same artifact.

## Around the law text

[Notes and Annotations](./notes-and-annotations) explains how comments and links attach to the legal text without being embedded in it, so they survive renumbering. [Branches of Law](./branches-of-law) surveys which areas of Dutch law map well to rule-based execution and which raise open research questions.
