---
title: "Demo"
description: "The RegelRecht demo: a presentation, the laws and their graph, scenarios, a population simulation, a citizen portal and a case system, with the engine running as WASM in the browser."
---

The demo shows in one workspace what RegelRecht does: the machine-readable law, the dependencies between laws, the scenarios that test a law, and the execution for one person on a portal and in a case system. It succeeds the separate `poc-machine-law` repository.

## Overview

- **Language**: Vue 3 / Vite, `@nldd/design-system`
- **Location**: `frontend-demo/`
- **Corpus**: `corpus/demo/`
- **Production URL**: `demo.regelrecht.rijks.app` (ZAD component `demo`; see [Deployment](/operations/deployment))
- **Interface languages**: Dutch (the source), English under `/en/` and Frisian under `/fy/`. The Frisian translation has not yet been reviewed by a Frisian speaker. The law texts themselves stay Dutch in every language, because a translation of the text in force has no legal standing.

## What it does

The workspace opens on a landing page (the house icon) with a short explanation, a button that starts the presentation, and a QR code that points at the demo itself, so someone in the audience can follow along on a phone. After it come seven tabs, in the order of a presentation. The labels below are the English ones, with the Dutch in parentheses.

1. **Presentation** (*Presentatie*): the slide deck in Rijkshuisstijl blue. The intro is shown full screen; after that the deck sits on the left as a rail, and each slide opens the tab it is about, switches persona and points at what the presenter means. The slides are content (`demo-config.yaml`), not code. Each persona has its own deck: a slide with `decks` belongs to that persona's story, the other slides are shared, and the deck is picked from the chosen persona when the presentation starts. Merijn's deck ends with Claudia; Claudia's deck stays with her, from the precario by-law to the terrace permit the municipality grants. A slide that names no law (`/regelwerken`) opens the chosen persona's `default_law`. Esc closes the deck and leaves the demo where it is; Shift+P opens it from anywhere.
2. **Ruleworks** (*Regelwerken*): the rulework of a law as a collapsible YAML tree, prepared per story (`expanded_paths` in `demo-config.yaml`: a path opens itself and everything above it, the rest stays closed; `folded_paths` names nodes on such a path that start closed for the presenter to open, with what lies below them already prepared). Every `source.regulation` is a link that opens the referenced law; a back button retraces the references followed. The list of all laws, grouped per organization, is a side panel that is closed by default.
3. **Graph** (*Graaf*): per law its sources, its inputs from other laws and its outputs, with lines to the law that supplies them, the persona's values on them, and one color per organization. The profile picks the laws of its story (`graph_laws`), and the graph shows those laws plus everything directly attached to them. A profile with `graph_focus` puts that law in focus and zooms in on it when the persona is chosen, the way Ruleworks and Scenarios open the profile's `default_law` and `default_feature`. The layout of the whole corpus is fixed, so "All" (*Alles*) adds laws without moving anything. Selecting a law colors the lines it reads from green and the lines along which other laws read it red.
4. **Scenarios** (*Scenario's*): the Gherkin scenarios per law. In Dutch the steps are rendered as Dutch sentences; English and Frisian show the canonical English steps, which is what the `.feature` files contain. "Run" (*Uitvoeren*) executes a scenario in the browser against the engine and opens the full execution trace, in which each law has its own color and a bar steps from one law to the next.
5. **Simulation** (*Simulatie*): a generated population of citizens or businesses (size, age and income distribution, business type, seed) is run through every regulation of that portal. The result shows who qualifies and for how much, broken down by age, income, partner, business type or size, and for citizens the disposable income per month: income minus taxes, plus allowances and benefits. Which outputs count, and whether per month or per year, is set in `simulation.disposable_income`. A law that another law reads as a decided case (the precario tax reads the granted terrace permit) is applied for first by the businesses that apply, and what it grants becomes a case before the rest runs. Constants from the laws (thresholds, percentages) can be changed per run, and so can those of the regulations the laws rely on, such as the standard premium (*standaardpremie*) that a ministerial regulation fills in for the zorgtoeslag. A run is named after what it changed and a variant gets its own color; runs sit side by side for comparison, which opens with the disposable income per run, and export as CSV or JSON.

   ![The Simulation tab for a population of 50 citizens: the settings on the left, and on the right the population summary and the average and median disposable income per month, broken down per regulation.](../../../assets/simulatie-screenshot.png)

6. **My government** (*Mijn overheid*; the label comes from `portal_tab_label` in `demo-config.yaml`, and on behalf of a business it is the business's name. A persona with `start_namens` opens on behalf of that business, as Claudia does for her café; with the authorizations feature on, *Mezelf* (myself) shows her own portal as a citizen): the portal of the active persona. Each regulation is computed live. Under "Data used" (*Gebruikte gegevens*) the portal shows where each piece of data comes from, and each one can be corrected. A regulation that ends in a *beschikking* (an individual administrative decision) is applied for from the portal, in a panel that follows the flow of the POC: the law computes with what the government already knows and asks, one question at a time, only for what no register holds (the rent, the location of a café terrace). It recomputes after each answer, lets the applicant check the outcome and the data used, and shows the status after submission. An application under a law listed in `review_laws` (Rotterdam's terrasvergunning) always goes to a caseworker, whatever the law concludes. Once the decision has been announced, the panel shows the date until which an objection (*bezwaar*) is possible. That date comes from the Awb itself (articles 6:7 and 6:8), not from the screen. The portal never jumps to the case system, which belongs to the other side of the counter.

   ![The My government tab for the persona Merijn: one card per regulation (kindgebonden budget, zorgtoeslag, bijstand, huurtoeslag, inkomstenbelasting) with the computed outcome, the number of data items used, and buttons to apply, see the calculation or read the law text.](../../../assets/portaal-screenshot.png)

7. **Case system** (*Zaaksysteem*): the case handler's side, per executing organization. It has a board with cases to assess, cases to announce and cases that have been announced, the regulations the organization executes, the engine's recalculation next to the result applied for, citizen corrections awaiting review, granting or refusing, announcing, and objection. Announcing is a separate action, because the Awb treats the decision (article 1:3) and its announcement (article 3:41) as two moments, and only the second starts the objection period. By default the demo announces a decision as soon as it is taken ("Announce decisions straight away" in the menu); switching that off brings the announcement back as a step of its own.

The toolbar switches the profile (Merijn, a citizen; Claudia, a business owner). With the authorizations feature on, "Acting for" offers the people or businesses the active profile may act for, when the law gives more than one option. The menu then holds four groups:

- **Features**: switches for features that are off or on per profile in `demo-config.yaml` (authorizations, reporting a change, harmonization, approving corrections immediately), plus "Review every application by hand" and "Announce decisions straight away" (on by default). A switched flag overrides the profile until "Back to the profile" resets it.
- **Language**: Dutch, English or Frisian.
- **Appearance**: the color scheme.
- **Demo**: full screen, and resetting the demo.

## How it works

The demo corpus in `corpus/demo/` holds the laws migrated from the POC (`regulation/nl/`, schema v0.5.8 and v0.5.9, with `source: {}` for external data), their scenarios (`**/scenarios/*.feature`, in the canonical grammar), `bindings.yaml` (which register table and column feeds which `source: {}` input), `profiles.yaml` (the fictitious personas), `demo-config.yaml` and `services.yaml`. `tools/` holds the one-off migration and conversion scripts.

Everything the demo computes happens in the browser; the one exception is the optional "why" explanation below. The engine runs as WebAssembly in the browser, the same engine as in the editor. At build time the demo corpus is copied to `public/data`. Persona data (`profiles.yaml`) is materialized per law into records through `bindings.yaml` and registered with the engine as a law-scoped data source; approved citizen corrections are layered on top as a second source with a higher priority. Applications, corrections and settings live in `localStorage`, so a page refresh during a presentation loses nothing.

An application for a *beschikking* passes through the phases the Awb gives it (RFC-007, RFC-008). In each phase the engine fires the hooks that belong to it and reports which piece of data it still lacks; the demo supplies it at the moment it exists, and stores the state with the case. That is how the objection period arrives as a date from the law, and how a special law that departs from article 6:7 is taken into account without extra code.

### The application as a fact in a chronicle

For zorgtoeslag the application and the decision on it are also recorded as facts in a chronicle (chronolex, [RFC-022](/rfcs/rfc-022)). The cell of the Belastingdienst/Toeslagen (`corpus/demo/cells/toeslagen`) runs in the browser next to the engine: the demo loads one WASM module, `regelrecht-cel` built with its `wasm` feature, which exports both the engine (`WasmEngine`) and the cell (`WasmCell`).

What the application contains is not configured but follows from executing the law. Awir article 15 establishes the application; because Zorgtoeslagwet article 2 takes a *beschikking* on it, Awb 4:2 (the core of every application) and 4:13 (the receipt is the moment that counts) hook onto it, and Awir 16 asks for the income the applicant expects. The portal shows these fields under "What the law asks", each with the article that asks it, and on submission the cell records the application.

Zorgtoeslagwet article 2 follows the procedure of the Awir (`tegemoetkoming`), with two decisions: the voorschot (stage VOORSCHOT) and the toekenning (stage TOEKENNING). At the voorschot, Awir 16 replaces the toetsingsinkomen with the estimate and gives the voorschotbedrag. When the case is decided, the case system records the decision of the stage the case is at: the cell reads what that stage asks back from its chronicle, executes that one stage, and records the outputs of article 2 and of the hooks at that stage (Awb 3:46 and 6:7 among them). The case system shows the grams under "Chronicle" on the case.

The cell also records each voorschot instalment, as an executogram. Awir 22 says per month whether an instalment falls due, and Toeslagen's (fictional) policy says how much it is; the cell executes that policy for a month and records a gram only when the law says one arises. At the toekenning the cell reads what was paid up to that moment from its chronicle: Awir 24 sets it off against the award, giving an amount still to pay and an amount to recover as two outcomes, and Awir 26a leaves an amount of at most € 118 (the 2025 text) unrecovered. After the toekenning no instalment is paid. The grams are kept in `localStorage` with the rest of the demo state (key `rr-demo-state-v2`; an older state is not converted).

The cell does not read the voorschot back with a lexostatus but with an article in Toeslagen's (fictional) policy, `fictief_beleid_kroniek_toeslagen`, written in the same rule language as the law. Its articles read the chronicle as an input without a source (`source: {}`); `registers:` in `cell.yaml` binds that input to the chronicle `toeslagen`, and the cell registers it with the engine as a data source scoped to that policy, holding the grams that count at the moment of reading.

An instalment is paid through a bank. Toeslagen records a payment order (`betaalopdracht_gegeven`: amount, the account the applicant gave in the application, execution date). A second cell, a fictional bank (`corpus/demo/cells/bank`), receives it and credits or refuses it under its own fictional terms (`fictieve_bankvoorwaarden`); Toeslagen receives that answer and records the instalment as paid or the payment as failed. Only credited payments count in the settlement. A refused amount goes along with the next month's order. The demo carries the messages between the cells; which gram goes to which cell and which field fills which parameter is configured under `channels` in `demo-config.yaml`, and what arises at the receiving end is decided by its rules (`WasmCell.receive`). A message holds from the moment of the gram it carries, so the bank's answer to the order of a month the clock skipped lies before the next month's order. A message that does not arrive goes into an outbox in the demo state and is offered again on every clock step and on load; the case shows the error for as long as the message is in the outbox. A cell records one answer per message and refuses a second one (error name `answered`), which the outbox counts as delivered: Toeslagen by the order the answer refers to, the bank by the betaalkenmerk of the transfer (`identified_by` in its terms). A cell refuses a receipt that has neither of the two. Channels that loop leave the message that started the loop in the outbox, marked as a loop, and it is not offered again until the channels change. The portal shows the persona's account at the fictional bank under "My account", with the balance and the transfers from the bank's chronicle. The account data (opening balance, `geblokkeerd`) is in the persona's `BANK` data in `profiles.yaml`.

Each of the two decisions is announced on its own: the procedure of the Awir has a stage after the voorschot and after the toekenning that *is* a BEKENDMAKING, so Awb 6:8 fires per decision and each has its own objection period. That list of stages is a modelling choice, since the Awir itself gives none; it follows Awb 3:40 and 3:41, which apply per decision.

### Time in the demo

The demo has one clock: the reference date (`referenceDate` in the demo state). Everything that happens in a case happens on that day, at the time of day of the wall clock: the submission, the decision, the announcement and the grams. The clock only moves forward. A gram never lies in the future (RFC-044), so the way back is to reset the demo.

The Chronicle view of a case in the case system shows the facts as a timeline, and below them what the law gives as the next moment, as an expectation and not as a gram. The cell executes the law without recording to find these: the next day with an instalment (the cell gives the days an instalment is executed on, from Awir 22 and the policy, and the demo asks each one), the end of the year the decision concerns, the dates the next decision waits for from the file (the aanslag of Awir 19), and the dates the law gives a decision, such as the latest date of the toekenning (Awir 19) or of the payment (Awir 24). Of a decision still to come it shows only the dates that do not move with the day the decision is taken: a demo heuristic that compares two previews of the decision in different calendar months. "To the next moment" sets the reference date to the earliest of these and lets the cell record what arises for every open case: an instalment for each day the cell gives as due, and a decision whose moment has come. That decision is taken by the demo itself when applications are not reviewed by hand (as at submission), and otherwise lands in the "to review" lane, with what the law would decide at that stage, the settlement included.

The case system also shows the chronicle from the back: "Chronicle" at the top of the board, next to "Cases", lists every gram the cell of that organisation has stored, for all cases, in the order of `recorded_at`. Each gram shows the case it belongs to (the application its references lead back to), when it counts (`effective_at` with its legal basis) next to when it was recorded, and where it comes from. That origin has two sides: what the law says in its own terms, from the shape the cell derives from it (`WasmCell.shape`: the establishing article, the legal character, the stage, and per article or hook the fields it asks for), and the registration in the cell's stream (the event name, the stream's `$id` and file, `establishes`, `stage`). Every provision (the establishing article, each `legal_basis` entry, the basis of the moment, the article or hook behind each field) links to its article under Rulework, which opens and shows the article named in the query (`?artikel=4:13`; a lid opens its article, a law outside the demo corpus is shown without a link). References between grams link to the gram referred to, the inputs of a decision show their provenance, and each gram opens as the JSON line the cell keeps. "See how the cell stores this" on a case opens this view filtered on that case.

"Lexostatuses", the third view next to "Cases" and "Chronicle", shows how the cell reads that chronicle back. Per lexostatus it lists the events that read it for their case (`reads` in the stream), its inputs, and how it reduces: either in the cell configuration (`lexostatuses.yaml`: the filter, the pick and per parameter the derivation with its legal basis), or with an article in the policy of the holder, with the register binding from `registers:` in `cell.yaml` and each article linking to Rulework. Below it, per case, what it gives on the reference date and the grams it was read from, each linking to that gram in the Chronicle view. The cell supplies both halves (`WasmCell.lexostatuses` and `WasmCell.readLexostatus`); the demo names no lexostatus itself.

The reference date can also be set directly under Demo in the menu ("Reference date"). A later date runs the clock in the same way, recording what arises along the way. An earlier date is only accepted while nothing has been recorded; with cases or grams, going back means resetting the demo, because a fact never lies in the future.

A decision may ask for a fact from the file of the administrative body that the cell does not read from its own chronicle: a parameter with origin `DOSSIER`. Where the demo finds it is configured under `dossier` in `demo-config.yaml`, in the shape of a binding (table, column, `select_on`, `absent`); for zorgtoeslag that is the date of the aanslag per year in the persona's Belastingdienst data. The toekenning can be taken once every such date has passed.

On the portal, the application shows what was received so far (the lexostatus the cell reads for the amounts paid, named under `received` in `demo-config.yaml`; the demo adds nothing up itself), each decision with its amounts and dates, and what is still to come.

### The "why" explanation

A portal tile can explain its outcome in plain language, as the "waarom?" link in `poc-machine-law` did. A language model writes the explanation from the engine's trace and the outcome as the tile shows it, in the language the demo is set to. The sheet that shows it says it was written by a language model and that the calculation stands where the two differ.

The feature is off until the presenter unlocks it with a password under Demo in the menu. The password is kept in `localStorage` under its own key (`rr-demo-why-password`), so resetting the demo leaves it in place. The server checks it on every request; the browser only passes it on.

The model runs through the Claude Code CLI, in a small Node server (`frontend-demo/server/why.mjs`) that nginx reaches on `127.0.0.1:7401` inside the same container. A subscription token from `claude setup-token` only works through that CLI, which is why this is a server and not a call from the browser. The server gives the CLI no tools and no settings, allows three explanations at a time, and stops the model when the visitor closes the sheet. It does not lock out after wrong guesses, because a lockout shared by every caller would let one script keep the presenter out; instead it refuses to start with a password shorter than 16 characters.

Without a server behind `/api/why` the app does not show the feature at all: no menu item, no button. That is the case for a plain `just demo` and for a deployment without the variables below.

| Variable | Purpose |
|----------|---------|
| `DEMO_WHY_PASSWORD` | The password that unlocks the button, at least 16 characters. Without it the server does not start |
| `CLAUDE_CODE_OAUTH_TOKEN` | Token for the Claude Code CLI (from `claude setup-token`) |
| `ANTHROPIC_API_KEY` | Alternative to the token: a Console key, billed per call |
| `DEMO_WHY_MODEL` | Model alias for the CLI, default `sonnet` |

## Running locally

```bash
just demo              # builds the WASM engine, starts Vite on :7400 and opens the browser
just dev-demo          # the same, without opening the browser
just demo-why          # the "why" backend on :7401, with your own Claude login; password "lokaal-demo-wachtwoord"
```

Vite forwards `/api/why` to `just demo-why`, so run the two side by side to try the explanation.

Checking, outside the browser:

```bash
just demo-check        # laws, Awb parity, service map, scenarios, frontend tests, WASM and build
just bdd-demo          # the scenarios only (plus the Awb lifecycle test)
just validate-demo     # the laws only (schema and type check)
```

## Further reading

- [Deployment](/operations/deployment)
