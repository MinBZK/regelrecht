---
name: law-mvt-research
description: >
  Searches for Memorie van Toelichting (explanatory memoranda) for a Dutch law
  and generates Gherkin test scenarios from legislature-intended examples.
  Use when you want MvT-derived BDD scenarios without generating machine_readable.
allowed-tools: Read, Write, WebFetch, Bash, Grep, Glob
user-invocable: true
---

# Law MvT Research — Find Parliamentary Examples and Generate Gherkin Scenarios

Searches for Memorie van Toelichting documents and converts legislature-intended
examples into Gherkin acceptance tests.

## Setup

1. Read the target law YAML file to extract `bwb_id`, title, and `effective_date`
2. Read an existing feature file as Gherkin style reference:
   `features/bijstand.feature`

## Step 1: Find MvT Documents

Extract the `bwb_id` (e.g., `BWBR0018451`) from the law YAML's `bwb_id` field.

Search for related parliamentary documents using the overheid.nl SRU API:

```
https://zoekservice.overheid.nl/sru/Search?operation=searchRetrieve&version=1.2&x-connection=officielepublicaties&query=dcterms.references=={BWB_ID}&maximumRecords=20
```

Use WebFetch to retrieve the results. Parse the XML response to find documents of
these types (in `<dcterms:type>`):
- **Memorie van toelichting** (explanatory memorandum)
- **Nota naar aanleiding van het verslag** (response to parliamentary report)
- **Nota van wijziging** (amendment note)
- **Brief van de minister** (ministerial letter with examples)

Also search by law title for additional coverage. **URL-encode the law title**
(replace spaces with `%20`, quotes with `%22`, etc.) before substituting:
```
https://zoekservice.overheid.nl/sru/Search?operation=searchRetrieve&version=1.2&x-connection=officielepublicaties&query=dcterms.title%20any%20%22{URL_ENCODED_LAW_TITLE}%22%20AND%20dcterms.type%3D%3D%22Memorie%20van%20toelichting%22&maximumRecords=10
```

There may be **multiple MvT documents** (original + amendments). Collect all of them.

**Error handling:** The SRU API may return empty results, HTTP errors, or malformed
XML. If a search returns no results:
1. Try the alternate query (BWB ID vs title, or vice versa)
2. Try broadening the title query (use fewer keywords)
3. If both fail, report "No MvT documents found" and proceed — this is not an error

## Step 2: Download and Read MvT Content

For each found document, extract the document identifier from the search results.
The `<dcterms:identifier>` field typically contains a full URI like
`https://identifier.overheid.nl/BWBR/sgd/kst-36450-3`. Extract only the **last
path segment** (e.g., `kst-36450-3`) and use it to download the HTML version:

```
https://zoek.officielebekendmakingen.nl/{DOCUMENT_ID}.html
```

Use WebFetch to retrieve the content. If HTML is too large, focus on sections that
contain:
- "voorbeeld" (example)
- "rekenvoorbeeld" (calculation example)
- "casus" (case)
- "scenario"
- "tabel" (table — often contains example calculations)
- "berekening" (calculation)
- "stel dat" (suppose that)
- "in het geval" (in the case of)

## Step 3: Extract Test-Relevant Information

From the MvT content, extract:

1. **Rekenvoorbeelden** (calculation examples):
   - Input values used by the legislature
   - Expected output values
   - Step-by-step calculations shown

2. **Concrete scenario's** (concrete scenarios):
   - Described situations with specific parameters
   - Expected outcomes stated by the legislature

3. **Randgevallen** (edge cases):
   - Boundary conditions explicitly discussed
   - Special cases the legislature considered

4. **Bedoelde uitkomsten** (intended outcomes):
   - "De bedoeling is dat..." (the intention is that...)
   - "Dit betekent dat een persoon die..." (this means that a person who...)

For each extracted example, note:
- Which article(s) it relates to
- The input parameters and their values
- The expected output/result
- The source document and page/section reference

## Step 4: Generate Gherkin Feature File

Write a `.feature` file to `features/{slug}.feature` based on the MvT examples,
where `{slug}` is the law's short name slug (e.g., `zorgtoeslag`, `bijstand`,
`participatiewet`) — matching the convention used by existing feature files.
Do NOT use the full `$id` or BWB ID as the filename.

Follow the existing project conventions (see `features/bijstand.feature` and
`features/zorgtoeslag.feature` for style).

**Structure:**
```gherkin
Feature: {Law title} — scenarios uit Memorie van Toelichting
  Testscenario's afgeleid uit de Memorie van Toelichting en parlementaire
  stukken bij {law_title}.

  # Bron: {MvT document identifier(s)}
  # URL: {MvT document URL(s)}

  Background:
    Given the calculation date is "{effective_date}"

  # === Rekenvoorbeelden uit MvT ===

  Scenario: {Description from MvT}
    # Bron: {document_id}, {section/page reference}
    Given a citizen with the following data:
      | parameter_1 | value_1 |
      | parameter_2 | value_2 |
    When the {law_execution} is executed for {law_id} article {N}
    Then the {output_name} is "{expected_value}" eurocent

  # === Randgevallen ===

  Scenario: {Edge case from MvT}
    # Bron: {document_id}, {section/page reference}
    ...
```

**Guidelines:**
- Each scenario MUST trace back to a specific MvT passage (add `# Bron:` comments)
- Convert monetary amounts in MvT to eurocent
- Use the same Given/When/Then step patterns as existing feature files
- If MvT examples reference external data sources (RVIG, Belastingdienst, etc.),
  use the appropriate Given steps for those sources
- If the MvT doesn't provide enough examples for a specific article, note this in
  a comment but do NOT invent scenarios — only use what the legislature provided
- Group scenarios by: rekenvoorbeelden, randgevallen, afwijzingsscenario's

## Step 5: Report MvT Findings

Report to the user before proceeding:

```
MvT Research for {LAW_NAME}

  Documents found: {COUNT}
  - {doc_id_1}: {title} ({date})
  - {doc_id_2}: {title} ({date})

  Extracted scenarios: {SCENARIO_COUNT}
  - Rekenvoorbeelden: {N}
  - Randgevallen: {N}
  - Afwijzingsscenario's: {N}

  Feature file: features/{slug}.feature

  Articles without MvT examples: {list}
  Note: No synthetic scenarios were added for these articles.
```

If NO MvT documents are found, report this clearly. The generation phase will
fall back to the JSON-based test approach.
