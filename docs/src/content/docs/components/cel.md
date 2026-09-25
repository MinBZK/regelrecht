---
title: "Cel"
description: "A proof-of-concept runtime for cells that record facts as chronolexograms and reduce them to lexostatuses, and for processes that combine those lexostatuses, run the engine and have a cell record the outcome."
---

The cell runtime is a small proof of concept for [RFC-022](/rfcs/rfc-022). It keeps two things apart that the position paper keeps apart as well. A **cell** records facts as chronolexograms in its own append-only chronicle, and a reduction turns that chronicle into a lexostatus. That is all a cell does. A **process** acts: it informs itself by combining lexostatuses from cells (synthesis), draws conclusions by running the engine, and asks a cell to record what it decided.

A process can have a portal: a fictitious organization logs in with a simulated eHerkenning, sees what the actor offers it, fills in an application form, checks it and submits it. The check asks the cell to reduce the draft on trial, then evaluates one outcome of a regulation (a `TOETS`) against that lexostatus, combined with what other cells know. A process can also have a case handler: a fictitious employee who logs in, sees the cases still waiting for a decision, opens one, has the engine compute a trial decision (`proefbesluit`) without recording anything, and then takes the decision. The cell records it as a stage decretogram.

Cells and processes are configuration, not code. The runtime loads every directory under `CELLS_PATH` that has a `cel.yaml` and every directory under `PROCESSES_PATH` that has a `proces.yaml`, so the same binary runs the generic test fixtures in this repository and the cells and processes of a private case corpus. Nothing in the code names a case.

## Overview

- **Language**: Rust (axum) for the runtime, Vue 3 with Vite and `@nldd/design-system` for the frontend
- **Location**: `packages/cel/` and `frontend-cel/`
- **Schemas**: `schema/chronolex/v0.1.0/` (`cel.json`, `proces.json`, `stream.json`, `lexostatus.json`, `gram.json`)
- **Run locally**: `just cel` starts the runtime on port 7170 with the fixture cells and processes, and the frontend on port 7171

## One runtime, cells and processes as configuration

```
cells/
  intake/
    cel.yaml            # id, recording_actor, stromen, lexostatussen
    lexostatussen.yaml
  register/
    cel.yaml
    lexostatussen.yaml
    startstand.jsonl    # optional: grams for an empty chronicle
processes/
  grant/
    proces.yaml         # id, actor, rollen?, portaal?, synthese?, behandeling?, voorbeelden?
```

Paths in `cel.yaml` and `proces.yaml` are relative to their directory. Each cell gets its own chronicle under `DATA_DIR/<id>/` and its own routes under `/cellen/<id>/api/`; each process gets its routes under `/processen/<id>/api/`. `GET /api/cellen` lists the cells with their chronicles and lexostatuses (inputs, parameters, extra fields). `GET /api/processen` lists the processes with their actor, their cell, their roles, whether they have a portal or case handling, and their synthesis sources. The frontend reads both lists. With one process that has a portal, it opens that process directly; every cell can show its chronicle and query its lexostatuses.

A cell has no login. Its routes are open to any consumer, the same way a register of another organization would be (deviation 10). `PROCESSES_PATH` is optional: without it, the runtime serves only cells.

The boundary between cell and process is real. A cell only sees its own grams. A process never reads a whole chronicle and never filters grams itself: filtering is reduction, and reduction happens in the cell. It sees the grams of one case only when it asks the cell for that case (`GET zaken/<zaakkenmerk>`), to show a case handler the file. It reaches every cell through a `Transport`, including the cell it records in, which is to the process a source like any other. The transport is internal when the cell runs in the same runtime (the same call as the HTTP route, through the router, without a network) and HTTP when a synthesis source has a `url`. This follows RFC-022 §4.3, where the runtime chooses the transport. In this step a process only records in a cell of its own runtime; reading may go either way.

## Four layers, and the process above them

The design keeps four things apart. The regulation does not know that a chronicle exists, and the chronicle does not know which article reads it. The reduction is the only place where the two meet. The first layer belongs to no one; a cell holds layers 2 to 4. The process stands above them, reading lexostatuses and asking the cell to record.

1. **Lexogram.** The regulations under `REGULATION_PATH`, unchanged and shared by all cells. An article declares the parameters it needs, such as `bevat_naam` or `aanvraagdatum`.
2. **Stream definition.** Which facts the cell records, by which actor, in which chronicle and on which legal ground (`grondslag`). Each field binds to `$intake.*` (who submitted it and through which channel), to `$external.*` (the content as submitted) or to a constant of the stream. A table field declares its columns: `{tabel: $external.<path>, kolommen: [...]}`. Each event says whether its grams belong to a case: `zaak: opent` (the cell gives a new `zaakkenmerk` when it records the gram), `zaak: volgt` (the gram carries the `zaakkenmerk` of an existing case, passed in with the submission) or `zaak: geen`, the default. This follows the sketch in RFC-022 §1.3.
3. **Reduction to lexostatus.** A `lexostatus_definitions` entry narrows the chronicle with `filter` and, when it needs one gram, picks it with `kies: laatste`. Each parameter gets a derivation (`afleiding`), of one of two kinds. On the chosen gram: `veld`, `gevuld`, `gelijk`, `tabel` with `elke_regel` or `een_regel` (optionally `alleen_waar`), and `moment`. On the chosen gram there is also `jaar_van`, the year of a date. Over the grams that pass the derivation's own `filter`: `bestaat: true`, `som: <field>`, and `kies: laatste` with `veld`, `jaar_van` or `bevat: {veld, waarde}`. `kies: laatste` with `veld` or `jaar_van` may add `geen_gram: <value>`, the value when no gram passes the filter, such as `null` for the date of something that did not happen. Field paths use dots, as in `inhoud.organen`.

   With `groepeer: zaakkenmerk` the lexostatus is a **list**: one row per case with at least one gram through `filter`, and, with `zonder: {filter}`, no gram through that filter. `kies` and the derivations then work per case, and the derivations are columns rather than parameters. A list is meant for the consumer, such as a case handler with a work queue, and never goes to the engine.
4. **The gram.** What the cell writes, one JSON line per gram in `DATA_DIR/<cel>/<chronicle>.jsonl`. A gram is appended and never changed; a correction is a new gram. It carries `kind`, `type`, `soort`, `stage` (for a stage decretogram), `name`, `chronicle`, `recording_actor`, `grondslag`, `op_moment`, `stroom {id, sha256}` and `fields`, and `herkomst: startstand` when it was placed at start-up. A gram of a decision the cell took itself also carries `legal_character`, `decision_type`, `regulation`, `regulation_valid_from`, `competent_authority` when the regulation names one, `inputs` and `receipt` (see "Taking the decision"). A gram of an event with a case also carries `zaak` and `zaakkenmerk`; `gram.json` requires the `zaakkenmerk` then and forbids it otherwise. An `op_moment` must be a moment with a time zone. The file is the source; the runtime also keeps the grams in memory with an index per case, so a reduction or a case query does not read the file again, and nobody but the runtime writes to it. A line counts only once it ends with a newline: an incomplete last line (the runtime stopped while writing) is cut off at start-up with a warning, and an unreadable line before it stops the runtime from starting.

The process holds everything about acting: who logs in, what the portal offers and checks, which sources it combines, and the decision. It never builds or reduces a gram itself. It sends the cell the event, the intake (who submitted, through which channel) and the external content, and the cell builds the gram from its stream, validates it, checks that the process's `actor` is the stream's `recording_actor`, checks that the gram fits its case, and records it or reduces it on trial. Whether a gram can be recorded is the cell's call, not the process's (the position paper: the cell determines which facts can be recorded).

A submitted application is recorded with `type: indiening` and `soort: aanvraag`, and its event opens a new case (`zaak: opent`). For that soort, `gram.json` fixes `fields.kern` to the elements Awb 4:2 paragraph 1 asks of every application: the applicant's name and address, the date, the decision requested and the signature (`ondertekend_via`). The content a specific regulation asks for goes under `fields.inhoud`, which each stream defines. A field left empty is recorded as `null`, so an incomplete application is recorded as well. Completeness is a judgment, and the gram holds none.

A decision of a register keeper (an entry, a removal, an established result) is recorded as `type: decretogram`. It belongs to no case: an election result or a register entry is not a case, and it has no `zaakkenmerk`. The grams are grouped by chronicle instead. A cell can keep several chronicles, one per stream, and each lexostatus reduces one of them.

## Roles and the trial decision

`rollen` in `proces.yaml` says who logs in: `aanvrager: eherkenning` for the portal and `behandelaar: medewerker` for case handling. The employee login is simulated like the eHerkenning one; it asks for a name and checks nothing else. A process has one session cookie, so logging in as the other role replaces the session. The portal routes answer 403 to a case handler, and the case routes answer 403 to an applicant. An applicant who follows up on a case must know it, which means a gram of their KvK number with that case identifier; the process checks that before it asks the cell. A process without roles has no login at all.

`behandeling` names the work queue, a list lexostatus of the cell, and the decision:

```yaml
synthese:
  - {cel: <cell>, lexostatus: <name>, zaak: true}   # a lexostatus of the case
  - ...                                               # other sources, see "Synthesis"
behandeling:
  werkvoorraad: {cel: <cell>, lexostatus: <list lexostatus>}
  besluit:
    regeling: <$id>                      # optional, see below
    uitkomsten: [<output>, ...]          # outputs of one article
    stand_bij_besluit:                   # facts that arise after the decision
      <parameter>: null
    rijen:                               # synthesis per row
      - parameter: <array parameter>
        tabel: {lexostatus: <of the case>, veld: <table field>}
        kolommen: {<column of the table>: <column of the parameter>}
        bronnen:
          - cel: <id>
            lexostatus: <name at the source>
            invoer: {<input>: {kolom: <column of the row>}}
            kolommen: {<name at the source>: <column of the parameter>}
    vastleggen:                          # where the decision is recorded
      cel: <cell>
      stroom: <$id>
      event: <event with zaak: volgt and a stage>
```

The portal, the work queue, the decision and the sources with `zaak: true` name one cell. A process in this step acts on the cases of one cell, and that cell runs in the same runtime. A source with `zaak: true` is a lexostatus of the case itself, with `zaakkenmerk` as its only input: the decision asks the cell for it with the case identifier and takes all its parameters and extra fields, which is what the cell's own reduction used to supply. Such a source names no inputs or parameters.

The process finds its decision in the law. Without `regeling`, the runtime looks at load time for the article that produces a `BESCHIKKING` and whose competent authority (of the article, else of the regulation) equals the process's `actor`, compared after normalizing case and punctuation. Exactly one such article is the decision, and every listed outcome must come from it; none or several is a start-up error that names the candidates, and then `regeling` settles it. Which outcomes make up the decision stays configuration.

Every parameter of the decision comes from exactly one source. The lexostatuses of the case reduce the case in the cell: the application, and the course of the case, such as a request to supplement or a suspension of the decision period. Their provenance is `eigen`: the process's own cell. The other synthesis sources supply facts of other cells, with their input taken from one of those lexostatuses. The decision form holds the judgments of the case handler, which only exist at the moment of deciding; they go in with provenance `behandelaar`. The form is not configured: it is every parameter of the decision with origin `OORDEEL` (see "Who supplies a parameter"), in the order the article declares them, labelled with the parameter's description (or the part after "Naam:" when there is one, else its name) and grouped by the article of its `grondslag`. The state at decision (`stand_bij_besluit`) covers facts that can only arise later, such as the notification of the decision: at the moment of deciding it has not happened, so the value is `null` or `false`, with provenance `stand_bij_besluit`.

The trial decision runs the article on today's date. When every configured outcome has a value, the answer has the outcomes. Otherwise it says "niet te nemen: mist <parameter>". The answer names the provenance of each parameter, and under `niet_geleverd` every parameter a caller of the article has to supply that no source supplied, with the description the regulation gives it. A call within the same regulation without `parameters` shares the caller's parameters; a call to another regulation receives only what `parameters` passes. Nothing is recorded, and nothing is filled in.

## Who supplies a parameter

A parameter can say who supplies it, per the law, with `origin` and always a `grondslag` (RFC-043): `BELANGHEBBENDE` (what the applicant supplies or chooses), `DOSSIER` (a fact from the course of the case), `OORDEEL` (a judgment given when deciding), `REGISTER` with `register: <regulation>` (a fact from a register kept under that regulation) and `KANAAL` (what the login says). An article of an implementing policy (`UITVOERINGSBELEID`) can override the origin of a parameter of another regulation with `origins` in `machine_readable`. For a process, the override counts when the competent authority of that policy is the process's actor; otherwise the law's origin applies. Two overrides that disagree are a start-up error.

At start-up the process checks every parameter a caller must supply for the check, the offer and the decision. Its applicable origin needs a supplier of the fitting kind:

| Origin | Supplier |
|---|---|
| `BELANGHEBBENDE` | a derivation of the check lexostatus or a lexostatus of the case, or synthesis per row from a table of the case; for the offer also the period the portal offers |
| `DOSSIER` | a derivation of such a lexostatus, or the state at decision |
| `OORDEEL` | the decision form, for the decision only |
| `REGISTER` | a synthesis source that lists the parameter, whose lexostatus names an article asking for it in `levert_aan`, and whose chronicle has a stream with a `grondslag` in the regulation of `register`; or synthesis per row from a table such a source passes on. A source with a url cannot be inspected and counts. |
| `KANAAL` | a derivation of the check lexostatus that reads only `$intake` fields |

A required parameter without such a supplier stops the runtime, and the message names the parameter, the origin and the `grondslag`, and where the value comes from now if something else supplies it. A parameter declared `required: false` without one is a warning: the engine runs without it, as it does today. A parameter without `origin` is a warning too, and so is a `BELANGHEBBENDE` parameter that is not `required: false` (RFC-036 lets an application-form field be declared that way; the runtime does not derive `required`). The runtime logs the warnings after start-up. The engine itself does not read `origin`.

## Synthesis per row

An article can ask for a table as a parameter of which the applicant fills in only part; the authority establishes the rest itself, row by row, from registers other cells keep. The process does not assemble that table in code. `rijen` says which table field (of a lexostatus of the case, or passed on by a source) supplies the rows, which column travels under which name, and which source is queried per row with which input.

Per row the process walks the sources in order, so a source can use a column an earlier one supplied. An input comes from the row (`kolom`), from a lexostatus of the case (`lexostatus` and `veld`) or from the merged parameters (`parameter`), converted where needed with `als: eerste_dag_van_het_jaar`, the counterpart of the `jaar_van` derivation, for an article that asks for a figure on the first of January of a year the cell knows as a number. A source supplies a column from its `parameters` or from its `extra_velden`, so a lexostatus that only supplies columns of someone else's table parameter supplies no article parameter at all.

When an input is missing, a source is unreachable, or it does not supply the value, that column stays out of the row and `mist` names it. Nothing is filled in, and the engine then reports the parameter it could not compute.

The check before submission takes the same block under `portaal.toets.rijen`. There the table comes from the trial reduction of the draft (the check lexostatus) or from a source that passes it on, and the check builds the rows before the engine runs, as the decision does. A `rijen` block supplies only the evaluation it belongs to: in the provenance check, the rows of the check count for the check and those of the decision for the decision.

## Taking the decision

`POST /processen/<id>/api/zaken/<zaakkenmerk>/besluit` computes the same thing and has the cell record the outcome, when `behandeling.besluit.vastleggen` says where. The gram is a stage decretogram of that event: `zaak: volgt` with the case identifier the application opened, and the outcomes as its fields. On top of that comes what makes the decision a decision: `legal_character` and `decision_type` from the article's `produces`, `regulation` and `regulation_valid_from`, `inputs` with every parameter's value and provenance ([RFC-013](/rfcs/rfc-013) `accepted_values`), and a `receipt` holding the loaded regulations and the cell's streams with a SHA-256 over both. The process assembles these, because the process runs the engine; the hashes of the streams are the ones the cell keeps. It sends them with the event to `POST /cellen/<cell>/api/grammen`, and the cell builds the gram from its stream and records it.

The competent authority comes from the regulation (the article, otherwise the regulation itself) and is tested against the process's `actor`. Equal means record; a different authority means refuse; when the regulation names none, the cell records with a warning and without `competent_authority`. Nothing makes one up.

Three things lead to a refusal with 409 and no gram: the trial decision is incomplete, the cell refuses because a gram with stage `BESLUIT` is already in the case (changing a decision is a later stage and out of scope), or the law names a different authority. The cell checks the stage under the same lock as the write, so two simultaneous decisions cannot both be recorded. The gram is validated against `gram.json` before it is recorded.

## Start state

A cell may name a `startstand`: a JSONL file of grams to put into an empty chronicle. Each line gives `stroom`, `name`, `op_moment`, `herkomst: startstand` and `fields`. A line has a `zaakkenmerk` when its event has a case, and only then; the runtime does not make one up. Type, soort, legal ground, chronicle, actor and the hash of the stream come from the stream, so a start state does not go stale when the stream changes. The fields must be exactly those of the event. The runtime loads the start state only when every chronicle of the cell is empty, and writes each file in one go (a temporary file, then renamed), so an interrupted start leaves no half start state.

## Examples

For a trial setup a process may name standard data per action, in a `voorbeelden` block: `inloggen` (a list of JSON files, each `{kvk, persoon}`; the label is the file name without extension), `aanvraag` (a JSON file with `{external: {...}}`, as the portal receives it) and `besluit` (a JSON file with `{formulier: {...}}`, as the decision receives it). Paths are relative to the process's directory. `GET /processen/<id>/api/voorbeelden` returns `{inloggen: [{label, kvk, persoon}], aanvraag, besluit}`, with `null` for an action without an example, and needs no login, since the login examples are there to log in with. The frontend offers each example to fill in the form first or to perform the action with it directly. An example is ordinary input: it is checked like any other and nothing about it is recorded.

## Synthesis

The check of a portal first asks its cell to reduce the draft on trial, to the lexostatus `portaal.toets.lexostatus` names (`POST /cellen/<cell>/api/lexostatus/<name>/proef`). The cell builds the gram in memory and reduces its chronicle with that gram added, so the answer is what the lexostatus would be if the draft were recorded; nothing is recorded. Then the process asks each other source in `synthese` for a lexostatus, with inputs taken from that one:

```yaml
synthese:
  - cel: <source cell id>
    # no url: the source runs in this runtime, so the transport is internal
    lexostatus: <name at the source>
    invoer: {<source input>: {lexostatus: <check lexostatus>, veld: <parameter or extra field>}}
    parameters: [<name>, ...]   # explicit, no wildcard
```

A value that is needed as input but is not a parameter of any article, such as the name the applicant registered under, is an `extra_velden` entry of the check lexostatus. It is marked apart and never goes to the engine.

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

The synthesis then runs in two rounds: first the sources that need only the check lexostatus (or, for a decision, a lexostatus of the case), then the ones that wait on an earlier source. When the earlier source passes nothing, the later one is not asked (`niet_bevraagd`), and nothing is filled in. Two rounds is the limit: a source cannot wait on a source that itself waits.

Each source gets three seconds. The check takes only the listed parameters from the answer and passes the merged set to the engine. Its answer shows, per parameter, where the value came from: the lexostatus of the draft or the case (`eigen`), or cell and lexostatus with the transport. Per source it shows the status: `bevraagd`, `onbereikbaar`, `fout`, or `niet_bevraagd` when an input was missing from the draft. Nothing from the synthesis is recorded; it informs the check and is not a fact of anyone. When a source is unreachable and the outcome therefore cannot be judged, the check says "niet te beoordelen: bron <id> onbereikbaar". It does not fill anything in.

## What an organisation can apply for

Nobody lists which applications an organisation can make, not in the law and not in the configuration. The actor decides what each portal offers, and on what grounds, in a policy regulation of its own. `portaal.aanbod` names one outcome of that regulation, and optionally a second outcome of the same article that gives the deadline. When the article asks for a period, the parameter with origin `BELANGHEBBENDE` and `grondslag` Awb 4:2 paragraph 1 (the decision requested), `aanbod.keuzes` says which values the portal offers: `jaren_vanaf_nu: [0, 1]` is this year and the next. `GET /api/mogelijkheden` builds a draft with only that period, adds what the login supplies, runs the synthesis, and evaluates the regulation in one run per period. When the check lexostatus derives the period from a field of the draft, the runtime fills in that field, so the provenance is the same as for a real application; otherwise the chosen value goes in with provenance `keuze`.

The offer tests only conditions that are settled beforehand: who logs in, what the registers know, and the period asked for. Whether all the facts an application needs will be there cannot be known before the form is filled in; that is the check after filling it in (completeness) and then the decision. So the verdict is simple. True (or a positive value) means an application is possible (`mogelijk`). A definite zero or false means the policy offers nothing (`uitgesloten`). Everything else is `niet_te_bepalen`: an empty outcome, a missing fact, an engine error. The verdict looks only at the offer outcome itself; a fact the deadline needs does not count.

The runtime refuses to start when the offer could lean on something that is not settled beforehand. Every parameter the article of the offer outcome asks of its caller must have origin `KANAAL` or `REGISTER`, or `BELANGHEBBENDE` with `grondslag` Awb 4:2 paragraph 1. Otherwise, or without an origin, the start-up check says "aanbod: voorwaarde leunt op '<parameter>' (<origin>), dat vooraf niet bekend is". The check works per article rather than per outcome, because the engine evaluates the whole article for any outcome of it. A period without `keuzes`, `keuzes` without a period, and more than one period are start-up errors too.

The regulation decides who may act. The login states only a KvK number and a person's name; whether that person may act for the organisation comes from a register cell through synthesis, and the policy regulation reads it like any other fact. The deadline is shown next to the verdict and never changes it.

No offer is not a refusal. An application sent another way (Awb 4:1) is still recorded and judged. Nothing of this is recorded.

The frontend shows one button per period to start an application, and fills in the field of the period when the application form opens. It is disabled unless the verdict is possible. A question mark next to it opens the verdict, the reason, the deadline and the trace of the run.

Every outcome the portal shows that comes from an engine run carries the trace of that run: the check before submitting, the trial decision, and the offer. The answer carries it as text (`trace_text`, the same box-drawing rendering the editor shows), and the frontend opens it from a small RegelRecht icon.

## Start-up checks

Each cell and each process is checked on its own. When one fails, the runtime does not start, and every message starts with `cel '<id>':` or `proces '<id>':`.

For a cell:

1. `cel.yaml`, the stream definitions and the lexostatus definitions validate against their schemas. The lexostatus file belongs to this cell, and every stream has the cell's `recording_actor`. Every legal ground names a loaded article, and a paragraph it names exists in the article text.
2. Every derivation points at a parameter of an article in the `grondslag` of an event its filter selects, or of an article in the lexostatus's `levert_aan`, and at field paths that exist in that event. A `tabel` derivation points at a table field and reads only columns the stream declares for it. A derivation on the chosen gram needs a lexostatus with `kies`.
3. No orphaned field: each field of each event is read by a derivation or a filter, or is listed under `niet_gereduceerd` with a reason.
4. No name collision: a parameter gets one derivation.
5. A filter or input on `zaakkenmerk` only selects events with a case (`zaak: opent` or `volgt`). A gram without a case has no `zaakkenmerk`, so such a filter would never match it.
6. The start state fits the streams of the cell.
7. A list lexostatus (`groepeer`) only selects events with a case, in `filter` and in `zonder`, and its `zonder` selects at least one event, since otherwise it would never leave a case out. Its columns need not be parameters and do not collide with other lexostatuses.
8. A lexostatus supplies something: at least one derivation or one extra field.

For a process:

1. `proces.yaml` validates against `proces.json`. The portal, the work queue, the decision and the sources of the case name one cell, and that cell runs in this runtime.
2. The `actor` is the `recording_actor` of every stream the process records in: the stream of the portal and that of the decision.
3. The `portaal` block names an existing event of that cell that binds only `$intake` paths the portal supplies, a lexostatus that reads that event and picks a gram (not a list, with `zaakkenmerk` as its only input), and a regulation outcome whose article is in the `grondslag` of the event. An offer names an outcome that exists, and a deadline outcome from the same article, because the two are evaluated in one run. Every parameter the article of the offer asks for has an origin that is settled beforehand, and a period has `keuzes` (see "What an organisation can apply for").
4. Synthesis needs a portal or a decision. Each input comes from a field of the check's lexostatus or of a lexostatus of the case, or from an extra field an earlier source passes on; that earlier source takes its own inputs from one of those. A source supplies a parameter or passes an extra field. Each parameter is a parameter of the article of the check, the decision or the offer, or of an article one of them calls, transitively through `source`. A parameter comes from exactly one place. An ordinary source is a cell other than the process's own; the own cell is read through sources with `zaak: true`.
5. A portal needs the applicant role and the other way around; case handling needs the case handler role, and a source of the case needs case handling. The work queue is a list. The outcomes of the decision come from one article. The lexostatuses of the case have `zaakkenmerk` as their only input and are not lists. Each parameter in the form, in the state at decision or in a `rijen` block is one a caller of the article has to supply, and every parameter of the decision comes from one source only.
6. Synthesis per row: the table comes from a lexostatus of the case, or from a source that passes it on; each column name comes from one place only (the table or one source); a `kolom` input names a column something before it fills; a source is another cell. Where the decision is recorded: an existing event with `zaak: volgt` and a stage, whose `$external` keys are exactly the outcomes of the decision.
7. Once synthesis and decision pass: every parameter of the check, the offer and the decision has a supplier that fits its applicable origin, and the overrides in implementing policy do not conflict (see "Who supplies a parameter").
8. Each example file exists and is JSON of the right shape: a login has a valid KvK number and a name, an application has an `external` object, a decision has a `formulier` object. An example for an action the process does not have (a login or application without a portal, a decision without case handling) is an error.

Whether a source is reachable and offers the lexostatus with those parameters and inputs is checked after start-up, through `GET /api/cellen` at the source. A problem there is a warning, not a refusal: the source may come up later.

## Configuration

| Variable | Meaning |
|---|---|
| `CELLS_PATH` | Directory with one subdirectory per cell, each with a `cel.yaml`. |
| `PROCESSES_PATH` | Directory with one subdirectory per process, each with a `proces.yaml`. Optional; without it the runtime serves only cells. |
| `REGULATION_PATH` | Directory with the regulations, shared by the whole runtime. Every YAML file with `$id` and `articles` is loaded; other files are skipped. |
| `DATA_DIR` | Where the chronicles are written, in a subdirectory per cell. |
| `CEL_PORT` | Port of the runtime, default 7170. It binds to `0.0.0.0`. |

`cel.yaml` holds `id`, `recording_actor`, `stromen`, `lexostatussen` and optionally `startstand`, and nothing else. The `portaal` block in `proces.yaml` says in which cell and which event a submission becomes (`cel`, `stroom`, `event`), which lexostatus and outcome the check before submitting evaluates (`toets: {lexostatus, regeling, uitkomst}`), optionally what the portal offers (`aanbod: {regeling, uitkomst, termijn, keuzes}`, with `termijn` and `keuzes` optional), and optionally which form file supplies labels and order (`formulier: {pad, scherm}`). The form file never changes behavior: a field it does not know gets its field name as label, and a field the stream does not know is skipped.

## Routes

| Route | Purpose |
|---|---|
| `GET /api/cellen` | The cells in this runtime, with their chronicles and lexostatuses |
| `GET /api/processen` | The processes in this runtime, with their actor, cell, roles and synthesis sources |
| `GET /cellen/<id>/api/kroniek` | The grams, each with its YAML. No login. |
| `GET /cellen/<id>/api/zaken/<zaakkenmerk>` | The grams of one case, each with its YAML; the cell filters. 404 when the cell does not know the case. |
| `GET /cellen/<id>/api/lexostatus/{naam}?<input>=...` | A reduction, with the inputs as query parameters. No login. |
| `POST /cellen/<id>/api/lexostatus/{naam}/proef` | `{concept, inputs}` to `{gram, lexostatus}`. Builds the gram of the draft in memory and reduces the chronicle with that gram added. Without an input `zaakkenmerk`, the draft's case identifier counts. Records nothing. |
| `POST /cellen/<id>/api/grammen` | `{actor, stroom, event, intake, external, zaakkenmerk?, besluit?}` to `{gram, yaml}` (201). Builds the gram from the stream, validates it, refuses with 403 when `actor` is not the stream's `recording_actor`, refuses with 400 when a following gram names an unknown case and with 409 when its stage is already in the case, and records it. `besluit` carries the fields of a decision (see "Taking the decision"). |
| `GET /cellen/<id>/api/stroom` | The stream definitions of the cell, each with its hash |
| `GET /processen/<id>/api/voorbeelden` | The examples per action from `voorbeelden` in `proces.yaml`, without login. Empty without that block. |
| `POST /processen/<id>/api/eherkenning/login` | Portal only. `{kvk, persoon}` to a session. A KvK number has eight digits and the name is not empty. The login checks nothing against a register: whether the person may act for the organisation is for the policy regulation to decide, through synthesis with a register cell. |
| `GET /processen/<id>/api/formulier` | Portal only. The stream definition and the form fields, from the event in the cell's stream and the process's form file |
| `POST /processen/<id>/api/aanvraag/toets` | Portal only. Has the cell reduce the draft on trial, runs the synthesis and evaluates the configured outcome. `ontbreekt` lists the parameters of a presence derivation (`gevuld`, `tabel` with `elke_regel`) that came out false. |
| `POST /processen/<id>/api/aanvraag` | Portal only. Has the cell record the gram and returns it |
| `GET /processen/<id>/api/mogelijkheden` | Portal only. What `portaal.aanbod` offers the logged-in person, per period from `aanbod.keuzes`, with the deadline and the trace of the run. Each possibility names its period as `tijdvak: {parameter, veld, waarde}`. Records nothing. |
| `POST /processen/<id>/api/medewerker/login` | Case handler role only. `{naam}` to a session; `GET .../sessie` and `POST .../logout` as for eHerkenning. |
| `GET /processen/<id>/api/werkvoorraad` | Case handler only. The work queue, a list lexostatus of the cell. |
| `GET /processen/<id>/api/zaken/<zaakkenmerk>` | Case handler only. The grams of the case, the decision form (the `OORDEEL` parameters of the decision), and a trial decision without judgments. |
| `POST /processen/<id>/api/zaken/<zaakkenmerk>/proefbesluit` | Case handler only. `{formulier}` to a trial decision. Records nothing. |
| `POST /processen/<id>/api/zaken/<zaakkenmerk>/besluit` | Case handler only. `{formulier}` to a recorded decision (201), or a refusal (409). |

## Deviations from RFC-022

These are deliberate. Each is a candidate for an amendment once the proof of concept has run on a real case.

The split between cell and process follows the position paper, which gives a cell three functions (recording, one or more chronicles, and reduction, which "always takes place in the cell where the chronolexograms concerned were recorded, at the request of a business process") and gives informing, concluding and having a cell record to the process ("synthesis happens at the consumer, not at the source"). RFC-022 agrees where it speaks ("the cell never does cross-cell work; it only reduces its own chronicles", "the cell is not the engine"), but it leaves the configuration of a cell to a later RFC and says nothing about how a process is configured. Deviations 7, 32 and 33 fill that gap.

1. **Type `indiening` with `soort: aanvraag`**, instead of an executogram. The position paper leaves the typology open, and RFC-022 §1 calls the set of types "not a closed set".
2. **`grondslag` is a list, and may name a paragraph.** An application rests on several articles at once; RFC-022 §1.3 has a single value. An article can hold more than one application or act, so a legal ground may name the paragraph: `<regulation>#<article> lid <n>`, such as `een_wet#102 lid 1`. Article numbers can contain a space (`kieswet#G 1`, deviation 13), so the paragraph is read after the last ` lid ` and only when a paragraph number (digits, optionally one letter) follows. A paragraph has no parameters of its own: the checks on derivations and on the portal's outcome stay per article. The start-up check only tests that the paragraph exists, as a line of the article text that starts with `<n>.` or `<n> `.
3. **The format of `lexostatus_definitions`.** RFC-022 §4.1 sketches it and calls it provisional. This runtime fills in `reduction` with `kroniek`, `filter`, `kies`, `afleidingen` and `extra_velden`, adds derivations over a set of grams, each with its own filter, and adds `jaar_van` for an article that asks for a year where the chronicle holds a date.
4. **`kern` and `inhoud`, constant fields, and `$intake.*`** next to `$external.*`. Who submitted and through which channel belongs to the intake, not to the content.
5. **`niet_gereduceerd`** in the stream, for the orphaned-field check.
6. **A separate schema directory**, `schema/chronolex/`, instead of the `chronicle.json` that RFC-022 §1.3 places in `schema/v0.6.0/`.
7. **Two kinds of configuration: `cel.yaml` and `proces.yaml`.** RFC-022 leaves the full cell configuration to a future RFC and has no process configuration at all. Here `cel.yaml` holds only what a cell does (its actor, streams, lexostatuses and start state), and `proces.yaml` holds who acts and how: roles, portal, synthesis, decision and examples, with an `actor` that must be the `recording_actor` of every stream the process records in. The own cell is a source of the process like any other, marked `zaak: true` where it supplies the lexostatuses of a case. The configuration, not the code, decides which outcome the check evaluates and where its other facts come from. `proces.json` is added to `schema/chronolex/v0.1.0/` and `cel.json` shrinks, without a new schema version, because the proof of concept has not been released.
8. **Decretograms from a start state, without an engine trace.** RFC-022 has a decretogram come out of the engine, with a trace. A start-state gram is placed, not computed, and says so with `herkomst: startstand`. Having the engine produce these decisions from the articles that ground them is a separate step.
9. **Absence in the own chronicle reads as "no".** The position paper does not say how a reduction reads absence. `bestaat` and `bevat` are false and `som` is zero when no gram passes the filter: a cell speaks only about its own register, closed within its own chronicle. A value that has no gram at all, such as a date, stays out.
10. **No security context between cells.** A lexostatus is queried over plain HTTP, without signing and without authorization (RFC-022 §2 and §4.3). There is no FSC and no Blauwe Knop in between.
11. **One cell per kind of register keeper.** In practice each body can have its own register keeper. The runtime allows any number of cells; a proof of concept can start with one.
12. **A lexostatus may speak the consumer's names (`levert_aan`).** A regulation that keeps a register can name a fact generically per article (`is_ingeschreven_in_register`), while the article that consumes it names the same fact per body (`is_ingeschreven_gemeenteraad`), or asks for a fact no article of the register regulation models at all. The source then cannot derive the consumer's parameter from the `grondslag` of its own event. `levert_aan` names the consuming articles, and the start-up check accepts their parameters as targets as well. The consumer checks the same names again against its own check article.
13. **An article number may contain a space** (`een_wet#A 1`), because the corpus numbers some articles that way.
14. **`zaakkenmerk` and `zaak: opent|volgt|geen`** extend RFC-022 §1.2, which uses a case identifier only to group the stage decretograms of one decision. Neither the law nor the position paper introduces them. The position paper groups facts with chronicles. Here a case identifier appears only on grams of an event that opens or follows a case, and the start-up check refuses a filter or input on `zaakkenmerk` that could reach an event without one.

15. **A lexostatus may be a list.** RFC-022 §4.1 has a lexostatus answer with parameters. `groepeer` and `zonder` let a lexostatus return one row per case instead. RFC-022 §4.1 sketches `group_by zaakkenmerk` in a comment; the list shape and the rule that a list never reaches the engine are filled in here.
16. **Roles in a process, with simulated logins.** RFC-022 leaves authorization to the security context (§2). Here `rollen` in `proces.yaml` decides who may act, with a simulated eHerkenning for the applicant and a simulated employee login for the case handler, and no security context.
17. **Judgments in the decretogram, the course of the case as grams.** The judgments of the case handler (careful preparation, proportionality, reasoning, the date of the decision) exist only at the moment of deciding and will be accepted input of the decretogram (RFC-013 `accepted_values`). Facts from the course of the case, such as a request to supplement, are grams of their own that follow the case, and the decision reads them through a reduction. When there is no such gram, the reduction reads it as "did not happen" (`geen_gram`, deviation 9).
18. **The state at decision.** An article can ask for facts about the notification of the decision, which only arise after it. The trial decision passes them as `null` or `false`, with provenance `stand_bij_besluit`, and does not make up a date.
19. **`stage` on a gram.** A stage decretogram carries its RFC-008 stage (such as `BESLUIT`), taken from the stream. So does a submission that opens a case: the application gram carries `stage: AANVRAAG`, because it is the recorded fact behind that stage (RFC-008: "Belanghebbende dient aanvraag in (Awb 4:1)", which requires `aanvraag_datum`; the reduction takes that date from `op_moment`). The schemas allow a stage only on a decretogram or a submission, and only on an event with a case. RFC-022 §1.2 names the stages but gives the chronicle stream no field for them. A filter on `stage: BESLUIT`, such as the work queue's `zonder`, does not match the application.
20. **Types of the grams in the course of a case.** An invitation to supplement or a suspension of the decision period is recorded as an executogram, and a supplement from the applicant as an `indiening` of soort `aanvulling`. The position paper does not classify procedural acts; this is a choice for the proof of concept.
21. **Synthesis per row** (`rijen`). RFC-022 §4.1 has synthesis take a lexostatus as one answer. An article that asks for a table the applicant only partly fills needs the source queried once per row, with values from that row as input. The row shape, the column mapping and the order of the sources are configuration here; nothing of it is in the RFC.
22. **A lexostatus that supplies only columns.** `afleidingen` may be empty when `extra_velden` supplies something. Such a lexostatus answers with no parameter of any article: its values are columns of a table parameter the consumer assembles. RFC-022 §4.1 has a lexostatus answer with the parameters of an article.
23. **A conversion on the way to a source** (`als: eerste_dag_van_het_jaar`), the counterpart of the `jaar_van` derivation. The alternative was to have the source cell hold the consuming article's rule about which reference date applies, which would put one authority's law in another authority's register.
24. **What a recorded decision carries.** The gram of a decision the cell took itself adds `legal_character`, `decision_type`, `regulation`, `regulation_valid_from`, `competent_authority`, `inputs` and `receipt` to the chronicle-stream shape of RFC-022 §1.3, which has none of them. `inputs` is the RFC-013 `accepted_values` idea applied to every parameter rather than to cross-organisational ones only, and the `receipt` is a short form of the RFC-013 Execution Receipt: the loaded regulations and the cell's streams with one hash over both, which is the generalisation RFC-022 §1.3 announces as an amendment to RFC-013.
25. **The competent authority is tested before recording, and finds the decision.** RFC-002 and RFC-007 model who is competent; nothing says a process has to compare that with its actor before it has a decision recorded. Here equal records, different refuses, and absent records with a warning. The same comparison also picks the decision when `proces.yaml` names no regulation: the actor is the one the law makes competent, so the law says which decision it takes.
26. **One gram per stage per case, enforced by the cell.** A second gram with the same stage (such as `BESLUIT`) in the same case is refused with 409, under the same lock as the write. This follows RFC-022 §1.2 (each stage of one besluit is its own elementary stage decretogram in a chronicle sharing a `zaakkenmerk`); it is not configuration. Changing a decision is a later stage of RFC-008 and out of scope for this step.
27. **Deriving what can be applied for.** The position paper and RFC-022 say nothing about which acts an actor may perform, or about applications at all. The process does not list them either: a policy regulation of the actor states what each portal offers and on what grounds, and the portal runs that regulation on an empty draft. This is informing by the actor, in the paper's sense, and records nothing. The offer tests only conditions that are settled beforehand, and an unknown outcome is no offer. An earlier version read "unknown because the application still has to supply facts" as "possible"; it ran the decision article on an empty draft, which kept the verdict at "possible" for an organisation that would almost certainly not qualify (a merger route open to any registered association). Running a law for a draft that does not exist yet is not tenable, so the offer now leans on the login and the registers alone, and the start-up check enforces it.
28. **The portal refuses nothing and offers what the policy allows.** The position paper is silent on refusing a recording. The general administrative law is not: an application is a request for a decision (Awb 1:3 paragraph 3), not treating it is a decision after receipt (Awb 4:5), and refusing an electronic message beforehand is limited to two grounds (Awb 2:15). So the portal refuses nothing it receives. What it offers follows the policy regulation of the actor, which is a choice about service and not a decision on the application.
29. **Passing values between sources.** RFC-022 §4.1 has each source answer from inputs the consumer already holds. Here a source can pass an extra field to a later source, so a register keyed on an organisation number can supply the name under which another register keeps the results.
30. **Traces in the answers.** The check, the trial decision and the possibilities return the engine trace of their run. RFC-013 puts a trace in the Execution Receipt; here it is part of the answer and is not recorded.
31. **A table from a register.** The derivation `verzamel` turns the grams that pass a filter into a list with one row per gram, and a source can pass that list on as an extra field. Synthesis per row then takes its rows from the source rather than from the application, so a decision can build a table from what a register decided (the rows of an election result) instead of from what the applicant stated. The position paper has reductions produce a lexostatus; a list as an extra field is a shape it does not describe.
32. **A trial reduction as a route of the cell.** The check before submitting needs the lexostatus of a draft that is not a fact. The position paper has reduction happen only in the cell, so the process does not reduce: it asks the cell (`POST .../lexostatus/<name>/proef`), and the cell builds the gram in memory, reduces its chronicle with that gram added, and records nothing. Neither the paper nor RFC-022 has a reduction over something that was not recorded.
33. **The process passes the intake without a security context.** The process tells the cell who submitted and through which channel (`intake`), and which actor asks to record. The cell refuses an actor that is not the stream's `recording_actor`, and otherwise trusts what the process says. Between process and cell there is no signing and no authorization, as between cells (deviation 10). A cell's chronicle and lexostatuses are therefore readable without logging in; that an applicant only follows up on cases of their own KvK number is checked by the process, not the cell.
34. **Who supplies a parameter (`origin`, `origins`).** The position paper speaks of synthesis at the consumer and does not say how a process knows which source supplies which fact. `origin` on a parameter and `origins` in implementing policy are a RegelRecht addition (RFC-043, draft), not part of the paper. The process configuration still names the parameters each synthesis source supplies; finding sources through their declaration alone is a later step.
35. **A missing supplier is a warning when the parameter is `required: false`.** RFC-043 makes a missing supplier a start-up error. A corpus that already runs with facts nobody supplies, because they are declared `required: false`, would then not start at all. Here the error holds for required parameters, and a parameter the engine can do without gives a warning.
36. **The period is still configuration.** Which values the portal offers for the period (`aanbod.keuzes`) is not in the law yet. It could follow from the policy that says when the counter opens.

## Open questions

1. **Typology.** Is `indiening` (application, notification, report, objection) a fourth class next to lexogram, decretogram and executogram? The position paper leaves this for further research, and so does RFC-022 §1. A candidate for an amendment to RFC-022.
2. **RFC-008.** The submission gram is the recorded fact behind the `AANVRAAG` stage, and carries it (deviation 19). Open: do the acknowledgement of receipt (Awb 4:3a) and a supplement (Awb 4:5) become grams of their own, stages of their own, or both?
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
16. **The receipt and the source cells.** The receipt covers what the process loaded (its regulations) and the streams of the cell that records the decision. What a source cell answered is in `inputs` with its provenance, but not with that cell's own receipt. RFC-013 §4 sketches the chain; a cell that signs its lexostatus answers would close it.
17. **`origins` needs a schema version.** `origin` on a parameter validates against the current law schemas, because a parameter field is open. `machine_readable` is closed, so an override in implementing policy fails schema validation until a new schema version declares `origins` (RFC-013: `$schema` URLs are tag-based). The runtime already reads it.
18. **The date of the decision.** Which origin does the date on which the body decides have? It is given when deciding, like a judgment, but it is a fact rather than an assessment, and a rule that classifies by the article a parameter is passed to can make it `DOSSIER`. The decision form needs it either way.
