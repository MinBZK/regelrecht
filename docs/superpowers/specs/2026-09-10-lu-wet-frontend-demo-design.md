# Design: LU-wet in frontend-demo (4LM)

Date: 2026-09-10  
Status: approved (pending user review of this file)

## Goal

Surface the Luxembourg flight-tax discount law (`vliegbelasting_korting_klimaatneutraal_lu`) in the RegelRecht frontend demo: **Wetten**, **Scenario's**, **Graaf** (hand-picked), and a **presentation mini-section** before the closing slides. Support a 4LM-oriented demo without mixing LU into the NL Merijn/Claudia stories.

## Non-goals

- Portal tiles / citizen discoverability for the LU law
- Simulation population / bindings for LU parameters
- Live EUR-Lex harvest
- Changing the Merijn or Claudia `graph_laws` stories
- A third persona profile (unless added later)

## Approach

**Curated demo copy** of the canonical corpus law into `corpus/demo/regulation/lu/…`, plus multi-jurisdiction support in `copy-demo-corpus.mjs`. Same pattern as the NL demo subset: demo is curated; `corpus/regulation/lu/` remains the source of truth for API/BDD outside the demo.

## Corpus layout

```
corpus/demo/regulation/lu/wet/vliegbelasting_korting_klimaatneutraal_lu/
  2026-07-21.yaml          # demo metadata added (name, service, discoverable)
  scenarios/korting.feature  # English Feature + Scenario titles for the demo
```

### Demo YAML metadata

Add (without changing rule semantics):

- `name`: human-readable Dutch or bilingual title for Wetten tab (e.g. “Vliegbelasting — duurzaamheidskorting (LU)”)
- `service: ADMIN_FISCALE_LU`
- `discoverable: HIDDEN` — appears in Wetten / Scenario's / Graaf sidebar, not on portal tiles

Canonical YAML under `corpus/regulation/lu/` stays as-is unless a later sync is needed.

### English Gherkin (demo only)

Demo `korting.feature` must be fully English for presentation:

- English `Feature:` title
- English `Scenario:` titles (steps are already English keywords)
- Same four cases and expected outputs as today

Canonical `corpus/regulation/lu/.../scenarios/korting.feature` may keep Dutch titles for now; demo copy is authoritative for the UI demo.

Example titles:

- Feature: Flight tax sustainability discount (LU)
- Climate-neutral domestic flight → discount
- Non-climate-neutral flight → no discount
- Climate-neutral short-haul flight → discount
- Climate-neutral long-haul flight → no discount

## Pipeline: `copy-demo-corpus.mjs`

Today the script only walks `corpus/demo/regulation/nl/`.

Change:

1. Walk every jurisdiction directory under `corpus/demo/regulation/`.
2. Keep `law_path` relative to that jurisdiction root (NL paths unchanged, e.g. `zorgtoeslagwet`; LU becomes `wet/vliegbelasting_korting_klimaatneutraal_lu`).
3. Copy YAML + `.feature` into `frontend-demo/public/data/laws/…` preserving relative structure.
4. Optionally index `execution.parameters` as inputs in `index.json` (nice-to-have; not required for Scenario's tab if Gherkin supplies params).

## Services

Add to `corpus/demo/services.yaml`:

```yaml
ADMIN_FISCALE_LU:
  name: Administration de l'enregistrement (LU)
```

No logo required (fallback to abbreviation).

## `demo-config.yaml`

### Wetten

- `expanded_paths` entry for the LU law so the presentatie/wetten highlight can open the relevant article YAML.

### Scenario's

- Feature appears automatically via regenerated `index.json`.
- Presentation demo slide routes to `/scenarios` (preferred) so the English feature is the focus.
- Do **not** change Merijn/Claudia `default_feature` (NL story intact).

### Graaf — separate selection

- Do **not** add the LU law to Merijn or Claudia `graph_laws`.
- Law appears under `ADMIN_FISCALE_LU` in the Graaf sidebar; presenter toggles it manually (`preset` becomes hand-picked).
- Preset “verhaal” remains NL-only.

### Presentatie — mini-section before closing

Insert **two slides** immediately before the existing `closing` slide:

1. **statement** — jurisdiction-agnostic / “also outside the Netherlands”
2. **demo** — LU flight-tax discount; `route: /scenarios` (English feature); optional `highlight` if a stable selector exists for the scenario list/feature title

Presentation copy may be Dutch (deck language) while the feature file itself is English.

## Engine / frontend assumptions

- WASM engine loads laws by `$id`; no NL-only path filter in the engine.
- Frontend `loadCorpus` / Scenario runner work once the law is in `index.json` and law text is loaded.
- Rust `just bdd-demo` picks up `corpus/demo/regulation/lu/` after the files exist (loader already scans jurisdictions).

## Success criteria

1. Wetten tab lists the LU law under Administration de l'enregistrement (LU).
2. Scenario's tab shows the English feature and all four scenarios pass in the browser.
3. Graaf: LU law selectable via sidebar; not in Merijn/Claudia story presets.
4. Presentatie: two new slides before closing; demo slide opens `/scenarios`.
5. `npm run predev` (copy script) regenerates `public/data` including LU.
6. `just validate-demo` and `just bdd-demo` (or equivalent) remain green for the demo corpus.

## Implementation order (high level)

1. Extend `copy-demo-corpus.mjs` for multi-jurisdiction.
2. Add demo YAML + English feature under `corpus/demo/regulation/lu/…`.
3. Add `ADMIN_FISCALE_LU` to `services.yaml`.
4. Update `demo-config.yaml` (`expanded_paths`, slides).
5. Regenerate public data; verify Wetten, Scenario's, Graaf, Presentatie in the running demo.

## Open points (resolved)

| Question | Decision |
|----------|----------|
| Full UI scope | Wetten + Scenario's + Graaf + presentatie |
| Corpus strategy | Curated demo copy (A) |
| Presentatie | Mini-section (2 slides) before closing |
| Graaf | Separate hand-pick; not in Merijn/Claudia |
| Feature language | English in demo feature file |
