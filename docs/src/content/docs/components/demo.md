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

1. **Presentation** (*Presentatie*): the slide deck in Rijkshuisstijl blue. The intro is shown full screen; after that the deck sits on the left as a rail, and each slide opens the tab it is about, switches persona and points at what the presenter means. The slides are content (`demo-config.yaml`), not code. Esc closes the deck and leaves the demo where it is; Shift+P opens it from anywhere.
2. **Laws** (*Wetten*): the machine-readable law as a collapsible YAML tree, prepared per story (`expanded_paths` in `demo-config.yaml`: a path opens itself and everything above it, the rest stays closed; `folded_paths` names nodes on such a path that start closed for the presenter to open, with what lies below them already prepared). Every `source.regulation` is a link that opens the referenced law; a back button retraces the references followed. The list of all laws, grouped per organization, is a side panel that is closed by default.
3. **Graph** (*Graaf*): per law its sources, its inputs from other laws and its outputs, with lines to the law that supplies them, the persona's values on them, and one color per organization. The profile picks the laws of its story (`graph_laws`), and the graph shows those laws plus everything directly attached to them. The layout of the whole corpus is fixed, so "All" (*Alles*) adds laws without moving anything. Selecting a law colors the lines it reads from green and the lines along which other laws read it red.
4. **Scenarios** (*Scenario's*): the Gherkin scenarios per law. In Dutch the steps are rendered as Dutch sentences; English and Frisian show the canonical English steps, which is what the `.feature` files contain. "Run" (*Uitvoeren*) executes a scenario in the browser against the engine and opens the full execution trace, in which each law has its own color and a bar steps from one law to the next.
5. **Simulation** (*Simulatie*): a generated population of citizens or businesses (size, age and income distribution, business type, seed) is run through every regulation of that portal. The result shows who qualifies and for how much, broken down by age, income, partner, business type or size, and for citizens the disposable income per month: income minus taxes, plus allowances and benefits. Which outputs count, and whether per month or per year, is set in `simulation.disposable_income`. Constants from the laws (thresholds, percentages) can be changed per run, and so can those of the regulations the laws rely on, such as the standard premium (*standaardpremie*) that a ministerial regulation fills in for the zorgtoeslag. A run is named after what it changed and a variant gets its own color; runs sit side by side for comparison, which opens with the disposable income per run, and export as CSV or JSON.

   ![The Simulation tab for a population of 50 citizens: the settings on the left, and on the right the population summary and the average and median disposable income per month, broken down per regulation.](../../../assets/simulatie-screenshot.png)

6. **My government** (*Mijn overheid*, or *Mijn onderneming* for a business; the label comes from `portal_tab_label` in `demo-config.yaml`): the portal of the active persona. Each regulation is computed live. Under "Data used" (*Gebruikte gegevens*) the portal shows where each piece of data comes from, and each one can be corrected. A regulation that ends in a *beschikking* (an individual administrative decision) is applied for from the portal, in a panel that follows the flow of the POC: the law computes with what the government already knows and asks, one question at a time, only for what no register holds (the rent, the location of a café terrace). It recomputes after each answer, lets the applicant check the outcome and the data used, and shows the status after submission. Once the decision has been announced, the panel shows the date until which an objection (*bezwaar*) is possible. That date comes from the Awb itself (articles 6:7 and 6:8), not from the screen. The portal never jumps to the case system, which belongs to the other side of the counter.

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

## The recorded walkthrough

Next to the live presentation there is a recorded one at `/rondleiding` (`/en/tour`), for people who go through the demo without a presenter. The home page and the Presentation tab only offer it when the build carries a recording.

The player shows the deck on the left and the demo as video on the right, with the presenter's voice and, if recorded, the presenter's face in a circle in the rail. The demo part is a recording, not a replay of clicks in the live app. A replay would recompute every amount on the day it is watched, give every case a new random id and depend on the layout of the viewer's window; the video shows what was said on the day it was recorded. The slides do stay live text: they render from the timeline, so a sentence on a slide can be corrected without recording again. "Try it yourself" opens the live demo at the same tab and persona.

The controls are design-system buttons in the deck's footer: play and pause, ten seconds back or forward, previous and next chapter (one chapter per slide), a chapter menu, playback speed, captions, the presenter bubble on or off, and a transcript per chapter in a side panel. The keys are Space or k to pause, the arrows for chapters, j and l for ten seconds, and c for captions. Frequently asked questions appear in the rail at the moment the presenter mentions them; each answer is a short recording of its own with its own address (`/rondleiding/<id>`), and "Back to the walkthrough" resumes where the viewer left off. The player remembers its position in `localStorage`.

The video player, the bubble, the caption overlay and the click ripple are custom CSS in `WalkthroughView.vue`, because the design system has no video components.

### Recording

```bash
just walkthrough-record   # the demo on :7400 in Chrome, with the recorder panel
```

The recorder is a panel in the top left corner, present only in the dev server and only with `?record` in the address; the production bundle does not contain it, and the deployed site's `Permissions-Policy` blocks the microphone and camera anyway. It records three tracks on one clock:

- the demo itself: the whole current tab through `getDisplayMedia`, with the microphone as its audio (echo cancellation, noise suppression and automatic gain off);
- the webcam, optionally;
- an event log of slide changes, routes, click positions and typing. It records that a key was pressed in a field, never which one.

A take is recorded in zaal mode, the way the demo is presented in a room: the deck covers the screen on a story slide and steps aside on a slide that opens the demo, so the demo fills the window. The whole window is recorded and not only the workspace, because side sheets and dialogs are placed against the window; a crop next to the rail left the law list out of the picture. The player puts the slide text back next to the video. During a take the panel leaves the screen and the tab title starts with "● REC". Shift+X marks a slip (say the sentence again from its start), Shift+R stops. Chunks stream to the Vite dev server, which writes them to `.walkthrough/takes/<take>/`; a crash loses seconds, not the take. A take can start at any slide, and the panel can restore the demo state as it was at that slide in an earlier take, so one chapter can be recorded again on its own.

Record in Chrome or Edge, which can capture their own tab without asking for a window.

### Post-processing

`script/walkthrough/` is a small Python project, run through `uv`:

```bash
just walkthrough prepare     # last take: extract, clean the voice, transcribe, draft cuts
just walkthrough transcript  # the spoken text per slide
just walkthrough build       # walkthrough.yaml to videos, captions and timeline.json
just walkthrough export      # a shareable MP4 per track (needs `just dev-demo` running)
just walkthrough status      # which takes exist and how far each one is processed
just walkthrough-test        # the tests of the cut and caption arithmetic
```

The voice is denoised with DeepFilterNet, filtered below 80 Hz, de-essed and compressed; loudness is normalized to -16 LUFS over the finished track, so every chapter is equally loud. WhisperX (Whisper large-v3 with a Dutch wav2vec2 aligner) gives every word a timestamp, with the `glossary` from `walkthrough.yaml` as a prompt for the spelling of domain terms. `prepare` then proposes cuts: long silences shortened, unless the presenter clicked or changed slide in them, and the sentence before each Shift+X. The proposal is a draft. The cuts that count are the ones in `corpus/demo/walkthrough/walkthrough.yaml`, which also lists the takes that make up the walkthrough, slide texts that replace the recorded ones, and the questions with the take of their answer and the moment they appear. The build refuses a cut through typing. DeepFilterNet, WhisperX and OpenCV (for finding the face in the webcam picture) each run in their own `uv` environment and are downloaded on first use; ffmpeg has to be installed.

`build` cuts every piece on whole frames, so the voice cannot drift from the picture over many cuts, and names every video after its checksum. The timeline, the captions and `walkthrough.yaml` are in git under `corpus/demo/walkthrough/`; the videos are not, because a re-recorded chapter would leave tens of megabytes in the history for good. They are assets of the GitHub release named in `timeline.json`. The Docker build fetches them with `scripts/fetch-walkthrough-media.mjs` and refuses a file whose checksum differs. nginx serves them from `/walkthrough/` with a one-year cache, which is safe because a new recording means new file names.

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
