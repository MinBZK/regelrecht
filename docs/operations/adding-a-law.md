# Adding a Law

This guide walks through adding a new law to the corpus, from downloading the legal text to running tests against it.

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

Fix any schema errors before proceeding.

## Step 5: Write BDD test scenarios

Derive test scenarios from the Memorie van Toelichting (MvT) - the explanatory memorandum that accompanies the law. The MvT contains worked examples of how the legislature intended the law to be applied.

Create a Gherkin feature file in `features/`:

```gherkin
Feature: Wet op de zorgtoeslag

  Scenario: MvT example - single person with low income
    Given the law "zorgtoeslagwet"
    And the reference date is "2025-01-01"
    And the parameter "bsn" is "999993653"
    When I evaluate "hoogte_zorgtoeslag"
    Then the result should be 123400
```

Run the tests:

```bash
just bdd
```

## Step 6: Open a pull request

Commit the new law file, any BDD scenarios, and open a PR. CI will run schema validation, BDD tests, and all other checks automatically. A preview deployment lets reviewers try the law in the editor.

## Further reading

- [Law Format](/concepts/law-format) - how to structure the YAML
- [Testing](/guide/testing) - more on writing and running tests
- [Validation Methodology](/concepts/methodology) - the execution-first validation approach
