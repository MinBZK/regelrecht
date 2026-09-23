---
title: "Cel"
description: "A proof-of-concept runtime for cells that record facts as chronolexograms, reduce them to lexostatuses, and combine lexostatuses from other cells."
---

The cell runtime is a small proof of concept for [RFC-022](/rfcs/rfc-022). A cell records facts as chronolexograms in its own append-only chronicle, and a reduction turns that chronicle into a lexostatus. A cell can have a portal: a fictitious organization logs in with a simulated eHerkenning, fills in an application form, checks it and submits it. The check evaluates one outcome of a regulation (a `TOETS`) against the lexostatus. When the regulation needs facts that another cell keeps, the check asks that cell for its lexostatus and combines the two (synthesis). A cell can also have a case handler: a fictitious employee who logs in, sees the cases still waiting for a decision, opens one, has the engine compute a trial decision (`proefbesluit`) without recording anything, and then takes the decision, which the cell records as a stage decretogram.

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
3. **Reduction to lexostatus.** A `lexostatus_definitions` entry narrows the chronicle with `filter` and, when it needs one gram, picks it with `kies: laatste`. Each parameter gets a derivation (`afleiding`), of one of two kinds. On the chosen gram: `veld`, `gevuld`, `gelijk`, `tabel` with `elke_regel` or `een_regel` (optionally `alleen_waar`), and `moment`. On the chosen gram there is also `jaar_van`, the year of a date. Over the grams that pass the derivation's own `filter`: `bestaat: true`, `som: <field>`, and `kies: laatste` with `veld`, `jaar_van` or `bevat: {veld, waarde}`. `kies: laatste` with `veld` or `jaar_van` may add `geen_gram: <value>`, the value when no gram passes the filter, such as `null` for the date of something that did not happen. Field paths use dots, as in `inhoud.organen`.

   With `groepeer: zaakkenmerk` the lexostatus is a **list**: one row per case with at least one gram through `filter`, and, with `zonder: {filter}`, no gram through that filter. `kies` and the derivations then work per case, and the derivations are columns rather than parameters. A list is meant for the consumer, such as a case handler with a work queue, and never goes to the engine.
4. **The gram.** What the cell writes, one JSON line per gram in `DATA_DIR/<cel>/<chronicle>.jsonl`. A gram is appended and never changed; a correction is a new gram. It carries `kind`, `type`, `soort`, `stage` (for a stage decretogram), `name`, `chronicle`, `recording_actor`, `grondslag`, `op_moment`, `stroom {id, sha256}` and `fields`, and `herkomst: startstand` when it was placed at start-up. A gram of a decision the cell took itself also carries `legal_character`, `decision_type`, `regulation`, `regulation_valid_from`, `competent_authority` when the regulation names one, `inputs` and `receipt` (see "Taking the decision"). A gram of an event with a case also carries `zaak` and `zaakkenmerk`; `gram.json` requires the `zaakkenmerk` then and forbids it otherwise.

A submitted application is recorded with `type: indiening` and `soort: aanvraag`, and its event opens a new case (`zaak: opent`). For that soort, `gram.json` fixes `fields.kern` to the elements Awb 4:2 paragraph 1 asks of every application: the applicant's name and address, the date, the decision requested and the signature (`ondertekend_via`). The content a specific regulation asks for goes under `fields.inhoud`, which each stream defines. A field left empty is recorded as `null`, so an incomplete application is recorded as well. Completeness is a judgment, and the gram holds none.

A decision of a register keeper (an entry, a removal, an established result) is recorded as `type: decretogram`. It belongs to no case: an election result or a register entry is not a case, and it has no `zaakkenmerk`. The grams are grouped by chronicle instead. A cell can keep several chronicles, one per stream, and each lexostatus reduces one of them.

## Roles and the trial decision

`rollen` in `cel.yaml` says who logs in: `aanvrager: eherkenning` for the portal and `behandelaar: medewerker` for case handling. The employee login is simulated like the eHerkenning one; it asks for a name and checks nothing else. A cell has one session cookie, so logging in as the other role replaces the session. With roles, the chronicle and the lexostatuses are only for someone logged in. The applicant sees the grams of their own KvK number, the case handler sees all of them. The portal routes answer 403 to a case handler, and the case routes answer 403 to an applicant. A cell without roles has no login at all.

`behandeling` names the work queue, a list lexostatus of the cell, and the decision:

```yaml
behandeling:
  werkvoorraad: <list lexostatus>
  besluit:
    regeling: <$id>                      # optional, see below
    uitkomsten: [<output>, ...]          # outputs of one article
    lexostatussen: [<name>, ...]         # own, with zaakkenmerk as the only input
    formulier:                           # judgments of the case handler
      - {parameter: <name>, label: <text>, groep: <text>}
    stand_bij_besluit:                   # facts that arise after the decision
      <parameter>: null
    rijen:                               # synthesis per row
      - parameter: <array parameter>
        tabel: {lexostatus: <own>, veld: <table field>}
        kolommen: {<column of the table>: <column of the parameter>}
        bronnen:
          - cel: <id>
            lexostatus: <name at the source>
            invoer: {<input>: {kolom: <column of the row>}}
            kolommen: {<name at the source>: <column of the parameter>}
    vastleggen:                          # where the decision is recorded
      stroom: <$id>
      event: <event with zaak: volgt and a stage>
```

The cell finds its decision in the law. Without `regeling`, the runtime looks at load time for the article that produces a `BESCHIKKING` and whose competent authority (of the article, else of the regulation) equals the cell's `recording_actor`, compared after normalizing case and punctuation. Exactly one such article is the decision, and every listed outcome must come from it; none or several is a start-up error that names the candidates, and then `regeling` settles it. Which outcomes make up the decision stays configuration, and the first one is what the portal reads as "can receive something" (open question 17).

Every parameter of the decision comes from exactly one source. The own lexostatuses reduce the case: the application, and the course of the case, such as a request to supplement or a suspension of the decision period. The synthesis sources of the cell supply facts of other cells, with their input taken from one of those lexostatuses. The decision form holds the judgments of the case handler, which only exist at the moment of deciding; they go in with provenance `behandelaar`. The state at decision (`stand_bij_besluit`) covers facts that can only arise later, such as the notification of the decision: at the moment of deciding it has not happened, so the value is `null` or `false`, with provenance `stand_bij_besluit`.

The trial decision runs the article on today's date. When every configured outcome has a value, the answer has the outcomes. Otherwise it says "niet te nemen: mist <parameter>". The answer names the provenance of each parameter, and under `niet_geleverd` every parameter a caller of the article has to supply that no source supplied, with the description the regulation gives it. A call within the same regulation without `parameters` shares the caller's parameters; a call to another regulation receives only what `parameters` passes. Nothing is recorded, and nothing is filled in.

## Synthesis per row

An article can ask for a table as a parameter of which the applicant fills in only part; the authority establishes the rest itself, row by row, from registers other cells keep. The cell does not assemble that table in code. `rijen` says which table field of one of its own lexostatuses supplies the rows, which column travels under which name, and which source is queried per row with which input.

Per row the cell walks the sources in order, so a source can use a column an earlier one supplied. An input comes from the row (`kolom`), from one of the cell's own lexostatuses (`lexostatus` and `veld`) or from the merged parameters (`parameter`), converted where needed with `als: eerste_dag_van_het_jaar`, the counterpart of the `jaar_van` derivation, for an article that asks for a figure on the first of January of a year the cell knows as a number. A source supplies a column from its `parameters` or from its `extra_velden`, so a lexostatus that only supplies columns of someone else's table parameter supplies no article parameter at all.

When an input is missing, a source is unreachable, or it does not supply the value, that column stays out of the row and `mist` names it. Nothing is filled in, and the engine then reports the parameter it could not compute.

## Taking the decision

`POST /cellen/<id>/api/zaken/<zaakkenmerk>/besluit` computes the same thing and records the outcome, when `behandeling.besluit.vastleggen` says where. The gram is a stage decretogram of that event: `zaak: volgt` with the case identifier the application opened, and the outcomes as its fields. On top of that comes what makes the decision a decision: `legal_character` and `decision_type` from the article's `produces`, `regulation` and `regulation_valid_from`, `inputs` with every parameter's value and provenance ([RFC-013](/rfcs/rfc-013) `accepted_values`), and a `receipt` holding the loaded regulations and the cell's streams with a SHA-256 over both.

The competent authority comes from the regulation (the article, otherwise the regulation itself) and is tested against the cell's `recording_actor`. Equal means record; a different authority means refuse; when the regulation names none, the cell records with a warning and without `competent_authority`. It does not make one up.

Three things lead to a refusal with 409 and no gram: the trial decision is incomplete, there is already a gram with stage `BESLUIT` in the case (changing a decision is a later stage and out of scope), or the law names a different authority. The gram is validated against `gram.json` before it is recorded.

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

A source can also pass a value on to a later source. It lists the field under `extra_velden` (and may then have no parameters at all), and the later source names that source's lexostatus as the origin of its input:

```yaml
synthese:
  - cel: <register>
    lexostatus: <by organisation number>
    invoer: {nummer: {lexostatus: <own>, veld: nummer}}
    parameters: []
    extra_velden: [aanduiding]         # not a parameter; input for the next source
  - cel: <register>
    lexostatus: <by name>
    invoer: {aanduiding: {lexostatus: <by organisation number>, veld: aanduiding}}
    parameters: [<name>, ...]
```

The synthesis then runs in two rounds: first the sources that need only the own lexostatus, then the ones that wait on an earlier source. When the earlier source passes nothing, the later one is not asked (`niet_bevraagd`), and nothing is filled in. Two rounds is the limit: a source cannot wait on a source that itself waits.

Each source gets three seconds. The check takes only the listed parameters from the answer and passes the merged set to the engine. Its answer shows, per parameter, where the value came from: the own lexostatus, or cell and lexostatus with the transport. Per source it shows the status: `bevraagd`, `onbereikbaar`, `fout`, or `niet_bevraagd` when an input was missing from the draft. Nothing from the synthesis is recorded; it informs the check and is not a fact of the consuming cell. When a source is unreachable and the outcome therefore cannot be judged, the check says "niet te beoordelen: bron <id> onbereikbaar". It does not fill anything in.

## What an organisation can apply for

Nobody lists which applications an organisation can make, not in the law and not in the cell. A portal cell derives it by running the law for whoever logs in. `GET /api/mogelijkheden` builds an empty draft (only the subsidy year, for this year and the next), reduces it, runs the synthesis, and evaluates up to three outcomes:

| Question | Outcome | Counts |
|---|---|---|
| May this person act for the organisation? | `portaal.mandaat` | yes |
| Can the organisation receive anything? | the first outcome of the decision | yes |
| By when? | `portaal.termijn` | shown only |

Each outcome gets a verdict. A definite zero or false means the regulation rules it out (`uitgesloten`). An unknown outcome, with the facts it lacks, means an application is possible: those facts are what the application has to supply (Awb 4:2 paragraph 2). A definite positive value is possible as well. An engine error that names no missing fact is `niet_te_bepalen`. One check that rules out decides for all of them.

This needs the regulation to let facts be unknown. RFC-036 says that application-form fields a caller never passes are `required: false`; a law that marks them `required: true` cannot be evaluated before the form exists ("a law cannot be evaluated for nobody"). The engine then returns an unknown that names the missing facts, and a definite `false` in an `AND` decides whatever the unknown turns out to be.

No possibility is not a refusal. The portal offers what the law allows; an application sent another way (Awb 4:1) is still recorded and judged. Nothing of this is recorded.

Every outcome the portal shows that comes from an engine run carries the trace of that run: the check before submitting, the trial decision, and each question above. The answer carries it as text (`trace_text`, the same box-drawing rendering the editor shows), and the frontend opens it from a small RegelRecht icon.

## Start-up checks

Each cell is checked on its own. When one fails, the runtime does not start, and every message starts with `cel '<id>':`.

1. `cel.yaml`, the stream definitions and the lexostatus definitions validate against their schemas. The lexostatus file belongs to this cell, and every stream has the cell's `recording_actor`.
2. Every derivation points at a parameter of an article in the `grondslag` of an event its filter selects, or of an article in the lexostatus's `levert_aan`, and at field paths that exist in that event. A `tabel` derivation points at a table field and reads only columns the stream declares for it. A derivation on the chosen gram needs a lexostatus with `kies`.
3. No orphaned field: each field of each event is read by a derivation or a filter, or is listed under `niet_gereduceerd` with a reason.
4. No name collision: a parameter gets one derivation.
5. A filter or input on `zaakkenmerk` only selects events with a case (`zaak: opent` or `volgt`). A gram without a case has no `zaakkenmerk`, so such a filter would never match it.
6. The `portaal` block names an existing event, a lexostatus that picks a gram, and a regulation outcome whose article is in the `grondslag` of the event.
7. Synthesis needs a portal or a decision. Each input comes from a field of the check's lexostatus, or from an extra field an earlier source passes on; that earlier source takes its own inputs from the check's lexostatus. A source supplies a parameter or passes an extra field. Each parameter is a parameter of the check's article or of an article it calls, transitively through `source`. A parameter comes from exactly one place: the own reduction or one source.
8. The start state fits the streams of the cell.
9. A list lexostatus (`groepeer`) only selects events with a case, in `filter` and in `zonder`, and its `zonder` selects at least one event, since otherwise it would never leave a case out. Its columns need not be parameters and do not collide with other lexostatuses. The portal check and the decision never use a list.
10. A portal needs the applicant role and the other way around; case handling needs the case handler role. The work queue is a list. The outcomes of the decision come from one article. The lexostatuses of the decision have `zaakkenmerk` as their only input. Each parameter in the form, in the state at decision or in a `rijen` block is one a caller of the article has to supply, and every parameter of the decision comes from one source only.
11. Synthesis per row: the table comes from a lexostatus of the decision that supplies it; each column name comes from one place only (the table or one source); a `kolom` input names a column something before it fills; a source is another cell. Where the decision is recorded: an existing event with `zaak: volgt` and a stage, whose `$external` keys are exactly the outcomes of the decision.
12. A lexostatus supplies something: at least one derivation or one extra field.

Whether a source is reachable and offers the lexostatus with those parameters and inputs is checked after start-up, through `GET /api/cellen` at the source. A problem there is a warning, not a refusal: the source may come up later.

## Configuration

| Variable | Meaning |
|---|---|
| `CELLS_PATH` | Directory with one subdirectory per cell, each with a `cel.yaml`. |
| `REGULATION_PATH` | Directory with the regulations, shared by all cells. Every YAML file with `$id` and `articles` is loaded; other files are skipped. |
| `DATA_DIR` | Where the chronicles are written, in a subdirectory per cell. |
| `CEL_PORT` | Port of the runtime, default 7170. It binds to `0.0.0.0`. |

The `portaal` block in `cel.yaml` says which event a submission becomes, which lexostatus and outcome the check before submitting evaluates (`toets: {lexostatus, regeling, uitkomst}`), optionally which outcomes answer "may this person act for the organisation" and "by when" (`mandaat` and `termijn`, each `{regeling, uitkomst}`), and optionally which form file supplies labels and order (`formulier: {pad, scherm}`). The form file never changes behavior: a field it does not know gets its field name as label, and a field the stream does not know is skipped.

## Routes

| Route | Purpose |
|---|---|
| `GET /api/cellen` | The cells in this runtime and what each offers |
| `GET /cellen/<id>/api/kroniek` | The grams, each with its YAML. With a portal: only those of the logged-in KvK number. |
| `GET /cellen/<id>/api/lexostatus/{naam}?<input>=...` | A reduction, with the inputs as query parameters. With a portal: only over your own grams. |
| `POST /cellen/<id>/api/eherkenning/login` | Portal only. `{kvk, persoon, machtiging}` to a session. A KvK number has eight digits and the mandate is `volledig` or `geen`. There is no register. What a missing mandate means is up to the regulation, not the login. |
| `GET /cellen/<id>/api/stroom` | Portal only. The stream definition and the form fields |
| `POST /cellen/<id>/api/aanvraag/toets` | Portal only. Builds the gram in memory without recording it, reduces it, runs the synthesis and evaluates the configured outcome. `ontbreekt` lists the parameters of a presence derivation (`gevuld`, `tabel` with `elke_regel`) that came out false. |
| `POST /cellen/<id>/api/aanvraag` | Portal only. Records the gram and returns it |
| `GET /cellen/<id>/api/mogelijkheden` | Portal only. What the law lets the logged-in organisation apply for, per subsidy year, with the trace of each check. Records nothing. |
| `POST /cellen/<id>/api/medewerker/login` | Case handler role only. `{naam}` to a session; `GET .../sessie` and `POST .../logout` as for eHerkenning. |
| `GET /cellen/<id>/api/werkvoorraad` | Case handler only. The work queue, a list lexostatus. |
| `GET /cellen/<id>/api/zaken/<zaakkenmerk>` | Case handler only. The grams of the case, the decision form, and a trial decision without judgments. |
| `POST /cellen/<id>/api/zaken/<zaakkenmerk>/proefbesluit` | Case handler only. `{formulier}` to a trial decision. Records nothing. |
| `POST /cellen/<id>/api/zaken/<zaakkenmerk>/besluit` | Case handler only. `{formulier}` to a recorded decision (201), or a refusal (409). |

## Deviations from RFC-022

These are deliberate. Each is a candidate for an amendment once the proof of concept has run on a real case.

1. **Type `indiening` with `soort: aanvraag`**, instead of an executogram. The position paper leaves the typology open, and RFC-022 §1 calls the set of types "not a closed set".
2. **`grondslag` is a list.** An application rests on several articles at once; RFC-022 §1.3 has a single value.
3. **The format of `lexostatus_definitions`.** RFC-022 §4.1 sketches it and calls it provisional. This runtime fills in `reduction` with `kroniek`, `filter`, `kies`, `afleidingen` and `extra_velden`, adds derivations over a set of grams, each with its own filter, and adds `jaar_van` for an article that asks for a year where the chronicle holds a date.
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

15. **A lexostatus may be a list.** RFC-022 §4.1 has a lexostatus answer with parameters. `groepeer` and `zonder` let a lexostatus return one row per case instead. RFC-022 §4.1 sketches `group_by zaakkenmerk` in a comment; the list shape and the rule that a list never reaches the engine are filled in here.
16. **Roles in a cell, with simulated logins.** RFC-022 leaves authorization to the security context (§2). Here `rollen` in `cel.yaml` decides who sees what, with a simulated eHerkenning for the applicant and a simulated employee login for the case handler, and no security context.
17. **Judgments in the decretogram, the course of the case as grams.** The judgments of the case handler (careful preparation, proportionality, reasoning, the date of the decision) exist only at the moment of deciding and will be accepted input of the decretogram (RFC-013 `accepted_values`). Facts from the course of the case, such as a request to supplement, are grams of their own that follow the case, and the decision reads them through a reduction. When there is no such gram, the reduction reads it as "did not happen" (`geen_gram`, deviation 9).
18. **The state at decision.** An article can ask for facts about the notification of the decision, which only arise after it. The trial decision passes them as `null` or `false`, with provenance `stand_bij_besluit`, and does not make up a date.
19. **`stage` on a gram.** A stage decretogram carries its RFC-008 stage (such as `BESLUIT`), taken from the stream. RFC-022 §1.2 names the stages but gives the chronicle stream no field for them.
20. **Types of the grams in the course of a case.** An invitation to supplement or a suspension of the decision period is recorded as an executogram, and a supplement from the applicant as an `indiening` of soort `aanvulling`. The position paper does not classify procedural acts; this is a choice for the proof of concept.
21. **Synthesis per row** (`rijen`). RFC-022 §4.1 has synthesis take a lexostatus as one answer. An article that asks for a table the applicant only partly fills needs the source queried once per row, with values from that row as input. The row shape, the column mapping and the order of the sources are configuration here; nothing of it is in the RFC.
22. **A lexostatus that supplies only columns.** `afleidingen` may be empty when `extra_velden` supplies something. Such a lexostatus answers with no parameter of any article: its values are columns of a table parameter the consumer assembles. RFC-022 §4.1 has a lexostatus answer with the parameters of an article.
23. **A conversion on the way to a source** (`als: eerste_dag_van_het_jaar`), the counterpart of the `jaar_van` derivation. The alternative was to have the source cell hold the consuming article's rule about which reference date applies, which would put one authority's law in another authority's register.
24. **What a recorded decision carries.** The gram of a decision the cell took itself adds `legal_character`, `decision_type`, `regulation`, `regulation_valid_from`, `competent_authority`, `inputs` and `receipt` to the chronicle-stream shape of RFC-022 §1.3, which has none of them. `inputs` is the RFC-013 `accepted_values` idea applied to every parameter rather than to cross-organisational ones only, and the `receipt` is a short form of the RFC-013 Execution Receipt: the loaded regulations and the cell's streams with one hash over both, which is the generalisation RFC-022 §1.3 announces as an amendment to RFC-013.
25. **The competent authority is tested before recording, and finds the decision.** RFC-002 and RFC-007 model who is competent; nothing says a cell has to compare that with itself before it writes. Here equal records, different refuses, and absent records with a warning. The same comparison also picks the decision when `cel.yaml` names no regulation: the cell is the actor the law makes competent, so the law says which decision it takes.
26. **One decision per case.** A second gram with stage `BESLUIT` in the same case is refused. Changing a decision is a later stage of RFC-008 and out of scope for this step.
27. **Deriving what can be applied for.** The position paper and RFC-022 say nothing about which acts an actor may perform, or about applications at all. The cell does not list them either: the portal runs the decision's regulation on an empty draft and reads the verdict from the outcome. This is informing by the actor, in the paper's sense, and records nothing.
28. **The portal refuses nothing and offers only what the law allows.** The position paper is silent on refusing a recording. The general administrative law is not: an application is a request for a decision (Awb 1:3 paragraph 3), not treating it is a decision after receipt (Awb 4:5), and refusing an electronic message beforehand is limited to two grounds (Awb 2:15). So the portal refuses nothing it receives. It leaves out an application the law rules out, which is a choice about what to offer.
29. **Passing values between sources.** RFC-022 §4.1 has each source answer from inputs the consumer already holds. Here a source can pass an extra field to a later source, so a register keyed on an organisation number can supply the name under which another register keeps the results.
30. **Traces in the answers.** The check, the trial decision and the possibilities return the engine trace of their run. RFC-013 puts a trace in the Execution Receipt; here it is part of the answer and is not recorded.
31. **A table from a register.** The derivation `verzamel` turns the grams that pass a filter into a list with one row per gram, and a source can pass that list on as an extra field. Synthesis per row then takes its rows from the source rather than from the application, so a decision can build a table from what a register decided (the rows of an election result) instead of from what the applicant stated. The position paper has reductions produce a lexostatus; a list as an extra field is a shape it does not describe.


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
14. **Proposed decisions.** Drafts before the decision (a *voorgenomen besluit*) are out of scope. The position paper asks whether they are decisions already.
15. **A column with no source.** A table parameter can have a column no cell can answer for, because no register holds the fact under that shape. The row then simply lacks the column, which the engine reports as a missing value only when it reads it. Should the configuration be able to name such a column and say why it stays empty, the way `niet_gereduceerd` does for an unread field?
16. **The receipt and the source cells.** The receipt covers what this cell loaded: its regulations and its own streams. What a source cell answered is in `inputs` with its provenance, but not with that cell's own receipt. RFC-013 §4 sketches the chain; a cell that signs its lexostatus answers would close it.
17. **Which outcome says "can receive anything".** The portal takes the first outcome of the decision. A regulation does not mark which outcome is the grant; for a decision with several amounts or a refusal outcome, that choice is configuration and could be wrong.
18. **Unknown that stays unknown.** A route in the law that depends only on facts the applicant supplies keeps the verdict at "possible" for an organisation that will almost certainly not qualify, such as a merger route open to any registered association. The law gives that answer; whether the portal should say more than "possible" is open.
