---
title: "Adding a Law"
description: "A step-by-step walkthrough from downloading a law's text to running tests against it."
---

Adding a new law to the corpus takes six steps, from downloading the legal text to running tests against it.

## Step 1: Find the law

Every Dutch national law has a BWB ID (format: `BWBR` + 7 digits). Find it on [wetten.overheid.nl](https://wetten.overheid.nl).

For example, the Zorgtoeslagwet is `BWBR0018451`.

## Step 2: Harvest the legal text

Use the harvester to download and convert the law from BWB XML to YAML:

```bash
# Download today's version
regelrecht-harvester download BWBR0018451

# Download for a specific date
regelrecht-harvester download BWBR0018451 --date 2025-01-01 --output corpus/regulation/nl
```

This produces a YAML file with the law's text but no `machine_readable` sections. The output path follows the convention: `corpus/regulation/nl/{layer}/{slug}/{date}.yaml`.

## Step 3: Add machine-readable logic

Each article that contains executable logic needs a `machine_readable` section. This can be done:

- **Manually** - write the `machine_readable` YAML by hand following the [law format](/concepts/law-format)
- **Via the pipeline** - trigger an enrichment job through the admin dashboard, which uses an LLM to generate candidate interpretations

If using LLM-generated interpretations, always validate the output (step 5).

## Step 4: Validate against the schema

```bash
# Validate a specific file
just validate corpus/regulation/nl/wet/your_law/2025-01-01.yaml

# Validate all files
just validate
```

The validator rejects files with an unknown or missing `$schema` version. Make sure the `$schema` URL uses a tag-based ref (`refs/tags/schema-vX.Y.Z`) and points to a released schema version.

Fix any schema errors before proceeding.

## Step 5: Write BDD test scenarios

Derive test scenarios from the Memorie van Toelichting (MvT), the explanatory memorandum that accompanies the law. The MvT contains worked examples of how the legislature intended the law to be applied.

A law's scenarios live next to the law, in a `scenarios/` directory beside the YAML file: `corpus/regulation/nl/wet/your_law/scenarios/eligibility.feature`. That is bucket A, the law-validation bucket, described in [Testing](/guide/testing).

The step vocabulary is not free text. `bdd/grammar.yaml` is the single source of truth, and the bindings are generated from it, so a step that is not in that file does not exist. A minimal scenario:

```gherkin
Feature: Wet op de zorgtoeslag

  Scenario: MvT example, single person, output present
    Given the calculation date is "2025-01-01"
    Given parameter "bsn" is "999993653"
    When I evaluate "hoogte_zorgtoeslag" of "wet_op_de_zorgtoeslag"
    Then the execution succeeds
    Then output "hoogte_zorgtoeslag" equals 157731
```

Laws that need source data (BRP, Belastingdienst, and the like) provide it with a data-table step keyed on the identifier the law looks up:

```gherkin
    Given the following "personal_data" data with key "bsn":
      | bsn       | geboortedatum | verblijfsadres |
      | 999993653 | 2005-01-01    | Amsterdam      |
```

See `corpus/regulation/nl/wet/wet_op_de_zorgtoeslag/scenarios/eligibility.feature` for a complete, data-driven example.

Run the tests:

```bash
just bdd
```

## Step 6: Open a pull request

Commit the new law file, any BDD scenarios, and open a PR. CI will run schema validation, BDD tests, and all other checks automatically. Add the `deploy:preview` label to the PR if reviewers should be able to try the law in a running editor.

End the PR body with a `Werkpakket:` line, which a required check enforces, and add a `Wet:` line naming the law's `$id`:

```
Werkpakket: referentie-casus-i
Wet: wet_op_de_zorgtoeslag
```

See [Contributing](/operations/contributing) for what both lines mean and how the slug is checked.

## Further reading

- [Law Format](/concepts/law-format) - how to structure the YAML
- [Testing](/guide/testing) - more on writing and running tests
- [Validation Methodology](/concepts/methodology) - the execution-first validation approach
