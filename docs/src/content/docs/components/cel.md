---
title: "Cel"
description: "A proof-of-concept runtime for cells that record facts as chronolexograms, reduce them to lexostatuses, and combine lexostatuses from other cells."
---

The cell runtime is a small proof of concept for [RFC-022](/rfcs/rfc-022). A cell records facts as chronolexograms in its own append-only chronicle, and a reduction turns that chronicle into a lexostatus. A cell can have a portal: a fictitious organization logs in with a simulated eHerkenning, fills in an application form, checks it and submits it. The check evaluates one outcome of a regulation (a `TOETS`) against the lexostatus. When the regulation needs facts that another cell keeps, the check asks that cell for its lexostatus and combines the two (synthesis).

A cell is configuration, not code. The runtime loads every directory under `CELLS_PATH` that has a `cel.yaml`, so the same binary runs the generic test fixtures in this repository and the cells of a private case corpus. Nothing in the code names a case.

## Overview

- **Language**: Rust (axum) for the runtime, Vue 3 with Vite and `@nldd/design-system` for the frontend
- **Location**: `packages/cel/` and `frontend-cel/`
- **Schemas**: `schema/chronolex/v0.1.0/` (`cel.json`, `stream.json`, `lexostatus.json`, `gram.json`)
- **Run locally**: `just cel` starts the runtime on port 7170 with the fixture cells, and the frontend on port 7171

## One runtime, cells as configuration

```
cells/
  applicant-facing/
    cel.yaml            # id, recording_actor, stromen, lexostatussen, portaal?, synthese?
    lexostatussen.yaml
  register/
    cel.yaml            # no portaal
    lexostatussen.yaml
    startstand.jsonl    # optional: grams for an empty chronicle
```

Paths in `cel.yaml` are relative to its directory. Each cell gets its own chronicle under `DATA_DIR/<id>/` and its own routes under `/cellen/<id>/api/`. `GET /api/cellen` lists the cells with what each one offers: whether it has a portal, its lexostatuses with their inputs and parameters, and its synthesis sources. The frontend reads that list. With one portal cell it opens that cell directly; every cell, with or without a portal, can show its chronicle and query its lexostatuses.

A cell without `portaal` has no eHerkenning and no application routes. Its chronicle and lexostatuses are readable without logging in.

The boundary between cells is real. A cell only sees its own grams. Another cell's facts reach it through a `Transport`: internal when the source cell runs in the same runtime (the same call as the HTTP route, through the router, without a network), HTTP when the synthesis source has a `url`. This follows RFC-022 §4.3, where the runtime chooses the transport.

## Four layers

The design keeps four things apart. The regulation does not know that a chronicle exists, and the chronicle does not know which article reads it. The reduction is the only place where the two meet.

1. **Lexogram.** The regulations under `REGULATION_PATH`, unchanged and shared by all cells. An article declares the parameters it needs, such as `bevat_naam` or `aanvraagdatum`.
2. **Stream definition.** Which facts the cell records, by which actor, in which chronicle and on which legal ground (`grondslag`). Each field binds to `$intake.*` (who submitted it and through which channel), to `$external.*` (the content as submitted) or to a constant of the stream. A table field declares its columns: `{tabel: $external.<path>, kolommen: [...]}`. Each event says whether its grams belong to a case: `zaak: opent` (the cell gives a new `zaakkenmerk` when it records the gram), `zaak: volgt` (the gram carries the `zaakkenmerk` of an existing case, passed in with the submission) or `zaak: geen`, the default. This follows the sketch in RFC-022 §1.3.
3. **Reduction to lexostatus.** A `lexostatus_definitions` entry narrows the chronicle with `filter` and, when it needs one gram, picks it with `kies: laatste`. Each parameter gets a derivation (`afleiding`), of one of two kinds. On the chosen gram: `veld`, `gevuld`, `gelijk`, `tabel` with `elke_regel` or `een_regel` (optionally `alleen_waar`), and `moment`. Over the grams that pass the derivation's own `filter`: `bestaat: true`, `som: <field>`, and `kies: laatste` with `veld` or with `bevat: {veld, waarde}`. Field paths use dots, as in `inhoud.organen`.
4. **The gram.** What the cell writes, one JSON line per gram in `DATA_DIR/<cel>/<chronicle>.jsonl`. A gram is appended and never changed; a correction is a new gram. It carries `kind`, `type`, `soort`, `name`, `chronicle`, `recording_actor`, `grondslag`, `op_moment`, `stroom {id, sha256}` and `fields`, and `herkomst: startstand` when it was placed at start-up. A gram of an event with a case also carries `zaak` and `zaakkenmerk`; `gram.json` requires the `zaakkenmerk` then and forbids it otherwise.

A submitted application is recorded with `type: indiening` and `soort: aanvraag`, and its event opens a new case (`zaak: opent`). For that soort, `gram.json` fixes `fields.kern` to the elements Awb 4:2 paragraph 1 asks of every application: the applicant's name and address, the date, the decision requested and the signature (`ondertekend_via`). The content a specific regulation asks for goes under `fields.inhoud`, which each stream defines. A field left empty is recorded as `null`, so an incomplete application is recorded as well. Completeness is a judgment, and the gram holds none.

A decision of a register keeper (an entry, a removal, an established result) is recorded as `type: decretogram`. It belongs to no case: an election result or a register entry is not a case, and it has no `zaakkenmerk`. The grams are grouped by chronicle instead. A cell can keep several chronicles, one per stream, and each lexostatus reduces one of them.

## Start state

A cell may name a `startstand`: a JSONL file of grams to put into an empty chronicle. Each line gives `stroom`, `name`, `op_moment`, `herkomst: startstand` and `fields`. A line has a `zaakkenmerk` when its event has a case, and only then; the runtime does not make one up. Type, soort, legal ground, chronicle, actor and the hash of the stream come from the stream, so a start state does not go stale when the stream changes. The fields must be exactly those of the event. The runtime loads the start state only when every chronicle of the cell is empty.

## Synthesis

The check of a portal cell first reduces the draft to its own lexostatus. Then it asks each source in `synthese` for a lexostatus, with inputs taken from its own lexostatus:

```yaml
synthese:
  - cel: <source cell id>
    # no url: the source runs in this runtime, so the transport is internal
    lexostatus: <name at the source>
    invoer: {<source input>: {lexostatus: <own check lexostatus>, veld: <parameter or extra field>}}
    parameters: [<name>, ...]   # explicit, no wildcard
```

A value that is needed as input but is not a parameter of any article, such as the name the applicant registered under, is an `extra_velden` entry of the own lexostatus. It is marked apart and never goes to the engine.

Each source gets three seconds. The check takes only the listed parameters from the answer and passes the merged set to the engine. Its answer shows, per parameter, where the value came from: the own lexostatus, or cell and lexostatus with the transport. Per source it shows the status: `bevraagd`, `onbereikbaar`, `fout`, or `niet_bevraagd` when an input was missing from the draft. Nothing from the synthesis is recorded; it informs the check and is not a fact of the consuming cell. When a source is unreachable and the outcome therefore cannot be judged, the check says "niet te beoordelen: bron <id> onbereikbaar". It does not fill anything in.

## Start-up checks

Each cell is checked on its own. When one fails, the runtime does not start, and every message starts with `cel '<id>':`.

1. `cel.yaml`, the stream definitions and the lexostatus definitions validate against their schemas. The lexostatus file belongs to this cell, and every stream has the cell's `recording_actor`.
2. Every derivation points at a parameter of an article in the `grondslag` of an event its filter selects, or of an article in the lexostatus's `levert_aan`, and at field paths that exist in that event. A `tabel` derivation points at a table field and reads only columns the stream declares for it. A derivation on the chosen gram needs a lexostatus with `kies`.
3. No orphaned field: each field of each event is read by a derivation or a filter, or is listed under `niet_gereduceerd` with a reason.
4. No name collision: a parameter gets one derivation.
5. A filter or input on `zaakkenmerk` only selects events with a case (`zaak: opent` or `volgt`). A gram without a case has no `zaakkenmerk`, so such a filter would never match it.
6. The `portaal` block names an existing event, a lexostatus that picks a gram, and a regulation outcome whose article is in the `grondslag` of the event.
7. Synthesis needs a portal. Each input comes from a field of the check's lexostatus. Each parameter is a parameter of the check's article or of an article it calls, transitively through `source`. A parameter comes from exactly one place: the own reduction or one source.
8. The start state fits the streams of the cell.

Whether a source is reachable and offers the lexostatus with those parameters and inputs is checked after start-up, through `GET /api/cellen` at the source. A problem there is a warning, not a refusal: the source may come up later.

## Configuration

| Variable | Meaning |
|---|---|
| `CELLS_PATH` | Directory with one subdirectory per cell, each with a `cel.yaml`. |
| `REGULATION_PATH` | Directory with the regulations, shared by all cells. Every YAML file with `$id` and `articles` is loaded; other files are skipped. |
| `DATA_DIR` | Where the chronicles are written, in a subdirectory per cell. |
| `CEL_PORT` | Port of the runtime, default 7170. It binds to `0.0.0.0`. |

The `portaal` block in `cel.yaml` says which event a submission becomes, which lexostatus and outcome the check before submitting evaluates (`toets: {lexostatus, regeling, uitkomst}`), and optionally which form file supplies labels and order (`formulier: {pad, scherm}`). The form file never changes behavior: a field it does not know gets its field name as label, and a field the stream does not know is skipped.

## Routes

| Route | Purpose |
|---|---|
| `GET /api/cellen` | The cells in this runtime and what each offers |
| `GET /cellen/<id>/api/kroniek` | The grams, each with its YAML. With a portal: only those of the logged-in KvK number. |
| `GET /cellen/<id>/api/lexostatus/{naam}?<input>=...` | A reduction, with the inputs as query parameters. With a portal: only over your own grams. |
| `POST /cellen/<id>/api/eherkenning/login` | Portal only. `{kvk, persoon, machtiging}` to a session. A KvK number has eight digits and the only accepted mandate is `volledig`. There is no register. |
| `GET /cellen/<id>/api/stroom` | Portal only. The stream definition and the form fields |
| `POST /cellen/<id>/api/aanvraag/toets` | Portal only. Builds the gram in memory without recording it, reduces it, runs the synthesis and evaluates the configured outcome. `ontbreekt` lists the parameters of a presence derivation (`gevuld`, `tabel` with `elke_regel`) that came out false. |
| `POST /cellen/<id>/api/aanvraag` | Portal only. Records the gram and returns it |

## Deviations from RFC-022

These are deliberate. Each is a candidate for an amendment once the proof of concept has run on a real case.

1. **Type `indiening` with `soort: aanvraag`**, instead of an executogram. The position paper leaves the typology open, and RFC-022 §1 calls the set of types "not a closed set".
2. **`grondslag` is a list.** An application rests on several articles at once; RFC-022 §1.3 has a single value.
3. **The format of `lexostatus_definitions`.** RFC-022 §4.1 sketches it and calls it provisional. This runtime fills in `reduction` with `kroniek`, `filter`, `kies`, `afleidingen` and `extra_velden`, and adds derivations over a set of grams, each with its own filter.
4. **`kern` and `inhoud`, constant fields, and `$intake.*`** next to `$external.*`. Who submitted and through which channel belongs to the intake, not to the content.
5. **`niet_gereduceerd`** in the stream, for the orphaned-field check.
6. **A separate schema directory**, `schema/chronolex/`, instead of the `chronicle.json` that RFC-022 §1.3 places in `schema/v0.6.0/`.
7. **The cell definition, `cel.yaml`,** with the `portaal`, `synthese` and `startstand` blocks. RFC-022 leaves the full cell configuration to a future RFC. The configuration, not the code, decides which outcome the check evaluates and where its other facts come from.
8. **Decretograms from a start state, without an engine trace.** RFC-022 has a decretogram come out of the engine, with a trace. A start-state gram is placed, not computed, and says so with `herkomst: startstand`. Having the engine produce these decisions from the articles that ground them is a separate step.
9. **Absence in the own chronicle reads as "no".** The position paper does not say how a reduction reads absence. `bestaat` and `bevat` are false and `som` is zero when no gram passes the filter: a cell speaks only about its own register, closed within its own chronicle. A value that has no gram at all, such as a date, stays out.
10. **No security context between cells.** A lexostatus is queried over plain HTTP, without signing and without authorization (RFC-022 §2 and §4.3). There is no FSC and no Blauwe Knop in between.
11. **One cell per kind of register keeper.** In practice each body can have its own register keeper. The runtime allows any number of cells; a proof of concept can start with one.
12. **A lexostatus may speak the consumer's names (`levert_aan`).** A regulation that keeps a register can name a fact generically per article (`is_ingeschreven_in_register`), while the article that consumes it names the same fact per body (`is_ingeschreven_gemeenteraad`), or asks for a fact no article of the register regulation models at all. The source then cannot derive the consumer's parameter from the `grondslag` of its own event. `levert_aan` names the consuming articles, and the start-up check accepts their parameters as targets as well. The consumer checks the same names again against its own check article.
13. **An article number may contain a space** (`een_wet#A 1`), because the corpus numbers some articles that way.
14. **`zaakkenmerk` and `zaak: opent|volgt|geen`** extend RFC-022 §1.2, which uses a case identifier only to group the stage decretograms of one decision. Neither the law nor the position paper introduces them. The position paper groups facts with chronicles. Here a case identifier appears only on grams of an event that opens or follows a case, and the start-up check refuses a filter or input on `zaakkenmerk` that could reach an event without one.

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
11. **Engine and single outcomes.** The engine evaluates the whole article when one outcome is asked for. Every action runs and every `input` is resolved, including cross-law inputs the requested outcome never reads. A required parameter that only another output uses is therefore needed as well. The engine stops at the first missing value (`Variable not found`); a `required: false` parameter that is not passed becomes an unknown value and is reported as missing. For the check before submitting, this means an outcome can only be judged when every fact the article touches is present, including facts of the authority itself. The cell does not fill anything in. It reports "niet te beoordelen: mist <parameter>", one missing value per run. A test (`engine_eist_het_hele_artikel_bij_een_uitkomst` in `packages/cel/src/toets.rs`) pins this behavior on the fixtures. An engine that resolves only what the requested outcome depends on would let the check answer from the application alone.
12. **Schema version.** `schema/chronolex/v0.1.0/` stands apart from `schema/v0.6.0/`. Merge them, or keep them separate?
13. **Claim in the submission, fact in the decision.** A regulation can require the application to *state* a fact that belongs to another cell, such as the number of seats a list received, while the calculation in the decision uses the fact itself ("per seat that *was allocated*"). The submission gram records the claim. The decision should take the fact from the source cell through synthesis and compare the two. Where is that comparison recorded, and what follows from a mismatch: a request to supplement the application under the general administrative law's completion procedure, or a decision that simply follows the source?
