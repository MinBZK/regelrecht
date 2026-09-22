---
title: "Aanvraag-cel"
description: "A proof-of-concept cell that records a submitted application as a chronolexogram and reduces it to a lexostatus a regulation can test."
---

The aanvraag-cel is a small proof of concept for [RFC-022](/rfcs/rfc-022). A fictitious organization logs in with a simulated eHerkenning, fills in an application form and submits it. The cell of the receiving authority records the receipt as a chronolexogram in an append-only chronicle. A reduction turns that chronicle into a lexostatus, and the engine evaluates one outcome of a regulation (a `TOETS`) against it, for example whether the application is complete.

Nothing in the code names a case. The stream definition, the lexostatus definitions and the corpus all come from configuration, so the same cell runs on the generic test fixtures in this repository and on a private case corpus.

## Overview

- **Language**: Rust (axum) for the cell, Vue 3 with Vite and `@nldd/design-system` for the frontend
- **Location**: `packages/aanvraag-cel/` and `frontend-aanvraag-cel/`
- **Schemas**: `schema/chronolex/v0.1.0/` (`stream.json`, `lexostatus.json`, `gram.json`)
- **Run locally**: `just aanvraag-cel` starts the cell on port 7170 and the frontend on port 7171

## Four layers

The design keeps four things apart. The regulation does not know that a chronicle exists, and the chronicle does not know which article reads it. The reduction is the only place where the two meet.

1. **Lexogram.** The regulations under `REGULATION_PATH`, unchanged. An article declares the parameters it needs, such as `bevat_naam` or `aanvraagdatum`.
2. **Stream definition.** Which facts the cell records, by which actor, in which chronicle and on which legal ground (`grondslag`). Each field binds to `$intake.*` (who submitted it and through which channel), to `$external.*` (the content as submitted) or to a constant of the stream. A table field declares its columns: `{tabel: $external.<path>, kolommen: [...]}`. This follows the sketch in RFC-022 §1.3.
3. **Reduction to lexostatus.** A `lexostatus_definitions` entry picks grams from the chronicle with `filter` and `kies: laatste`, and derives each parameter of an article with an `afleiding`. The vocabulary is small and fixed: `veld`, `gevuld`, `gelijk`, `tabel` with `elke_regel` or `een_regel` (optionally `alleen_waar`), and `moment`. Field paths use dots, as in `inhoud.organen`.
4. **The gram.** What the cell writes, one JSON line per gram in `DATA_DIR/<chronicle>.jsonl`. A gram is appended and never changed; a correction is a new gram. It carries `kind`, `type`, `soort`, `name`, `chronicle`, `recording_actor`, `grondslag`, `op_moment`, `zaakkenmerk`, `stroom {id, sha256}` and `fields`.

A submitted application is recorded with `type: indiening` and `soort: aanvraag`, and it opens a new case (`zaakkenmerk`). For that soort, `gram.json` fixes `fields.kern` to the elements Awb 4:2 paragraph 1 asks of every application: the applicant's name and address, the date, the decision requested and the signature (`ondertekend_via`). The content a specific regulation asks for goes under `fields.inhoud`, which each stream defines. A field left empty is recorded as `null`, so an incomplete application is recorded as well. Completeness is a judgment, and the gram holds none.

## Start-up checks

The cell refuses to start when a check fails, and names the field or parameter in the message.

1. The stream definitions and the cell configuration validate against their schemas.
2. Every derivation points at a parameter of an article in the `grondslag` of the event its filter selects, and at field paths that exist in that event. A `tabel` derivation points at a table field and reads only columns the stream declares for it.
3. No orphaned field: each field of each event is read by a derivation, or is listed under `niet_gereduceerd` with a reason.
4. No name collision: a parameter gets one derivation.

The same pass checks that the `portaal` block in the cell configuration names an existing event, lexostatus and regulation outcome, and that the article with that outcome is in the `grondslag` of the event.

## Configuration

| Variable | Meaning |
|---|---|
| `REGULATION_PATH` | Directory with the regulations. Every YAML file with `$id` and `articles` is loaded; other files are skipped. |
| `CHRONICLES_PATH` | A stream definition, or a directory of them. |
| `CELL_CONFIG_PATH` | The cell configuration: `cel`, `lexostatus_definitions` and the `portaal` block. |
| `DATA_DIR` | Where the chronicles are written. |
| `AANVRAAG_CEL_PORT` | Port of the cell, default 7170. The cell binds to `0.0.0.0`. |

The `portaal` block says which event a submission becomes, which lexostatus and outcome the check before submitting evaluates (`toets: {lexostatus, regeling, uitkomst}`), and optionally which form file supplies labels and order (`formulier: {pad, scherm}`). The form file never changes behavior: a field it does not know gets its field name as label, and a field the stream does not know is skipped.

## Routes

| Route | Purpose |
|---|---|
| `POST /api/eherkenning/login` | `{kvk, persoon, machtiging}` to a session. A KvK number has eight digits and the only accepted mandate is `volledig`. There is no register. |
| `GET /api/stroom` | The stream definition and the form fields |
| `POST /api/aanvraag/toets` | Builds the gram in memory without recording it, reduces it and evaluates the configured outcome. `ontbreekt` lists the parameters of a presence derivation (`gevuld`, `tabel` with `elke_regel`) that came out false. |
| `POST /api/aanvraag` | Records the gram and returns it |
| `GET /api/kroniek` | The grams of the logged-in KvK number, each with its YAML |
| `GET /api/lexostatus/{naam}?zaakkenmerk=...` | The reduction of one of your own cases |

## Deviations from RFC-022

These are deliberate. Each is a candidate for an amendment once the proof of concept has run on a real case.

1. **Type `indiening` with `soort: aanvraag`**, instead of an executogram. The position paper leaves the typology open, and RFC-022 §1 calls the set of types "not a closed set".
2. **`grondslag` is a list.** An application rests on several articles at once; RFC-022 §1.3 has a single value.
3. **The format of `lexostatus_definitions`.** RFC-022 §4.1 sketches it and calls it provisional. This cell fills in `reduction` with `kroniek`, `filter`, `kies` and `afleidingen`.
4. **`kern` and `inhoud`, constant fields, and `$intake.*`** next to `$external.*`. Who submitted and through which channel belongs to the intake, not to the content.
5. **`niet_gereduceerd`** in the stream, for the orphaned-field check.
6. **A separate schema directory**, `schema/chronolex/`, instead of the `chronicle.json` that RFC-022 §1.3 places in `schema/v0.6.0/`.
7. **The `portaal` block** in the cell configuration. It is how the configuration, not the code, decides which outcome the check evaluates.

## Choices the cell makes

RFC-022 does not settle the points below. The cell settles them as follows.

**The gram keeps the shape of the stream.** A field left empty is recorded as `null`, because `kern` is fixed and an incomplete application is recorded too. A table field is recorded row by row, and each row has every declared column in the order of the stream, with `null` for a column the submission left out. The cell refuses a submission (HTTP 400) that does not fit this shape, and names the field path in the message:

- a field under `external` that no `$external` binding reads;
- a key under a nested `$external` object that the stream does not bind, such as `adres.huisnummer` when only `$external.adres.straat` is bound;
- a column the table field does not declare, such as `organen[1].kleur`;
- a list or object where the stream expects a single value, or a table that is not a list of rows.

What has no `grondslag` is not recorded. A form file cannot widen this. Its columns are shown only when the stream declares them, and whether a field is a table is up to the stream, not the form.

**`filter` selects on the gram itself.** The keys are `name`, `type`, `soort`, `zaakkenmerk`, `recording_actor` and `chronicle`, and none of them is required. The fixtures filter on `type`, `soort` and `zaakkenmerk` without `name`, so a second event of the same soort would be read by the same reduction. A value `$x` comes from the inputs of the lexostatus.

**`ontbreekt` lists presence derivations that came out false.** A presence derivation is `gevuld`, or `tabel` with `elke_regel`. When one of those is false, the application lacks something, and the parameter is listed under `ontbreekt` in the answer of `POST /api/aanvraag/toets`. This follows the `bevat_*` parameters of a regulation without relying on their names. A false `gelijk` or `een_regel` is an answer, not a gap, so it is not listed. A `veld` or `gelijk` derivation on an empty field yields no value at all; the parameter stays out of the lexostatus and the cell does not fill it in.

**`tabel` with `elke_regel`** is true when the table has at least one row and every row has the named column filled. Filled means not `null`, not empty text, and not an empty list or object. An empty or missing table gives false. With `alleen_waar: <column>`, only rows where that column is exactly `true` count. If no row qualifies, the result is true as long as the table has rows, since nothing that the condition asks for is missing. `tabel` with `een_regel` is true when at least one row has the named column exactly `true`.

## Open questions

1. **Typology.** Is `indiening` (application, notification, report, objection) a fourth class next to lexogram, decretogram and executogram? The position paper leaves this for further research, and so does RFC-022 §1. A candidate for an amendment to RFC-022.
2. **RFC-008.** Is the submission gram the recorded fact behind the `AANVRAAG` stage? And do the acknowledgement of receipt (Awb 4:3a) and a supplement (Awb 4:5) become grams of their own, stages of their own, or both?
3. **Correction and supplement.** A supplement is a new gram with the same `zaakkenmerk`. Does the reduction then take `laatste`, or does it merge field by field?
4. **Whose cell.** Only the receiving authority records the submission now. Does it also belong in a cell of the applicant, and how do the two grams relate?
5. **`grondslag` as a list.** A submission rests on several articles at once, where RFC-022 §1.3 has one `grondslag`.
6. **The format of the reduction.** RFC-022 §4.1 calls `lexostatus_definitions` provisional. Does the derivation vocabulary (`veld`, `gevuld`, `gelijk`, `tabel` and the rest) belong in an RFC?
7. **Value or `bevat_*`.** Regulations model the content of an application as booleans ("the application contains ..."). Should an article ask for the value itself as a parameter, so a decision can use it?
8. **Awb 4:2 paragraph 1.** The fixed core (name, address, date, requested decision, signature) is not machine-readable in the corpus, so `kern` is not reduced. Should paragraph 1 get a `TOETS`?
9. **Signature.** Does the login method count as a signature (Awb 2:16)? And how does `ondertekend_via` relate to signing the gram itself (RFC-009)?
10. **Prefilling.** If data is later prefilled from registers, how do you record that the applicant saw and confirmed it, without keeping a shadow copy of the source?
11. **Engine and single outcomes.** The engine evaluates the whole article when one outcome is asked for. Every action runs and every `input` is resolved, including cross-law inputs the requested outcome never reads. A required parameter that only another output uses is therefore needed as well. The engine stops at the first missing value (`Variable not found`); a `required: false` parameter that is not passed becomes an unknown value and is reported as missing. For the check before submitting, this means an outcome can only be judged when every fact the article touches is present, including facts of the authority itself. The cell does not fill anything in. It reports "niet te beoordelen: mist <parameter>", one missing value per run. A test (`engine_eist_het_hele_artikel_bij_een_uitkomst` in `packages/aanvraag-cel/src/toets.rs`) pins this behavior on the fixtures. An engine that resolves only what the requested outcome depends on would let the check answer from the application alone.
12. **Schema version.** `schema/chronolex/v0.1.0/` stands apart from `schema/v0.6.0/`. Merge them, or keep them separate?
